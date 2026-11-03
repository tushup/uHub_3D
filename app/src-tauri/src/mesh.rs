//! Geometry loading for the viewer and the path tracer.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::LibError;
use crate::view::Bounds;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MeshGroup {
    pub material: String,
    pub start: u32,
    pub count: u32,
}

#[derive(Debug, Clone)]
pub struct Mesh {
    /// Corner-expanded attributes (one entry per index).
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub uvs: Vec<[f32; 2]>,
    pub indices: Vec<u32>,
    pub groups: Vec<MeshGroup>,
    pub has_uvs: bool,
}

impl Mesh {
    pub fn bounds(&self) -> Bounds {
        let mut min = [f32::INFINITY; 3];
        let mut max = [f32::NEG_INFINITY; 3];
        for p in &self.positions {
            for i in 0..3 {
                min[i] = min[i].min(p[i]);
                max[i] = max[i].max(p[i]);
            }
        }
        if self.positions.is_empty() {
            min = [0.0; 3];
            max = [0.0; 3];
        }
        Bounds { min, max }
    }

    /// Flat binary layout for the WebGL viewer (little-endian):
    /// `u32 vertexCount, u32 indexCount, u32 groupCount, u32 hasUvs`,
    /// positions f32×3, normals f32×3, uvs f32×2, indices u32, then per
    /// group `u32 start, u32 count, u16 nameLen, name bytes`.
    pub fn to_preview_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        for v in [
            self.positions.len() as u32,
            self.indices.len() as u32,
            self.groups.len() as u32,
            self.has_uvs as u32,
        ] {
            out.extend_from_slice(&v.to_le_bytes());
        }
        for p in &self.positions {
            for v in p {
                out.extend_from_slice(&v.to_le_bytes());
            }
        }
        for n in &self.normals {
            for v in n {
                out.extend_from_slice(&v.to_le_bytes());
            }
        }
        for uv in &self.uvs {
            for v in uv {
                out.extend_from_slice(&v.to_le_bytes());
            }
        }
        for i in &self.indices {
            out.extend_from_slice(&i.to_le_bytes());
        }
        for g in &self.groups {
            out.extend_from_slice(&g.start.to_le_bytes());
            out.extend_from_slice(&g.count.to_le_bytes());
            let name = g.material.as_bytes();
            out.extend_from_slice(&(name.len() as u16).to_le_bytes());
            out.extend_from_slice(name);
        }
        out
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum PreviewSource {
    #[serde(rename = "file")]
    File { path: PathBuf },
    #[serde(rename = "sphere")]
    Sphere,
}

pub fn load_source(source: &PreviewSource) -> Result<Mesh, LibError> {
    match source {
        PreviewSource::File { path } => load_mesh(path),
        PreviewSource::Sphere => Ok(uv_sphere(96, 48)),
    }
}

/// Load an FBX/OBJ file, triangulated, corner-expanded, Y-up, metres.
/// Node transforms are baked from each mesh's first instance
/// (shortcut: extra instances of the same mesh are not duplicated).
pub fn load_mesh(path: &Path) -> Result<Mesh, LibError> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase());
    match ext.as_deref() {
        Some("fbx") | Some("obj") => {}
        _ => return Err(LibError::UnsupportedFormat(path.display().to_string())),
    }
    let path_str = path
        .to_str()
        .ok_or_else(|| LibError::Parse(format!("path is not valid UTF-8: {}", path.display())))?;
    let opts = ufbx::LoadOpts {
        load_external_files: true,
        target_axes: ufbx::CoordinateAxes::right_handed_y_up(),
        target_unit_meters: 1.0,
        ..Default::default()
    };
    let scene = ufbx::load_file(path_str, opts).map_err(|e| {
        let info = e.info();
        if info.is_empty() {
            LibError::Parse(e.description.to_string())
        } else {
            LibError::Parse(format!("{} ({})", e.description, info))
        }
    })?;

    let mut out = Mesh {
        positions: Vec::new(),
        normals: Vec::new(),
        uvs: Vec::new(),
        indices: Vec::new(),
        groups: Vec::new(),
        has_uvs: false,
    };

    for mesh in &scene.meshes {
        // Bake the first instance's geometry-to-world matrix (identity
        // when the mesh is not instanced).
        let matrix = mesh
            .element
            .instances
            .first()
            .map(|node| node.geometry_to_world);
        let xform = |p: [f32; 3]| -> [f32; 3] {
            match &matrix {
                None => p,
                Some(m) => [
                    (m.m00 * p[0] as f64 + m.m01 * p[1] as f64 + m.m02 * p[2] as f64 + m.m03) as f32,
                    (m.m10 * p[0] as f64 + m.m11 * p[1] as f64 + m.m12 * p[2] as f64 + m.m13) as f32,
                    (m.m20 * p[0] as f64 + m.m21 * p[1] as f64 + m.m22 * p[2] as f64 + m.m23) as f32,
                ],
            }
        };
        // Normal matrix is the same rotation part without translation.
        let xform_dir = |p: [f32; 3]| -> [f32; 3] {
            match &matrix {
                None => p,
                Some(m) => [
                    (m.m00 * p[0] as f64 + m.m01 * p[1] as f64 + m.m02 * p[2] as f64) as f32,
                    (m.m10 * p[0] as f64 + m.m11 * p[1] as f64 + m.m12 * p[2] as f64) as f32,
                    (m.m20 * p[0] as f64 + m.m21 * p[1] as f64 + m.m22 * p[2] as f64) as f32,
                ],
            }
        };

        let base_vertex = out.positions.len() as u32;
        let base_index = out.indices.len() as u32;
        let mut index_cursor = base_index;
        let mut current_group: Option<(String, u32)> = None; // (material, start)
        let mut tri_buf: Vec<u32> = vec![0; mesh.max_face_triangles.max(1) * 3];

        for (face_i, face) in mesh.faces.iter().enumerate() {
            let material_name = mesh
                .face_material
                .get(face_i)
                .and_then(|&mi| mesh.materials.get(mi as usize))
                .map(|m| m.element.name.to_string())
                .unwrap_or_default();
            match &current_group {
                Some((name, _)) if *name == material_name => {}
                _ => {
                    if let Some((name, start)) = current_group.take() {
                        out.groups.push(MeshGroup {
                            material: name,
                            start,
                            count: index_cursor - start,
                        });
                    }
                    current_group = Some((material_name, index_cursor));
                }
            }

            let tri_count = mesh.triangulate_face(&mut tri_buf, *face) as usize;
            for corner in tri_buf.iter().take(tri_count * 3) {
                let ci = *corner as usize;
                let pos = mesh.vertex_position[ci];
                out.positions.push(xform([pos.x as f32, pos.y as f32, pos.z as f32]));
                if mesh.vertex_normal.exists {
                    let n = mesh.vertex_normal[ci];
                    out.normals.push(normalize(xform_dir([n.x as f32, n.y as f32, n.z as f32])));
                }
                if mesh.vertex_uv.exists {
                    let uv = mesh.vertex_uv[ci];
                    out.uvs.push([uv.x as f32, uv.y as f32]);
                } else {
                    out.uvs.push([0.0, 0.0]);
                }
                out.indices.push(base_vertex + (out.positions.len() - 1) as u32);
                index_cursor += 1;
            }
        }
        if let Some((name, start)) = current_group.take() {
            out.groups.push(MeshGroup {
                material: name,
                start,
                count: index_cursor - start,
            });
        }
        if mesh.vertex_uv.exists {
            out.has_uvs = true;
        }
    }

    if out.normals.len() != out.positions.len() {
        compute_flat_normals(&mut out);
    }

    Ok(out)
}

