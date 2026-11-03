//! CPU path tracer (spec §A1.6): scene build, BVH, camera, film,
//! the progressive loop and the principled integrator.

pub mod bsdf;
pub mod bvh;
pub mod camera;
pub mod denoise;
pub mod film;
pub mod integrator;
pub mod light;
pub mod sampler;
pub mod texture;
pub mod tonemap;

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Instant;

use image::RgbaImage;

use crate::error::LibError;
use crate::mesh::{Mesh, MeshGroup};
use crate::model::{AttrKind, AttrValue, MaterialDraft, NormalY};
use crate::view::{framing, Bounds, View};

pub use bvh::{Bvh, Hit, Triangle};
pub use camera::Camera;
pub use film::Film;
pub use light::{three_point, Env, RectLight};
pub use texture::Texture;
pub use tonemap::agx;

// ------------------------------------------------------------- Vec3

pub type Vec3 = [f32; 3];

pub fn add(a: Vec3, b: Vec3) -> Vec3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

pub fn sub(a: Vec3, b: Vec3) -> Vec3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

pub fn scale(a: Vec3, s: f32) -> Vec3 {
    [a[0] * s, a[1] * s, a[2] * s]
}

pub fn mul(a: Vec3, b: Vec3) -> Vec3 {
    [a[0] * b[0], a[1] * b[1], a[2] * b[2]]
}

