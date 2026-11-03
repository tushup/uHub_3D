//! Shared view math (spec Amendment A1 §A1.3). Every number here is
//! mirrored in `src/viewer/viewMath.ts`; both are checked against
//! `src/viewer/view_golden.json`.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct View {
    pub yaw: f32,
    pub pitch: f32,
    pub zoom: f32,
}

impl Default for View {
    fn default() -> Self {
        View { yaw: -30.0, pitch: 15.0, zoom: 1.0 }
    }
}

impl View {
    pub fn apply_drag(&mut self, dx: f32, dy: f32) {
        self.yaw += dx * 0.4;
        self.pitch += dy * 0.4;
        *self = self.clamped();
    }

    pub fn apply_wheel(&mut self, delta_y: f32) {
        self.zoom *= 1.1f32.powf(-delta_y / 100.0);
        *self = self.clamped();
    }

    pub fn clamped(self) -> View {
        View {
            yaw: self.yaw,
            pitch: self.pitch.clamp(-89.0, 89.0),
            zoom: self.zoom.clamp(0.5, 4.0),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Bounds {
    pub min: [f32; 3],
    pub max: [f32; 3],
}

#[derive(Debug, Clone, Copy)]
pub struct Framing {
    pub model: [[f32; 4]; 4],
    pub distance: f32,
    pub fov_y_deg: f32,
}

fn center_and_scale(bounds: &Bounds) -> ([f64; 3], f64) {
    let mut center = [0.0f64; 3];
    let mut extent = [0.0f64; 3];
    for i in 0..3 {
        let lo = bounds.min[i] as f64;
        let hi = bounds.max[i] as f64;
        center[i] = (lo + hi) / 2.0;
        extent[i] = hi - lo;
    }
    let max_extent = extent[0].max(extent[1]).max(extent[2]);
    let s = if max_extent == 0.0 { 1.0 } else { 1.0 / max_extent };
    (center, s)
}

type Mat4 = [[f64; 4]; 4];

fn mat_mul(a: &Mat4, b: &Mat4) -> Mat4 {
    let mut out = [[0.0f64; 4]; 4];
    for (r, row) in out.iter_mut().enumerate() {
        for (c, cell) in row.iter_mut().enumerate() {
            *cell = (0..4).map(|k| a[r][k] * b[k][c]).sum();
        }
    }
    out
}

fn rot_x(deg: f64) -> Mat4 {
    let (s, c) = deg.to_radians().sin_cos();
    [
        [1.0, 0.0, 0.0, 0.0],
        [0.0, c, -s, 0.0],
        [0.0, s, c, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ]
}

fn rot_y(deg: f64) -> Mat4 {
    let (s, c) = deg.to_radians().sin_cos();
    [
        [c, 0.0, s, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [-s, 0.0, c, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ]
}

fn apply_mat(m: &Mat4, p: [f64; 3]) -> [f64; 3] {
    [
        m[0][0] * p[0] + m[0][1] * p[1] + m[0][2] * p[2] + m[0][3],
        m[1][0] * p[0] + m[1][1] * p[1] + m[1][2] * p[2] + m[1][3],
        m[2][0] * p[0] + m[2][1] * p[1] + m[2][2] * p[2] + m[2][3],
    ]
}

fn framing_f64(bounds: &Bounds, view: &View) -> (Mat4, f64) {
    let (center, s) = center_and_scale(bounds);
    let extent = [
        bounds.max[0] as f64 - bounds.min[0] as f64,
        bounds.max[1] as f64 - bounds.min[1] as f64,
        bounds.max[2] as f64 - bounds.min[2] as f64,
    ];
    let len = (extent[0] * extent[0] + extent[1] * extent[1] + extent[2] * extent[2]).sqrt();
    let r = s * len / 2.0;
    let d0 = 1.08 * r / (15.0f64).to_radians().sin();
    let distance = d0 / view.zoom as f64;

    let translate: Mat4 = [
        [1.0, 0.0, 0.0, -center[0]],
        [0.0, 1.0, 0.0, -center[1]],
        [0.0, 0.0, 1.0, -center[2]],
        [0.0, 0.0, 0.0, 1.0],
    ];
    let scale: Mat4 = [
        [s, 0.0, 0.0, 0.0],
        [0.0, s, 0.0, 0.0],
        [0.0, 0.0, s, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ];
    // M = Rx(pitch) · Ry(yaw) · S(s) · T(−c)
    let model = mat_mul(
        &rot_x(view.pitch as f64),
        &mat_mul(&rot_y(view.yaw as f64), &mat_mul(&scale, &translate)),
    );
    (model, distance)
}

pub fn framing(bounds: &Bounds, view: &View) -> Framing {
    let (model, distance) = framing_f64(bounds, view);
    let mut m = [[0.0f32; 4]; 4];
    for (r, row) in m.iter_mut().enumerate() {
        for (c, cell) in row.iter_mut().enumerate() {
            *cell = model[r][c] as f32;
        }
    }
    Framing { model: m, distance: distance as f32, fov_y_deg: 30.0 }
}

/// Project a model-space point to pixel coordinates for a square
/// `size`×`size` image. The camera sits at (0, 0, distance) looking −Z.
pub fn project(f: &Framing, p: [f32; 3], size: u32) -> [f32; 2] {
    let m: Mat4 = {
        let mut out = [[0.0f64; 4]; 4];
        for (r, row) in out.iter_mut().enumerate() {
            for (c, cell) in row.iter_mut().enumerate() {
                *cell = f.model[r][c] as f64;
            }
        }
        out
    };
    let q = apply_mat(&m, [p[0] as f64, p[1] as f64, p[2] as f64]);
    let zc = q[2] - f.distance as f64;
    let tan_half = (f.fov_y_deg as f64 / 2.0).to_radians().tan();
    let x_ndc = q[0] / (-zc * tan_half);
    let y_ndc = q[1] / (-zc * tan_half);
    let size = size as f64;
    [
        ((x_ndc + 1.0) / 2.0 * size) as f32,
        ((1.0 - y_ndc) / 2.0 * size) as f32,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn apply_f32(m: &[[f32; 4]; 4], p: [f32; 3]) -> [f32; 3] {
        [
            m[0][0] * p[0] + m[0][1] * p[1] + m[0][2] * p[2] + m[0][3],
            m[1][0] * p[0] + m[1][1] * p[1] + m[1][2] * p[2] + m[1][3],
            m[2][0] * p[0] + m[2][1] * p[1] + m[2][2] * p[2] + m[2][3],
        ]
    }

    #[test]
    fn normalize_max_extent_is_one() {
        let b = Bounds { min: [-1.0, 0.0, -0.5], max: [1.0, 2.0, 0.5] };
        let (center, s) = center_and_scale(&b);
        assert_eq!(center, [0.0, 1.0, 0.0]);
        assert_eq!(s, 0.5);
        let f = framing(&b, &View::default());
        let c = apply_f32(&f.model, [0.0, 1.0, 0.0]);
        assert!(c.iter().all(|v| v.abs() < 1e-6), "{c:?}");
        // The far corner sits at |(0.5, 0.5, 0.25)| = 0.75 from the
        // origin after normalize (rotation preserves length).
        let corner = apply_f32(&f.model, [1.0, 2.0, 0.5]);
        let len = (corner[0] * corner[0] + corner[1] * corner[1] + corner[2] * corner[2]).sqrt();
        assert!((len - 0.75).abs() < 1e-5, "{len}");
    }

    #[test]
    fn zero_extent_uses_unit_scale() {
        let b = Bounds { min: [2.0, 3.0, 4.0], max: [2.0, 3.0, 4.0] };
        let (_c, s) = center_and_scale(&b);
        assert_eq!(s, 1.0);
        let f = framing(&b, &View::default());
        assert_eq!(f.distance, 0.0);
        let c = apply_f32(&f.model, [2.0, 3.0, 4.0]);
        assert!(c.iter().all(|v| v.abs() < 1e-6), "{c:?}");
    }

    #[test]
    fn view_clamps() {
        let v = View { yaw: 0.0, pitch: 120.0, zoom: 10.0 }.clamped();
        assert_eq!(v.pitch, 89.0);
        assert_eq!(v.zoom, 4.0);
        let v = View { yaw: 0.0, pitch: -120.0, zoom: 0.1 }.clamped();
        assert_eq!(v.pitch, -89.0);
        assert_eq!(v.zoom, 0.5);
        let mut v = View::default();
        v.apply_drag(100.0, 100_000.0);
        assert_eq!(v.yaw, 10.0);
        assert_eq!(v.pitch, 89.0);
        let mut v = View::default();
        v.apply_wheel(-100_000.0);
        assert_eq!(v.zoom, 4.0);
        v.apply_wheel(100_000.0);
        assert_eq!(v.zoom, 0.5);
    }

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

    #[test]
    fn view_golden_matches() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/viewer/view_golden.json");
        let text = std::fs::read_to_string(path).expect("golden file exists");
        let golden: Golden = serde_json::from_str(&text).unwrap();
        assert_eq!(golden.cases.len(), 6);
        for (ci, case) in golden.cases.iter().enumerate() {
            let f = framing(&case.bounds, &case.view);
            let corners: Vec<[f32; 3]> = (0..8)
                .map(|i| {
                    [
                        if i & 1 == 0 { case.bounds.min[0] } else { case.bounds.max[0] },
                        if i & 2 == 0 { case.bounds.min[1] } else { case.bounds.max[1] },
                        if i & 4 == 0 { case.bounds.min[2] } else { case.bounds.max[2] },
                    ]
                })
                .collect();
            for (pi, corner) in corners.iter().enumerate() {
                let got = project(&f, *corner, case.size);
                let want = case.points[pi];
                assert!(
                    (got[0] - want[0]).abs() <= 1e-3 && (got[1] - want[1]).abs() <= 1e-3,
                    "case {ci} point {pi}: got {got:?} want {want:?}"
                );
            }
        }
    }
}
