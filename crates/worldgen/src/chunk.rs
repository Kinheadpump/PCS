//! Spike S11a: world chunks that generate on demand from the seed (Q-016).
//!
//! This is a prototype. The game does not use it. It shows that the first world layers
//! (height, temperature, moisture, land) can be a pure function of the seed and the cell
//! position. A chunk is then only a window on that function, so chunk edges have no seam and
//! the order of chunk generation does not matter.
//!
//! The layers that need the whole world are not here: rivers, lakes, the regions, the history,
//! and the rain shadow. `docs/INFINITE_WORLD.md` describes how each one changes.

use crate::climate::{MOISTURE_BY_LATITUDE, TEMP_BY_LATITUDE, latitude_value};
use crate::noise::fbm;
use crate::plates::ridge_shape;
use crate::world::WorldParams;
use base::Grid;
use base::rng::mix;

/// Side of a chunk in world cells.
pub const CHUNK_SIZE: u32 = 64;

/// Distance from pole to pole in world cells. The latitude repeats with this period, so the
/// world has no pole edge: a pole is followed by the same climate bands in mirror order.
pub const LATITUDE_SPAN: i32 = 512;

/// Fixed sea level in height units. It is the 60th percentile of the height, so the land is about 40%. The finite world finds it from the land share. A world
/// without an edge cannot do that, so the sea level is a constant (see the study, §4.1).
pub const SEA_LEVEL: i16 = 475;

/// Position of a chunk in chunk units. Negative values are valid.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ChunkPos {
    /// Chunk column.
    pub cx: i32,
    /// Chunk row.
    pub cy: i32,
}

impl ChunkPos {
    /// The chunk that holds the world cell `(x, y)`.
    pub fn of_cell(x: i32, y: i32) -> Self {
        let s = CHUNK_SIZE as i32;
        Self {
            cx: x.div_euclid(s),
            cy: y.div_euclid(s),
        }
    }

    /// World cell of the top left corner of the chunk.
    pub fn origin(self) -> (i32, i32) {
        let s = CHUNK_SIZE as i32;
        (self.cx * s, self.cy * s)
    }
}

/// The first world layers of one chunk.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Chunk {
    /// Position of the chunk.
    pub pos: ChunkPos,
    /// Height in height units.
    pub height: Grid<i16>,
    /// Mean temperature in degrees Celsius.
    pub temperature: Grid<i8>,
    /// Moisture, 0 to 255.
    pub moisture: Grid<u8>,
}

impl Chunk {
    /// True if the cell at chunk-local `(x, y)` is land.
    pub fn is_land(&self, x: i32, y: i32) -> bool {
        self.height.get(x, y).is_some_and(|&h| h >= SEA_LEVEL)
    }
}

/// Height of one world cell. It is the formula of `height::make_height` without the edge ring
/// and without the clamp of the warped position at zero.
pub fn cell_height(seed: u64, x: i32, y: i32, params: &WorldParams) -> i16 {
    let warp_x = mix(seed ^ 0x11);
    let warp_y = mix(seed ^ 0x22);
    let detail = mix(seed ^ 0x33);
    let dx = (i32::from(fbm(warp_x, x, y, 48, 3)) - 32768) >> 11;
    let dy = (i32::from(fbm(warp_y, x, y, 48, 3)) - 32768) >> 11;
    let noise = i64::from(fbm(detail, x + dx, y + dy, params.height_cell, 6)) - 32768;
    let mut h = (noise * i64::from(params.height_amp) / 32768) as i32;
    let shape = ridge_shape(seed, x, y, params.ridge_cell);
    h += (shape * i64::from(params.ridge_height) / 65536) as i32;
    h.clamp(-4000, i32::from(i16::MAX)) as i16
}

/// Position of a row inside the latitude cycle, 0 to `LATITUDE_SPAN`, where the middle is the
/// equator and the ends are the poles.
fn latitude_row(y: i32) -> i32 {
    y.rem_euclid(LATITUDE_SPAN)
}

/// Mean temperature of one world cell from its latitude and its height.
pub fn cell_temperature(seed: u64, x: i32, y: i32, height: i16, params: &WorldParams) -> i8 {
    let base = latitude_value(&TEMP_BY_LATITUDE, latitude_row(y), LATITUDE_SPAN);
    let rel = (i32::from(height) - i32::from(SEA_LEVEL)).max(0);
    let lapse = rel * params.lapse_per_10k / 10000;
    let noise = (i32::from(fbm(mix(seed ^ 1), x, y, 64, 3)) - 32768) / 8192;
    (base - lapse + noise).clamp(-60, 60) as i8
}

/// Moisture of one world cell. It is an approximation: the latitude band plus noise. The finite
/// world carries moisture along the wind from the ocean, which needs the cells upwind.
pub fn cell_moisture(seed: u64, x: i32, y: i32, height: i16) -> u8 {
    if height < SEA_LEVEL {
        return 255;
    }
    let base = latitude_value(&MOISTURE_BY_LATITUDE, latitude_row(y), LATITUDE_SPAN);
    let noise = (i32::from(fbm(mix(seed ^ 2), x, y, 96, 4)) - 32768) / 546;
    (base + noise).clamp(0, 255) as u8
}

