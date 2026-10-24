//! Drought, irrigation, and ditches (S16, R-025).
//!
//! The sim counts the days in a row without rain. At `drought_days` the drought starts. It ends
//! on the next day with rain. A crop on a tile that is not irrigated grows slower during a
//! drought, and wild plants do not grow again. A tile is irrigated when a water tile or a ditch
//! that connects to water is near. Colonists dig ditches. The fire layer is in `fire.rs`.
//! Crops and wild plants come in a later sprint: this module gives the growth chain and the
//! predicates that those systems read.

use crate::Sim;
use crate::job::{Job, JobKind, Reject};
use crate::time::{Season, TICKS_PER_DAY};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use worldgen::{Ground, RockKind};

/// The `arg` of a mine job that digs a ditch on its tile.
pub const DIG_ARG: u16 = 1;

/// A fire on a tree tile.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Fire {
    /// Fire strength, 1 to 10. It scales the chance to spread.
    pub intensity: u32,
    /// Fire steps left before the tree is gone.
    pub fuel: u32,
}

/// A thing that happened with drought or fire. The caller takes it with
/// `Sim::drain_drought_events`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DroughtEvent {
    /// The dry days reached the limit.
    DroughtBegan {
        /// Game day number.
        day: u64,
    },
    /// A day with rain ended the drought.
    DroughtEnded {
        /// Game day number.
        day: u64,
    },
    /// Lightning struck a tree (R-026).
    Lightning {
        /// Tile column.
        x: u16,
        /// Tile row.
        y: u16,
        /// True if the tree caught fire. False if it was already burning.
        started: bool,
    },
    /// A tree burned down.
    TreeBurned {
        /// Tile column.
        x: u16,
        /// Tile row.
        y: u16,
    },
    /// A burning hearth set a tile on fire (R-023). The fire alert starts here.
    HearthFire {
        /// Tile column.
        x: u16,
        /// Tile row.
        y: u16,
    },
    /// Items or buildings burned down on a tile.
    ThingsBurned {
        /// Tile column.
        x: u16,
        /// Tile row.
        y: u16,
        /// Item stacks that are gone.
        items: u32,
        /// Buildings that are gone.
        buildings: u32,
    },
    /// Colonists put out the fire on a tile.
    FireOut {
        /// Tile column.
        x: u16,
        /// Tile row.
        y: u16,
    },
}

/// What a part of the growth speed of a crop is made of.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GrowthCause {
    /// Normal growth.
    Base,
    /// A drought on a tile that is not irrigated.
    Drought,
}

/// One contribution to the growth speed, in percent (R-025).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GrowthPart {
    /// What it is.
    pub cause: GrowthCause,
    /// Percent points that it adds. Can be negative.
    pub pct: i32,
}

/// The growth speed of a crop and its parts. `total` is the sum of the parts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Growth {
    /// Sum of the part values, in percent.
    pub total: i32,
    /// The parts: the base, then the drought.
    pub parts: Vec<GrowthPart>,
}

/// Dry days, ditches, and fires. Part of the state.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct DroughtState {
    pub(crate) dry_days: u32,
    pub(crate) ditches: BTreeSet<(u16, u16)>,
    pub(crate) fires: BTreeMap<(u16, u16), Fire>,
    /// Derived cache: the ditches that connect to water.
    #[serde(skip)]
    connected: BTreeSet<(u16, u16)>,
}

impl Job {
    /// True if this is a mine job that digs a ditch.
    pub fn is_dig(&self) -> bool {
        self.kind == JobKind::Mine && self.arg == DIG_ARG
    }
}

impl Sim {
    /// Days in a row without rain.
    pub fn dry_days(&self) -> u32 {
        self.drought.dry_days
    }

    /// True while a drought lasts (R-025).
    pub fn drought_active(&self) -> bool {
        self.drought.dry_days >= self.rules.drought.drought_days
    }

    /// True if wild plants grow again. They do not during a drought (R-025).
    pub fn wild_plants_regrow(&self) -> bool {
        !self.drought_active()
    }

    /// Take the drought and fire events that the last ticks made.
    pub fn drain_drought_events(&mut self) -> Vec<DroughtEvent> {
        std::mem::take(&mut self.drought_events)
    }

    /// True if a ditch is on the tile.
    pub fn has_ditch(&self, x: u16, y: u16) -> bool {
        self.drought.ditches.contains(&(x, y))
    }

