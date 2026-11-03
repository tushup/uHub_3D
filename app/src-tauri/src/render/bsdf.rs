//! Principled BSDF subset (spec §A1.6): Lambert diffuse weighted by
//! 1 − metallic, GGX specular with visible-normal sampling and
//! height-correlated Smith G2, Schlick Fresnel. Dielectric
//! F0 = 0.08 · specular; metal F0 = baseColor; alpha = roughness²
//! with roughness clamped to ≥ 0.02.
// shortcut: no multiscatter GGX compensation; metals lose the
// energy single-scatter GGX misses, as in Cycles without it.

use std::f32::consts::PI;

use super::{cross, dot, normalize, Vec3};

#[derive(Debug, Clone, Copy)]
pub struct Frame {
    pub t: Vec3,
    pub b: Vec3,
    pub n: Vec3,
}

impl Frame {
    pub fn new(n: Vec3) -> Frame {
        let helper = if n[1].abs() < 0.9 { [0.0, 1.0, 0.0] } else { [1.0, 0.0, 0.0] };
        let t = normalize(cross(helper, n));
        let b = cross(n, t);
        Frame { t, b, n }
    }

    pub fn from_tangent(n: Vec3, t: Vec3) -> Frame {
        // Orthonormalize t against n.
        let t = normalize([
            t[0] - n[0] * dot(n, t),
            t[1] - n[1] * dot(n, t),
            t[2] - n[2] * dot(n, t),
        ]);
        let b = cross(n, t);
        Frame { t, b, n }
    }

    pub fn to_local(&self, v: Vec3) -> Vec3 {
        [dot(v, self.t), dot(v, self.b), dot(v, self.n)]
    }

    pub fn to_world(&self, v: Vec3) -> Vec3 {
        [
            self.t[0] * v[0] + self.b[0] * v[1] + self.n[0] * v[2],
            self.t[1] * v[0] + self.b[1] * v[1] + self.n[1] * v[2],
            self.t[2] * v[0] + self.b[2] * v[1] + self.n[2] * v[2],
        ]
    }
}

#[derive(Debug, Clone, Copy)]
pub struct BsdfParams {
    /// base_color · (1 − metallic)
    pub diffuse: Vec3,
    /// mix(0.08 · specular, base_color, metallic)
    pub f0: Vec3,
    /// max(roughness, 0.02)²
    pub alpha: f32,
    pub emissive: Vec3,
    pub opacity: f32,
}

pub fn luminance(c: Vec3) -> f32 {
    0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2]
}

fn fresnel(cos: f32, f0: Vec3) -> Vec3 {
    let m = (1.0 - cos.clamp(0.0, 1.0)).powi(5);
    [
        f0[0] + (1.0 - f0[0]) * m,
        f0[1] + (1.0 - f0[1]) * m,
        f0[2] + (1.0 - f0[2]) * m,
    ]
}

/// (p_diffuse, p_specular) from estimated albedos: the specular
/// weight is the Fresnel reflectance at the view angle, the diffuse
/// weight is what it leaves of the diffuse albedo.
pub fn lobe_probs(p: &BsdfParams, frame: &Frame, wo: Vec3) -> (f32, f32) {
    let wo_l = frame.to_local(wo);
    let f = fresnel(wo_l[2], p.f0);
    let w_spec = luminance(f).max(0.0);
    let w_diff = luminance(p.diffuse).max(0.0) * (1.0 - w_spec);
    let total = w_diff + w_spec;
    if total <= 1e-6 {
        (1.0, 0.0)
    } else {
        (w_diff / total, w_spec / total)
    }
}

fn d_ggx(cos_nh: f32, alpha: f32) -> f32 {
    if cos_nh <= 0.0 {
        return 0.0;
    }
    let a2 = alpha * alpha;
    let denom = cos_nh * cos_nh * (a2 - 1.0) + 1.0;
    a2 / (PI * denom * denom)
}

fn lambda(cos: f32, alpha: f32) -> f32 {
    if cos <= 0.0 {
        return 0.0;
    }
    let tan2 = (1.0 - cos * cos) / (cos * cos);
    ((1.0 + alpha * alpha * tan2).sqrt() - 1.0) / 2.0
}

