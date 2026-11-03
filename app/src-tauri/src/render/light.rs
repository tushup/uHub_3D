//! Three-point rect area lights and the studio environment, with
//! the values fixed in spec §A1.3.

use super::{cross, length, normalize, scale, Vec3};

#[derive(Debug, Clone, Copy)]
pub struct RectLight {
    pub center: Vec3,
    /// Unit axes spanning the rect.
    pub u: Vec3,
    pub v: Vec3,
    /// Side length (m); the rect is square.
    pub size: f32,
    pub normal: Vec3,
    /// Emitted radiance L = E · d² / side², per channel.
    pub radiance: Vec3,
}

impl RectLight {
    pub fn new(center: Vec3, size: f32, irradiance: f32, color: Vec3) -> RectLight {
        let d = length(center);
        let normal = normalize(scale(center, -1.0));
        let radiance = scale(color, irradiance * d * d / (size * size));
        let helper = if normal[1].abs() < 0.9 { [0.0, 1.0, 0.0] } else { [1.0, 0.0, 0.0] };
        let u = normalize(cross(helper, normal));
        let v = cross(normal, u);
        RectLight { center, u, v, size, normal, radiance }
    }

    pub fn area(&self) -> f32 {
        self.size * self.size
    }

    pub fn sample_point(&self, u1: f32, u2: f32) -> Vec3 {
        let a = (u1 - 0.5) * self.size;
        let b = (u2 - 0.5) * self.size;
        [
            self.center[0] + self.u[0] * a + self.v[0] * b,
            self.center[1] + self.u[1] * a + self.v[1] * b,
            self.center[2] + self.u[2] * a + self.v[2] * b,
        ]
    }
}

/// Key, fill and rim from the §A1.3 table, in that order.
pub fn three_point() -> Vec<RectLight> {
    vec![
        RectLight::new([-2.0, 2.0, 2.4], 1.0, 3.0, [1.00, 0.96, 0.90]),
        RectLight::new([2.6, 0.6, 2.0], 1.6, 1.0, [0.90, 0.95, 1.00]),
        RectLight::new([1.2, 2.2, -2.6], 0.8, 4.0, [1.00, 1.00, 1.00]),
    ]
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Env {
    Uniform(f32),
    StudioGradient,
}

impl Env {
    pub fn uniform(v: f32) -> Env {
        Env::Uniform(v)
    }

    pub fn studio_gradient() -> Env {
        Env::StudioGradient
    }

    /// Neutral radiance in a direction. The gradient runs over
    /// dir.y: 0.02 at −1, 0.06 at 0, 0.20 at +1, linear between.
    pub fn radiance(&self, dir: Vec3) -> Vec3 {
        match self {
            Env::Uniform(v) => [*v; 3],
            Env::StudioGradient => {
                let y = dir[1].clamp(-1.0, 1.0);
                let g = if y < 0.0 { 0.06 + y * 0.04 } else { 0.06 + y * 0.14 };
                [g; 3]
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn three_point_values() {
        let lights = three_point();
        assert_eq!(lights.len(), 3);
        // Key: E 3.0 at d = |(-2, 2, 2.4)|, side 1.
        let d2 = 4.0 + 4.0 + 5.76f32;
        let want = [3.0 * d2, 3.0 * d2 * 0.96, 3.0 * d2 * 0.90];
        for i in 0..3 {
            assert!((lights[0].radiance[i] - want[i]).abs() < 1e-3, "{:?}", lights[0].radiance);
        }
        assert_eq!(lights[1].size, 1.6);
        assert_eq!(lights[2].size, 0.8);
        // Every light faces the origin.
        for l in &lights {
            let to_origin = normalize(scale(l.center, -1.0));
            assert!((super::super::dot(l.normal, to_origin) - 1.0).abs() < 1e-6);
        }
    }

    #[test]
    fn gradient_endpoints() {
        let env = Env::studio_gradient();
        assert_eq!(env.radiance([0.0, -1.0, 0.0]), [0.02; 3]);
        assert_eq!(env.radiance([0.0, 0.0, 1.0]), [0.06; 3]);
        assert_eq!(env.radiance([0.0, 1.0, 0.0]), [0.20; 3]);
        assert_eq!(Env::uniform(2.0).radiance([1.0, 0.0, 0.0]), [2.0; 3]);
    }
}