    /// All ditch tiles, in `(x, y)` order.
    pub fn ditches(&self) -> impl Iterator<Item = (u16, u16)> + '_ {
        self.drought.ditches.iter().copied()
    }

    /// True if a ditch is on the tile and it connects to water through ditch tiles.
    pub fn ditch_connected(&self, x: u16, y: u16) -> bool {
        self.drought.connected.contains(&(x, y))
    }

    /// True if the tile is irrigated: a water tile or a connected ditch tile is within
    /// `irrigation_radius` tiles (the larger of column and row distance).
    pub fn irrigated(&self, x: u16, y: u16) -> bool {
        let r = i32::from(u16::try_from(self.rules.drought.irrigation_radius).unwrap_or(32));
        let (cx, cy) = (i32::from(x), i32::from(y));
        for ty in cy - r..=cy + r {
            for tx in cx - r..=cx + r {
                if !self.map.in_bounds(tx, ty) {
                    continue;
                }
                let water = self.map.ground(tx, ty) == Some(Ground::Water);
                let ditch = self.drought.connected.contains(&(tx as u16, ty as u16));
                if water || ditch {
                    return true;
                }
            }
        }
        false
    }

    /// The growth speed of a crop on a tile, with its parts (R-025). `dry_soil` is true for a
    /// crop that does not suffer from a drought. A drought halves the growth of the others
    /// unless the tile is irrigated.
    pub fn growth_pct(&self, x: u16, y: u16, dry_soil: bool) -> Growth {
        let mut parts = vec![GrowthPart {
            cause: GrowthCause::Base,
            pct: 100,
        }];
        if self.drought_active() && !dry_soil && !self.irrigated(x, y) {
            let dry = i32::try_from(self.rules.drought.dry_growth_pct).unwrap_or(100);
            parts.push(GrowthPart {
                cause: GrowthCause::Drought,
                pct: dry - 100,
            });
        }
        let total = parts.iter().map(|p| p.pct).sum();
        Growth { total, parts }
    }

    /// Why a ditch cannot be dug on a tile, or `Ok` if it can.
    pub(crate) fn ditch_allowed(&self, x: u16, y: u16) -> Result<(), Reject> {
        let (ix, iy) = (i32::from(x), i32::from(y));
        if !self.map.in_bounds(ix, iy) {
            return Err(Reject::OutOfMap);
        }
        let soft = matches!(
            self.map.ground(ix, iy),
            Some(Ground::Soil | Ground::Sand | Ground::Clay)
        );
        let rock = self.map.rock(ix, iy).is_none_or(|r| r != RockKind::None);
        if !soft || rock || self.map.tree(ix, iy) || self.map.wall(ix, iy) {
            return Err(Reject::TileBlocked);
        }
        let built = [
            crate::BuildLayer::Floor,
            crate::BuildLayer::Object,
            crate::BuildLayer::Roof,
        ]
        .into_iter()
        .any(|l| {
            self.buildings.get(x, y, l).is_some() || self.buildings.blueprint(x, y, l).is_some()
        });
        if self.has_ditch(x, y) || built {
            return Err(Reject::TileOccupied);
        }
        if !self.items.at_tile(x, y).is_empty() {
            return Err(Reject::TileBlocked);
        }
        Ok(())
    }

    /// The player orders a ditch on a tile. A colonist digs it as a mine job.
    pub(crate) fn dig_ditch(&mut self, x: u16, y: u16) {
        if let Err(reason) = self.ditch_allowed(x, y) {
            self.reject(reason);
            return;
        }
        if self.designations.insert((x, y), JobKind::Mine).is_some() {
            self.reject(Reject::AlreadyOrdered);
            return;
        }
        self.add_job_arg(JobKind::Mine, x, y, DIG_ARG);
    }

    /// The ditch is dug: nobody walks on the tile any more.
    pub(crate) fn finish_ditch(&mut self, x: u16, y: u16) {
        self.drought.ditches.insert((x, y));
        self.map.set_solid(i32::from(x), i32::from(y), true);
        self.rebuild_ditches();
        self.passability_changed(x, y);
        self.rooms_dirty = true;
    }

    /// Find the ditch tiles that connect to water: a ditch tile that touches a water tile, or
    /// a connected ditch tile, on one of its four sides.
    pub(crate) fn rebuild_ditches(&mut self) {
        let water = |s: &Self, x: i32, y: i32| s.map.ground(x, y) == Some(Ground::Water);
        let mut connected = BTreeSet::new();
        let mut queue = VecDeque::new();
        for &(x, y) in &self.drought.ditches {
            let touches = crate::job::NEIGHBORS
                .iter()
                .any(|&(dx, dy)| water(self, i32::from(x) + dx, i32::from(y) + dy));
            if touches && connected.insert((x, y)) {
                queue.push_back((x, y));
            }
        }
        while let Some((x, y)) = queue.pop_front() {
            for (dx, dy) in crate::job::NEIGHBORS {
                let (nx, ny) = (i32::from(x) + dx, i32::from(y) + dy);
                let (Ok(nx), Ok(ny)) = (u16::try_from(nx), u16::try_from(ny)) else {
                    continue;
                };
                if self.drought.ditches.contains(&(nx, ny)) && connected.insert((nx, ny)) {
                    queue.push_back((nx, ny));
                }
            }
        }
        self.drought.connected = connected;
    }

    /// Check a loaded state: all ditches and fires are on the map.
    pub(crate) fn drought_state_ok(&self) -> bool {
        let inside = |&(x, y): &(u16, u16)| self.map.in_bounds(i32::from(x), i32::from(y));
        self.drought.ditches.iter().all(inside) && self.drought.fires.keys().all(inside)
    }

    /// Called after the tick number increased. Each game day: count dry days, roll lightning.
    /// Each fire step: spread and burn (fire.rs).
    pub(crate) fn drought_tick(&mut self) {
        if self.tick > 0 && self.tick.is_multiple_of(TICKS_PER_DAY) {
            self.drought_day();
        }
        let step = u64::from(self.rules.drought.fire_step_ticks.max(1));
        if self.tick.is_multiple_of(step) && !self.drought.fires.is_empty() {
            self.fire_step();
        }
    }

    fn drought_day(&mut self) {
        let day = self.tick / TICKS_PER_DAY;
        let was = self.drought_active();
        if self.weather_today().rain {
            self.drought.dry_days = 0;
        } else {
            self.drought.dry_days = self.drought.dry_days.saturating_add(1);
        }
        let now = self.drought_active();
        if now && !was {
            self.drought_events.push(DroughtEvent::DroughtBegan { day });
        } else if was && !now {
            self.crisis_survived();
            self.drought_events.push(DroughtEvent::DroughtEnded { day });
        }
        self.lightning();
    }

    /// A summer day with a drought: lightning strikes a random tree with the chance of
    /// `lightning_pct` (R-026).
    fn lightning(&mut self) {
        let rules = self.rules.drought;
        if self.time().season != Season::Summer || !self.drought_active() {
            return;
        }
        if !self.rng.chance(rules.lightning_pct, 100) {
            return;
        }
        let trees = self.tree_tiles();
        let Ok(count) = u32::try_from(trees.len()) else {
            return;
        };
        if count == 0 {
            return;
        }
        let (x, y) = trees[self.rng.below(count) as usize];
        let started = self.ignite(x, y, rules.lightning_intensity);
        self.drought_events
            .push(DroughtEvent::Lightning { x, y, started });
    }

    /// All tree tiles in row-major order.
    fn tree_tiles(&self) -> Vec<(u16, u16)> {
        let mut out = Vec::new();
        for y in 0..self.map.height() as i32 {
            for x in 0..self.map.width() as i32 {
                if self.map.tree(x, y) {
                    out.push((x as u16, y as u16));
                }
            }
        }
        out
    }

    // ---- debug hooks (scenario tests) ----

    /// Set the dry days. For tests.
    pub fn debug_set_dry_days(&mut self, days: u32) {
        self.drought.dry_days = days;
    }

    /// Put a finished ditch on a tile if a ditch can be dug there. Return true if it did. For
    /// tests.
    pub fn debug_add_ditch(&mut self, x: u16, y: u16) -> bool {
        if self.ditch_allowed(x, y).is_err() {
            return false;
        }
        self.finish_ditch(x, y);
        true
    }

    /// Set a tree on fire. For tests.
    pub fn debug_ignite(&mut self, x: u16, y: u16, intensity: u32) -> bool {
        self.ignite(x, y, intensity)
    }
}
