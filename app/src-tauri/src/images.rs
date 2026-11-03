use std::io::Cursor;
use std::path::Path;

use base64::Engine;
use image::{DynamicImage, GenericImageView, ImageBuffer, ImageFormat, Rgb};

use crate::error::LibError;
use crate::naming::texture_ext;

const MAX_THUMB: u32 = 512;

fn image_error(context: &str, e: image::ImageError) -> LibError {
    LibError::Io(std::io::Error::new(
        std::io::ErrorKind::InvalidData,
        format!("{context}: {e}"),
    ))
}

fn fitted_thumbnail(src: &Path) -> Result<DynamicImage, LibError> {
    texture_ext(src)?;
    let img = image::open(src).map_err(|e| image_error("Failed to read image", e))?;
    let (w, h) = img.dimensions();
    if w <= MAX_THUMB && h <= MAX_THUMB {
        Ok(img)
    } else {
        Ok(img.thumbnail(MAX_THUMB, MAX_THUMB))
    }
}

/// Fit `src` (png/jpg/jpeg/tga) inside 512x512 keeping aspect, never
/// upscale, and write it as PNG to `dst`.
pub fn write_thumbnail(src: &Path, dst: &Path) -> Result<(), LibError> {
    fitted_thumbnail(src)?
        .save_with_format(dst, ImageFormat::Png)
        .map_err(|e| image_error("Failed to write thumbnail", e))
}

/// Same fit as `write_thumbnail`, returned as a PNG data URL so the
/// frontend never needs file-system access to preview a picked image.
pub fn image_preview_data_url(src: &Path) -> Result<String, LibError> {
    let mut buf = Cursor::new(Vec::new());
    fitted_thumbnail(src)?
        .write_to(&mut buf, ImageFormat::Png)
        .map_err(|e| image_error("Failed to encode image preview", e))?;
    let encoded = base64::engine::general_purpose::STANDARD.encode(buf.into_inner());
    Ok(format!("data:image/png;base64,{encoded}"))
}

/// Dimensions of an HDR file, read from its header.
pub fn hdr_size(src: &Path) -> Result<(u32, u32), LibError> {
    image::ImageReader::open(src)
        .map_err(LibError::Io)?
        .with_guessed_format()?
        .into_dimensions()
        .map_err(|e| image_error("Failed to read HDR header", e))
}

pub const HDR_THUMB_WIDTH: u32 = 1024;
pub const HDR_THUMB_HEIGHT: u32 = 512;

/// Equirect HDRI image at 1024x512 (spec §A1.4): linear downscale,
/// horizontal shift by `rotation` degrees, multiplied by
/// `intensity`, tonemapped with the same AgX as the path tracer so
/// preview equals thumbnail.
fn hdr_image_1024(src: &Path, rotation: f32, intensity: f32) -> Result<DynamicImage, LibError> {
    let hdr = image::open(src)
        .map_err(|e| image_error("Failed to read HDR", e))?
        .to_rgb32f();
    let small = image::imageops::resize(
        &hdr,
        HDR_THUMB_WIDTH,
        HDR_THUMB_HEIGHT,
        image::imageops::FilterType::Triangle,
    );
    let shift = ((rotation.rem_euclid(360.0) / 360.0) * HDR_THUMB_WIDTH as f32).round() as i32;
    let mut ldr: ImageBuffer<Rgb<u8>, Vec<u8>> =
        ImageBuffer::new(HDR_THUMB_WIDTH, HDR_THUMB_HEIGHT);
    for (x, y, pixel) in ldr.enumerate_pixels_mut() {
        let sx = (x as i32 + shift).rem_euclid(HDR_THUMB_WIDTH as i32) as u32;
        let src_px = small.get_pixel(sx, y);
        let d = crate::render::agx([
            src_px.0[0] * intensity,
            src_px.0[1] * intensity,
            src_px.0[2] * intensity,
        ]);
        *pixel = Rgb([
            (d[0] * 255.0).round() as u8,
            (d[1] * 255.0).round() as u8,
            (d[2] * 255.0).round() as u8,
        ]);
    }
    Ok(DynamicImage::ImageRgb8(ldr))
}

