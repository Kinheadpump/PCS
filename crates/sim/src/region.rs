//! Connectivity regions (TECHNICAL_DESIGN §6.3): a flood-fill region ID for each tile.
//!
//! Two tiles share a region if an A* path exists between them. Region 0 means "not
//! passable". The regions are a derived cache: `Sim` rebuilds them when passability
//! changes and after a load.

use crate::map::LocalMap;
use crate::path::{DIRS, can_step};

/// Region IDs for every tile of the map.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Regions {
    width: u32,
    ids: Vec<u32>,
    count: u32,
}

impl Regions {
    /// Build the regions of a map.
    pub fn build(map: &LocalMap) -> Self {
        let mut regions = Self::default();
        regions.rebuild(map);
        regions
    }

    /// Flood fill the whole map again. Regions are numbered from 1 in row-major order of
    /// their first tile.
    pub fn rebuild(&mut self, map: &LocalMap) {
        let (w, h) = (map.width() as i32, map.height() as i32);
        self.width = map.width();
        self.ids.clear();
        self.ids.resize(w as usize * h as usize, 0);
        self.count = 0;
        let mut stack: Vec<(i32, i32)> = Vec::new();
        for sy in 0..h {
            for sx in 0..w {
                let si = (sy * w + sx) as usize;
                if self.ids[si] != 0 || !map.passable(sx, sy) {
                    continue;
                }
                self.count += 1;
                let id = self.count;
                self.ids[si] = id;
                stack.push((sx, sy));
                while let Some((x, y)) = stack.pop() {
                    for (dx, dy) in DIRS {
                        if !can_step(map, x, y, dx, dy) {
                            continue;
                        }
                        let ni = ((y + dy) * w + x + dx) as usize;
                        if self.ids[ni] == 0 {
                            self.ids[ni] = id;
                            stack.push((x + dx, y + dy));
                        }
                    }
                }
            }
        }
    }

    /// Region ID of a tile. 0 outside the map and on tiles that are not passable.
    pub fn id_at(&self, x: i32, y: i32) -> u32 {
        if x < 0 || y < 0 || x as u32 >= self.width {
            return 0;
        }
        self.ids
            .get(y as usize * self.width as usize + x as usize)
            .copied()
            .unwrap_or(0)
    }

    /// Number of regions.
    pub fn count(&self) -> u32 {
        self.count
    }

    /// True if both tiles are passable and in the same region.
    pub fn connected(&self, a: (i32, i32), b: (i32, i32)) -> bool {
        let id = self.id_at(a.0, a.1);
        id != 0 && id == self.id_at(b.0, b.1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scenario::map_from_ascii;

    fn regions(text: &str) -> Regions {
        Regions::build(&map_from_ascii(text).unwrap().0)
    }

    #[test]
    fn open_map_is_one_region() {
        let r = regions("....\n....");
        assert_eq!(r.count(), 1);
        assert!(r.connected((0, 0), (3, 1)));
    }

    #[test]
    fn wall_line_splits_the_map() {
        let r = regions("..#..\n..#..");
        assert_eq!(r.count(), 2);
        assert!(!r.connected((0, 0), (4, 0)));
        assert!(r.connected((0, 0), (1, 1)));
        assert_eq!(r.id_at(2, 0), 0);
    }

    #[test]
    fn gap_in_the_wall_joins_the_regions() {
        let r = regions("..#..\n.....");
        assert_eq!(r.count(), 1);
    }

    #[test]
    fn diagonal_wall_pair_blocks_the_corner() {
        let r = regions(".#\n#.");
        assert_eq!(r.count(), 2);
    }

    #[test]
    fn water_is_not_in_a_region_but_a_tree_is() {
        let r = regions(".~T.");
        assert_eq!(r.count(), 2, "a tree does not block walking");
        assert_eq!(r.id_at(1, 0), 0);
        assert_ne!(r.id_at(2, 0), 0);
        assert_eq!(r.id_at(-1, 0), 0);
        assert_eq!(r.id_at(9, 0), 0);
    }

    #[test]
    fn regions_agree_with_a_star() {
        let (map, _) =
            map_from_ascii("..#....\n..#.##.\n###.#..\n...##.#\n.#.#..#\n.#...#.").unwrap();
        let regions = Regions::build(&map);
        let mut finder = crate::path::PathFinder::default();
        let mut out = Vec::new();
        let tiles: Vec<(u16, u16)> = (0..6)
            .flat_map(|y| (0..7).map(move |x| (x, y)))
            .filter(|&(x, y)| map.passable(i32::from(x), i32::from(y)))
            .collect();
        for &a in &tiles {
            for &b in &tiles {
                let found = finder.find(&map, a, b, &mut out).found;
                let same = regions.connected(
                    (i32::from(a.0), i32::from(a.1)),
                    (i32::from(b.0), i32::from(b.1)),
                );
                assert_eq!(found, same, "{a:?} -> {b:?}");
            }
        }
    }
}