fn normalize(v: [f32; 3]) -> [f32; 3] {
    let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if len == 0.0 {
        [0.0, 1.0, 0.0]
    } else {
        [v[0] / len, v[1] / len, v[2] / len]
    }
}

fn compute_flat_normals(mesh: &mut Mesh) {
    mesh.normals = vec![[0.0, 1.0, 0.0]; mesh.positions.len()];
    for tri in mesh.indices.as_chunks::<3>().0 {
        let (a, b, c) = (
            mesh.positions[tri[0] as usize],
            mesh.positions[tri[1] as usize],
            mesh.positions[tri[2] as usize],
        );
        let u = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
        let v = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
        let n = normalize([
            u[1] * v[2] - u[2] * v[1],
            u[2] * v[0] - u[0] * v[2],
            u[0] * v[1] - u[1] * v[0],
        ]);
        for &i in tri {
            mesh.normals[i as usize] = n;
        }
    }
}

/// Analytic UV sphere, radius 0.5, poles on Y. Vertex `(0,0)` is the
/// top pole; v runs 1 at the top to 0 at the bottom.
pub fn uv_sphere(segments: u32, rings: u32) -> Mesh {
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut uvs = Vec::new();
    let mut indices = Vec::new();
    for j in 0..=rings {
        let phi = std::f32::consts::PI * j as f32 / rings as f32;
        // Exact poles: f32 sin(pi) is not 0, which would scatter
        // the pole vertices instead of collapsing them to a point.
        let sin_phi = if j == 0 || j == rings { 0.0 } else { phi.sin() };
        for i in 0..=segments {
            let theta = 2.0 * std::f32::consts::PI * i as f32 / segments as f32;
            // three.js SphereGeometry convention: negated X with
            // the index order below makes triangles face outward,
            // and U runs 0..2 (2x tiling) increasing to the right
            // when seen from +Z.
            let p = [
                -0.5 * sin_phi * theta.cos(),
                0.5 * phi.cos(),
                0.5 * sin_phi * theta.sin(),
            ];
            positions.push(p);
            normals.push(normalize(p));
            uvs.push([2.0 * i as f32 / segments as f32, 1.0 - j as f32 / rings as f32]);
        }
    }
    let cols = segments + 1;
    for j in 0..rings {
        for i in 0..segments {
            let a = j * cols + i;
            let b = a + 1;
            let c = a + cols;
            let d = c + 1;
            indices.extend_from_slice(&[a, c, b, b, c, d]);
        }
    }
    let count = indices.len() as u32;
    Mesh {
        positions,
        normals,
        uvs,
        indices,
        groups: vec![MeshGroup { material: String::new(), start: 0, count }],
        has_uvs: true,
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    #[test]
    fn sphere_winding_outward() {
        let m = uv_sphere(32, 16);
        let sub = |a: [f32; 3], b: [f32; 3]| [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
        for tri in m.indices.chunks_exact(3) {
            let (a, b, c) = (
                m.positions[tri[0] as usize],
                m.positions[tri[1] as usize],
                m.positions[tri[2] as usize],
            );
            let (u, v) = (sub(b, a), sub(c, a));
            let n = [
                u[1] * v[2] - u[2] * v[1],
                u[2] * v[0] - u[0] * v[2],
                u[0] * v[1] - u[1] * v[0],
            ];
            if (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt() < 1e-9 {
                continue; // degenerate pole triangle
            }
            let centroid = [
                (a[0] + b[0] + c[0]) / 3.0,
                (a[1] + b[1] + c[1]) / 3.0,
                (a[2] + b[2] + c[2]) / 3.0,
            ];
            let dot = n[0] * centroid[0] + n[1] * centroid[1] + n[2] * centroid[2];
            assert!(dot > 0.0, "triangle {tri:?} faces inward (dot {dot})");
        }
    }

    #[test]
    fn sphere_normals_outward() {
        let m = uv_sphere(32, 16);
        for (p, n) in m.positions.iter().zip(&m.normals) {
            let dot = p[0] * n[0] + p[1] * n[1] + p[2] * n[2];
            assert!(dot > 0.0, "normal at {p:?} points inward");
        }
    }

    #[test]
    fn sphere_u_increases_to_right_at_front() {
        // Seen from +Z (the camera side), moving right (+X) across
        // the front of the sphere must increase U, or the texture
        // is mirrored.
        let m = uv_sphere(64, 32);
        let mut patch: Vec<(f32, f32)> = m
            .positions
            .iter()
            .zip(&m.uvs)
            .filter(|(p, _)| p[2] > 0.40 && p[1].abs() < 0.20)
            .map(|(p, uv)| (p[0], uv[0]))
            .collect();
        assert!(patch.len() >= 4, "front patch too small: {}", patch.len());
        patch.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
        for w in patch.windows(2) {
            assert!(
                w[1].1 >= w[0].1,
                "u must not shrink as x grows at the front: ({}, {}) -> ({}, {})",
                w[0].0,
                w[0].1,
                w[1].0,
                w[1].1
            );
        }
        assert!(
            patch.last().unwrap().1 > patch.first().unwrap().1,
            "u must grow across the front patch"
        );
    }

    #[test]
    fn sphere_u_tiles_twice() {
        let m = uv_sphere(32, 16);
        let (mut lo, mut hi) = (f32::INFINITY, f32::NEG_INFINITY);
        for uv in &m.uvs {
            lo = lo.min(uv[0]);
            hi = hi.max(uv[0]);
        }
        assert!(lo.abs() < 1e-6 && (hi - 2.0).abs() < 1e-6, "u spans [{lo}, {hi}]");
    }

    fn fixture(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(name)
    }

    #[test]
    fn loads_fbx_cube() {
        let m = load_mesh(&fixture("cube_2mat.fbx")).unwrap();
        assert!(!m.positions.is_empty());
        assert_eq!(m.positions.len(), m.normals.len());
        assert_eq!(m.positions.len(), m.uvs.len());
        assert_eq!(m.indices.len() % 3, 0);
        assert!(m.indices.iter().all(|&i| (i as usize) < m.positions.len()));
    }

    #[test]
    fn fbx_loads_y_up() {
        // The fixture is a 2x2x2 cube centered on the origin; the FBX is
        // Z-up in Blender space, so a correct Y-up load keeps the same
        // extents as the unambiguous Y-up OBJ of the same model.
        let fbx = load_mesh(&fixture("cube_2mat.fbx")).unwrap();
        let obj = load_mesh(&fixture("cube_2mat.obj")).unwrap();
        let (fb, ob) = (fbx.bounds(), obj.bounds());
        for axis in 0..3 {
            assert!((fb.min[axis] - ob.min[axis]).abs() < 1e-4, "axis {axis} min");
            assert!((fb.max[axis] - ob.max[axis]).abs() < 1e-4, "axis {axis} max");
            assert!((fb.min[axis] + 1.0).abs() < 1e-4, "axis {axis} fbx min");
            assert!((fb.max[axis] - 1.0).abs() < 1e-4, "axis {axis} fbx max");
        }
    }

    #[test]
    fn obj_keeps_y_up_extents() {
        // Hand-written 1 x 2 x 3 box: if Y were not the height axis the
        // extents would come back permuted.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("box.obj");
        let mut text = String::new();
        for (x, y, z) in [
            (0.0, 0.0, 0.0), (1.0, 0.0, 0.0), (1.0, 2.0, 0.0), (0.0, 2.0, 0.0),
            (0.0, 0.0, 3.0), (1.0, 0.0, 3.0), (1.0, 2.0, 3.0), (0.0, 2.0, 3.0),
        ] {
            text.push_str(&format!("v {x} {y} {z}\n"));
        }
        text.push_str("f 1 2 3 4\nf 5 8 7 6\nf 1 5 6 2\nf 2 6 7 3\nf 3 7 8 4\nf 4 8 5 1\n");
        std::fs::write(&path, text).unwrap();
        let m = load_mesh(&path).unwrap();
        let b = m.bounds();
        assert!((b.max[0] - b.min[0] - 1.0).abs() < 1e-6);
        assert!((b.max[1] - b.min[1] - 2.0).abs() < 1e-6);
        assert!((b.max[2] - b.min[2] - 3.0).abs() < 1e-6);
    }

    #[test]
    fn obj_matches_fbx_counts() {
        let fbx = load_mesh(&fixture("cube_2mat.fbx")).unwrap();
        let obj = load_mesh(&fixture("cube_2mat.obj")).unwrap();
        assert_eq!(fbx.positions.len(), obj.positions.len());
        assert_eq!(fbx.indices.len(), obj.indices.len());
    }

    #[test]
    fn groups_cover_all_indices() {
        let m = load_mesh(&fixture("cube_2mat.fbx")).unwrap();
        let names: Vec<_> = m.groups.iter().map(|g| g.material.as_str()).collect();
        assert_eq!(names, ["Red", "Blue"]);
        let total: u32 = m.groups.iter().map(|g| g.count).sum();
        assert_eq!(total as usize, m.indices.len());
        let mut cursor = 0;
        for g in &m.groups {
            assert_eq!(g.start, cursor);
            cursor += g.count;
        }
    }

    #[test]
    fn sphere_is_closed_unit() {
        let m = uv_sphere(32, 16);
        let b = m.bounds();
        for axis in 0..3 {
            assert!((b.min[axis] + 0.5).abs() < 1e-4, "axis {axis} min {}", b.min[axis]);
            assert!((b.max[axis] - 0.5).abs() < 1e-4, "axis {axis} max {}", b.max[axis]);
        }
        assert_eq!(m.indices.len() % 3, 0);
        for n in &m.normals {
            let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
            assert!((len - 1.0).abs() < 1e-4);
        }
    }

    #[test]
    fn preview_bytes_round_trip() {
        let m = uv_sphere(8, 4);
        let bytes = m.to_preview_bytes();
        let u32_at = |off: usize| u32::from_le_bytes(bytes[off..off + 4].try_into().unwrap());
        let f32_at = |off: usize| f32::from_le_bytes(bytes[off..off + 4].try_into().unwrap());
        assert_eq!(u32_at(0) as usize, m.positions.len());
        assert_eq!(u32_at(4) as usize, m.indices.len());
        assert_eq!(u32_at(8) as usize, m.groups.len());
        assert_eq!(u32_at(12), 1);
        assert_eq!(f32_at(16), m.positions[0][0]);
        assert_eq!(f32_at(20), m.positions[0][1]);
        assert_eq!(f32_at(24), m.positions[0][2]);
        let idx_off = 16 + m.positions.len() * 12 + m.normals.len() * 12 + m.uvs.len() * 8;
        assert_eq!(u32_at(idx_off), m.indices[0]);
    }
}
