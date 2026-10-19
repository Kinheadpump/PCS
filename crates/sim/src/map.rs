//! The local map of the `Sim`: terrain layers and passability (TECHNICAL_DESIGN §5).
//!
//! Layers are plain `Vec`s in row-major order. The terrain enums of `worldgen` have no
//! `serde` support, so the map stores their `u8` codes.

use serde::{Deserialize, Serialize};
use worldgen::{Ground, LocalMapData, Ore, RockKind};

/// Largest map side that the save loader accepts.
pub const MAX_MAP_SIDE: u32 = 4096;

/// Terrain layers of the colony map.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalMap {
    width: u32,
    height: u32,
    /// `Ground` codes.
    ground: Vec<u8>,
    /// `RockKind` codes.
    rock: Vec<u8>,
    /// `Ore` codes.
    ore: Vec<u8>,
    /// True where a tree stands. A tree does not block walking (Sprint 2 decision).
    tree: Vec<bool>,
    /// True where a built or scenario wall stands.
    wall: Vec<bool>,
    /// Derived cache: true where a building that blocks walking stands. `Sim` fills it
    /// from the buildings. Rebuilt on load.
    #[serde(skip)]
    solid: Vec<bool>,
    /// Derived cache: true where a colonist can stand (not water, rock, or wall). Rebuilt on load.
    #[serde(skip)]
    passable: Vec<bool>,
}

impl LocalMap {
    /// Copy the layers of generated map data. The wall layer starts empty.
    pub fn from_data(data: &LocalMapData) -> Self {
        let len = data.ground.as_slice().len();
        let mut map = Self {
            width: data.ground.width(),
            height: data.ground.height(),
            ground: data.ground.as_slice().iter().map(|&g| g as u8).collect(),
            rock: data.rock.as_slice().iter().map(|&r| r as u8).collect(),
            ore: data.ore.as_slice().iter().map(|&o| o as u8).collect(),
            tree: data.tree.as_slice().to_vec(),
            wall: vec![false; len],
            solid: Vec::new(),
            passable: Vec::new(),
        };
        map.rebuild_passable();
        map
    }

    /// Build a map from flat cell layers (row-major), for scenario tests. The layers
    /// must have `width * height` cells.
    pub(crate) fn from_cells(
        width: u32,
        height: u32,
        water: &[bool],
        tree: &[bool],
        wall: &[bool],
        rock: &[bool],
    ) -> Self {
        let len = width as usize * height as usize;
        let mut map = Self {
            width,
            height,
            ground: water
                .iter()
                .map(|&w| {
                    if w {
                        Ground::Water as u8
                    } else {
                        Ground::Soil as u8
                    }
                })
                .collect(),
            rock: rock
                .iter()
                .map(|&r| {
                    if r {
                        RockKind::Granite as u8
                    } else {
                        RockKind::None as u8
                    }
                })
                .collect(),
            ore: vec![Ore::None as u8; len],
            tree: tree.to_vec(),
            wall: wall.to_vec(),
            solid: Vec::new(),
            passable: Vec::new(),
        };
        map.rebuild_passable();
        map
    }

    /// True if the size is in range and all layers match the size.
    pub fn is_valid(&self) -> bool {
        if self.width == 0
            || self.height == 0
            || self.width > MAX_MAP_SIDE
            || self.height > MAX_MAP_SIDE
        {
            return false;
        }
        let len = self.width as usize * self.height as usize;
        self.ground.len() == len
            && self.rock.len() == len
            && self.ore.len() == len
            && self.tree.len() == len
            && self.wall.len() == len
    }

    /// Rebuild the passability cache from the layers. Call only on a valid map.
    pub fn rebuild_passable(&mut self) {
        self.passable = (0..self.ground.len())
            .map(|i| self.compute_passable(i))
            .collect();
    }

    fn compute_passable(&self, i: usize) -> bool {
        self.ground[i] != Ground::Water as u8
            && self.rock[i] == RockKind::None as u8
            && !self.wall[i]
            && !self.solid.get(i).copied().unwrap_or(false)
    }

    /// True if a building that blocks walking stands on the tile.
    pub fn solid(&self, x: i32, y: i32) -> bool {
        self.index(x, y)
            .and_then(|i| self.solid.get(i))
            .copied()
            .unwrap_or(false)
    }

    /// Set the tiles that a building blocks, and rebuild the passability. Tiles outside
    /// the map are ignored.
    pub(crate) fn reset_solid(&mut self, tiles: impl Iterator<Item = (u16, u16)>) {
        self.solid = vec![false; self.ground.len()];
        for (x, y) in tiles {
            if let Some(i) = self.index(i32::from(x), i32::from(y)) {
                self.solid[i] = true;
            }
        }
        self.rebuild_passable();
    }

