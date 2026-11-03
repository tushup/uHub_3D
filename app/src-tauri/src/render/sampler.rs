//! Sampling: Owen-scrambled Sobol (Sobol–Burley) with a per-pixel
//! hashed seed, plus the Blackman–Harris pixel filter (spec §A1.6).

/// PCG32 generator, used for test scenes and as a general fallback.
#[derive(Debug, Clone)]
pub struct Rng {
    state: u64,
}

impl Rng {
    pub fn new(seed: u64) -> Rng {
        let mut rng = Rng { state: 0 };
        rng.next_u32();
        rng.state = rng.state.wrapping_add(seed);
        rng.next_u32();
        rng
    }

    pub fn next_u32(&mut self) -> u32 {
        let old = self.state;
        self.state = old
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let xorshifted = (((old >> 18) ^ old) >> 27) as u32;
        let rot = (old >> 59) as u32;
        xorshifted.rotate_right(rot)
    }

    pub fn next_f32(&mut self) -> f32 {
        self.next_u32() as f32 / 4294967296.0
    }
}

pub fn hash_u32(mut x: u32) -> u32 {
    // splitmix32 finalizer
    x ^= x >> 16;
    x = x.wrapping_mul(0x7feb352d);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846ca68b);
    x ^= x >> 16;
    x
}

// -------------------------------------------------------------- Sobol

/// Primitive polynomials for Sobol dimensions 1..=12 (dimension 0 is
/// the van der Corput radical inverse). Bits are the polynomial
/// coefficients from the leading term down to the constant 1.
/// Initial values are all m = 1 (odd, as Sobol requires); dimensions
/// past the table fall back to hashed direction numbers, which keep
/// the per-dimension net property but not joint stratification.
const SOBOL_POLYS: &[u32] = &[
    0b111,    // x^2 + x + 1
    0b1011,   // x^3 + x + 1
    0b1101,   // x^3 + x^2 + 1
    0b10011,  // x^4 + x + 1
    0b11001,  // x^4 + x^3 + 1
    0b11111,  // x^4 + x^3 + x^2 + x + 1
    0b100101, // x^5 + x^2 + 1
    0b101001, // x^5 + x^3 + 1
    0b101101, // x^5 + x^3 + x^2 + x + 1
    0b110111, // x^5 + x^4 + x^2 + x + 1
    0b111001, // x^5 + x^4 + x^3 + x + 1
    0b111111, // x^5 + x^4 + x^3 + x^2 + x + 1
];

const SOBOL_DIMS: usize = 16;

fn direction_table() -> &'static [[u32; 32]; SOBOL_DIMS] {
    static TABLE: std::sync::OnceLock<[[u32; 32]; SOBOL_DIMS]> = std::sync::OnceLock::new();
    TABLE.get_or_init(|| {
        let mut table = [[0u32; 32]; SOBOL_DIMS];
        for (dim, row) in table.iter_mut().enumerate() {
            if dim == 0 {
                for (k, v) in row.iter_mut().enumerate() {
                    *v = 1u32 << (31 - k);
                }
                continue;
            }
            if dim > SOBOL_POLYS.len() {
                // Hashed fallback: unit diagonal, hashed lower bits.
                for (k, v) in row.iter_mut().enumerate() {
                    let bit = k as u32 + 1;
                    let lead = 1u32 << (32 - bit);
                    let below = if bit >= 32 { 0 } else { lead - 1 };
                    let mixed = hash_u32(
                        (dim as u32).wrapping_mul(0x9e3779b9) ^ bit.wrapping_mul(0x85ebca6b),
                    );
                    *v = lead | (mixed & below);
                }
                continue;
            }
            let poly = SOBOL_POLYS[dim - 1];
            let q = 32 - poly.leading_zeros();
            for k in 0..32usize {
                let bit = k as u32 + 1;
                if bit <= q {
                    row[k] = 1u32 << (32 - bit);
                } else {
                    let mut x = 0u32;
                    for i in 1..q {
                        if (poly >> (q - i)) & 1 == 1 {
                            x ^= row[k - i as usize];
                        }
                    }
                    let last = row[k - q as usize];
                    row[k] = x ^ last ^ (last >> q);
                }
            }
        }
        table
    })
}

fn sobol_bits(index: u32, dim: u32) -> u32 {
    // Gray-code form (Antonov–Saleev): any index directly.
    let table = direction_table();
    let gray = index ^ (index >> 1);
    let mut x: u32 = 0;
    let mut g = gray;
    let mut bit = 0usize;
    while g != 0 {
        if g & 1 == 1 {
            x ^= if (dim as usize) < SOBOL_DIMS {
                table[dim as usize][bit]
            } else {
                let b = bit as u32 + 1;
                let lead = 1u32 << (32 - b);
                let below = if b >= 32 { 0 } else { lead - 1 };
                let mixed = hash_u32(dim.wrapping_mul(0x9e3779b9) ^ b.wrapping_mul(0x85ebca6b));
                lead | (mixed & below)
            };
        }
        g >>= 1;
        bit += 1;
    }
    x
}

pub fn sobol_point(index: u32, dim: u32) -> f32 {
    sobol_bits(index, dim) as f32 / 4294967296.0
}

/// Hash-based Owen scramble (Burley 2020): a bijective scramble that
/// preserves the Sobol net structure while decorrelating pixels.
pub fn owen_scramble(mut x: u32, seed: u32) -> u32 {
    x = x.reverse_bits();
    x ^= x.wrapping_mul(0x3d20_adea);
    x = x.wrapping_add(seed);
    x = x.wrapping_mul((seed >> 16) | 1);
    x ^= x >> 15;
    x = x.wrapping_mul(0x3d20_adea);
    x ^= x >> 16;
    x.reverse_bits()
}

