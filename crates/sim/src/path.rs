//! Grid A* with 8 directions (TECHNICAL_DESIGN §6.3).
//!
//! Costs are 10 (straight) and 14 (diagonal). The heuristic is octile. The open list
//! orders nodes by f, then h, then tile index, so paths are deterministic. The open
//! list is one small heap for each f value (a bucket queue): f is an integer and grows
//! slowly, so the pop order equals the order of one global heap. The buffers carry a
//! generation stamp, so a search does not clear them.

use crate::map::LocalMap;

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

/// Search data of one tile (8 bytes). `tag` holds the generation stamp in the upper
/// bits (`2 * generation` if open, `2 * generation + 1` if closed) and the direction
/// of the step that reached the tile in the lowest 3 bits.
#[derive(Clone, Copy, Debug, Default)]
struct Node {
    tag: u32,
    g: u32,
}

/// Highest generation: the stamp `2 * generation + 1` must fit in 29 bits.
const MAX_GENERATION: u32 = (1 << 28) - 1;

const COORD_BITS: u32 = 13;
const COORD_MASK: u64 = (1 << COORD_BITS) - 1;

/// Pack an open-list entry into one integer that sorts by h, then tile index. The
/// tile is given by its padded coordinates (13 bits each: a map side is at most 4096).
/// Sorting by (y, x) equals sorting by tile index. h has 16 bits (it saturates).
fn pack(h: u32, x: i32, y: i32) -> u64 {
    (u64::from(h.min(0xFFFF)) << (2 * COORD_BITS)) | ((y as u64) << COORD_BITS) | x as u64
}

/// Reusable A* search buffers. This is a derived cache: it is not saved.
#[derive(Clone, Debug, Default)]
pub struct PathFinder {
    generation: u32,
    nodes: Vec<Node>,
    /// One list for each f value, from `f_base`. Entries sort by (h, index).
    buckets: Vec<Vec<u64>>,
    /// The bucket that is sorted now (descending, so the minimum is last).
    sorted: usize,
    /// Padded copy of the passability grid.
    pass: Vec<bool>,
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
        if s == t {
            return SearchResult {
                found: true,
                expanded: 0,
                cost: 0,
            };
        }
        let (s, t) = (s as u32, t as u32);
        let w = map.width() as i32;
        let hgt = map.height() as i32;
        // Work on a padded grid (one blocked tile around the map), so the neighbor
        // loop needs no bounds checks. Padded indices sort like map indices.
        let pw = w + 2;
        self.begin(pw as usize * (hgt + 2) as usize);
        let open_id = self.generation * 2;
        let closed_id = open_id + 1;
        self.pass.clear();
        self.pass.resize(self.nodes.len(), false);
        let src = map.passable_slice();
        for y in 0..hgt as usize {
            let d = (y + 1) * pw as usize + 1;
            let r = y * w as usize;
            self.pass[d..d + w as usize].copy_from_slice(&src[r..r + w as usize]);
        }
        let pad = |i: u32| -> u32 { (i as i32 / w * pw + i as i32 % w + pw + 1) as u32 };
        let (s, t) = (pad(s), pad(t));
        let offsets: [(i32, i32); 8] =
            DIRS.map(|(dx, dy)| (dx + dy * pw, step_cost(dx, dy) as i32));

        self.nodes[s as usize] = Node {
            tag: open_id << 3,
            g: 0,
        };
        let h0 = octile(sx, sy, gx, gy);
        let f_base = h0;
        self.push(f_base, h0, h0, sx + 1, sy + 1);
        let mut cur = 0usize;
        self.sorted = usize::MAX;

