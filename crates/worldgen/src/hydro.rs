//! Steps 5 and 8: lakes (Priority-Flood depression filling) and rivers (D8 flow accumulation).

use crate::world::{WORLD_SIZE, WorldParams, world_grid};
use base::Grid;
use std::cmp::Reverse;
use std::collections::BinaryHeap;

/// Drain value of cells with no downstream cell (ocean cells).
pub const NO_DIR: u8 = 255;

/// The 8 neighbor offsets (dx, dy). Direction d and direction (d + 4) % 8 are opposite.
pub const DIRS: [(i32, i32); 8] = [
    (1, 0),
    (1, 1),
    (0, 1),
    (-1, 1),
    (-1, 0),
    (-1, -1),
    (0, -1),
    (1, -1),
];

/// Output of step 5.
#[derive(Clone, Debug)]
pub struct Lakes {
    /// Height after depression filling.
    pub filled: Grid<i16>,
    /// Lake ID (0 for none). IDs count up in row-major order of the first cell.
    pub lake: Grid<u16>,
    /// Direction index of the next cell downstream, or NO_DIR in the ocean.
    pub drain: Grid<u8>,
}

/// Output of step 8.
#[derive(Clone, Debug)]
pub struct Rivers {
    /// Weighted flow accumulation of each cell.
    pub flow: Grid<u32>,
    /// River size: 0 none, 1 small, 2 medium, 3 large.
    pub river: Grid<u8>,
}

/// Index of the neighbor of cell i in direction d, or None outside the world.
fn neighbor(i: usize, d: usize) -> Option<usize> {
    let w = WORLD_SIZE as i32;
    let x = (i as i32 % w) + DIRS[d].0;
    let y = (i as i32 / w) + DIRS[d].1;
    (x >= 0 && y >= 0 && x < w && y < w).then(|| (y * w + x) as usize)
}

/// Priority-Flood from the ocean. Ties in the queue break by cell index. The cell that
/// first reaches a land cell becomes its drain, so the drain graph has no cycle and every
/// path ends in the ocean. Deep filled depressions become lakes.
pub fn fill_lakes(height: &Grid<i16>, sea_level: i16, params: &WorldParams) -> Lakes {
    let hs = height.as_slice();
    let n = hs.len();
    let mut filled = hs.to_vec();
    let mut drain = vec![NO_DIR; n];
    let mut visited: Vec<bool> = hs.iter().map(|&h| h < sea_level).collect();
    let mut heap = BinaryHeap::new();
    for i in 0..n {
        let borders_land = (0..8).any(|d| neighbor(i, d).is_some_and(|j| !visited[j]));
        if visited[i] && borders_land {
            heap.push(Reverse((filled[i], i as u32)));
        }
    }
    while let Some(Reverse((level, i))) = heap.pop() {
        for d in 0..8 {
            let Some(j) = neighbor(i as usize, d) else {
                continue;
            };
            if visited[j] {
                continue;
            }
            visited[j] = true;
            filled[j] = hs[j].max(level);
            drain[j] = ((d + 4) % 8) as u8;
            heap.push(Reverse((filled[j], j as u32)));
        }
    }
    let lake = label_lakes(hs, &filled, sea_level, params.lake_depth_min);
    Lakes {
        filled: world_grid(filled, 0),
        lake: world_grid(lake, 0),
        drain: world_grid(drain, NO_DIR),
    }
}

/// Mark land cells that are at least min_depth below the filled level, and number the
/// 4-connected lake areas.
fn label_lakes(hs: &[i16], filled: &[i16], sea_level: i16, min_depth: i16) -> Vec<u16> {
    let n = hs.len();
    let is_lake: Vec<bool> = (0..n)
        .map(|i| hs[i] >= sea_level && filled[i] - hs[i] >= min_depth)
        .collect();
    let mut lake = vec![0u16; n];
    let mut next_id: u32 = 0;
    for start in 0..n {
        if !is_lake[start] || lake[start] != 0 {
            continue;
        }
        next_id += 1;
        let id = u16::try_from(next_id).unwrap_or(u16::MAX);
        lake[start] = id;
        let mut stack = vec![start];
        while let Some(i) = stack.pop() {
            for d in [0usize, 2, 4, 6] {
                if let Some(j) = neighbor(i, d)
                    && is_lake[j]
                    && lake[j] == 0
                {
                    lake[j] = id;
                    stack.push(j);
                }
            }
        }
    }
    lake
}

/// Flow accumulation along the drain graph and river size classes. A land cell adds
/// 1 + moisture / 32 units of water, so wet land makes bigger rivers. Cells are processed
/// from the leaves of the drain tree to the ocean, which is the same as descending height order.
pub fn make_rivers(
    height: &Grid<i16>,
    sea_level: i16,
    lakes: &Lakes,
    moisture: &Grid<u8>,
    params: &WorldParams,
) -> Rivers {
    let hs = height.as_slice();
    let n = hs.len();
    let drain = lakes.drain.as_slice();
    let lake = lakes.lake.as_slice();
    let mut flow: Vec<u32> = (0..n)
        .map(|i| {
            if hs[i] >= sea_level {
                1 + u32::from(moisture.as_slice()[i]) / 32
            } else {
                0
            }
        })
        .collect();
    let mut inflow = vec![0u32; n];
    for i in 0..n {
        if let Some(j) = downstream(i, drain) {
            inflow[j] += 1;
        }
    }
    let mut stack: Vec<usize> = (0..n)
        .filter(|&i| hs[i] >= sea_level && inflow[i] == 0)
        .collect();
    while let Some(i) = stack.pop() {
        let Some(j) = downstream(i, drain) else {
            continue;
        };
        flow[j] += flow[i];
        inflow[j] -= 1;
        if inflow[j] == 0 && hs[j] >= sea_level {
            stack.push(j);
        }
    }
    let river: Vec<u8> = (0..n)
        .map(|i| {
            if hs[i] < sea_level || lake[i] != 0 {
                0
            } else if flow[i] >= params.river_large {
                3
            } else if flow[i] >= params.river_medium {
                2
            } else if flow[i] >= params.river_small {
                1
            } else {
                0
            }
        })
        .collect();
    Rivers {
        flow: world_grid(flow, 0),
        river: world_grid(river, 0),
    }
}

/// The cell downstream of cell i, or None for ocean cells.
fn downstream(i: usize, drain: &[u8]) -> Option<usize> {
    let d = drain[i];
    if d == NO_DIR {
        None
    } else {
        neighbor(i, usize::from(d))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opposite_directions_cancel() {
        for d in 0..8 {
            let (a, b) = (DIRS[d], DIRS[(d + 4) % 8]);
            assert_eq!((a.0 + b.0, a.1 + b.1), (0, 0));
        }
    }

    #[test]
    fn a_pit_is_filled_and_becomes_a_lake() {
        // A bowl of height 0 in a plain of height 100 next to the ocean.
        let size = WORLD_SIZE as i32;
        let mut cells = vec![100i16; (size * size) as usize];
        for x in 0..size {
            cells[x as usize] = -50;
        }
        for y in 100..110 {
            for x in 100..110 {
                cells[(y * size + x) as usize] = 0;
            }
        }
        let height = world_grid(cells, 0);
        let lakes = fill_lakes(&height, -20, &WorldParams::default());
        let at = |x: i32, y: i32| (y * size + x) as usize;
        assert_eq!(lakes.filled.as_slice()[at(105, 105)], 100);
        assert_eq!(lakes.lake.as_slice()[at(105, 105)], 1);
        assert_eq!(lakes.lake.as_slice()[at(50, 50)], 0);
    }
}