pub fn dot(a: Vec3, b: Vec3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

pub fn cross(a: Vec3, b: Vec3) -> Vec3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

pub fn length(a: Vec3) -> f32 {
    dot(a, a).sqrt()
}

pub fn normalize(a: Vec3) -> Vec3 {
    let len = length(a);
    if len > 0.0 {
        scale(a, 1.0 / len)
    } else {
        a
    }
}

// --------------------------------------------------------- material

/// One render material (spec §A1.6 principled subset). A texture,
/// when present, replaces the matching scalar factor at a UV.
/// Defaults: baseColor 0.8, roughness 0.5, metallic 0, specular 0.5,
/// opacity 1, no emission.
#[derive(Debug, Clone)]
pub struct RenderMaterial {
    pub base_color: Vec3,
    pub base_color_map: Option<Arc<Texture>>,
    pub roughness: f32,
    pub roughness_map: Option<Arc<Texture>>,
    pub metallic: f32,
    pub metallic_map: Option<Arc<Texture>>,
    pub specular: f32,
    pub specular_map: Option<Arc<Texture>>,
    pub opacity: f32,
    pub opacity_map: Option<Arc<Texture>>,
    pub emissive: Vec3,
    pub emissive_map: Option<Arc<Texture>>,
    pub normal_map: Option<Arc<Texture>>,
    pub normal_strength: f32,
    pub normal_directx: bool,
}

impl Default for RenderMaterial {
    fn default() -> Self {
        RenderMaterial {
            base_color: [0.8, 0.8, 0.8],
            base_color_map: None,
            roughness: 0.5,
            roughness_map: None,
            metallic: 0.0,
            metallic_map: None,
            specular: 0.5,
            specular_map: None,
            opacity: 1.0,
            opacity_map: None,
            emissive: [0.0; 3],
            emissive_map: None,
            normal_map: None,
            normal_strength: 1.0,
            normal_directx: false,
        }
    }
}

impl RenderMaterial {
    /// Convert a draft material, loading its textures once (fit
    /// 2048, sRGB for BaseColor and Emissive only). Height and AO
    /// are not rendered and are skipped.
    pub fn from_draft(m: &MaterialDraft) -> Result<RenderMaterial, LibError> {
        fn scalar(value: &Option<AttrValue>) -> Option<f32> {
            match value {
                Some(AttrValue::Scalar(v)) => Some(*v),
                Some(AttrValue::Color(c)) => Some(c[0]),
                None => None,
            }
        }
        let mut out = RenderMaterial::default();
        for attr in &m.attributes {
            match attr.kind {
                AttrKind::BaseColor => {
                    if let Some(AttrValue::Color(c)) = attr.value {
                        out.base_color = c;
                    }
                    if let Some(path) = &attr.texture_src {
                        out.base_color_map = Some(Arc::new(Texture::load(path, true)?));
                    }
                }
                AttrKind::Roughness => {
                    if let Some(v) = scalar(&attr.value) {
                        out.roughness = v;
                    }
                    if let Some(path) = &attr.texture_src {
                        out.roughness_map = Some(Arc::new(Texture::load(path, false)?));
                    }
                }
                AttrKind::Metallic => {
                    if let Some(v) = scalar(&attr.value) {
                        out.metallic = v;
                    }
                    if let Some(path) = &attr.texture_src {
                        out.metallic_map = Some(Arc::new(Texture::load(path, false)?));
                    }
                }
                AttrKind::Specular => {
                    if let Some(v) = scalar(&attr.value) {
                        out.specular = v;
                    }
                    if let Some(path) = &attr.texture_src {
                        out.specular_map = Some(Arc::new(Texture::load(path, false)?));
                    }
                }
                AttrKind::Opacity => {
                    if let Some(v) = scalar(&attr.value) {
                        out.opacity = v;
                    }
                    if let Some(path) = &attr.texture_src {
                        out.opacity_map = Some(Arc::new(Texture::load(path, false)?));
                    }
                }
                AttrKind::Emissive => {
                    if let Some(AttrValue::Color(c)) = attr.value {
                        out.emissive = c;
                    }
                    if let Some(path) = &attr.texture_src {
                        out.emissive_map = Some(Arc::new(Texture::load(path, true)?));
                    }
                }
                AttrKind::Normal => {
                    if let Some(path) = &attr.texture_src {
                        out.normal_map = Some(Arc::new(Texture::load(path, false)?));
                    }
                    if let Some(s) = attr.strength {
                        out.normal_strength = s;
                    }
                    out.normal_directx = matches!(attr.normal_y, Some(NormalY::DirectX));
                }
                AttrKind::Height | AttrKind::AO => {}
            }
        }
        Ok(out)
    }
}

/// The binding rule from spec §A1.6: a group uses the draft whose
/// `original_name` equals the group name; a single draft covers
/// every group. Returns one index per group; unbound groups get
/// `usize::MAX`, meaning "use the default material".
pub fn bind_materials(groups: &[MeshGroup], drafts: &[MaterialDraft]) -> Vec<usize> {
    if drafts.len() == 1 {
        return vec![0; groups.len()];
    }
    groups
        .iter()
        .map(|g| {
            drafts
                .iter()
                .position(|d| d.original_name == g.material)
                .unwrap_or(usize::MAX)
        })
        .collect()
}

// ------------------------------------------------------------ scene

#[derive(Debug)]
pub struct Scene {
    pub triangles: Vec<Triangle>,
    pub materials: Vec<RenderMaterial>,
    pub lights: Vec<RectLight>,
    pub env: Env,
    pub camera: Camera,
    pub bvh: Bvh,
}

fn apply_matrix(m: &[[f32; 4]; 4], p: Vec3) -> Vec3 {
    [
        m[0][0] * p[0] + m[0][1] * p[1] + m[0][2] * p[2] + m[0][3],
        m[1][0] * p[0] + m[1][1] * p[1] + m[1][2] * p[2] + m[1][3],
        m[2][0] * p[0] + m[2][1] * p[1] + m[2][2] * p[2] + m[2][3],
    ]
}

fn apply_matrix_dir(m: &[[f32; 4]; 4], p: Vec3) -> Vec3 {
    normalize([
        m[0][0] * p[0] + m[0][1] * p[1] + m[0][2] * p[2],
        m[1][0] * p[0] + m[1][1] * p[1] + m[1][2] * p[2],
        m[2][0] * p[0] + m[2][1] * p[1] + m[2][2] * p[2],
    ])
}

impl Scene {
    /// Build a render scene from a mesh. The framing model matrix is
    /// applied to the vertices once, so the camera stays fixed at
    /// (0, 0, D). `materials` is per group: group i uses
    /// materials[min(i, len-1)], so a single material covers every
    /// group (the binding rule itself lives in Task 4).
    pub fn build(mesh: &Mesh, bounds: &Bounds, view: &View, materials: &[RenderMaterial]) -> Scene {
        let frame = framing(bounds, view);
        let materials: Vec<RenderMaterial> = if materials.is_empty() {
            vec![RenderMaterial::default()]
        } else {
            materials.to_vec()
        };
        let fallback_group;
        let groups: &[MeshGroup] = if mesh.groups.is_empty() {
            fallback_group = [MeshGroup {
                material: String::new(),
                start: 0,
                count: mesh.indices.len() as u32,
            }];
            &fallback_group
        } else {
            &mesh.groups
        };
        let has_normals = mesh.normals.len() == mesh.positions.len();
        let mut triangles = Vec::new();
        for (gi, group) in groups.iter().enumerate() {
            let material = gi.min(materials.len() - 1) as u32;
            let start = group.start as usize;
            let end = (group.start + group.count) as usize;
            let range = &mesh.indices[start..end.min(mesh.indices.len())];
            for tri in range.as_chunks::<3>().0 {
                let idx = [tri[0] as usize, tri[1] as usize, tri[2] as usize];
                let p = [
                    apply_matrix(&frame.model, mesh.positions[idx[0]]),
                    apply_matrix(&frame.model, mesh.positions[idx[1]]),
                    apply_matrix(&frame.model, mesh.positions[idx[2]]),
                ];
                let n = if has_normals {
                    [
                        apply_matrix_dir(&frame.model, mesh.normals[idx[0]]),
                        apply_matrix_dir(&frame.model, mesh.normals[idx[1]]),
                        apply_matrix_dir(&frame.model, mesh.normals[idx[2]]),
                    ]
                } else {
                    let face = normalize(cross(sub(p[1], p[0]), sub(p[2], p[0])));
                    [face; 3]
                };
                let uv = [
                    mesh.uvs.get(idx[0]).copied().unwrap_or([0.0, 0.0]),
                    mesh.uvs.get(idx[1]).copied().unwrap_or([0.0, 0.0]),
                    mesh.uvs.get(idx[2]).copied().unwrap_or([0.0, 0.0]),
                ];
                triangles.push(Triangle { p, n, uv, material });
            }
        }
        let bvh = Bvh::build(&triangles);
        Scene {
            triangles,
            materials,
            lights: three_point(),
            env: Env::studio_gradient(),
            camera: Camera::from_framing(&frame),
            bvh,
        }
    }

    /// Test constructor: no lights, caller-provided environment.
    pub fn without_lights(
        mesh: &Mesh,
        bounds: &Bounds,
        view: &View,
        materials: &[RenderMaterial],
        env: Env,
    ) -> Scene {
        let mut scene = Scene::build(mesh, bounds, view, materials);
        scene.lights = Vec::new();
        scene.env = env;
        scene
    }
}

// ----------------------------------------------------------- render

#[derive(Debug, Clone, Copy)]
pub struct RenderOptions {
    pub size: u32,
    pub seconds: f32,
    pub max_samples: Option<u32>,
    pub seed: u32,
    /// Denoise the final image with OIDN (default true). Live
    /// progress previews are never denoised.
    pub denoise: bool,
}

impl Default for RenderOptions {
    fn default() -> Self {
        RenderOptions {
            size: 512,
            seconds: 10.0,
            max_samples: None,
            seed: 0,
            denoise: true,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Progress {
    pub samples: u32,
    pub elapsed: f32,
    /// True only on the final progress event when the final image
    /// was successfully denoised.
    pub denoised: bool,
}

/// Wrapper that lets worker threads write disjoint pixels of one
/// pass film. Safety: within a pass each pixel is written by exactly
/// one worker (rows are handed out atomically), and the film is only
/// read after the thread scope joins.
struct SharedFilm(std::cell::UnsafeCell<Film>);
unsafe impl Sync for SharedFilm {}

impl SharedFilm {
    fn add(&self, pixel: usize, sample: integrator::Sample) {
        // SAFETY: see struct docs.
        unsafe {
            (*self.0.get()).add(
                pixel,
                sample.color,
                sample.alpha,
                sample.albedo,
                sample.normal,
            );
        }
    }

    fn into_inner(self) -> Film {
        self.0.into_inner()
    }
}

/// Trace one pass (1 spp over the whole frame). Returns false when
/// the pass was aborted (cancel or time budget) and must be dropped.
fn run_pass(
    scene: &Scene,
    opts: &RenderOptions,
    sample_index: u32,
    cancel: &AtomicBool,
    start: Instant,
) -> Option<Film> {
    let size = opts.size.max(1);
    let shared = SharedFilm(std::cell::UnsafeCell::new(Film::new(size)));
    let next_row = AtomicU32::new(0);
    let aborted = AtomicBool::new(false);
    let workers = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
        .min(size as usize)
        .max(1);
    std::thread::scope(|scope| {
        for _ in 0..workers {
            scope.spawn(|| loop {
                if cancel.load(Ordering::Relaxed) || start.elapsed().as_secs_f32() >= opts.seconds {
                    aborted.store(true, Ordering::Relaxed);
                    return;
                }
                let y = next_row.fetch_add(1, Ordering::Relaxed);
                if y >= size {
                    return;
                }
                for x in 0..size {
                    let pixel = y * size + x;
                    let sample = integrator::trace(scene, opts.seed, pixel, sample_index, size);
                    shared.add(pixel as usize, sample);
                }
            });
        }
    });
    if aborted.load(Ordering::Relaxed) {
        None
    } else {
        Some(shared.into_inner())
    }
}

/// Progressive render: 1 spp per pass over the whole frame, stopping
/// at `seconds` elapsed, at `max_samples`, or on cancel (which
/// returns `LibError::Cancelled`). `on_pass` runs after every
/// completed pass with the accumulated film.
pub fn render(
    scene: &Scene,
    opts: &RenderOptions,
    cancel: &AtomicBool,
    mut on_pass: impl FnMut(&Progress, &Film),
) -> Result<RgbaImage, LibError> {
    let start = Instant::now();
    let mut film = Film::new(opts.size.max(1));
    loop {
        if cancel.load(Ordering::Relaxed) {
            return Err(LibError::Cancelled);
        }
        if let Some(max) = opts.max_samples {
            if film.samples >= max {
                break;
            }
        } else if film.samples >= 1 && start.elapsed().as_secs_f32() >= opts.seconds {
            break;
        }
        let Some(pass) = run_pass(scene, opts, film.samples, cancel, start) else {
            if cancel.load(Ordering::Relaxed) {
                return Err(LibError::Cancelled);
            }
            break; // time budget hit mid-pass: keep completed passes
        };
        film.absorb(&pass);
        let progress = Progress {
            samples: film.samples,
            elapsed: start.elapsed().as_secs_f32(),
            denoised: false,
        };
        on_pass(&progress, &film);
        if cancel.load(Ordering::Relaxed) {
            return Err(LibError::Cancelled);
        }
        if let Some(max) = opts.max_samples {
            if film.samples >= max {
                break;
            }
        }
        if start.elapsed().as_secs_f32() >= opts.seconds {
            break;
        }
    }
    // Only the final image is denoised; progress previews above stay
    // un-denoised. A denoise failure keeps the noisy image.
    let size = opts.size.max(1);
    let mut denoised = false;
    let image = if opts.denoise && film.samples > 0 {
        match denoise::run(
            &film.color_linear(),
            &film.albedo_linear(),
            &film.normal_linear(),
            size,
            size,
        ) {
            Ok(linear) => {
                denoised = true;
                film.to_rgba8_with_color(&linear, 0.0)
            }
            Err(e) => {
                eprintln!("denoise failed, keeping noisy image: {e}");
                film.to_rgba8(0.0)
            }
        }
    } else {
        film.to_rgba8(0.0)
    };
    let final_progress = Progress {
        samples: film.samples,
        elapsed: start.elapsed().as_secs_f32(),
        denoised,
    };
    on_pass(&final_progress, &film);
    Ok(image)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::uv_sphere;

    fn sphere_scene() -> Scene {
        let mesh = uv_sphere(48, 24);
        let bounds = mesh.bounds();
        Scene::build(&mesh, &bounds, &View::default(), &[])
    }

    fn opts(size: u32) -> RenderOptions {
        RenderOptions {
            size,
            seconds: 60.0,
            max_samples: Some(1),
            seed: 7,
            denoise: false,
        }
    }

    #[test]
    fn empty_scene_is_transparent() {
        let mesh = Mesh {
            positions: Vec::new(),
            normals: Vec::new(),
            uvs: Vec::new(),
            indices: Vec::new(),
            groups: Vec::new(),
            has_uvs: false,
        };
        let bounds = Bounds {
            min: [0.0; 3],
            max: [0.0; 3],
        };
        let scene = Scene::build(&mesh, &bounds, &View::default(), &[]);
        let cancel = AtomicBool::new(false);
        let img = render(&scene, &opts(16), &cancel, |_, _| {}).unwrap();
        assert!(img.pixels().all(|p| p.0 == [0, 0, 0, 0]));
    }

    #[test]
    fn grey_card_pipeline() {
        // Diffuse quad with albedo 0.18 facing the camera under a
        // uniform environment of 1.0: renders linear 0.18, which the
        // full film -> AgX -> PNG chain must display at ~128.
        let mesh = Mesh {
            positions: vec![
                [-1.0, -1.0, 0.0],
                [1.0, -1.0, 0.0],
                [1.0, 1.0, 0.0],
                [-1.0, 1.0, 0.0],
            ],
            normals: vec![[0.0, 0.0, 1.0]; 4],
            uvs: vec![[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
            indices: vec![0, 1, 2, 0, 2, 3],
            groups: vec![MeshGroup {
                material: String::new(),
                start: 0,
                count: 6,
            }],
            has_uvs: true,
        };
        let bounds = mesh.bounds();
        let material = RenderMaterial {
            base_color: [0.18; 3],
            roughness: 1.0,
            specular: 0.0,
            ..RenderMaterial::default()
        };
        let view = View {
            yaw: 0.0,
            pitch: 0.0,
            zoom: 1.0,
        };
        let scene = Scene::without_lights(&mesh, &bounds, &view, &[material], Env::uniform(1.0));
        let cancel = AtomicBool::new(false);
        let o = RenderOptions {
            size: 16,
            seconds: 60.0,
            max_samples: Some(16),
            seed: 7,
            denoise: false,
        };
        let img = render(&scene, &o, &cancel, |_, _| {}).unwrap();
        let center = img.get_pixel(8, 8).0;
        assert_eq!(center[3], 255, "centre alpha: {center:?}");
        for (i, v) in center[..3].iter().enumerate() {
            assert!(
                (120..=136).contains(v),
                "channel {i} of centre pixel: {center:?}"
            );
        }
    }

    #[test]
    fn aov_albedo_and_normal_written() {
        let mesh = uv_sphere(48, 24);
        let bounds = mesh.bounds();
        let material = RenderMaterial {
            base_color: [0.5, 0.5, 0.5],
            roughness: 1.0,
            specular: 0.0,
            ..RenderMaterial::default()
        };
        let scene = Scene::build(&mesh, &bounds, &View::default(), &[material]);
        let cancel = AtomicBool::new(false);
        let o = RenderOptions {
            size: 32,
            seconds: 60.0,
            max_samples: Some(4),
            seed: 7,
            denoise: false,
        };
        let captured = std::cell::RefCell::new(None);
        render(&scene, &o, &cancel, |_, f| {
            *captured.borrow_mut() = Some(f.clone());
        })
        .unwrap();
        let film = captured.into_inner().unwrap();
        let inv = 1.0 / film.samples.max(1) as f32;
        let center = (16 * 32 + 16) as usize;
        let albedo = film.albedo[center];
        for (c, ch) in albedo.iter().enumerate() {
            assert!(
                (ch * inv - 0.5).abs() < 0.02,
                "center albedo channel {c}: {:?} (samples {})",
                film.albedo[center],
                film.samples
            );
        }
        let normal = film.normal[center];
        let n = [normal[0] * inv, normal[1] * inv, normal[2] * inv];
        // The centre pixel ray is slightly off-axis, so the normal
        // is near +Z rather than exactly +Z.
        assert!(
            n[2] > 0.99 && n[0].abs() < 0.1 && n[1].abs() < 0.1,
            "center normal {n:?}"
        );
        // Misses are 0.
        assert_eq!(film.albedo[0], [0.0; 3]);
        assert_eq!(film.normal[0], [0.0; 3]);
    }

    #[test]
    fn denoise_reduces_error() {
        let mesh = uv_sphere(48, 24);
        let bounds = mesh.bounds();
        let material = RenderMaterial {
            base_color: [0.6, 0.35, 0.2],
            roughness: 1.0,
            specular: 0.0,
            ..RenderMaterial::default()
        };
        // Zoom fills the frame with the sphere surface, so the RMSE
        // measures shading noise. Silhouette coverage is alpha,
        // which is never denoised.
        let view = View {
            yaw: -30.0,
            pitch: 15.0,
            zoom: 4.0,
        };
        let scene = Scene::build(&mesh, &bounds, &view, &[material]);
        let cancel = AtomicBool::new(false);
        let render_with = |samples: u32, denoise: bool| {
            let o = RenderOptions {
                size: 32,
                seconds: 600.0,
                max_samples: Some(samples),
                seed: 7,
                denoise,
            };
            render(&scene, &o, &cancel, |_, _| {}).unwrap()
        };
        let reference = render_with(1024, false);
        let noisy = render_with(4, false);
        let denoised = render_with(4, true);
        let rmse = |a: &RgbaImage, b: &RgbaImage| -> f64 {
            let mut sum = 0.0f64;
            let mut count = 0u64;
            for (pa, pb) in a.pixels().zip(b.pixels()) {
                for c in 0..3 {
                    let d = pa.0[c] as f64 / 255.0 - pb.0[c] as f64 / 255.0;
                    sum += d * d;
                    count += 1;
                }
            }
            (sum / count as f64).sqrt()
        };
        let noisy_rmse = rmse(&noisy, &reference);
        let denoised_rmse = rmse(&denoised, &reference);
        assert!(
            denoised_rmse <= 0.5 * noisy_rmse,
            "denoised RMSE {denoised_rmse} must be <= 0.5 x noisy RMSE {noisy_rmse}"
        );
    }

    #[test]
    fn denoise_off_is_bitwise_previous_output() {
        assert!(RenderOptions::default().denoise, "denoise defaults to true");
        let scene = sphere_scene();
        let cancel = AtomicBool::new(false);
        let o = RenderOptions {
            size: 32,
            seconds: 60.0,
            max_samples: Some(4),
            seed: 7,
            denoise: false,
        };
        let captured = std::cell::RefCell::new(None);
        let img = render(&scene, &o, &cancel, |_, f| {
            *captured.borrow_mut() = Some(f.clone());
        })
        .unwrap();
        let film = captured.into_inner().unwrap();
        assert_eq!(img.as_raw(), film.to_rgba8(0.0).as_raw());
    }

    /// Ring opacity mask (white annulus on black), built in code.
    fn ring_opacity_texture() -> std::sync::Arc<Texture> {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ring.png");
        let mut img = image::RgbaImage::new(128, 128);
        for (x, y, p) in img.enumerate_pixels_mut() {
            let dx = x as f32 - 64.0;
            let dy = y as f32 - 64.0;
            let r = (dx * dx + dy * dy).sqrt();
            let v = if (20.0..=44.0).contains(&r) { 255 } else { 0 };
            *p = image::Rgba([v, v, v, 255]);
        }
        img.save(&path).unwrap();
        std::sync::Arc::new(Texture::load(&path, false).unwrap())
    }

    fn facing_quad_scene(material: RenderMaterial) -> Scene {
        let mesh = Mesh {
            positions: vec![
                [-1.0, -1.0, 0.0],
                [1.0, -1.0, 0.0],
                [1.0, 1.0, 0.0],
                [-1.0, 1.0, 0.0],
            ],
            normals: vec![[0.0, 0.0, 1.0]; 4],
            uvs: vec![[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
            indices: vec![0, 1, 2, 0, 2, 3],
            groups: vec![MeshGroup {
                material: String::new(),
                start: 0,
                count: 6,
            }],
            has_uvs: true,
        };
        let bounds = mesh.bounds();
        let view = View {
            yaw: 0.0,
            pitch: 0.0,
            zoom: 1.0,
        };
        Scene::without_lights(&mesh, &bounds, &view, &[material], Env::uniform(1.0))
    }

    #[test]
    fn opacity_mask_cuts_holes() {
        let material = RenderMaterial {
            base_color: [0.8; 3],
            roughness: 1.0,
            specular: 0.0,
            opacity_map: Some(ring_opacity_texture()),
            ..RenderMaterial::default()
        };
        let scene = facing_quad_scene(material);
        let cancel = AtomicBool::new(false);
        let o = RenderOptions {
            size: 32,
            seconds: 60.0,
            max_samples: Some(16),
            seed: 7,
            denoise: false,
        };
        let img = render(&scene, &o, &cancel, |_, _| {}).unwrap();
        // Centre of the ring mask is black: fully transparent.
        assert_eq!(img.get_pixel(16, 16).0[3], 0);
        // On the white annulus: fully opaque.
        assert_eq!(img.get_pixel(21, 16).0[3], 255);
        // Outside the annulus: black again.
        assert_eq!(img.get_pixel(28, 16).0[3], 0);
    }

    #[test]
    fn opacity_value_half_partial_alpha() {
        let material = RenderMaterial {
            base_color: [0.8; 3],
            roughness: 1.0,
            specular: 0.0,
            opacity: 0.5,
            ..RenderMaterial::default()
        };
        let scene = facing_quad_scene(material);
        let cancel = AtomicBool::new(false);
        let o = RenderOptions {
            size: 16,
            seconds: 60.0,
            max_samples: Some(64),
            seed: 7,
            denoise: false,
        };
        let img = render(&scene, &o, &cancel, |_, _| {}).unwrap();
        let alpha = img.get_pixel(8, 8).0[3] as i32;
        assert!(
            (128 - 26..=128 + 26).contains(&alpha),
            "centre alpha {alpha}"
        );
    }

    #[test]
    fn shadow_rays_pass_through_holes() {
        // Wall at z = -1, smaller cut-out quad at z = 0, key light
        // only, no environment. Wall point A's light ray threads
        // the mask's black centre (lit); wall point B's ray hits
        // the white annulus (shadowed).
        let quad = |z: f32, half: f32, material: u32| -> Vec<Triangle> {
            let p = [
                [-half, -half, z],
                [half, -half, z],
                [half, half, z],
                [-half, half, z],
            ];
            let n = [[0.0, 0.0, 1.0]; 4];
            let uv = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
            vec![
                Triangle {
                    p: [p[0], p[1], p[2]],
                    n: [n[0], n[1], n[2]],
                    uv: [uv[0], uv[1], uv[2]],
                    material,
                },
                Triangle {
                    p: [p[0], p[2], p[3]],
                    n: [n[0], n[2], n[3]],
                    uv: [uv[0], uv[2], uv[3]],
                    material,
                },
            ]
        };
        let mut triangles = quad(-1.0, 2.0, 0);
        triangles.extend(quad(0.0, 1.3, 1));
        let wall = RenderMaterial {
            base_color: [0.8; 3],
            roughness: 1.0,
            specular: 0.0,
            ..RenderMaterial::default()
        };
        let cutter = RenderMaterial {
            opacity_map: Some(ring_opacity_texture()),
            ..wall.clone()
        };
        let bounds = Bounds {
            min: [-2.0, -2.0, -1.0],
            max: [2.0, 2.0, 0.0],
        };
        let view = View {
            yaw: 0.0,
            pitch: 0.0,
            zoom: 1.0,
        };
        // Apply the framing transform, as Scene::build would; the
        // sample points below are derived from the transformed
        // triangles and the world-space light, so they stay exact.
        let frame = framing(&bounds, &view);
        let mut triangles = triangles;
        for tri in &mut triangles {
            tri.p = tri.p.map(|p| apply_matrix(&frame.model, p));
        }
        let scene = Scene {
            bvh: Bvh::build(&triangles),
            triangles,
            materials: vec![wall, cutter],
            lights: vec![three_point()[0]],
            env: Env::uniform(0.0),
            camera: Camera::from_framing(&frame),
        };
        let cancel = AtomicBool::new(false);
        let o = RenderOptions {
            size: 64,
            seconds: 60.0,
            max_samples: Some(64),
            seed: 7,
            denoise: false,
        };
        let img = render(&scene, &o, &cancel, |_, _| {}).unwrap();
        // Derive the wall sample points in world space: back-project
        // the key light through the mask's black centre (hole) and
        // through the white annulus (blocks light) onto the wall.
        let wall_z = scene.triangles[0].p[0][2];
        let c = [
            scene.triangles[2].p[0],
            scene.triangles[2].p[1],
            scene.triangles[2].p[2],
            scene.triangles[3].p[2],
        ];
        let centre = [
            (c[0][0] + c[2][0]) / 2.0,
            (c[0][1] + c[2][1]) / 2.0,
            (c[0][2] + c[2][2]) / 2.0,
        ];
        let half = ((c[1][0] - c[0][0]).abs() / 2.0).max(1e-6);
        let key = scene.lights[0].center;
        let on_wall = |through: [f32; 3]| -> [f32; 3] {
            let t = (wall_z - key[2]) / (through[2] - key[2]);
            [
                key[0] + (through[0] - key[0]) * t,
                key[1] + (through[1] - key[1]) * t,
                wall_z,
            ]
        };
        let lum = |p: [f32; 3]| -> i32 {
            let px = scene.camera.project_point(p, 64);
            let pix = img.get_pixel(px[0] as u32, px[1] as u32).0;
            (pix[0] as i32 + pix[1] as i32 + pix[2] as i32) / 3
        };
        let lit = lum(on_wall(centre));
        let shadowed = lum(on_wall([centre[0] + 0.5 * half, centre[1], centre[2]]));
        assert!(
            lit > shadowed + 40,
            "hole-lit {lit} must exceed ring-shadowed {shadowed} by 40"
        );
    }

    #[test]
    fn sphere_covers_center() {
        let scene = sphere_scene();
        let cancel = AtomicBool::new(false);
        let img = render(&scene, &opts(64), &cancel, |_, _| {}).unwrap();
        let center = img.get_pixel(32, 32).0;
        assert_eq!(center[3], 255);
        assert!(center[0] > 40, "lit sphere centre: {center:?}");
        assert_eq!(img.get_pixel(0, 0).0[3], 0);
        assert_eq!(img.get_pixel(63, 63).0[3], 0);
    }

    #[test]
    fn deterministic_with_samples() {
        let scene = sphere_scene();
        let cancel = AtomicBool::new(false);
        let mut o = opts(32);
        o.max_samples = Some(4);
        let a = render(&scene, &o, &cancel, |_, _| {}).unwrap();
        let b = render(&scene, &o, &cancel, |_, _| {}).unwrap();
        assert_eq!(a.as_raw(), b.as_raw());
    }

    #[test]
    fn time_budget_respected() {
        let scene = sphere_scene();
        let cancel = AtomicBool::new(false);
        let o = RenderOptions {
            size: 64,
            seconds: 0.3,
            max_samples: None,
            seed: 7,
            denoise: false,
        };
        let samples = std::cell::Cell::new(0u32);
        let t0 = Instant::now();
        let img = render(&scene, &o, &cancel, |p, _| samples.set(p.samples)).unwrap();
        let elapsed = t0.elapsed().as_secs_f32();
        assert!(elapsed < 1.0, "took {elapsed}s");
        assert!(samples.get() >= 1, "samples {}", samples.get());
        assert_eq!(img.get_pixel(32, 32).0[3], 255);
    }

    fn draft(name: &str) -> MaterialDraft {
        MaterialDraft {
            original_name: name.to_string(),
            name: name.to_string(),
            attributes: Vec::new(),
        }
    }

    #[test]
    fn bind_single_material_to_all_groups() {
        let groups = vec![
            MeshGroup {
                material: "A".into(),
                start: 0,
                count: 3,
            },
            MeshGroup {
                material: "B".into(),
                start: 3,
                count: 3,
            },
        ];
        assert_eq!(bind_materials(&groups, &[draft("Only")]), vec![0, 0]);
    }

    #[test]
    fn bind_by_original_name() {
        let groups = vec![
            MeshGroup {
                material: "A".into(),
                start: 0,
                count: 3,
            },
            MeshGroup {
                material: "B".into(),
                start: 3,
                count: 3,
            },
        ];
        assert_eq!(
            bind_materials(&groups, &[draft("B"), draft("A")]),
            vec![1, 0]
        );
    }

    #[test]
    fn unbound_group_gets_defaults() {
        let groups = vec![
            MeshGroup {
                material: "A".into(),
                start: 0,
                count: 3,
            },
            MeshGroup {
                material: "C".into(),
                start: 3,
                count: 3,
            },
        ];
        assert_eq!(
            bind_materials(&groups, &[draft("A"), draft("B")]),
            vec![0, usize::MAX]
        );
    }

    #[test]
    fn cancel_stops_render() {
        let scene = sphere_scene();
        let cancel = AtomicBool::new(false);
        let o = RenderOptions {
            size: 32,
            seconds: 600.0,
            max_samples: None,
            seed: 7,
            denoise: false,
        };
        let result = render(&scene, &o, &cancel, |_, _| {
            cancel.store(true, Ordering::Relaxed);
        });
        assert!(matches!(result, Err(LibError::Cancelled)), "{result:?}");
    }
}
