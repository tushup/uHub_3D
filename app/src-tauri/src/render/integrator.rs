//! Path integrator (spec §A1.6): 8 max bounces, Russian roulette
//! from bounce 3, indirect clamp 10, stochastic transparency on
//! camera, bounce and shadow rays, and MIS with the power heuristic
//! between rect-light sampling and BSDF sampling. The environment
//! is reached by bounce rays only; camera misses stay transparent.
// shortcut: no denoiser and no adaptive sampling; noise falls as 1/sqrt(spp), revisit if thumbnails look grainy at 10 s.

use super::bsdf::{self, BsdfParams, Frame};
use super::light::RectLight;
use super::sampler;
use super::{add, dot, mul, normalize, scale, sub, Scene, Triangle, Vec3};

const MAX_BOUNCE: u32 = 8;

struct Ctx<'a> {
    scene: &'a Scene,
    seed: u32,
    pixel: u32,
    sample_index: u32,
}

impl Ctx<'_> {
    fn s(&self, dim: u32) -> f32 {
        sampler::sample(self.seed, self.pixel, self.sample_index, dim)
    }
}

/// Dimension slots per bounce: 0 pick, 1-2 BSDF, 3 roulette,
/// 4..10 light samples (3 lights × 2).
fn dim_bounce(bounce: u32, slot: u32) -> u32 {
    2 + bounce * 16 + slot
}

fn opacity_at(scene: &Scene, tri: &Triangle, bary: [f32; 3]) -> f32 {
    let mat = &scene.materials[tri.material as usize];
    match &mat.opacity_map {
        Some(tex) => tex.sample(tri.uv(bary)[0], tri.uv(bary)[1])[0],
        None => mat.opacity,
    }
}

fn surface_params(
    scene: &Scene,
    tri: &Triangle,
    bary: [f32; 3],
    dir: Vec3,
) -> (BsdfParams, Frame, Vec3) {
    let mat = &scene.materials[tri.material as usize];
    let uv = tri.uv(bary);
    let base_color = match &mat.base_color_map {
        Some(tex) => {
            let s = tex.sample(uv[0], uv[1]);
            [s[0], s[1], s[2]]
        }
        None => mat.base_color,
    };
    let scalar = |map: &Option<std::sync::Arc<super::texture::Texture>>, value: f32| -> f32 {
        match map {
            Some(tex) => tex.sample(uv[0], uv[1])[0],
            None => value,
        }
    };
    let roughness = scalar(&mat.roughness_map, mat.roughness).clamp(0.0, 1.0);
    let metallic = scalar(&mat.metallic_map, mat.metallic).clamp(0.0, 1.0);
    let specular = scalar(&mat.specular_map, mat.specular).clamp(0.0, 1.0);
    let opacity = scalar(&mat.opacity_map, mat.opacity).clamp(0.0, 1.0);
    let emissive = match &mat.emissive_map {
        Some(tex) => {
            let s = tex.sample(uv[0], uv[1]);
            [s[0], s[1], s[2]]
        }
        None => mat.emissive,
    };

    let mut n = tri.normal(bary);
    // Tangent frame from the triangle's UV derivatives.
    let mut frame = Frame::new(n);
    if let Some(map) = &mat.normal_map {
        let s = map.sample(uv[0], uv[1]);
        let x = (s[0] * 2.0 - 1.0) * mat.normal_strength;
        let mut y = (s[1] * 2.0 - 1.0) * mat.normal_strength;
        if mat.normal_directx {
            y = -y;
        }
        let z = s[2] * 2.0 - 1.0;
        let e1 = sub(tri.p[1], tri.p[0]);
        let e2 = sub(tri.p[2], tri.p[0]);
        let duv1 = [tri.uv[1][0] - tri.uv[0][0], tri.uv[1][1] - tri.uv[0][1]];
        let duv2 = [tri.uv[2][0] - tri.uv[0][0], tri.uv[2][1] - tri.uv[0][1]];
        let det = duv1[0] * duv2[1] - duv2[0] * duv1[1];
        if det.abs() > 1e-8 {
            let f = 1.0 / det;
            let t = scale(sub(scale(e1, duv2[1]), scale(e2, duv1[1])), f);
            let b = scale(sub(scale(e2, duv1[0]), scale(e1, duv2[0])), f);
            let perturbed = normalize(add(add(scale(t, x), scale(b, y)), scale(n, z)));
            if dot(perturbed, n) > 0.0 {
                frame = Frame::from_tangent(perturbed, t);
                n = perturbed;
            }
        }
    }
    if dot(n, dir) > 0.0 {
        // Two-sided: face the shading normal towards the ray.
        n = scale(n, -1.0);
        frame = Frame { n, ..frame };
        if dot(frame.t, n) > 1e-6 {
            frame = Frame::new(n);
        }
    }

    let diffuse = scale(base_color, 1.0 - metallic);
    let f0 = [
        (0.08 * specular) * (1.0 - metallic) + base_color[0] * metallic,
        (0.08 * specular) * (1.0 - metallic) + base_color[1] * metallic,
        (0.08 * specular) * (1.0 - metallic) + base_color[2] * metallic,
    ];
    let albedo = [
        base_color[0].clamp(0.0, 1.0),
        base_color[1].clamp(0.0, 1.0),
        base_color[2].clamp(0.0, 1.0),
    ];
    (
        BsdfParams {
            diffuse,
            f0,
            alpha: roughness.max(0.02).powi(2),
            emissive,
            opacity,
        },
        frame,
        albedo,
    )
}

