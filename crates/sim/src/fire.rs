//! Fire on trees (S16, R-024, R-026).
//!
//! Only a tree tile burns. Each fire step a fire can spread to each of its four neighbors that
//! has a tree. The chance in percent is `tree_flammability` times the intensity divided by 10.
//! A drought raises it by `fire_drought_pct`. Each step a burning tree loses one unit of fuel.
//! At 0 the tree is gone. Rain puts out all fires. A firebreak of 2 tiles with no tree stops
//! the spread. Fires from hearths and the extinguish job come in a later sprint.

use crate::Sim;
use crate::drought::{DroughtEvent, Fire};
use crate::job::NEIGHBORS;

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
        let rules = &self.rules.drought;
        let base = rules.tree_flammability.saturating_mul(intensity) / 10;
        let pct = if self.drought_active() {
            base.saturating_mul(rules.fire_drought_pct) / 100
        } else {
            base
        };
        pct.min(100)
    }

    /// Set the tree on a tile on fire. Return false if there is no tree or it burns already.
    pub(crate) fn ignite(&mut self, x: u16, y: u16, intensity: u32) -> bool {
        if !self.map.tree(i32::from(x), i32::from(y)) || self.drought.fires.contains_key(&(x, y)) {
            return false;
        }
        let fire = Fire {
            intensity: intensity.clamp(1, 10),
            fuel: self.rules.drought.tree_fuel,
        };
        self.drought.fires.insert((x, y), fire);
        true
    }

    /// One fire step: rain puts the fires out, else the fires spread and then burn down.
    pub(crate) fn fire_step(&mut self) {
        if self.weather_today().rain {
            self.drought.fires.clear();
            return;
        }
        let burning: Vec<((u16, u16), Fire)> = self.fires().collect();
        let mut caught: Vec<((u16, u16), u32)> = Vec::new();
        for &((x, y), fire) in &burning {
            let pct = self.fire_spread_pct(fire.intensity);
            for (dx, dy) in NEIGHBORS {
                let (nx, ny) = (i32::from(x) + dx, i32::from(y) + dy);
                let (Ok(ux), Ok(uy)) = (u16::try_from(nx), u16::try_from(ny)) else {
                    continue;
                };
                let free = self.map.tree(nx, ny)
                    && !self.drought.fires.contains_key(&(ux, uy))
                    && !caught.iter().any(|&(t, _)| t == (ux, uy));
                if free && self.rng.chance(pct, 100) {
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
                self.map.clear_tree(i32::from(x), i32::from(y));
                self.drought_events.push(DroughtEvent::TreeBurned { x, y });
            }
        }
        for ((x, y), intensity) in caught {
            self.ignite(x, y, intensity);
        }
    }
}