        let mut expanded = 0u32;
        loop {
            while cur < self.buckets.len() && self.buckets[cur].is_empty() {
                cur += 1;
            }
            if cur >= self.buckets.len() {
                break;
            }
            if self.sorted != cur {
                self.buckets[cur].sort_unstable_by(|a, b| b.cmp(a));
                self.sorted = cur;
            }
            let Some(key) = self.buckets[cur].pop() else {
                continue;
            };
            let x = (key & COORD_MASK) as i32;
            let y = ((key >> COORD_BITS) & COORD_MASK) as i32;
            let iu = (y * pw + x) as usize;
            let i = iu as u32;
            let tag = self.nodes[iu].tag;
            if tag >> 3 == closed_id {
                continue;
            }
            self.nodes[iu].tag = (closed_id << 3) | (tag & 7);
            expanded += 1;
            let gi = self.nodes[iu].g;
            if i == t {
                let mut cur = i;
                while cur != s {
                    let (px, py) = (cur as i32 % pw - 1, cur as i32 / pw - 1);
                    out.push((px as u16, py as u16));
                    let dir = (self.nodes[cur as usize].tag & 7) as usize;
                    cur = (cur as i32 - offsets[dir].0) as u32;
                }
                return SearchResult {
                    found: true,
                    expanded,
                    cost: gi,
                };
            }
            // Bit k is set if a step in direction k is allowed (same rule as
            // `can_step`: no corner cutting). Directions 0..4 are straight.
            let pass = &self.pass[..];
            let p = |off: i32| u32::from(pass[(iu as i32 + off) as usize]);
            let (n, e, so, we) = (
                p(offsets[0].0),
                p(offsets[1].0),
                p(offsets[2].0),
                p(offsets[3].0),
            );
            let mask = n
                | (e << 1)
                | (so << 2)
                | (we << 3)
                | ((n & e & p(offsets[4].0)) << 4)
                | ((e & so & p(offsets[5].0)) << 5)
                | ((so & we & p(offsets[6].0)) << 6)
                | ((we & n & p(offsets[7].0)) << 7);
            let mut mask = mask;
            while mask != 0 {
                let k = mask.trailing_zeros() as usize;
                mask &= mask - 1;
                let (off, cost) = offsets[k];
                let nu = (iu as i32 + off) as usize;
                let node = &mut self.nodes[nu];
                let nstamp = node.tag >> 3;
                if nstamp == closed_id {
                    continue;
                }
                let ng = gi + cost as u32;
                if nstamp != open_id || ng < node.g {
                    node.tag = (open_id << 3) | k as u32;
                    node.g = ng;
                    let (dx, dy) = DIRS[k];
                    let h = octile(x + dx, y + dy, gx + 1, gy + 1);
                    let f = ng + h;
                    self.push(f_base, f, h, x + dx, y + dy);
                    // The heuristic is consistent, so f never falls. Stay exact anyway.
                    cur = cur.min((f - f_base) as usize);
                }
            }
        }
        SearchResult {
            found: false,
            expanded,
            cost: 0,
        }
    }

    /// Add an entry to the bucket of `f`.
    fn push(&mut self, f_base: u32, f: u32, h: u32, x: i32, y: i32) {
        let b = (f.max(f_base) - f_base) as usize;
        if b >= self.buckets.len() {
            self.buckets.resize_with(b + 1, Vec::new);
        }
        let key = pack(h, x, y);
        let list = &mut self.buckets[b];
        if b == self.sorted {
            // The new key is usually the smallest, so it goes near the end.
            if list.last().is_none_or(|&k| k > key) {
                list.push(key);
            } else {
                let at = list.partition_point(|&k| k > key);
                list.insert(at, key);
            }
        } else {
            list.push(key);
        }
    }

    /// Prepare the buffers for a new search over `len` tiles.
    fn begin(&mut self, len: usize) {
        if self.nodes.len() != len {
            self.nodes = vec![Node::default(); len];
            self.generation = 0;
        }
        if self.generation >= MAX_GENERATION {
            self.nodes.fill(Node::default());
            self.generation = 0;
        }
        self.generation += 1;
        for bucket in &mut self.buckets {
            bucket.clear();
        }
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
