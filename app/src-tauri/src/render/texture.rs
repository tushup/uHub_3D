//! Render textures: loaded once per render, downscaled to fit
//! 2048, bilinear sampling with repeat wrap (spec §A1.6). Colour
//! maps for BaseColor and Emissive are stored sRGB-decoded; every
//! other map is stored as raw linear values.

use std::path::Path;

use crate::error::LibError;
use crate::naming::texture_ext;

const MAX_TEXTURE: u32 = 2048;

#[derive(Debug)]
pub struct Texture {
    pub width: u32,
    pub height: u32,
    pixels: Vec<[f32; 4]>,
}

fn srgb_decode(v: f32) -> f32 {
    if v <= 0.04045 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}

impl Texture {
    pub fn load(path: &Path, srgb: bool) -> Result<Texture, LibError> {
        texture_ext(path)?;
        let img = image::open(path).map_err(|e| {
            LibError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("Failed to read texture {}: {e}", path.display()),
            ))
        })?;
        let img = if img.width() > MAX_TEXTURE || img.height() > MAX_TEXTURE {
            img.resize(MAX_TEXTURE, MAX_TEXTURE, image::imageops::FilterType::Triangle)
        } else {
            img
        };
        let rgba = img.to_rgba8();
        let (width, height) = (rgba.width(), rgba.height());
        let mut pixels = Vec::with_capacity((width * height) as usize);
        for p in rgba.pixels() {
            let mut px = [
                p.0[0] as f32 / 255.0,
                p.0[1] as f32 / 255.0,
                p.0[2] as f32 / 255.0,
                p.0[3] as f32 / 255.0,
            ];
            if srgb {
                px[0] = srgb_decode(px[0]);
                px[1] = srgb_decode(px[1]);
                px[2] = srgb_decode(px[2]);
            }
            pixels.push(px);
        }
        Ok(Texture { width, height, pixels })
    }

    fn texel(&self, x: i32, y: i32) -> [f32; 4] {
        let x = x.rem_euclid(self.width as i32) as usize;
        let y = y.rem_euclid(self.height as i32) as usize;
        self.pixels[y * self.width as usize + x]
    }

    /// Bilinear sample. v = 1 is the top row of the image, matching
    /// the UV convention of the loaded meshes.
    pub fn sample(&self, u: f32, v: f32) -> [f32; 4] {
        let x = u * self.width as f32 - 0.5;
        let y = (1.0 - v) * self.height as f32 - 0.5;
        let x0 = x.floor() as i32;
        let y0 = y.floor() as i32;
        let fx = x - x0 as f32;
        let fy = y - y0 as f32;
        let mut out = [0.0f32; 4];
        for (i, px) in out.iter_mut().enumerate() {
            let top = self.texel(x0, y0)[i] * (1.0 - fx) + self.texel(x0 + 1, y0)[i] * fx;
            let bottom =
                self.texel(x0, y0 + 1)[i] * (1.0 - fx) + self.texel(x0 + 1, y0 + 1)[i] * fx;
            *px = top * (1.0 - fy) + bottom * fy;
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_png(dir: &Path, name: &str, rgba: [u8; 4]) -> std::path::PathBuf {
        let path = dir.join(name);
        let img = image::RgbaImage::from_pixel(1, 1, image::Rgba(rgba));
        img.save(&path).unwrap();
        path
    }

    #[test]
    fn srgb_texture_decodes() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_png(dir.path(), "mid.png", [128, 128, 128, 255]);
        // sRGB 128/255 decodes to 0.2158 linear for colour maps.
        let colour = Texture::load(&path, true).unwrap();
        let s = colour.sample(0.5, 0.5);
        assert!((s[0] - 0.2158).abs() < 0.002, "{s:?}");
        // Data maps are not decoded: 128/255 = 0.502.
        let data = Texture::load(&path, false).unwrap();
        let s = data.sample(0.5, 0.5);
        assert!((s[0] - 0.502).abs() < 0.002, "{s:?}");
    }

    #[test]
    fn repeat_wrap_and_alpha() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_png(dir.path(), "c.png", [255, 0, 0, 128]);
        let tex = Texture::load(&path, false).unwrap();
        let a = tex.sample(0.5, 0.5);
        let b = tex.sample(2.5, -0.5);
        assert_eq!(a, b);
        assert!((a[3] - 128.0 / 255.0).abs() < 1e-6);
    }
}
