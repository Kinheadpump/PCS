//! Fire (S16 trees, S15 hearths, items, and buildings; R-023, R-024, R-026).
//!
//! A tile burns if it holds something flammable: a tree, an item stack, or a building. The base
//! spread chance of a tile is its flammability in percent: `tree_flammability` for a tree, the
//! material flammability (per-mille) divided by 10 for an item or a building (the highest one
//! counts). Each fire step a fire spreads to each of its four neighbors that is flammable and
//! does not burn, with the chance `base × intensity / 10` percent. A drought raises it by
//! `fire_drought_pct`. Each step a burning tile loses one unit of fuel. At 0 the fire is out
//! and what was flammable on the tile is gone: the tree, the items with flammable material, and
//! the buildings with flammable material. Rain puts out all fires on tiles with no roof. A
//! firebreak of 2 tiles with nothing flammable stops the spread.
//!
//! Fire starts from lightning on a tree (R-026) and from a burning hearth (R-023): each game hour
//! each flammable item and object (bed, door, workshop) within 1 tile of a lit hearth has the
//! chance `hearth_fire_pct` to start burning. The first `hearth_fire_grace_days` days of a run
//! have no hearth fire. A colonist puts out a fire with an extinguish job: a `Mine` job with
//! [`EXTINGUISH_ARG`]. Each job removes `extinguish_drop` intensity in `extinguish_ticks`. A
//! colonist on a burning tile loses health each game hour.

use crate::Sim;
use crate::build::BuildLayer;
use crate::drought::{DroughtEvent, Fire};
use crate::item::ItemLoc;
use crate::job::{Job, JobKind, NEIGHBORS};
use crate::time::TICKS_PER_DAY;

/// The `arg` of a mine job that puts out the fire on its tile.
pub const EXTINGUISH_ARG: u16 = 2;

/// The eight tiles around a tile.
const AROUND: [(i32, i32); 8] = [
    (-1, -1),
    (0, -1),
    (1, -1),
    (-1, 0),
    (1, 0),
    (-1, 1),
    (0, 1),
    (1, 1),
];

impl Job {
    /// True if this is a mine job that puts out a fire.
    pub fn is_extinguish(&self) -> bool {
        self.kind == JobKind::Mine && self.arg == EXTINGUISH_ARG
    }
}

impl Sim {
    /// The fire on a tile, if any.
    pub fn fire_at(&self, x: u16, y: u16) -> Option<Fire> {
        self.drought.fires.get(&(x, y)).copied()
    }

    /// Number of burning tiles.
    pub fn fire_count(&self) -> usize {
        self.drought.fires.len()
    }