/// Make one chunk. The result depends only on `seed`, `params`, and `pos`.
pub fn generate_chunk(seed: u64, params: &WorldParams, pos: ChunkPos) -> Chunk {
    let side = CHUNK_SIZE as usize;
    let (ox, oy) = pos.origin();
    let mut height = Vec::with_capacity(side * side);
    let mut temperature = Vec::with_capacity(side * side);
    let mut moisture = Vec::with_capacity(side * side);
    for ly in 0..CHUNK_SIZE as i32 {
        for lx in 0..CHUNK_SIZE as i32 {
            let (x, y) = (ox + lx, oy + ly);
            let h = cell_height(seed, x, y, params);
            height.push(h);
            temperature.push(cell_temperature(seed, x, y, h, params));
            moisture.push(cell_moisture(seed, x, y, h));
        }
    }
    Chunk {
        pos,
        height: Grid::from_vec(CHUNK_SIZE, CHUNK_SIZE, height)
            .unwrap_or_else(|| Grid::new(CHUNK_SIZE, CHUNK_SIZE, 0)),
        temperature: Grid::from_vec(CHUNK_SIZE, CHUNK_SIZE, temperature)
            .unwrap_or_else(|| Grid::new(CHUNK_SIZE, CHUNK_SIZE, 0)),
        moisture: Grid::from_vec(CHUNK_SIZE, CHUNK_SIZE, moisture)
            .unwrap_or_else(|| Grid::new(CHUNK_SIZE, CHUNK_SIZE, 0)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p() -> WorldParams {
        WorldParams::default()
    }

    #[test]
    fn a_chunk_is_repeatable_and_depends_on_the_seed() {
        let pos = ChunkPos { cx: 3, cy: -2 };
        assert_eq!(generate_chunk(1, &p(), pos), generate_chunk(1, &p(), pos));
        assert_ne!(generate_chunk(1, &p(), pos), generate_chunk(2, &p(), pos));
    }

    #[test]
    fn chunk_cells_equal_the_cell_function_so_edges_have_no_seam() {
        // Two neighbor chunks across a negative border: each cell equals the direct value.
        for pos in [ChunkPos { cx: -1, cy: -1 }, ChunkPos { cx: 0, cy: -1 }] {
            let c = generate_chunk(7, &p(), pos);
            let (ox, oy) = pos.origin();
            for ly in 0..CHUNK_SIZE as i32 {
                for lx in 0..CHUNK_SIZE as i32 {
                    let h = cell_height(7, ox + lx, oy + ly, &p());
                    assert_eq!(c.height.get(lx, ly).copied(), Some(h));
                    assert_eq!(
                        c.temperature.get(lx, ly).copied(),
                        Some(cell_temperature(7, ox + lx, oy + ly, h, &p()))
                    );
                }
            }
        }
    }

    #[test]
    fn chunk_position_of_a_cell_handles_negative_cells() {
        assert_eq!(ChunkPos::of_cell(0, 0), ChunkPos { cx: 0, cy: 0 });
        assert_eq!(ChunkPos::of_cell(63, 64), ChunkPos { cx: 0, cy: 1 });
        assert_eq!(ChunkPos::of_cell(-1, -64), ChunkPos { cx: -1, cy: -1 });
        assert_eq!(ChunkPos::of_cell(-65, 0), ChunkPos { cx: -2, cy: 0 });
        assert_eq!(ChunkPos { cx: -1, cy: -1 }.origin(), (-64, -64));
    }

    #[test]
    fn latitude_repeats_so_the_climate_has_no_pole_edge() {
        let t = |y| cell_temperature(5, 0, y, SEA_LEVEL, &p());
        // The pole rows are cold, the equator row is warm, and the cycle repeats.
        assert!(t(0) < -15 && t(LATITUDE_SPAN) < -15 && t(-LATITUDE_SPAN) < -15);
        assert!(t(LATITUDE_SPAN / 2) > 20 && t(-LATITUDE_SPAN / 2) > 20);
        assert!(t(LATITUDE_SPAN * 40 + 3).abs_diff(t(3)) <= 2);
    }

    #[test]
    fn far_chunks_work_and_the_land_share_is_stable_everywhere() {
        // A coarse sample of 1 in 8 cells of 9 distant 4x4 chunk blocks.
        let blocks = [
            (0, 0),
            (500, 0),
            (-500, 0),
            (0, 500),
            (0, -500),
            (3000, 3000),
            (-3000, 700),
            (9000, -9000),
            (-20000, 20000),
        ];
        for (bx, by) in blocks {
            let (mut land, mut total) = (0u32, 0u32);
            for cy in by..by + 4 {
                for cx in bx..bx + 4 {
                    let c = generate_chunk(11, &p(), ChunkPos { cx, cy });
                    for ly in (0..CHUNK_SIZE as i32).step_by(8) {
                        for lx in (0..CHUNK_SIZE as i32).step_by(8) {
                            total += 1;
                            land += u32::from(c.is_land(lx, ly));
                        }
                    }
                }
            }
            let share = land * 1000 / total;
            eprintln!("block ({bx},{by}) land share {share} per-mille");
            assert!(
                (150..=750).contains(&share),
                "block ({bx},{by}) share {share}"
            );
        }
    }
}
