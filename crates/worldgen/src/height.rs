//! Steps 3 and 4: height and sea level.

use crate::noise::{fbm, threshold_for_share};
use crate::world::{WORLD_SIZE, WorldParams, world_grid};
use base::Grid;
use base::rng::mix;

/// Height of the cells in the outer ring. It is far below any sea level.
const DEEP_OCEAN: i32 = -4000;

/// Make the height: fBm (6 octaves) with one domain warp, plus uplift, with ocean at the edges.
pub fn make_height(seed: u64, uplift: &Grid<i16>, params: &WorldParams) -> Grid<i16> {
    let size = WORLD_SIZE as i32;
    let n = (WORLD_SIZE * WORLD_SIZE) as usize;
    let warp_x = mix(seed ^ 0x11);
    let warp_y = mix(seed ^ 0x22);
    let detail = mix(seed ^ 0x33);
    let amp = i64::from(params.height_amp);
    let mut cells = Vec::with_capacity(n);
    for y in 0..size {
        for x in 0..size {
            let dx = (i32::from(fbm(warp_x, x, y, 48, 3)) - 32768) >> 11;
            let dy = (i32::from(fbm(warp_y, x, y, 48, 3)) - 32768) >> 11;
            let wx = (x + dx).max(0);
            let wy = (y + dy).max(0);
            let noise = i64::from(fbm(detail, wx, wy, params.height_cell, 6)) - 32768;
            let mut h = (noise * amp / 32768) as i32;
            h += i32::from(uplift.get(x, y).copied().unwrap_or(0));
            let edge = x.min(y).min(size - 1 - x).min(size - 1 - y);
            if edge < params.edge_width {
                let d = params.edge_width - edge;
                h -= d * d * 6;
            }
            if edge <= 1 {
                h = DEEP_OCEAN;
            }
            cells.push(h.clamp(DEEP_OCEAN, i32::from(i16::MAX)) as i16);
        }
    }
    world_grid(cells, 0)
}

/// Sea level: the percentile of the heights that leaves the land share given in the parameters.
/// Cells with a height at or above the sea level are land.
pub fn sea_level(height: &Grid<i16>, params: &WorldParams) -> i16 {
    let shifted: Vec<u16> = height
        .as_slice()
        .iter()
        .map(|&h| (i32::from(h) + 32768) as u16)
        .collect();
    let ocean_share = 1000 - params.land_per_mille.min(1000);
    let t = threshold_for_share(&shifted, ocean_share);
    (t as i32 - 32768).clamp(i32::from(i16::MIN), i32::from(i16::MAX)) as i16
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edges_are_ocean_and_sea_level_gives_the_land_share() {
        let p = WorldParams::default();
        let up = world_grid(vec![0i16; (WORLD_SIZE * WORLD_SIZE) as usize], 0);
        let h = make_height(4, &up, &p);
        let sea = sea_level(&h, &p);
        for i in 0..WORLD_SIZE as i32 {
            assert!(h.get(i, 0).copied().unwrap_or(0) < sea);
            assert!(h.get(0, i).copied().unwrap_or(0) < sea);
            assert!(h.get(i, 255).copied().unwrap_or(0) < sea);
        }
        let land = h.as_slice().iter().filter(|&&v| v >= sea).count();
        let share = land * 1000 / h.as_slice().len();
        assert!(share.abs_diff(400) <= 5, "land share {share}");
    }
}
