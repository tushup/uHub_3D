//! Fixed camera from spec §A1.3: at (0, 0, D) looking down −Z, +Y
//! up, 30° vertical FOV, square aspect. The scene geometry is
//! already transformed by the framing model matrix, so camera rays
//! and `view::project` agree by construction.

use crate::view::Framing;

use super::{normalize, Vec3};

#[derive(Debug, Clone, Copy)]
pub struct Camera {
    pub origin: Vec3,
    pub distance: f32,
    tan_half: f32,
}

impl Camera {
    pub fn from_framing(framing: &Framing) -> Camera {
        Camera {
            origin: [0.0, 0.0, framing.distance],
            distance: framing.distance,
            tan_half: (framing.fov_y_deg / 2.0).to_radians().tan(),
        }
    }

    /// Ray through pixel (px, py) of a square `size` image.
    /// `offset` is the filter-sampled sub-pixel offset in pixels.
    pub fn ray(&self, px: u32, py: u32, size: u32, offset: [f32; 2]) -> (Vec3, Vec3) {
        let ndc_x = ((px as f32 + 0.5 + offset[0]) / size as f32) * 2.0 - 1.0;
        let ndc_y = 1.0 - ((py as f32 + 0.5 + offset[1]) / size as f32) * 2.0;
        let dir = normalize([ndc_x * self.tan_half, ndc_y * self.tan_half, -1.0]);
        (self.origin, dir)
    }

    /// Pixel coordinates of a point already transformed by the
    /// framing model matrix (camera space: camera at z = distance).
    pub fn project_point(&self, p: Vec3, size: u32) -> [f32; 2] {
        let zc = p[2] - self.distance;
        let x_ndc = p[0] / (-zc * self.tan_half);
        let y_ndc = p[1] / (-zc * self.tan_half);
        let size = size as f32;
        [(x_ndc + 1.0) / 2.0 * size, (1.0 - y_ndc) / 2.0 * size]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::view::{framing, Bounds, View};

    #[derive(serde::Deserialize)]
    struct Golden {
        cases: Vec<GoldenCase>,
    }

    #[derive(serde::Deserialize)]
    struct GoldenCase {
        bounds: Bounds,
        view: View,
        size: u32,
        points: Vec<[f32; 2]>,
    }

    fn apply(m: &[[f32; 4]; 4], p: [f32; 3]) -> [f32; 3] {
        [
            m[0][0] * p[0] + m[0][1] * p[1] + m[0][2] * p[2] + m[0][3],
            m[1][0] * p[0] + m[1][1] * p[1] + m[1][2] * p[2] + m[1][3],
            m[2][0] * p[0] + m[2][1] * p[1] + m[2][2] * p[2] + m[2][3],
        ]
    }

    #[test]
    fn framing_matches_view_golden() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/viewer/view_golden.json");
        let text = std::fs::read_to_string(path).expect("golden file exists");
        let golden: Golden = serde_json::from_str(&text).unwrap();
        let case = &golden.cases[0];
        let f = framing(&case.bounds, &case.view);
        let camera = Camera::from_framing(&f);
        let corners: Vec<[f32; 3]> = (0..8)
            .map(|i| {
                [
                    if i & 1 == 0 { case.bounds.min[0] } else { case.bounds.max[0] },
                    if i & 2 == 0 { case.bounds.min[1] } else { case.bounds.max[1] },
                    if i & 4 == 0 { case.bounds.min[2] } else { case.bounds.max[2] },
                ]
            })
            .collect();
        let projected: Vec<[f32; 2]> = corners
            .iter()
            .map(|c| camera.project_point(apply(&f.model, *c), case.size))
            .collect();
        let extreme = |pts: &[[f32; 2]], axis: usize, pick_max: bool| -> f32 {
            pts.iter()
                .map(|p| p[axis])
                .reduce(|a, b| if pick_max { a.max(b) } else { a.min(b) })
                .unwrap()
        };
        for axis in 0..2 {
            for pick_max in [false, true] {
                let got = extreme(&projected, axis, pick_max);
                let want = extreme(&case.points, axis, pick_max);
                assert!(
                    (got - want).abs() <= 1.5,
                    "axis {axis} max={pick_max}: got {got} want {want}"
                );
            }
        }
        // Every corner also projects close to its golden point.
        for (got, want) in projected.iter().zip(case.points.iter()) {
            assert!(
                (got[0] - want[0]).abs() <= 1.5 && (got[1] - want[1]).abs() <= 1.5,
                "got {got:?} want {want:?}"
            );
        }
    }

    #[test]
    fn center_ray_looks_down_minus_z() {
        let f = framing(
            &Bounds { min: [-1.0; 3], max: [1.0; 3] },
            &View::default(),
        );
        let camera = Camera::from_framing(&f);
        let (origin, dir) = camera.ray(32, 32, 65, [0.0, 0.0]);
        assert_eq!(origin, [0.0, 0.0, f.distance]);
        assert!(dir[0].abs() < 1e-6 && dir[1].abs() < 1e-6 && (dir[2] + 1.0).abs() < 1e-6);
    }
}