fn g2(cos_o: f32, cos_i: f32, alpha: f32) -> f32 {
    1.0 / (1.0 + lambda(cos_o, alpha) + lambda(cos_i, alpha))
}

fn g1(cos: f32, alpha: f32) -> f32 {
    1.0 / (1.0 + lambda(cos, alpha))
}

/// f(wo, wi) without the cosine factor. `wo` and `wi` point away
/// from the surface.
pub fn eval(p: &BsdfParams, frame: &Frame, wo: Vec3, wi: Vec3) -> Vec3 {
    let wo_l = frame.to_local(wo);
    let wi_l = frame.to_local(wi);
    if wi_l[2] <= 0.0 || wo_l[2] <= 0.0 {
        return [0.0; 3];
    }
    let mut out = [
        p.diffuse[0] / PI,
        p.diffuse[1] / PI,
        p.diffuse[2] / PI,
    ];
    let wh = normalize([
        wo_l[0] + wi_l[0],
        wo_l[1] + wi_l[1],
        wo_l[2] + wi_l[2],
    ]);
    if wh[2] > 0.0 {
        let cos_d = (wo_l[0] * wh[0] + wo_l[1] * wh[1] + wo_l[2] * wh[2]).clamp(0.0, 1.0);
        let f = fresnel(cos_d, p.f0);
        let d = d_ggx(wh[2], p.alpha);
        let g = g2(wo_l[2], wi_l[2], p.alpha);
        let scale = d * g / (4.0 * wo_l[2] * wi_l[2]);
        out[0] += f[0] * scale;
        out[1] += f[1] * scale;
        out[2] += f[2] * scale;
    }
    out
}

fn spec_pdf(p: &BsdfParams, wo_l: Vec3, wi_l: Vec3) -> f32 {
    let wh = normalize([
        wo_l[0] + wi_l[0],
        wo_l[1] + wi_l[1],
        wo_l[2] + wi_l[2],
    ]);
    if wh[2] <= 0.0 {
        return 0.0;
    }
    let cos_d = (wo_l[0] * wh[0] + wo_l[1] * wh[1] + wo_l[2] * wh[2]).clamp(0.0, 1.0);
    if cos_d <= 0.0 {
        return 0.0;
    }
    // Visible-normal pdf, converted from half-vector to direction.
    let pdf_wh = g1(wo_l[2], p.alpha) * d_ggx(wh[2], p.alpha) * cos_d / wo_l[2];
    pdf_wh / (4.0 * cos_d)
}

/// Mixture pdf of `sample` for direction `wi`.
pub fn pdf(p: &BsdfParams, frame: &Frame, wo: Vec3, wi: Vec3) -> f32 {
    let wo_l = frame.to_local(wo);
    let wi_l = frame.to_local(wi);
    if wi_l[2] <= 0.0 || wo_l[2] <= 0.0 {
        return 0.0;
    }
    let (p_diff, p_spec) = lobe_probs(p, frame, wo);
    p_diff * (wi_l[2] / PI) + p_spec * spec_pdf(p, wo_l, wi_l)
}

/// Visible-normal sample of the GGX half-vector (Heitz 2014).
fn sample_wh_visible(wo_l: Vec3, alpha: f32, u1: f32, u2: f32) -> Vec3 {
    let vh = normalize([alpha * wo_l[0], alpha * wo_l[1], wo_l[2].max(0.0)]);
    let lensq = vh[0] * vh[0] + vh[1] * vh[1];
    let t1 = if lensq > 0.0 {
        let inv = 1.0 / lensq.sqrt();
        [-vh[1] * inv, vh[0] * inv, 0.0]
    } else {
        [1.0, 0.0, 0.0]
    };
    let t2 = cross(vh, t1);
    let r = u1.sqrt();
    let phi = 2.0 * PI * u2;
    let t1c = r * phi.cos();
    let mut t2c = r * phi.sin();
    let s = 0.5 * (1.0 + vh[2]);
    t2c = (1.0 - s) * (1.0 - t1c * t1c).max(0.0).sqrt() + s * t2c;
    let nz = (1.0 - t1c * t1c - t2c * t2c).max(0.0).sqrt();
    let nh = [
        t1[0] * t1c + t2[0] * t2c + vh[0] * nz,
        t1[1] * t1c + t2[1] * t2c + vh[1] * nz,
        t1[2] * t1c + t2[2] * t2c + vh[2] * nz,
    ];
    normalize([alpha * nh[0], alpha * nh[1], nh[2].max(0.0)])
}

