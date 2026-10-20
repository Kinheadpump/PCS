//! Road path search for the `BuildRoad` event: a bounded Dijkstra over land cells.

use crate::biome::WorldBiome;
use crate::world::{WORLD_SIZE, WorldData};
use std::cmp::Reverse;
use std::collections::BinaryHeap;

/// Offsets of the 4 neighbors, in a fixed order.
const DIRS: [(i32, i32); 4] = [(0, -1), (-1, 0), (1, 0), (0, 1)];
/// Value of a cell that the search did not reach.
const FAR: u32 = u32::MAX;

/// Cost to enter a cell. Water is not passable (`None`).
fn step_cost(w: &WorldData, from: usize, to: usize) -> Option<u32> {
    if w.regions.id.as_slice()[to] == 0 {
        return None;
    }
    let dh =
        (i32::from(w.height.as_slice()[to]) - i32::from(w.height.as_slice()[from])).unsigned_abs();
    let mut cost = 10 + dh / 8;
    let river = u32::from(w.river.as_slice()[to]);
    if river > 0 {
        cost += 30 + 20 * river;
    }
    cost += match w.biome.as_slice()[to] {
        WorldBiome::HighMountain => 80,
        WorldBiome::Glacier => 120,
        WorldBiome::Volcanic => 30,
        WorldBiome::Marsh => 20,
        _ => 0,
    };
    Some(cost)
}

/// Find the cheapest 4-connected land path between two cells inside a box around them.
/// Ties are broken by cell index, so the result is stable. The path lists cells from `from`
/// to `to`. It is `None` if the cells are not connected inside the box.
pub fn find_path(w: &WorldData, from: u32, to: u32, margin: u32) -> Option<Vec<u32>> {
    let size = WORLD_SIZE as i32;
    let (fx, fy) = ((from % WORLD_SIZE) as i32, (from / WORLD_SIZE) as i32);
    let (tx, ty) = ((to % WORLD_SIZE) as i32, (to / WORLD_SIZE) as i32);
    let m = margin as i32;
    let x0 = (fx.min(tx) - m).max(0);
    let x1 = (fx.max(tx) + m).min(size - 1);
    let y0 = (fy.min(ty) - m).max(0);
    let y1 = (fy.max(ty) + m).min(size - 1);
    let bw = (x1 - x0 + 1) as usize;
    let local = |x: i32, y: i32| (y - y0) as usize * bw + (x - x0) as usize;
    let cells = bw * (y1 - y0 + 1) as usize;
    let mut dist = vec![FAR; cells];
    let mut parent = vec![FAR; cells];
    let mut heap = BinaryHeap::new();
    dist[local(fx, fy)] = 0;
    heap.push(Reverse((0u32, from)));
    while let Some(Reverse((d, cell))) = heap.pop() {
        let (cx, cy) = ((cell % WORLD_SIZE) as i32, (cell / WORLD_SIZE) as i32);
        if d > dist[local(cx, cy)] {
            continue;
        }
        if cell == to {
            break;
        }
        for (dx, dy) in DIRS {
            let (nx, ny) = (cx + dx, cy + dy);
            if nx < x0 || nx > x1 || ny < y0 || ny > y1 {
                continue;
            }
            let next = (ny * size + nx) as u32;
            let Some(cost) = step_cost(w, cell as usize, next as usize) else {
                continue;
            };
            let nd = d + cost;
            let slot = local(nx, ny);
            if nd < dist[slot] {
                dist[slot] = nd;
                parent[slot] = cell;
                heap.push(Reverse((nd, next)));
            }
        }
    }
    if dist[local(tx, ty)] == FAR {
        return None;
    }
    let mut path = vec![to];
    let mut at = to;
    while at != from {
        at = parent[local((at % WORLD_SIZE) as i32, (at / WORLD_SIZE) as i32)];
        path.push(at);
    }
    path.reverse();
    Some(path)
}
