//! Step 2: mountain uplift and volcanic flags.
//!
//! This is the fallback of the plate model (TECHNICAL_DESIGN §8): ridged noise makes long
//! mountain ranges. A later sprint can replace it with jittered Voronoi plates. The
//! outputs (uplift, volcanic flag) stay the same.

use crate::noise::{lattice, value_noise};
use crate::world::{WORLD_SIZE, WorldParams, world_grid};
use base::Grid;
use base::rng::mix;

/// Ridge value 0 to 65535 of one noise octave. It is highest where the noise crosses the middle.
fn ridge(seed: u64, x: i32, y: i32, cell: u32) -> i64 {
    let n = i64::from(value_noise(seed, x, y, cell));
    65535 - 2 * (n - 32768).abs()
}

/// Shape of the mountain range at a cell, 0 to 65535. It is zero away from the ridge lines.
pub fn ridge_shape(seed: u64, x: i32, y: i32, cell: u32) -> i64 {
    let main = ridge(mix(seed ^ 1), x, y, cell);
    let detail = ridge(mix(seed ^ 2), x, y, (cell / 2).max(1));
    let t = (main * 3 + detail) / 4;
    let t2 = (t * t) >> 16;
    let t4 = (t2 * t2) >> 16;
    ((t4 - 20000).max(0) * 65536 / (65536 - 20000)).min(65535)
}

/// Make the uplift (height units) and the volcanic flag of each cell.
pub fn make_uplift(seed: u64, params: &WorldParams) -> (Grid<i16>, Grid<bool>) {
    let n = (WORLD_SIZE * WORLD_SIZE) as usize;
    let mut uplift = Vec::with_capacity(n);
    let mut volcanic = Vec::with_capacity(n);
    let volcano_seed = mix(seed ^ 3);
    let roll_limit = i64::from(params.volcano_per_mille) * 65536 / 1000;
    for y in 0..WORLD_SIZE as i32 {
        for x in 0..WORLD_SIZE as i32 {
            let shape = ridge_shape(seed, x, y, params.ridge_cell);
            let up = shape * i64::from(params.ridge_height) / 65536;
            uplift.push(up.clamp(0, i64::from(i16::MAX)) as i16);
            let rolled = i64::from(lattice(volcano_seed, x, y)) < roll_limit;
            volcanic.push(shape >= 60000 && rolled);
        }
    }
    (world_grid(uplift, 0), world_grid(volcanic, false))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uplift_is_non_negative_and_bounded() {
        let p = WorldParams::default();
        let (up, _) = make_uplift(5, &p);
        assert!(
            up.as_slice()
                .iter()
                .all(|&v| v >= 0 && i32::from(v) <= p.ridge_height)
        );
        assert!(up.as_slice().iter().any(|&v| v > 500));
        assert!(up.as_slice().contains(&0));
    }

    #[test]
    fn uplift_is_repeatable() {
        let p = WorldParams::default();
        assert_eq!(make_uplift(9, &p), make_uplift(9, &p));
    }
}