/// Sample a bounce direction. Returns the world-space direction, or
/// None when the sampled direction falls below the surface.
pub fn sample(
    p: &BsdfParams,
    frame: &Frame,
    wo: Vec3,
    u_pick: f32,
    u1: f32,
    u2: f32,
) -> Option<Vec3> {
    let wo_l = frame.to_local(wo);
    if wo_l[2] <= 0.0 {
        return None;
    }
    let (_, p_spec) = lobe_probs(p, frame, wo);
    let wi_l = if u_pick < p_spec {
        let wh = sample_wh_visible(wo_l, p.alpha, u1, u2);
        let cos_d = wo_l[0] * wh[0] + wo_l[1] * wh[1] + wo_l[2] * wh[2];
        if cos_d <= 0.0 {
            return None;
        }
        [
            2.0 * cos_d * wh[0] - wo_l[0],
            2.0 * cos_d * wh[1] - wo_l[1],
            2.0 * cos_d * wh[2] - wo_l[2],
        ]
    } else {
        // Cosine-weighted hemisphere.
        let r = u1.sqrt();
        let phi = 2.0 * PI * u2;
        [r * phi.cos(), r * phi.sin(), (1.0 - u1).max(0.0).sqrt()]
    };
    if wi_l[2] <= 0.0 {
        return None;
    }
    Some(frame.to_world(normalize(wi_l)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn diffuse_params() -> BsdfParams {
        BsdfParams {
            diffuse: [1.0; 3],
            f0: [0.04; 3],
            alpha: 0.25,
            emissive: [0.0; 3],
            opacity: 1.0,
        }
    }

    #[test]
    fn diffuse_pdf_is_cosine_over_pi() {
        let p = diffuse_params();
        let frame = Frame::new([0.0, 0.0, 1.0]);
        let wo = [0.0, 0.0, 1.0];
        // Mostly diffuse: p_diff close to 1 at normal incidence.
        let got = pdf(&p, &frame, wo, wo);
        let (_, p_spec) = lobe_probs(&p, &frame, wo);
        let want = (1.0 - p_spec) / PI + p_spec * spec_pdf(&p, wo, wo);
        assert!((got - want).abs() < 1e-6, "{got} vs {want}");
        assert!((0.30..0.40).contains(&got), "{got}");
    }

    #[test]
    fn sample_directions_stay_above_surface() {
        let p = BsdfParams { diffuse: [0.5; 3], f0: [1.0; 3], ..diffuse_params() };
        let frame = Frame::new([0.0, 1.0, 0.0]);
        let wo = normalize([0.3, 0.9, 0.2]);
        let mut rng = super::super::sampler::Rng::new(3);
        for _ in 0..1000 {
            let wi = sample(
                &p,
                &frame,
                wo,
                rng.next_f32(),
                rng.next_f32(),
                rng.next_f32(),
            );
            if let Some(wi) = wi {
                assert!(dot(wi, frame.n) > 0.0, "{wi:?}");
                assert!((super::super::length(wi) - 1.0).abs() < 1e-4);
            }
        }
    }

    #[test]
    fn eval_black_below_surface() {
        let p = diffuse_params();
        let frame = Frame::new([0.0, 0.0, 1.0]);
        assert_eq!(eval(&p, &frame, [0.0, 0.0, 1.0], [0.0, 0.0, -1.0]), [0.0; 3]);
        assert_eq!(pdf(&p, &frame, [0.0, 0.0, 1.0], [0.0, 0.0, -1.0]), 0.0);
    }
}