    /// All fires, in `(x, y)` order.
    pub fn fires(&self) -> impl Iterator<Item = ((u16, u16), Fire)> + '_ {
        self.drought.fires.iter().map(|(&t, &f)| (t, f))
    }

    /// The chance in percent that a fire of this intensity spreads to one neighbor tree in one
    /// fire step (R-024). At most 100.
    pub fn fire_spread_pct(&self, intensity: u32) -> u32 {
        self.spread_pct(self.rules.drought.tree_flammability, intensity)
    }

    fn spread_pct(&self, base: u32, intensity: u32) -> u32 {
        let base = base.saturating_mul(intensity) / 10;
        let pct = if self.drought_active() {
            base.saturating_mul(self.rules.drought.fire_drought_pct) / 100
        } else {
            base
        };
        pct.min(100)
    }

    /// The base spread chance of a tile in percent: how well it burns (R-024). 0 for a tile with
    /// nothing flammable. The best of the tree, the item stacks, and the buildings counts.
    pub fn flammability_pct(&self, x: u16, y: u16) -> u32 {
        let mut best = if self.map.tree(i32::from(x), i32::from(y)) {
            self.rules.drought.tree_flammability
        } else {
            0
        };
        for &id in self.items.at_tile(x, y) {
            if let Some(item) = self.items.get(id) {
                best = best.max(self.material_flammability(item.material));
            }
        }
        for layer in [BuildLayer::Floor, BuildLayer::Object, BuildLayer::Roof] {
            if let Some(b) = self.buildings.get(x, y, layer) {
                best = best.max(self.material_flammability(b.material));
            }
        }
        best
    }

    fn material_flammability(&self, material: u16) -> u32 {
        self.rules
            .materials
            .get(usize::from(material))
            .map_or(0, |m| u32::from(m.flammability) / 10)
    }

    /// Set a flammable tile on fire. Return false if nothing on it burns or it burns already.
    pub(crate) fn ignite(&mut self, x: u16, y: u16, intensity: u32) -> bool {
        if self.flammability_pct(x, y) == 0 || self.drought.fires.contains_key(&(x, y)) {
            return false;
        }
        let rules = &self.rules.drought;
        let fuel = if self.map.tree(i32::from(x), i32::from(y)) {
            rules.tree_fuel
        } else {
            rules.thing_fuel
        };
        self.drought.fires.insert(
            (x, y),
            Fire {
                intensity: intensity.clamp(1, 10),
                fuel,
            },
        );
        true
    }

    /// One fire step: rain puts out the fires on tiles with no roof, else the fires spread and
    /// then burn down.
    pub(crate) fn fire_step(&mut self) {
        if self.weather_today().rain {
            let covered: Vec<(u16, u16)> = self
                .drought
                .fires
                .keys()
                .copied()
                .filter(|&(x, y)| self.buildings.roof_at(x, y).is_some())
                .collect();
            self.drought.fires.retain(|t, _| covered.contains(t));
            if self.drought.fires.is_empty() {
                return;
            }
        }
        let burning: Vec<((u16, u16), Fire)> = self.fires().collect();
        let mut caught: Vec<((u16, u16), u32)> = Vec::new();
        for &((x, y), fire) in &burning {
            for (dx, dy) in NEIGHBORS {
                let (nx, ny) = (i32::from(x) + dx, i32::from(y) + dy);
                let (Ok(ux), Ok(uy)) = (u16::try_from(nx), u16::try_from(ny)) else {
                    continue;
                };
                if !self.map.in_bounds(nx, ny)
                    || self.drought.fires.contains_key(&(ux, uy))
                    || caught.iter().any(|&(t, _)| t == (ux, uy))
                {
                    continue;
                }
                let base = self.flammability_pct(ux, uy);
                if base > 0 && self.rng.chance(self.spread_pct(base, fire.intensity), 100) {
                    caught.push(((ux, uy), fire.intensity));
                }
            }
        }
        for ((x, y), fire) in burning {
            if fire.fuel > 1 {
                self.drought.fires.insert(
                    (x, y),
                    Fire {
                        fuel: fire.fuel - 1,
                        ..fire
                    },
                );
            } else {
                self.drought.fires.remove(&(x, y));
                self.burn_down(x, y);
            }
        }
        for ((x, y), intensity) in caught {
            self.ignite(x, y, intensity);
        }
        self.sync_fire_jobs();
    }

    /// The fire on a tile is out of fuel: the tree, the flammable items, and the flammable
    /// buildings on it are gone.
    fn burn_down(&mut self, x: u16, y: u16) {
        if self.map.tree(i32::from(x), i32::from(y)) {
            self.map.clear_tree(i32::from(x), i32::from(y));
            self.drought_events.push(DroughtEvent::TreeBurned { x, y });
        }
        let stacks: Vec<_> = self
            .items
            .at_tile(x, y)
            .iter()
            .copied()
            .filter(|&id| {
                self.items.get(id).is_some_and(|i| {
                    matches!(i.loc, ItemLoc::Tile(..)) && self.material_flammability(i.material) > 0
                })
            })
            .collect();
        let mut items = 0u32;
        for id in stacks {
            if self.items.remove(id).is_some() {
                items += 1;
            }
        }
        let mut buildings = 0u32;
        for layer in [BuildLayer::Floor, BuildLayer::Object, BuildLayer::Roof] {
            let burns = self
                .buildings
                .get(x, y, layer)
                .is_some_and(|b| self.material_flammability(b.material) > 0);
            if burns && self.remove_building(x, y, layer).is_some() {
                buildings += 1;
            }
        }
        if items + buildings > 0 {
            self.drought_events.push(DroughtEvent::ThingsBurned {
                x,
                y,
                items,
                buildings,
            });
        }
    }

    // ---- hearth fires (R-023) ----

    /// Each game hour: every lit hearth may set the flammable items and objects within 1 tile on
    /// fire. Colonists on burning tiles lose health.
    pub(crate) fn fire_hour(&mut self) {
        self.hearth_fire_roll();
        let loss = i32::try_from(self.rules.drought.fire_health_loss).unwrap_or(i32::MAX);
        if loss == 0 || self.drought.fires.is_empty() {
            return;
        }
        let hurt: Vec<_> = self
            .colonists
            .iter()
            .filter(|(_, c)| self.drought.fires.contains_key(&c.tile()))
            .map(|(id, _)| id)
            .collect();
        for id in hurt {
            if let Some(c) = self.colonists.get_mut(id) {
                c.health = (c.health - loss).max(0);
                c.wounded = true;
            }
        }
    }

    fn hearth_fire_roll(&mut self) {
        let rules = self.rules.drought;
        let day = self.tick / TICKS_PER_DAY;
        if day < u64::from(rules.hearth_fire_grace_days) || rules.hearth_fire_pct == 0 {
            return;
        }
        for (hx, hy) in self.hearth_tiles() {
            if !self.hearth_lit(hx, hy) {
                continue;
            }
            let mut lit: Vec<(u16, u16)> = Vec::new();
            for (dx, dy) in AROUND {
                let (nx, ny) = (i32::from(hx) + dx, i32::from(hy) + dy);
                let (Ok(x), Ok(y)) = (u16::try_from(nx), u16::try_from(ny)) else {
                    continue;
                };
                if !self.map.in_bounds(nx, ny) || self.drought.fires.contains_key(&(x, y)) {
                    continue;
                }
                let mut things = 0u32;
                for &id in self.items.at_tile(x, y) {
                    let burns = self.items.get(id).is_some_and(|i| {
                        matches!(i.loc, ItemLoc::Tile(..))
                            && self.material_flammability(i.material) > 0
                    });
                    things += u32::from(burns);
                }
                if self
                    .buildings
                    .get(x, y, BuildLayer::Object)
                    .is_some_and(|b| self.material_flammability(b.material) > 0)
                {
                    things += 1;
                }
                if (0..things).any(|_| self.rng.chance(rules.hearth_fire_pct, 100)) {
                    lit.push((x, y));
                }
            }
            for (x, y) in lit {
                if self.ignite(x, y, rules.hearth_fire_intensity) {
                    self.drought_events.push(DroughtEvent::HearthFire { x, y });
                }
            }
        }
        self.sync_fire_jobs();
    }

    // ---- extinguish jobs ----

    /// True if something of the colony is within `extinguish_radius` tiles of the tile: a
    /// building or an item.
    fn fire_threatens(&self, x: u16, y: u16) -> bool {
        let r = i32::try_from(self.rules.drought.extinguish_radius).unwrap_or(0);
        for dy in -r..=r {
            for dx in -r..=r {
                let (Ok(tx), Ok(ty)) = (
                    u16::try_from(i32::from(x) + dx),
                    u16::try_from(i32::from(y) + dy),
                ) else {
                    continue;
                };
                let built = [BuildLayer::Floor, BuildLayer::Object, BuildLayer::Roof]
                    .into_iter()
                    .any(|l| self.buildings.get(tx, ty, l).is_some());
                if built || !self.items.at_tile(tx, ty).is_empty() {
                    return true;
                }
            }
        }
        false
    }

    /// Every fire near the colony has an extinguish job on the board.
    pub(crate) fn sync_fire_jobs(&mut self) {
        let tiles: Vec<(u16, u16)> = self.drought.fires.keys().copied().collect();
        for (x, y) in tiles {
            if self
                .jobs
                .find_arg(JobKind::Mine, x, y, EXTINGUISH_ARG)
                .is_none()
                && self.fire_threatens(x, y)
            {
                self.add_job_arg(JobKind::Mine, x, y, EXTINGUISH_ARG);
            }
        }
    }

    /// The extinguish job is done: the fire loses intensity, or it is out.
    pub(crate) fn extinguish_done(&mut self, x: u16, y: u16) {
        let drop = self.rules.drought.extinguish_drop;
        let Some(fire) = self.drought.fires.get_mut(&(x, y)) else {
            return;
        };
        if fire.intensity > drop {
            fire.intensity -= drop;
        } else {
            self.drought.fires.remove(&(x, y));
            self.drought_events.push(DroughtEvent::FireOut { x, y });
        }
    }
}
