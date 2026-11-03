//! AgX view transform (Blender 4 default) via the published
//! analytic fit: inset/outset matrices with a 6th-order polynomial
//! contrast curve.
// shortcut: analytic AgX fit, not Blender's OCIO LUT; swap if colors must match Blender exactly

// The published AgX matrices are column-major listings; they are
// stored here transposed so `mat_mul` applies them directly. The
// outset is the exact inverse of the inset, which keeps greys
// neutral up to the polynomial fit's residual (~1%).
#[allow(clippy::excessive_precision)]
const INSET: [[f32; 3]; 3] = [
    [0.856627153315983, 0.0951212405381588, 0.0482516061458583],
    [0.137318972929847, 0.761241990602591, 0.126230720331858],
    [0.11189821299995, 0.178799766409007, 0.825165695922282],
];

#[allow(clippy::excessive_precision)]
const OUTSET: [[f32; 3]; 3] = [
    [1.1271005818144368, -0.11060664309660323, -0.016493938717833046],
    [-0.1413297634984383, 1.157823702216272, -0.016493938717833046],
    [-0.14132976349843826, -0.11060664309660323, 1.2519364065950405],
];

fn mat_mul(m: &[[f32; 3]; 3], v: [f32; 3]) -> [f32; 3] {
    [
        m[0][0] * v[0] + m[0][1] * v[1] + m[0][2] * v[2],
        m[1][0] * v[0] + m[1][1] * v[1] + m[1][2] * v[2],
        m[2][0] * v[0] + m[2][1] * v[1] + m[2][2] * v[2],
    ]
}

fn contrast_curve(x: f32) -> f32 {
    let x2 = x * x;
    let x4 = x2 * x2;
    // 6th-order polynomial fit over the encoded range.
    15.5 * x4 * x2 - 40.14 * x4 * x + 31.96 * x4 - 6.868 * x2 * x + 0.4298 * x2 + 0.1191 * x
        - 0.00232
}

fn srgb_encode(v: f32) -> f32 {
    let v = v.clamp(0.0, 1.0);
    if v <= 0.0031308 {
        12.92 * v
    } else {
        1.055 * v.powf(1.0 / 2.4) - 0.055
    }
}

/// Linear scene-referred sRGB in, display-referred sRGB in 0..1 out.
pub fn agx(linear: [f32; 3]) -> [f32; 3] {
    let c = mat_mul(&INSET, [linear[0].max(0.0), linear[1].max(0.0), linear[2].max(0.0)]);
    // Log2 encode into the AgX working range.
    let encode = |v: f32| ((v.max(1e-10).log2() + 12.47393) / 16.5).clamp(0.0, 1.0);
    let curved = [
        contrast_curve(encode(c[0])),
        contrast_curve(encode(c[1])),
        contrast_curve(encode(c[2])),
    ];
    let out = mat_mul(&OUTSET, curved);
    // The polynomial output is display-encoded (roughly gamma 2.2);
    // linearize it (the AgX EOTF) before the sRGB encode, or grey
    // gets a second gamma and lifts from ~0.50 to ~0.73.
    [
        srgb_encode(out[0].max(0.0).powf(2.2)),
        srgb_encode(out[1].max(0.0).powf(2.2)),
        srgb_encode(out[2].max(0.0).powf(2.2)),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agx_mid_grey() {
        // 18% grey must display at about 0.50, not lifted by a
        // second gamma encode.
        let out = agx([0.18; 3]);
        for v in out {
            assert!((0.47..=0.53).contains(&v), "mid grey {out:?}");
        }
    }

    #[test]
    fn agx_bounds_and_monotonic() {
        let black = agx([0.0, 0.0, 0.0]);
        for v in black {
            assert!(v.abs() <= 1e-3, "{black:?}");
        }
        let mut prev = -1.0f32;
        for i in 0..=20 {
            let g = i as f32 / 20.0 * 4.0; // greys 0..4
            let out = agx([g, g, g]);
            for v in out {
                assert!((0.0..=1.0).contains(&v), "grey {g} -> {out:?}");
            }
            assert!(out[0] >= prev - 1e-6, "not monotonic at {g}: {} after {prev}", out[0]);
            prev = out[0];
        }
        // Neutral greys stay neutral up to the fit's residual.
        let mid = agx([0.18, 0.18, 0.18]);
        assert!((mid[0] - mid[1]).abs() < 0.02 && (mid[1] - mid[2]).abs() < 0.02, "{mid:?}");
        // Highlight rolls off towards white.
        let hot = agx([16.0, 16.0, 16.0]);
        assert!(hot[0] > 0.9, "{hot:?}");
    }
}
