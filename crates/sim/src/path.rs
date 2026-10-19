//! Grid A* with 8 directions (TECHNICAL_DESIGN §6.3).
//!
//! Costs are 10 (straight) and 14 (diagonal). The heuristic is octile. The open list
//! orders nodes by f, then h, then tile index, so paths are deterministic. The buffers
//! carry a generation stamp, so a search does not clear them.

use crate::map::LocalMap;
use std::cmp::Reverse;
use std::collections::BinaryHeap;

/// Cost of a straight step.
pub const COST_STRAIGHT: u32 = 10;
/// Cost of a diagonal step.
pub const COST_DIAGONAL: u32 = 14;

/// The 8 directions. The order is fixed: it is part of the tie-break.
pub const DIRS: [(i32, i32); 8] = [
    (0, -1),
    (1, 0),
    (0, 1),
    (-1, 0),
    (1, -1),
    (1, 1),
    (-1, 1),
    (-1, -1),
];

/// Cost of one step in direction `(dx, dy)`.
pub fn step_cost(dx: i32, dy: i32) -> u32 {
    if dx != 0 && dy != 0 {
        COST_DIAGONAL
    } else {
        COST_STRAIGHT
    }
}

/// True if a colonist on `(x, y)` can step by `(dx, dy)`. A diagonal step needs both
/// neighbor tiles to be passable (no corner cutting).
pub fn can_step(map: &LocalMap, x: i32, y: i32, dx: i32, dy: i32) -> bool {
    if !map.passable(x + dx, y + dy) {
        return false;
    }
    dx == 0 || dy == 0 || (map.passable(x + dx, y) && map.passable(x, y + dy))
}

/// Octile distance between two tiles: 10 per straight step, 14 per diagonal step.
pub fn octile(ax: i32, ay: i32, bx: i32, by: i32) -> u32 {
    let dx = ax.abs_diff(bx);
    let dy = ay.abs_diff(by);
    COST_STRAIGHT * dx.max(dy) + (COST_DIAGONAL - COST_STRAIGHT) * dx.min(dy)
}

/// Result of one search.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SearchResult {
    /// True if a path exists (the output is filled).
    pub found: bool,
    /// Nodes taken from the open list and expanded.
    pub expanded: u32,
    /// Total path cost (0 if not found).
    pub cost: u32,
}

/// Search data of one tile. `seen` and `closed` hold the generation stamp.
#[derive(Clone, Copy, Debug, Default)]
struct Node {
    seen: u32,
    closed: u32,
    g: u32,
    parent: u32,
}

const INDEX_BITS: u32 = 24;
const INDEX_MASK: u64 = (1 << INDEX_BITS) - 1;

/// Pack an open-list entry into one integer that sorts by f, then h, then tile index.
/// f has 24 bits (it saturates; a map side is at most 4096), h 16 bits, index 24 bits.
fn pack(f: u32, h: u32, index: u32) -> u64 {
    (u64::from(f.min(0xFF_FFFF)) << 40)
        | (u64::from(h.min(0xFFFF)) << INDEX_BITS)
        | u64::from(index)
}

/// Reusable A* search buffers. This is a derived cache: it is not saved.
#[derive(Clone, Debug, Default)]
pub struct PathFinder {
    generation: u32,
    nodes: Vec<Node>,
    open: BinaryHeap<Reverse<u64>>,
}

impl PathFinder {
    /// Find the cheapest path from `start` to `goal`.
    ///
    /// `out` is cleared and filled in reverse order: the goal first, the first step
    /// last, the start excluded. A caller takes steps with `pop`. If `start == goal`,
    /// the path is empty and `found` is true.
    pub fn find(
        &mut self,
        map: &LocalMap,
        start: (u16, u16),
        goal: (u16, u16),
        out: &mut Vec<(u16, u16)>,
    ) -> SearchResult {
        out.clear();
        let fail = SearchResult {
            found: false,
            expanded: 0,
            cost: 0,
        };
        let (sx, sy) = (i32::from(start.0), i32::from(start.1));
        let (gx, gy) = (i32::from(goal.0), i32::from(goal.1));
        if !map.passable(sx, sy) || !map.passable(gx, gy) {
            return fail;
        }
        let (Some(s), Some(t)) = (map.index(sx, sy), map.index(gx, gy)) else {
            return fail;
        };
        let (s, t) = (s as u32, t as u32);
        if s == t {
            return SearchResult {
                found: true,
                expanded: 0,
                cost: 0,
            };
        }
        self.begin(map.width() as usize * map.height() as usize);
        let gen_id = self.generation;
        let w = map.width() as i32;
        let hgt = map.height() as i32;
        let pass = map.passable_slice();

        self.nodes[s as usize] = Node {
            seen: gen_id,
            closed: 0,
            g: 0,
            parent: s,
        };
        let h0 = octile(sx, sy, gx, gy);
        self.open.push(Reverse(pack(h0, h0, s)));

        let mut expanded = 0u32;
        while let Some(Reverse(key)) = self.open.pop() {
            let i = (key & INDEX_MASK) as u32;
            let iu = i as usize;
            if self.nodes[iu].closed == gen_id {
                continue;
            }
            self.nodes[iu].closed = gen_id;
            expanded += 1;
            let gi = self.nodes[iu].g;
            if i == t {
                let mut cur = i;
                while cur != s {
                    out.push(((cur as i32 % w) as u16, (cur as i32 / w) as u16));
                    cur = self.nodes[cur as usize].parent;
                }
                return SearchResult {
                    found: true,
                    expanded,
                    cost: gi,
                };
            }
            let (x, y) = (i as i32 % w, i as i32 / w);
            for (dx, dy) in DIRS {
                let (nx, ny) = (x + dx, y + dy);
                if nx < 0 || ny < 0 || nx >= w || ny >= hgt {
                    continue;
                }
                let nu = (ny * w + nx) as usize;
                if !pass[nu] {
                    continue;
                }
                // Same rule as `can_step`: no corner cutting.
                if dx != 0
                    && dy != 0
                    && !(pass[(y * w + nx) as usize] && pass[(ny * w + x) as usize])
                {
                    continue;
                }
                let node = &mut self.nodes[nu];
                if node.closed == gen_id {
                    continue;
                }
                let ng = gi + step_cost(dx, dy);
                if node.seen != gen_id || ng < node.g {
                    node.seen = gen_id;
                    node.g = ng;
                    node.parent = i;
                    let h = octile(nx, ny, gx, gy);
                    self.open.push(Reverse(pack(ng + h, h, nu as u32)));
                }
            }
        }
        SearchResult {
            found: false,
            expanded,
            cost: 0,
        }
    }

