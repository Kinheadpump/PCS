//! Hearth fuel and refuel jobs (R-022).
//!
//! A hearth holds fuel in burning hours. It burns 1 hour of fuel in each game hour. A
//! hearth without fuel gives no heat. A hearth with little fuel gets a hauling job: a
//! colonist carries a stack of fuel to the hearth. This job is a `JobKind::Haul` job with
//! [`REFUEL_ARG`] as `arg`. It has no item; the colonist picks the stack when it takes the
//! job.

use crate::Sim;
use crate::item::{ItemId, ItemLoc};
use crate::job::{AbortReason, ActiveJob, Job, JobKind, octile};
use base::content::BuildingKind;

/// The `arg` of a hauling job that brings fuel to the hearth on the job tile.
pub const REFUEL_ARG: u16 = 1;

/// The best stack for a delivery: where to pick it, where to go with it, and the stack.
type Best = ((u8, u32, u32), ItemId, (u16, u16));
type Plan = ((u16, u16), Option<(u16, u16)>, Option<ItemId>);

impl Job {
    /// True if this is a hauling job that brings fuel to a hearth.
    pub fn is_refuel(&self) -> bool {
        self.kind == JobKind::Haul && self.arg == REFUEL_ARG
    }
}

impl Sim {
    /// The tiles of all hearths, in `(x, y)` order.
    pub fn hearth_tiles(&self) -> Vec<(u16, u16)> {
        self.buildings
            .object_tiles(|def| self.rules.is_kind(def, BuildingKind::Hearth))
    }

    /// The fuel of the hearth on a tile, in burning hours. 0 if there is no hearth.
    pub fn hearth_fuel(&self, x: u16, y: u16) -> u32 {
        self.buildings
            .object_at(x, y)
            .filter(|b| self.rules.is_kind(b.def, BuildingKind::Hearth))
            .map_or(0, |b| b.fuel_hours)
    }

    /// True if the hearth on a tile burns now: it has fuel.
    pub fn hearth_burning(&self, x: u16, y: u16) -> bool {
        self.hearth_fuel(x, y) > 0
    }

    /// True if the hearth burns now: it has fuel, and its room is below the upper comfort
    /// limit. A hearth in a room that is too warm is banked: it keeps its fuel and gives no
    /// heat. A hearth outdoors always burns.
    pub fn hearth_lit(&self, x: u16, y: u16) -> bool {
        self.hearth_fuel(x, y) > 0
            && self
                .room_temp(x, y)
                .is_none_or(|t| i32::from(t) < i32::from(self.rules.needs.comfort_high))
    }

    /// Put fuel in a hearth at once, in burning hours. For scenario tests and debug tools.
    /// The value can be above the capacity. Return true if there is a hearth.
    pub fn debug_set_hearth_fuel(&mut self, x: u16, y: u16, hours: u32) -> bool {
        let is_hearth = self
            .buildings
            .object_at(x, y)
            .is_some_and(|b| self.rules.is_kind(b.def, BuildingKind::Hearth));
        if is_hearth {
            self.buildings.set_fuel(x, y, hours.min(1_000_000));
        }
        is_hearth
    }

    /// Each game hour: every hearth with fuel burns 1 hour of it. Then hearths with little
    /// fuel get a refuel job.
    pub(crate) fn burn_hearths(&mut self) {
        self.fire_hour();
        let hearths = self.hearth_tiles();
        for &(x, y) in &hearths {
            let fuel = self.hearth_fuel(x, y);
            if self.hearth_lit(x, y) {
                self.buildings.set_fuel(x, y, fuel - 1);
            }
        }
        let low = self.rules.climate.refuel_below_hours;
        for (x, y) in hearths {
            if self.hearth_fuel(x, y) <= low
                && self.refuel_wanted(x, y)
                && self
                    .jobs
                    .find_arg(JobKind::Haul, x, y, REFUEL_ARG)
                    .is_none()
            {
                self.add_job_arg(JobKind::Haul, x, y, REFUEL_ARG);
            }
        }
    }