pub fn write_hdr_thumbnail(
    src: &Path,
    dst: &Path,
    rotation: f32,
    intensity: f32,
) -> Result<(), LibError> {
    hdr_image_1024(src, rotation, intensity)?
        .save_with_format(dst, ImageFormat::Png)
        .map_err(|e| image_error("Failed to write HDR thumbnail", e))
}

pub fn hdr_preview_data_url(src: &Path, rotation: f32, intensity: f32) -> Result<String, LibError> {
    let mut buf = Cursor::new(Vec::new());
    hdr_image_1024(src, rotation, intensity)?
        .write_to(&mut buf, ImageFormat::Png)
        .map_err(|e| image_error("Failed to encode HDR preview", e))?;
    let encoded = base64::engine::general_purpose::STANDARD.encode(buf.into_inner());
    Ok(format!("data:image/png;base64,{encoded}"))
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn fx(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(name)
    }

    fn tmp() -> tempfile::TempDir {
        tempfile::tempdir().unwrap()
    }

    #[test]
    fn downscales_big() {
        let d = tmp();
        write_thumbnail(&fx("big.png"), &d.path().join("t.png")).unwrap();
        assert_eq!(image::image_dimensions(d.path().join("t.png")).unwrap(), (512, 512));
    }

    #[test]
    fn keeps_small() {
        let d = tmp();
        write_thumbnail(&fx("red.png"), &d.path().join("t.png")).unwrap();
        assert_eq!(image::image_dimensions(d.path().join("t.png")).unwrap(), (64, 64));
    }

    #[test]
    fn reads_tga() {
        let d = tmp();
        write_thumbnail(&fx("gray.tga"), &d.path().join("t.png")).unwrap();
        assert_eq!(image::image_dimensions(d.path().join("t.png")).unwrap(), (64, 64));
    }

    #[test]
    fn hdr_dims() {
        assert_eq!(hdr_size(&fx("sky.hdr")).unwrap(), (512, 256));
    }

    #[test]
    fn hdr_thumbnail_is_1024x512() {
        let d = tmp();
        let dst = d.path().join("t.png");
        write_hdr_thumbnail(&fx("sky.hdr"), &dst, 0.0, 1.0).unwrap();
        assert_eq!(image::image_dimensions(&dst).unwrap(), (1024, 512));
    }

    #[test]
    fn hdr_rotation_180_shifts_half() {
        let at0 = hdr_image_1024(&fx("sky.hdr"), 0.0, 1.0).unwrap().to_rgb8();
        let at180 = hdr_image_1024(&fx("sky.hdr"), 180.0, 1.0).unwrap().to_rgb8();
        for y in [0, 100, 255, 511] {
            assert_eq!(at180.get_pixel(0, y), at0.get_pixel(512, y), "row {y}");
            assert_eq!(at180.get_pixel(300, y), at0.get_pixel(812, y), "row {y}");
        }
    }

    #[test]
    fn hdr_intensity_scales_brightness() {
        let dark = hdr_image_1024(&fx("sky.hdr"), 0.0, 0.5).unwrap().to_rgb8();
        let bright = hdr_image_1024(&fx("sky.hdr"), 0.0, 2.0).unwrap().to_rgb8();
        let sum = |img: &image::RgbImage| -> u64 {
            img.pixels().map(|p| p.0.iter().map(|v| *v as u64).sum::<u64>()).sum()
        };
        assert!(sum(&bright) > sum(&dark));
    }

    #[test]
    fn image_preview_returns_png_data_url() {
        let url = image_preview_data_url(&fx("big.png")).unwrap();
        let b64 = url
            .strip_prefix("data:image/png;base64,")
            .expect("png data url prefix");
        let bytes = base64::engine::general_purpose::STANDARD.decode(b64).unwrap();
        let img = image::load_from_memory(&bytes).unwrap();
        assert_eq!(img.dimensions(), (512, 512));
    }

    #[test]
    fn image_preview_rejects_exr() {
        let err = image_preview_data_url(&fx("fake.exr")).unwrap_err();
        assert!(matches!(err, LibError::UnsupportedFormat(_)), "{err:?}");
    }

    #[test]
    fn hdr_data_url() {
        assert!(hdr_preview_data_url(&fx("sky.hdr"), 0.0, 1.0)
            .unwrap()
            .starts_with("data:image/png;base64,"));
    }
}