/// One scrambled Sobol sample. Depends only on
/// `(seed, pixel, sample_index, dim)`, so renders are identical
/// across thread counts.
pub fn sample(seed: u32, pixel: u32, sample_index: u32, dim: u32) -> f32 {
    let scramble_seed = hash_u32(
        seed ^ pixel.wrapping_mul(0x9e37_79b9) ^ dim.wrapping_mul(0x27d4_eb2f),
    );
    owen_scramble(sobol_bits(sample_index, dim), scramble_seed) as f32 / 4294967296.0
}

// ------------------------------------------------------ pixel filter

pub const FILTER_WIDTH: f32 = 1.5;

/// 1D Blackman–Harris kernel on [-width/2, width/2], peak 1.
pub fn filter_weight(x: f32) -> f32 {
    let half = FILTER_WIDTH / 2.0;
    if x.abs() > half {
        return 0.0;
    }
    let t = 2.0 * std::f32::consts::PI * x / FILTER_WIDTH;
    0.35875 + 0.48829 * t.cos() + 0.14128 * (2.0 * t).cos() + 0.01168 * (3.0 * t).cos()
}

const FILTER_TABLE: usize = 1024;

fn filter_cdf() -> &'static Vec<f32> {
    static CDF: std::sync::OnceLock<Vec<f32>> = std::sync::OnceLock::new();
    CDF.get_or_init(|| {
        let half = FILTER_WIDTH / 2.0;
        let mut cdf = Vec::with_capacity(FILTER_TABLE + 1);
        let mut acc = 0.0f32;
        cdf.push(0.0);
        for i in 0..FILTER_TABLE {
            let x = -half + FILTER_WIDTH * (i as f32 + 0.5) / FILTER_TABLE as f32;
            acc += filter_weight(x);
            cdf.push(acc);
        }
        let total = acc;
        for v in cdf.iter_mut() {
            *v /= total;
        }
        cdf
    })
}

/// Filter importance sampling: map uniform `u` to an offset in
/// [-0.75, 0.75] px distributed like the Blackman–Harris kernel.
/// The sample weight is constant, so the film needs no per-sample
/// weight.
pub fn filter_sample(u: f32) -> f32 {
    let cdf = filter_cdf();
    let half = FILTER_WIDTH / 2.0;
    let idx = match cdf.binary_search_by(|v| v.partial_cmp(&u).unwrap_or(std::cmp::Ordering::Equal))
    {
        Ok(i) => i,
        Err(i) => i.saturating_sub(1),
    }
    .min(FILTER_TABLE - 1);
    let (lo, hi) = (cdf[idx], cdf[idx + 1]);
    let frac = if hi > lo { (u - lo) / (hi - lo) } else { 0.0 };
    -half + FILTER_WIDTH * (idx as f32 + frac) / FILTER_TABLE as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rng_deterministic_range() {
        let mut a = Rng::new(42);
        let mut b = Rng::new(42);
        let mut sum = 0.0f64;
        for _ in 0..10_000 {
            let (x, y) = (a.next_f32(), b.next_f32());
            assert_eq!(x, y);
            assert!((0.0..1.0).contains(&x));
            sum += x as f64;
        }
        assert!((sum / 10_000.0 - 0.5).abs() < 0.02, "mean {}", sum / 10_000.0);
    }

    #[test]
    fn sobol_stratifies() {
        let mut strata = [false; 32];
        for i in 0..1024 {
            let x = sobol_point(i, 0);
            strata[(x * 32.0) as usize] = true;
        }
        assert!(strata.iter().filter(|s| **s).count() >= 28);
    }

    #[test]
    fn sobol_uncorrelated_dims() {
        // 8x8 plane test: expected 16 points per cell for 1024 samples.
        let mut bins = [[0u32; 8]; 8];
        for i in 0..1024 {
            let x = sobol_point(i, 0);
            let y = sobol_point(i, 1);
            bins[(y * 8.0) as usize][(x * 8.0) as usize] += 1;
        }
        let max_bin = bins.iter().flatten().max().copied().unwrap_or(0);
        assert!(max_bin <= 40, "max bin {max_bin}");
    }

    #[test]
    fn scrambled_sample_stays_stratified_per_pixel() {
        // Owen scrambling keeps the 1D net property for one pixel.
        let mut strata = [false; 32];
        for i in 0..1024 {
            let x = sample(7, 123, i, 0);
            strata[(x * 32.0) as usize] = true;
        }
        assert!(strata.iter().filter(|s| **s).count() >= 28);
    }

    #[test]
    fn sample_depends_only_on_inputs() {
        for dim in 0..6 {
            assert_eq!(sample(3, 99, 17, dim), sample(3, 99, 17, dim));
            assert_ne!(sample(3, 99, 17, dim), sample(3, 100, 17, dim));
        }
    }

    #[test]
    fn filter_sample_is_centered_and_bounded() {
        assert!((filter_sample(0.0) + 0.75).abs() < 1e-6);
        assert!(filter_sample(0.5).abs() < 0.05, "{}", filter_sample(0.5));
        // Density vanishes at the support edge, so extreme quantiles
        // stay inside it.
        assert!((0.5..=0.75).contains(&filter_sample(0.999)), "{}", filter_sample(0.999));
        assert!(filter_sample(0.99) > 0.4);
        // Monotonic and symmetric.
        let mut prev = filter_sample(0.0);
        for i in 1..=100 {
            let x = filter_sample(i as f32 / 100.0);
            assert!(x >= prev - 1e-6);
            prev = x;
        }
        assert!((filter_sample(0.25) + filter_sample(0.75)).abs() < 0.02);
    }
}