    /// Mark or unmark a tile as blocked by a building. Return true if passability
    /// changed. `Sim` calls this and then updates the regions and the paths.
    pub(crate) fn set_solid(&mut self, x: i32, y: i32, solid: bool) -> bool {
        let Some(i) = self.index(x, y) else {
            return false;
        };
        if self.solid.len() != self.ground.len() {
            self.solid = vec![false; self.ground.len()];
        }
        self.solid[i] = solid;
        let now = self.compute_passable(i);
        let changed = self.passable[i] != now;
        self.passable[i] = now;
        changed
    }

    /// Map width in tiles.
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Map height in tiles.
    pub fn height(&self) -> u32 {
        self.height
    }

    /// True if the tile is inside the map.
    pub fn in_bounds(&self, x: i32, y: i32) -> bool {
        x >= 0 && y >= 0 && (x as u32) < self.width && (y as u32) < self.height
    }

    /// Row-major index of a tile, or `None` outside the map.
    pub fn index(&self, x: i32, y: i32) -> Option<usize> {
        self.in_bounds(x, y)
            .then(|| y as usize * self.width as usize + x as usize)
    }

    /// Passability of all tiles in row-major order (for the path search).
    pub(crate) fn passable_slice(&self) -> &[bool] {
        &self.passable
    }

    /// True if a colonist can stand on the tile. False outside the map.
    pub fn passable(&self, x: i32, y: i32) -> bool {
        self.index(x, y)
            .and_then(|i| self.passable.get(i))
            .copied()
            .unwrap_or(false)
    }

    /// Ground of a tile.
    pub fn ground(&self, x: i32, y: i32) -> Option<Ground> {
        let code = *self.ground.get(self.index(x, y)?)?;
        Some(match code {
            0 => Ground::Soil,
            1 => Ground::Sand,
            2 => Ground::Clay,
            3 => Ground::Water,
            _ => Ground::Stone,
        })
    }

    /// Rock wall of a tile.
    pub fn rock(&self, x: i32, y: i32) -> Option<RockKind> {
        let code = *self.rock.get(self.index(x, y)?)?;
        Some(match code {
            0 => RockKind::None,
            1 => RockKind::Sandstone,
            2 => RockKind::Limestone,
            _ => RockKind::Granite,
        })
    }

    /// Ore of a tile.
    pub fn ore(&self, x: i32, y: i32) -> Option<Ore> {
        let code = *self.ore.get(self.index(x, y)?)?;
        Some(match code {
            0 => Ore::None,
            1 => Ore::Coal,
            2 => Ore::Copper,
            _ => Ore::Iron,
        })
    }

    /// True if a tree stands on the tile.
    pub fn tree(&self, x: i32, y: i32) -> bool {
        self.index(x, y)
            .and_then(|i| self.tree.get(i))
            .copied()
            .unwrap_or(false)
    }

    /// True if a wall stands on the tile.
    pub fn wall(&self, x: i32, y: i32) -> bool {
        self.index(x, y)
            .and_then(|i| self.wall.get(i))
            .copied()
            .unwrap_or(false)
    }

    /// Remove the tree on a tile. Return true if a tree stood there.
    pub(crate) fn clear_tree(&mut self, x: i32, y: i32) -> bool {
        match self.index(x, y) {
            Some(i) if self.tree[i] => {
                self.tree[i] = false;
                true
            }
            _ => false,
        }
    }

    /// Mine the rock wall on a tile: the rock and the ore are gone and the ground is bare
    /// stone. Return the rock and the ore that were there, or `None` if there was no rock.
    pub(crate) fn mine_rock(&mut self, x: i32, y: i32) -> Option<(RockKind, Ore)> {
        let i = self.index(x, y)?;
        let rock = self.rock(x, y)?;
        if rock == RockKind::None {
            return None;
        }
        let ore = self.ore(x, y).unwrap_or(Ore::None);
        self.rock[i] = RockKind::None as u8;
        self.ore[i] = Ore::None as u8;
        self.ground[i] = Ground::Stone as u8;
        self.passable[i] = self.compute_passable(i);
        Some((rock, ore))
    }

    /// Put a wall on a tile or remove it. Return true if passability changed.
    /// `Sim::set_wall` calls this and then updates the regions and the paths.
    pub(crate) fn set_wall(&mut self, x: i32, y: i32, wall: bool) -> bool {
        let Some(i) = self.index(x, y) else {
            return false;
        };
        if self.wall[i] == wall {
            return false;
        }
        self.wall[i] = wall;
        let now = self.compute_passable(i);
        let changed = self.passable[i] != now;
        self.passable[i] = now;
        changed
    }
}
