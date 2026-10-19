//! Integer value noise and fractal Brownian motion (fBm).
//!
//! All functions are stateless: the output depends only on the seed and the coordinates.

use base::rng::mix;

/// Fixed-point scale for interpolation weights.
const ONE: i64 = 4096;

/// Stateless random value for the lattice point `(x, y)`.
pub fn lattice(seed: u64, x: i32, y: i32) -> u16 {
    let key = (u64::from(x as u32) << 32) | u64::from(y as u32);
    (mix(seed ^ mix(key)) >> 48) as u16
}

fn smooth(t: i64) -> i64 {
    t * t * (3 * ONE - 2 * t) / (ONE * ONE)
}

fn lerp(a: i64, b: i64, s: i64) -> i64 {
    a + (b - a) * s / ONE
}

/// Value noise at tile `(x, y)`. `cell` is the lattice spacing in tiles (at least 1).
/// Output range is `0..=65535`.
pub fn value_noise(seed: u64, x: i32, y: i32, cell: u32) -> u16 {
    let cell = cell.clamp(1, 1 << 20) as i32;
    let gx = x.div_euclid(cell);
    let gy = y.div_euclid(cell);
    let tx = smooth(i64::from(x.rem_euclid(cell)) * ONE / i64::from(cell));
    let ty = smooth(i64::from(y.rem_euclid(cell)) * ONE / i64::from(cell));
    let v = |dx: i32, dy: i32| i64::from(lattice(seed, gx + dx, gy + dy));
    let top = lerp(v(0, 0), v(1, 0), tx);
    let bottom = lerp(v(0, 1), v(1, 1), tx);
    lerp(top, bottom, ty).clamp(0, 65535) as u16
}

/// Fractal noise: `octaves` (1 to 8) layers of value noise. The first layer has lattice
/// spacing `base_cell`. Each next layer halves the spacing and the weight.
/// Output range is `0..=65535`.
pub fn fbm(seed: u64, x: i32, y: i32, base_cell: u32, octaves: u32) -> u16 {
    let octaves = octaves.clamp(1, 8);
    let mut sum: u64 = 0;
    let mut weights: u64 = 0;
    for o in 0..octaves {
        let weight = 1u64 << (octaves - 1 - o);
        let cell = (base_cell >> o).max(1);
        sum += u64::from(value_noise(mix(seed ^ u64::from(o)), x, y, cell)) * weight;
        weights += weight;
    }
    (sum / weights) as u16
}

/// Find the threshold `t` so that about `per_mille / 1000` of the values are below `t`.
/// Use it to turn noise into an exact area share. It uses a counting sort, so the
/// result is deterministic.
pub fn threshold_for_share(values: &[u16], per_mille: u32) -> u32 {
    let per_mille = per_mille.min(1000) as usize;
    let mut counts = vec![0u32; 65536];
    for &v in values {
        counts[usize::from(v)] += 1;
    }
    let target = values.len() * per_mille / 1000;
    let mut below = 0usize;
    for (v, &c) in counts.iter().enumerate() {
        if below + c as usize > target {
            return v as u32;
        }
        below += c as usize;
    }
    65536
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn value_noise_is_repeatable_and_seed_dependent() {
        assert_eq!(value_noise(1, 10, 20, 8), value_noise(1, 10, 20, 8));
        let differs = (0..32).any(|i| value_noise(1, i, 3, 8) != value_noise(2, i, 3, 8));
        assert!(differs);
    }

    #[test]
    fn value_noise_hits_lattice_values() {
        assert_eq!(value_noise(5, 16, 24, 8), lattice(5, 2, 3));
        assert_eq!(value_noise(5, -8, -8, 8), lattice(5, -1, -1));
    }

    #[test]
    fn value_noise_is_smooth() {
        for x in 0..64 {
            let a = i32::from(value_noise(3, x, 7, 16));
            let b = i32::from(value_noise(3, x + 1, 7, 16));
            assert!((a - b).abs() < 9000, "jump at {x}");
        }
    }

    #[test]
    fn fbm_spans_a_wide_range() {
        let (mut lo, mut hi) = (u16::MAX, 0u16);
        for y in 0..64 {
            for x in 0..64 {
                let v = fbm(9, x * 3, y * 3, 32, 4);
                lo = lo.min(v);
                hi = hi.max(v);
            }
        }
        assert!(hi - lo > 20000, "range {lo}..{hi}");
    }

    #[test]
    fn share_threshold_is_exact_for_flat_data() {
        let values: Vec<u16> = (0..1000).map(|i| (i * 60) as u16).collect();
        let t = threshold_for_share(&values, 100);
        let below = values.iter().filter(|&&v| u32::from(v) < t).count();
        assert_eq!(below, 100);
    }
}
