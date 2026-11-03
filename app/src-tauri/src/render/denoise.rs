//! Final-image denoising with Intel Open Image Denoise 2.3.3
//! (D-29, D-32): the `RT` filter on linear HDR colour, guided by
//! first-hit albedo and world normal. Alpha is never denoised.

use crate::error::LibError;

fn io_error(message: String) -> LibError {
    LibError::Io(std::io::Error::other(message))
}

/// Denoise `color` (linear, per-pixel RGB) guided by `albedo` and
/// `normal`. All three buffers hold `width * height` pixels.
/// Returns a new linear colour buffer of the same length.
pub fn run(
    color: &[[f32; 3]],
    albedo: &[[f32; 3]],
    normal: &[[f32; 3]],
    width: u32,
    height: u32,
) -> Result<Vec<[f32; 3]>, LibError> {
    let expected = (width as usize)
        .checked_mul(height as usize)
        .ok_or_else(|| io_error("OIDN: image dimensions overflow".to_string()))?;
    if color.len() != expected || albedo.len() != expected || normal.len() != expected {
        return Err(io_error(format!(
            "OIDN: buffer size mismatch (color {}, albedo {}, normal {}, expected {expected})",
            color.len(),
            albedo.len(),
            normal.len()
        )));
    }
    if expected == 0 {
        return Ok(Vec::new());
    }
    let flatten = |buf: &[[f32; 3]]| -> Vec<f32> {
        let mut out = Vec::with_capacity(buf.len() * 3);
        for px in buf {
            out.extend_from_slice(px);
        }
        out
    };
    let color_flat = flatten(color);
    let albedo_flat = flatten(albedo);
    let normal_flat = flatten(normal);
    let mut output_flat = vec![0.0f32; color_flat.len()];

    let device = oidn::Device::cpu();
    let mut filter = oidn::RayTracing::new(&device);
    filter
        .hdr(true)
        .clean_aux(true)
        .filter_quality(oidn::Quality::High)
        .image_dimensions(width as usize, height as usize)
        .albedo_normal(&albedo_flat[..], &normal_flat[..]);
    filter
        .filter(&color_flat[..], &mut output_flat[..])
        .map_err(|e| io_error(format!("OIDN filter failed: {e:?}")))?;
    if let Err((code, message)) = device.get_error() {
        return Err(io_error(format!("OIDN device error {code:?}: {message}")));
    }
    let mut out = Vec::with_capacity(expected);
    for px in output_flat.as_chunks::<3>().0 {
        out.push(*px);
    }
    Ok(out)
}
