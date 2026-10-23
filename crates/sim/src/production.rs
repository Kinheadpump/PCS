//! Production orders "keep N" (GAME_DESIGN §5).
//!
//! A keep order says: this workshop keeps N units of the first output of a recipe in stock.
//! Each game hour the sim counts the stock (on the ground and carried, not rotten) and the
//! units that open craft orders will make. If the count is below N, it orders more runs. It
//! orders no more when the count reaches N, so the stock stops at N and starts again below N.

use crate::Command;
use crate::Sim;
use crate::item::ItemLoc;
use crate::job::Reject;
use crate::time::TICKS_PER_HOUR;

/// The highest keep target.
pub const MAX_KEEP: u32 = 10_000;

impl Sim {
    /// The keep target of a recipe at a workshop. 0: none.
    pub fn keep_target(&self, x: u16, y: u16, recipe: u16) -> u32 {
        self.keep.get(&(x, y, recipe)).copied().unwrap_or(0)
    }

    /// All keep orders as `(x, y, recipe, target)`, in key order.
    pub fn keep_orders(&self) -> impl Iterator<Item = (u16, u16, u16, u32)> + '_ {
        self.keep.iter().map(|(&(x, y, r), &n)| (x, y, r, n))
    }

    /// Units of the first output of a recipe that lie around or are carried, not rotten.
    pub fn keep_stock(&self, recipe: u16) -> u64 {
        let Some(material) = self
            .rules
            .recipe(recipe)
            .and_then(|r| r.outputs.first().map(|o| o.0))
        else {
            return 0;
        };
        self.items
            .iter()
            .filter(|(_, i)| {
                i.material == material
                    && !self.is_rotten(i)
                    && matches!(i.loc, ItemLoc::Tile(..) | ItemLoc::Carried(_))
            })
            .map(|(_, i)| u64::from(i.count))
            .sum()
    }

    pub(crate) fn apply_keep_command(&mut self, command: &Command) {
        let Command::SetKeep {
            x,
            y,
            recipe,
            target,
        } = *command
        else {
            return;
        };
        if target == 0 {
            self.keep.remove(&(x, y, recipe));
            return;
        }
        let Some(def) = self.rules.recipe(recipe) else {
            self.reject(Reject::UnknownRecipe);
            return;
        };
        let workshop = def.workshop;
        if target > MAX_KEEP {
            self.reject(Reject::BadCount);
            return;
        }
        if self
            .buildings
            .object_at(x, y)
            .is_none_or(|b| b.def != workshop)
        {
            self.reject(Reject::NoWorkshop);
            return;
        }
        self.keep.insert((x, y, recipe), target);
        self.keep_orders_now();
    }

    /// Each game hour: order the runs that the keep orders need.
    pub(crate) fn production_tick(&mut self) {
        if self.tick.is_multiple_of(TICKS_PER_HOUR) {
            self.keep_orders_now();
        }
    }

    fn keep_orders_now(&mut self) {
        let orders: Vec<(u16, u16, u16, u32)> = self.keep_orders().collect();
        for (x, y, recipe, target) in orders {
            let Some(def) = self.rules.recipe(recipe) else {
                self.keep.remove(&(x, y, recipe));
                continue;
            };
            let per_run = def.outputs.first().map_or(1, |o| o.1.max(1));
            let workshop = def.workshop;
            let Some(building) = self.buildings.object_at(x, y).filter(|b| b.def == workshop)
            else {
                self.keep.remove(&(x, y, recipe));
                continue;
            };
            let pending = building
                .orders
                .iter()
                .find(|o| o.recipe == recipe)
                .map_or(0, |o| u64::from(o.remaining));
            let have = self.keep_stock(recipe) + pending * u64::from(per_run);
            let want = u64::from(target);
            if have < want {
                let runs = (want - have).div_ceil(u64::from(per_run));
                self.order_craft(x, y, recipe, u32::try_from(runs).unwrap_or(u32::MAX));
            }
        }
    }

    /// Check the keep orders of a loaded state.
    pub(crate) fn keep_state_ok(&self) -> bool {
        self.keep.iter().all(|(&(x, y, r), &n)| {
            self.map.in_bounds(i32::from(x), i32::from(y))
                && self.rules.recipe(r).is_some()
                && n <= MAX_KEEP
        })
    }
}
