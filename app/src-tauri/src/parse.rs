use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::error::LibError;
use crate::model::AttrKind;
use crate::naming::texture_ext;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ParsedMaterial {
    pub name: String,
    pub textures: Vec<(AttrKind, PathBuf)>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ParsedModel {
    pub format: String,
    pub meshes: u32,
    pub vertices: u32,
    pub materials: Vec<ParsedMaterial>,
}

/// Resolve a texture referenced by the model file to an existing path.
/// Stored absolute paths go stale when a model is downloaded or moved,
/// so after the stored name we fall back to the model folder (and its
/// `textures` subfolder) by file name. Unsupported extensions are dropped.
fn resolve_texture(tex: &ufbx::Texture, model_dir: &Path) -> Option<PathBuf> {
    let filename = tex.filename.as_ref() as &str;
    let mut candidates = vec![
        PathBuf::from(filename),
        model_dir.join(tex.relative_filename.as_ref() as &str),
    ];
    if let Some(name) = Path::new(filename).file_name() {
        candidates.push(model_dir.join(name));
        candidates.push(model_dir.join("textures").join(name));
    }
    candidates
        .into_iter()
        .find(|candidate| candidate.is_file() && texture_ext(candidate).is_ok())
}

pub fn parse_model(path: &Path) -> Result<ParsedModel, LibError> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase());
    let format = match ext.as_deref() {
        Some("fbx") => "fbx",
        Some("obj") => "obj",
        _ => return Err(LibError::UnsupportedFormat(path.display().to_string())),
    };
    let path_str = path
        .to_str()
        .ok_or_else(|| LibError::Parse(format!("path is not valid UTF-8: {}", path.display())))?;
    // OBJ needs its .mtl loaded for material names/textures.
    let opts = ufbx::LoadOpts {
        load_external_files: true,
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

    let mut vertices: u32 = 0;
    for mesh in &scene.meshes {
        vertices += mesh.num_vertices as u32;
    }

    let model_dir = path.parent().map(|p| p.to_path_buf()).unwrap_or_default();
    let mut materials = Vec::new();
    for mat in &scene.materials {
        let mut textures = Vec::new();
        let maps: [(AttrKind, &ufbx::MaterialMap); 9] = [
            (AttrKind::BaseColor, &mat.pbr.base_color),
            (AttrKind::Metallic, &mat.pbr.metalness),
            (AttrKind::Roughness, &mat.pbr.roughness),
            (AttrKind::Specular, &mat.pbr.specular_factor),
            (AttrKind::Normal, &mat.pbr.normal_map),
            (AttrKind::Height, &mat.pbr.displacement_map),
            (AttrKind::AO, &mat.pbr.ambient_occlusion),
            (AttrKind::Opacity, &mat.pbr.opacity),
            (AttrKind::Emissive, &mat.pbr.emission_color),
        ];
        for (kind, map) in maps {
            if let Some(tex) = &map.texture {
                if let Some(resolved) = resolve_texture(tex, &model_dir) {
                    textures.push((kind, resolved));
                }
            }
        }
        materials.push(ParsedMaterial {
            name: mat.element.name.to_string(),
            textures,
        });
    }

    Ok(ParsedModel {
        format: format.to_string(),
        meshes: scene.meshes.len() as u32,
        vertices,
        materials,
    })
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::*;
    use crate::error::LibError;
    use crate::model::AttrKind;

    fn fixture(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(name)
    }

    #[test]
    fn parses_fbx_materials() {
        let m = parse_model(&fixture("cube_2mat.fbx")).unwrap();
        assert_eq!(m.format, "fbx");
        let names: Vec<_> = m.materials.iter().map(|x| x.name.as_str()).collect();
        assert_eq!(names, ["Red", "Blue"]); // file order
        assert_eq!(m.meshes, 1);
        assert!(m.vertices >= 8);
    }

    #[test]
    fn parses_obj_materials() {
        let m = parse_model(&fixture("cube_2mat.obj")).unwrap();
        assert_eq!(m.format, "obj");
        let names: Vec<_> = m.materials.iter().map(|x| x.name.as_str()).collect();
        assert_eq!(names, ["Red", "Blue"]); // file order
        assert_eq!(m.meshes, 1);
        assert!(m.vertices >= 8);
    }

    /// Copy the FBX fixture with its stored absolute texture paths made
    /// stale (same byte length, nonexistent folder), like a downloaded model.
    fn stale_path_fbx(dir: &Path) -> PathBuf {
        let src = std::fs::read(fixture("cube_2mat.fbx")).unwrap();
        let needle: &[u8] = b"E:\\MetaHackathon";
        let replacement: &[u8] = b"E:\\XetaHackathon";
        assert_eq!(needle.len(), replacement.len());
        let mut out = src.clone();
        let mut replaced = 0;
        for i in 0..out.len().saturating_sub(needle.len()) {
            if &out[i..i + needle.len()] == needle {
                out[i..i + needle.len()].copy_from_slice(replacement);
                replaced += 1;
            }
        }
        assert!(replaced > 0, "fixture stores an absolute texture path");
        let dst = dir.join("model.fbx");
        std::fs::write(&dst, out).unwrap();
        dst
    }

    #[test]
    fn texture_prefill_falls_back_to_model_dir() {
        let dir = tempfile::tempdir().unwrap();
        let fbx = stale_path_fbx(dir.path());
        std::fs::copy(fixture("red.png"), dir.path().join("red.png")).unwrap();
        let m = parse_model(&fbx).unwrap();
        let (_, resolved) = m.materials[0]
            .textures
            .iter()
            .find(|(k, _)| *k == AttrKind::BaseColor)
            .expect("base color texture resolves");
        assert_eq!(resolved, &dir.path().join("red.png"));
    }

    #[test]
    fn texture_prefill_checks_textures_subfolder() {
        let dir = tempfile::tempdir().unwrap();
        let fbx = stale_path_fbx(dir.path());
        std::fs::create_dir(dir.path().join("textures")).unwrap();
        std::fs::copy(fixture("red.png"), dir.path().join("textures").join("red.png")).unwrap();
        let m = parse_model(&fbx).unwrap();
        let (_, resolved) = m.materials[0]
            .textures
            .iter()
            .find(|(k, _)| *k == AttrKind::BaseColor)
            .expect("base color texture resolves");
        assert_eq!(resolved, &dir.path().join("textures").join("red.png"));
    }

    #[test]
    fn fbx_texture_prefill() {
        let m = parse_model(&fixture("cube_2mat.fbx")).unwrap();
        assert!(m.materials[0]
            .textures
            .iter()
            .any(|(k, p)| *k == AttrKind::BaseColor && p.ends_with("red.png")));
    }

    #[test]
    fn broken_file_is_parse_error() {
        assert!(matches!(parse_model(&fixture("broken.fbx")), Err(LibError::Parse(_))));
    }

    #[test]
    fn rejects_other_ext() {
        assert!(matches!(
            parse_model(Path::new("a.gltf")),
            Err(LibError::UnsupportedFormat(_))
        ));
    }
}
