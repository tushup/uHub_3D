//! Linear HDR film with straight-alpha accumulation. One sample per
//! pixel per pass; per-pixel accumulation order is pass order, so
//! results do not depend on thread scheduling.

use image::{Rgba, RgbaImage};

#[derive(Debug, Clone)]
pub struct Film {
    pub size: u32,
    pub samples: u32,
    pub rgb: Vec<[f32; 3]>,
    pub alpha: Vec<f32>,
    /// First-hit albedo summed over samples (misses are 0).
    pub albedo: Vec<[f32; 3]>,
    /// First-hit world normal summed over samples (misses are 0).
    pub normal: Vec<[f32; 3]>,
}

impl Film {
    pub fn new(size: u32) -> Film {
        let n = (size as usize) * (size as usize);
        Film {
            size,
            samples: 0,
            rgb: vec![[0.0; 3]; n],
            alpha: vec![0.0; n],
            albedo: vec![[0.0; 3]; n],
            normal: vec![[0.0; 3]; n],
        }
    }

    pub fn add(
        &mut self,
        pixel: usize,
        color: [f32; 3],
        alpha: f32,
        albedo: [f32; 3],
        normal: [f32; 3],
    ) {
        let slot = &mut self.rgb[pixel];
        slot[0] += color[0];
        slot[1] += color[1];
        slot[2] += color[2];
        self.alpha[pixel] += alpha;
        let a = &mut self.albedo[pixel];
        a[0] += albedo[0];
        a[1] += albedo[1];
        a[2] += albedo[2];
        let n = &mut self.normal[pixel];
        n[0] += normal[0];
        n[1] += normal[1];
        n[2] += normal[2];
    }

    /// Merge one completed pass into this film.
    pub fn absorb(&mut self, pass: &Film) {
        for (a, b) in self.rgb.iter_mut().zip(pass.rgb.iter()) {
            a[0] += b[0];
            a[1] += b[1];
            a[2] += b[2];
        }
        for (a, b) in self.alpha.iter_mut().zip(pass.alpha.iter()) {
            *a += b;
        }
        for (a, b) in self.albedo.iter_mut().zip(pass.albedo.iter()) {
            a[0] += b[0];
            a[1] += b[1];
            a[2] += b[2];
        }
        for (a, b) in self.normal.iter_mut().zip(pass.normal.iter()) {
            a[0] += b[0];
            a[1] += b[1];
            a[2] += b[2];
        }
        self.samples += 1;
    }

    fn averaged(buf: &[[f32; 3]], samples: u32) -> Vec<[f32; 3]> {
        let inv = 1.0 / samples.max(1) as f32;
        buf.iter()
            .map(|c| [c[0] * inv, c[1] * inv, c[2] * inv])
            .collect()
    }

    /// Linear colour averaged over samples.
    pub fn color_linear(&self) -> Vec<[f32; 3]> {
        Self::averaged(&self.rgb, self.samples)
    }

    /// First-hit albedo averaged over samples (misses are 0).
    pub fn albedo_linear(&self) -> Vec<[f32; 3]> {
        Self::averaged(&self.albedo, self.samples)
    }

    /// First-hit world normal averaged over samples (misses are 0).
    pub fn normal_linear(&self) -> Vec<[f32; 3]> {
        Self::averaged(&self.normal, self.samples)
    }

    /// Display-referred RGBA through the AgX view transform.
    /// Exposure is in EV; alpha is the plain coverage average.
    pub fn to_rgba8(&self, exposure_ev: f32) -> RgbaImage {
        self.to_rgba8_with_color(&self.color_linear(), exposure_ev)
    }

    /// Like `to_rgba8`, but from a caller-provided linear colour
    /// buffer (e.g. the denoised result). Alpha is unchanged.
    pub fn to_rgba8_with_color(&self, linear: &[[f32; 3]], exposure_ev: f32) -> RgbaImage {
        let inv = 1.0 / self.samples.max(1) as f32;
        let gain = 2.0f32.powf(exposure_ev);
        let mut img = RgbaImage::new(self.size, self.size);
        for (i, px) in img.pixels_mut().enumerate() {
            let c = linear.get(i).copied().unwrap_or([0.0; 3]);
            let d = super::tonemap::agx([c[0] * gain, c[1] * gain, c[2] * gain]);
            let a = (self.alpha[i] * inv).clamp(0.0, 1.0);
            *px = Rgba([
                (d[0] * 255.0).round() as u8,
                (d[1] * 255.0).round() as u8,
                (d[2] * 255.0).round() as u8,
                (a * 255.0).round() as u8,
            ]);
        }
        img
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_film_is_transparent() {
        let film = Film::new(4);
        let img = film.to_rgba8(0.0);
        assert!(img.pixels().all(|p| p.0 == [0, 0, 0, 0]));
    }

    #[test]
    fn average_and_alpha() {
        let mut film = Film::new(2);
        film.add(0, [1.0, 0.5, 0.0], 1.0, [0.5; 3], [0.0, 0.0, 1.0]);
        film.samples = 1;
        let img = film.to_rgba8(0.0);
        let px = img.get_pixel(0, 0).0;
        assert_eq!(px[3], 255);
        // AgX of (1, 0.5, 0): bright warm colour, channels ordered.
        assert!(px[0] > 150 && px[0] > px[1] && px[1] > px[2], "{px:?}");
        assert_eq!(img.get_pixel(1, 1).0, [0, 0, 0, 0]);
    }
}