/// Stochastic shadow ray: each surface along the ray blocks it with
/// probability equal to its opacity.
fn blocked(ctx: &Ctx, origin: Vec3, dir: Vec3, t_max: f32, layer: &mut u32) -> bool {
    let scene = ctx.scene;
    let mut o = origin;
    let mut remaining = t_max;
    for _ in 0..16 {
        let Some(hit) = scene.bvh.intersect(&scene.triangles, o, dir) else {
            return false;
        };
        if hit.t >= remaining {
            return false;
        }
        let tri = &scene.triangles[hit.tri];
        *layer += 1;
        if ctx.s(900 + *layer) < opacity_at(scene, tri, hit.bary) {
            return true;
        }
        let p = add(o, scale(dir, hit.t));
        o = add(p, scale(dir, 1e-4));
        remaining -= hit.t + 1e-4;
    }
    false
}

fn power_heuristic(a: f32, b: f32) -> f32 {
    let a2 = a * a;
    let b2 = b * b;
    if a2 + b2 <= 0.0 {
        0.0
    } else {
        a2 / (a2 + b2)
    }
}

fn direct_light(
    ctx: &Ctx,
    light: &RectLight,
    point: Vec3,
    frame: &Frame,
    params: &BsdfParams,
    wo: Vec3,
    u: [f32; 2],
) -> Vec3 {
    let lp = light.sample_point(u[0], u[1]);
    let to = sub(lp, point);
    let dist2 = dot(to, to);
    if dist2 <= 0.0 {
        return [0.0; 3];
    }
    let dist = dist2.sqrt();
    let wi = scale(to, 1.0 / dist);
    let cos_s = dot(frame.n, wi);
    if cos_s <= 0.0 {
        return [0.0; 3];
    }
    let cos_l = dot(scale(wi, -1.0), light.normal);
    if cos_l <= 0.0 {
        return [0.0; 3];
    }
    let mut layer = 0u32;
    let origin = add(point, scale(frame.n, 1e-4));
    if blocked(ctx, origin, wi, dist - 1e-3, &mut layer) {
        return [0.0; 3];
    }
    let pdf_light = dist2 / (cos_l * light.area());
    if pdf_light <= 0.0 {
        return [0.0; 3];
    }
    let f = bsdf::eval(params, frame, wo, wi);
    if f == [0.0; 3] {
        return [0.0; 3];
    }
    let pdf_bsdf = bsdf::pdf(params, frame, wo, wi);
    let w = power_heuristic(pdf_light, pdf_bsdf);
    scale(mul(f, light.radiance), cos_s * w / pdf_light)
}

/// One camera sample: linear radiance, coverage alpha, and the
/// first-hit albedo / world normal guides for denoising (0 on miss).
pub struct Sample {
    pub color: Vec3,
    pub alpha: f32,
    pub albedo: Vec3,
    pub normal: Vec3,
}