    /// Prepare the buffers for a new search over `len` tiles.
    fn begin(&mut self, len: usize) {
        if self.nodes.len() != len {
            self.nodes = vec![Node::default(); len];
            self.generation = 0;
        }
        if self.generation == u32::MAX {
            self.nodes.fill(Node::default());
            self.generation = 0;
        }
        self.generation += 1;
        self.open.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scenario::map_from_ascii;

    fn path(text: &str, a: (u16, u16), b: (u16, u16)) -> (SearchResult, Vec<(u16, u16)>) {
        let (map, _) = map_from_ascii(text).unwrap();
        let mut out = Vec::new();
        let r = PathFinder::default().find(&map, a, b, &mut out);
        (r, out)
    }

    #[test]
    fn straight_path_costs_10_per_tile() {
        let (r, out) = path(".....", (0, 0), (4, 0));
        assert!(r.found);
        assert_eq!(r.cost, 40);
        assert_eq!(out.len(), 4);
        assert_eq!(out.first(), Some(&(4, 0)));
        assert_eq!(out.last(), Some(&(1, 0)));
    }

    #[test]
    fn diagonal_path_costs_14_per_tile() {
        let (r, out) = path("...\n...\n...", (0, 0), (2, 2));
        assert_eq!(r.cost, 28);
        assert_eq!(out, vec![(2, 2), (1, 1)]);
    }

    #[test]
    fn start_equals_goal_is_an_empty_path() {
        let (r, out) = path("...", (1, 0), (1, 0));
        assert!(r.found);
        assert!(out.is_empty());
    }

    #[test]
    fn path_goes_around_a_wall() {
        let (r, out) = path("...#...\n...#...\n.......", (0, 0), (6, 0));
        assert!(r.found);
        assert!(out.contains(&(3, 2)), "path must use the gap: {out:?}");
        assert!(!out.contains(&(3, 0)) && !out.contains(&(3, 1)));
    }

    #[test]
    fn blocked_goal_and_closed_room_fail() {
        let (r, _) = path("..#..", (0, 0), (2, 0));
        assert!(!r.found);
        let (r, out) = path("..#..", (0, 0), (4, 0));
        assert!(!r.found && out.is_empty());
        assert!(r.expanded > 0, "the search explored the closed area");
    }

    #[test]
    fn diagonal_does_not_cut_a_corner() {
        let (r, out) = path(".#\n..", (0, 0), (1, 1));
        assert_eq!(r.cost, 20, "must go (0,1) then (1,1): {out:?}");
        let (r, _) = path(".#\n#.", (0, 0), (1, 1));
        assert!(!r.found, "two wall corners block the diagonal");
    }

    #[test]
    fn equal_cost_paths_pick_the_same_one_every_time() {
        let text = ".......\n.......\n.......\n.......";
        let (_, first) = path(text, (0, 0), (6, 3));
        for _ in 0..20 {
            assert_eq!(path(text, (0, 0), (6, 3)).1, first);
        }
    }

    #[test]
    fn buffers_are_reused_across_searches() {
        let (map, _) = map_from_ascii("....\n.##.\n....").unwrap();
        let mut finder = PathFinder::default();
        let mut out = Vec::new();
        let a = finder.find(&map, (0, 0), (3, 2), &mut out);
        let first = out.clone();
        for _ in 0..5 {
            finder.find(&map, (3, 2), (0, 0), &mut out);
        }
        let again = finder.find(&map, (0, 0), (3, 2), &mut out);
        assert_eq!((a, first), (again, out));
    }

    #[test]
    fn octile_heuristic_values() {
        assert_eq!(octile(0, 0, 3, 0), 30);
        assert_eq!(octile(0, 0, 3, 3), 42);
        assert_eq!(octile(0, 0, 5, 2), 58);
    }
}