    /// True if the job with this ID is a refuel job.
    pub(crate) fn is_refuel_job(&self, job: crate::JobId) -> bool {
        self.jobs.get(job).is_some_and(Job::is_refuel)
    }

    /// Burning hours of the shortest fuel unit.
    fn shortest_fuel_hours(&self) -> Option<u32> {
        self.rules.fuel.iter().map(|f| f.hours).min()
    }

    /// True if a hearth stands on the tile and has room for one more unit of fuel.
    pub(crate) fn refuel_wanted(&self, x: u16, y: u16) -> bool {
        let Some(unit) = self.shortest_fuel_hours() else {
            return false;
        };
        let cap = self.rules.climate.hearth_fuel_capacity_hours;
        self.buildings
            .object_at(x, y)
            .is_some_and(|b| self.rules.is_kind(b.def, BuildingKind::Hearth))
            && self.hearth_fuel(x, y).saturating_add(unit) <= cap
    }

    /// Units of a material that the hearth of a refuel job can take. 0 if the material is
    /// not fuel or the hearth is full.
    pub(crate) fn refuel_units_needed(&self, job: &Job, material: u16) -> u32 {
        let Some(def) = self.rules.fuel.iter().find(|f| f.material == material) else {
            return 0;
        };
        let cap = self.rules.climate.hearth_fuel_capacity_hours;
        cap.saturating_sub(self.hearth_fuel(job.x, job.y)) / def.hours.max(1)
    }

    /// Check a refuel job in a region. Return the tile of the fuel stack, the tile next to the
    /// hearth where the colonist goes with it, and the stack. A stack in a stockpile is
    /// first; then the nearest; the stack index breaks ties.
    pub(crate) fn eval_refuel(&self, region: u32, from: (u16, u16), job: &Job) -> Option<Plan> {
        if !self.refuel_wanted(job.x, job.y) {
            return None;
        }
        let mut best: Option<Best> = None;
        for def in &self.rules.fuel {
            if self.refuel_units_needed(job, def.material) == 0 {
                continue;
            }
            let Some(item) = self.find_material(region, from, def.material) else {
                continue;
            };
            let ItemLoc::Tile(ix, iy) = self.items.get(item)?.loc else {
                continue;
            };
            let key = (
                u8::from(!self.is_stockpiled(item)),
                octile(from, (ix, iy)),
                item.index(),
            );
            if best.is_none_or(|(k, _, _)| key < k) {
                best = Some((key, item, (ix, iy)));
            }
        }
        let (_, item, tile) = best?;
        let drop = self.work_stand(region, tile, job.x, job.y)?;
        Some((tile, Some(drop), Some(item)))
    }

    /// A colonist arrives at the hearth with fuel: the hearth takes what it can hold. The
    /// rest of the stack stays on the tile. The job is done.
    pub(crate) fn deliver_refuel(&mut self, id: crate::ColonistId, active: ActiveJob) {
        let Some(job) = self.jobs.get(active.job).cloned() else {
            self.abort_job(id, AbortReason::JobGone, false);
            return;
        };
        let Some(stack) = active.item.and_then(|i| self.items.get(i)) else {
            self.abort_job(id, AbortReason::JobGone, false);
            return;
        };
        let (material, count) = (stack.material, stack.count);
        let need = self.refuel_units_needed(&job, material);
        if need == 0 {
            self.abort_job(id, AbortReason::JobGone, false);
            return;
        }
        let take = count.min(need);
        let hours = self
            .rules
            .fuel
            .iter()
            .find(|f| f.material == material)
            .map_or(0, |f| f.hours);
        if let Some(item) = active.item {
            self.items.remove(item);
        }
        let here = self.colonists.get(id).map(|c| c.tile()).unwrap_or_default();
        if let Some(c) = self.colonists.get_mut(id) {
            c.carrying = None;
        }
        let fuel = self.hearth_fuel(job.x, job.y);
        self.buildings
            .set_fuel(job.x, job.y, fuel.saturating_add(take * hours));
        if count > take {
            self.spawn_units(material, count - take, here.0, here.1);
        }
        self.finish_job(id, active, job.x, job.y);
    }
}