/// Trace one camera sample.
pub fn trace(scene: &Scene, seed: u32, pixel: u32, sample_index: u32, size: u32) -> Sample {
    let ctx = Ctx {
        scene,
        seed,
        pixel,
        sample_index,
    };
    let px = pixel % size;
    let py = pixel / size;
    let offset = [
        sampler::filter_sample(ctx.s(0)),
        sampler::filter_sample(ctx.s(1)),
    ];
    let (mut origin, mut dir) = scene.camera.ray(px, py, size, offset);
    let mut radiance = [0.0; 3];
    let mut throughput = [1.0f32; 3];
    let mut alpha = 0.0f32;
    let mut aov_albedo = [0.0; 3];
    let mut aov_normal = [0.0; 3];
    let mut opacity_layer = 0u32;

    for bounce in 0..=MAX_BOUNCE {
        // First shaded hit along the ray, with stochastic
        // transparency pass-through.
        let mut seg_origin = origin;
        let found = loop {
            let Some(hit) = scene.bvh.intersect(&scene.triangles, seg_origin, dir) else {
                break None;
            };
            let tri = &scene.triangles[hit.tri];
            opacity_layer += 1;
            if ctx.s(500 + opacity_layer) < opacity_at(scene, tri, hit.bary) {
                break Some((hit, seg_origin));
            }
            if opacity_layer > 64 {
                break None;
            }
            let p = add(seg_origin, scale(dir, hit.t));
            seg_origin = add(p, scale(dir, 1e-4));
        };
        let Some((hit, seg_origin)) = found else {
            if bounce > 0 {
                radiance = add(radiance, mul(throughput, scene.env.radiance(dir)));
            }
            break;
        };
        let tri = &scene.triangles[hit.tri];
        let point = add(seg_origin, scale(dir, hit.t));
        let (params, frame, albedo) = surface_params(scene, tri, hit.bary, dir);
        if bounce == 0 {
            alpha = 1.0;
            aov_albedo = albedo;
            aov_normal = frame.n;
        }
        radiance = add(radiance, mul(throughput, params.emissive));
        if bounce == MAX_BOUNCE {
            break;
        }
        let wo = scale(dir, -1.0);
        for (li, light) in scene.lights.iter().enumerate() {
            let base = dim_bounce(bounce, 4 + li as u32 * 2);
            let direct = direct_light(
                &ctx,
                light,
                point,
                &frame,
                &params,
                wo,
                [ctx.s(base), ctx.s(base + 1)],
            );
            radiance = add(radiance, mul(throughput, direct));
        }
        let base = dim_bounce(bounce, 0);
        let Some(wi) = bsdf::sample(
            &params,
            &frame,
            wo,
            ctx.s(base),
            ctx.s(base + 1),
            ctx.s(base + 2),
        ) else {
            break;
        };
        let cos_i = dot(frame.n, wi);
        if cos_i <= 0.0 {
            break;
        }
        let f = bsdf::eval(&params, &frame, wo, wi);
        let pdf = bsdf::pdf(&params, &frame, wo, wi);
        if pdf <= 0.0 {
            break;
        }
        throughput = mul(throughput, scale(f, cos_i / pdf));
        throughput = [
            throughput[0].min(10.0),
            throughput[1].min(10.0),
            throughput[2].min(10.0),
        ];
        if bounce >= 3 {
            let q = throughput[0]
                .max(throughput[1])
                .max(throughput[2])
                .clamp(0.0, 0.95);
            if q <= 0.0 || ctx.s(dim_bounce(bounce, 3)) > q {
                break;
            }
            throughput = scale(throughput, 1.0 / q);
        }
        origin = add(point, scale(frame.n, 1e-4));
        dir = wi;
    }
    Sample {
        color: radiance,
        alpha,
        albedo: aov_albedo,
        normal: aov_normal,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::uv_sphere;
    use crate::render::light::{three_point, Env};
    use crate::render::{render, RenderMaterial, RenderOptions, Scene};
    use crate::view::{Bounds, View};
    use std::sync::atomic::AtomicBool;

    fn render_film(
        scene: &Scene,
        size: u32,
        samples: u32,
    ) -> (crate::render::Film, image::RgbaImage) {
        let cancel = AtomicBool::new(false);
        let opts = RenderOptions {
            size,
            seconds: 600.0,
            max_samples: Some(samples),
            seed: 3,
            denoise: false,
        };
        let film = std::cell::RefCell::new(None);
        let img = render(scene, &opts, &cancel, |_, f| {
            *film.borrow_mut() = Some(f.clone());
        })
        .unwrap();
        (film.into_inner().unwrap(), img)
    }

    fn mean_covered(film: &crate::render::Film) -> [f64; 3] {
        let inv = 1.0 / film.samples.max(1) as f64;
        let mut sum = [0.0f64; 3];
        let mut count = 0u32;
        for (i, a) in film.alpha.iter().enumerate() {
            if *a * inv as f32 > 0.5 {
                for c in 0..3 {
                    sum[c] += film.rgb[i][c] as f64 * inv;
                }
                count += 1;
            }
        }
        assert!(count > 0);
        [
            sum[0] / count as f64,
            sum[1] / count as f64,
            sum[2] / count as f64,
        ]
    }

    #[test]
    fn furnace_white_diffuse() {
        let mesh = uv_sphere(48, 24);
        let bounds: Bounds = mesh.bounds();
        let mat = RenderMaterial {
            base_color: [1.0; 3],
            roughness: 1.0,
            ..RenderMaterial::default()
        };
        let scene =
            Scene::without_lights(&mesh, &bounds, &View::default(), &[mat], Env::uniform(1.0));
        let (film, _img) = render_film(&scene, 32, 64);
        let mean = mean_covered(&film);
        for (c, v) in mean.iter().enumerate() {
            assert!((0.97..=1.03).contains(v), "channel {c}: {v}");
        }
    }

    #[test]
    fn furnace_metal_ggx() {
        let mesh = uv_sphere(48, 24);
        let bounds: Bounds = mesh.bounds();
        let mat = RenderMaterial {
            base_color: [1.0; 3],
            metallic: 1.0,
            roughness: 0.5,
            ..RenderMaterial::default()
        };
        let scene =
            Scene::without_lights(&mesh, &bounds, &View::default(), &[mat], Env::uniform(1.0));
        let (film, _img) = render_film(&scene, 32, 64);
        let mean = mean_covered(&film);
        for (c, v) in mean.iter().enumerate() {
            assert!((0.85..=1.01).contains(v), "channel {c}: {v}");
        }
    }

    #[test]
    fn opacity_zero_is_transparent() {
        let mesh = uv_sphere(48, 24);
        let bounds: Bounds = mesh.bounds();
        let mat = RenderMaterial {
            opacity: 0.0,
            ..RenderMaterial::default()
        };
        let scene = Scene::build(&mesh, &bounds, &View::default(), &[mat]);
        let (_film, img) = render_film(&scene, 32, 4);
        assert!(
            img.pixels().all(|p| p.0[3] == 0),
            "centre {:?}",
            img.get_pixel(16, 16)
        );
    }

    #[test]
    fn rect_light_irradiance() {
        // White diffuse receiver at the origin facing the key light,
        // key only, no env: outgoing radiance is E·colour/π. The
        // scene geometry is empty so nothing blocks the shadow ray.
        let mesh = crate::mesh::Mesh {
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
        let mut scene = Scene::without_lights(
            &mesh,
            &bounds,
            &View::default(),
            &[RenderMaterial::default()],
            Env::uniform(0.0),
        );
        let key = three_point()[0];
        scene.lights = vec![key];
        let ctx = Ctx {
            scene: &scene,
            seed: 5,
            pixel: 0,
            sample_index: 0,
        };
        let n = normalize(key.center);
        let frame = Frame::new(n);
        let params = BsdfParams {
            diffuse: [1.0; 3],
            f0: [0.0; 3],
            alpha: 1.0,
            emissive: [0.0; 3],
            opacity: 1.0,
        };
        let mut sum = [0.0f64; 3];
        let n_samples = 4096;
        for i in 0..n_samples {
            let ctx = Ctx {
                sample_index: i,
                ..ctx
            };
            let base = dim_bounce(0, 4);
            let c = direct_light(
                &ctx,
                &key,
                [0.0; 3],
                &frame,
                &params,
                n,
                [ctx.s(base), ctx.s(base + 1)],
            );
            for ch in 0..3 {
                sum[ch] += c[ch] as f64;
            }
        }
        let want = [
            3.0 / std::f64::consts::PI,
            3.0 * 0.96 / std::f64::consts::PI,
            3.0 * 0.90 / std::f64::consts::PI,
        ];
        for ch in 0..3 {
            let got = sum[ch] / n_samples as f64;
            assert!(
                (got / want[ch] - 1.0).abs() < 0.05,
                "channel {ch}: got {got} want {}",
                want[ch]
            );
        }
    }
}
