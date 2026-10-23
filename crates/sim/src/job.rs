//! Jobs: the `JobBoard`, job state machines, reservations, designations, and job choice
//! (TECHNICAL_DESIGN §5, §6.4).
//!
//! A job is a small state machine (no behavior tree):
//! - Cut and mine: `GoToWork`, `Work`, then the item appears.
//! - Haul: `GoToItem`, pick up, `GoToDest`, drop.
//!
//! An idle colonist takes the unclaimed job with the lowest work priority number. Ties
//! go to the shorter estimated distance, then to the lower job ID. A job outside the
//! connectivity region of the colonist is never given. A claim reserves the target: the
//! item, the destination tile, or the designated tile. Nobody else can reserve these.

use crate::colonist::{ColonistId, NeedAct};
use crate::item::{ItemId, ItemLoc};
use crate::skill::{self, Skill, WorkSpeed};
use crate::{Command, PathRequest, Sim};
use base::{Arena, Id};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Type of a job and of a designation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum JobKind {
    /// Cut a tree. It gives logs.
    Cut,
    /// Mine a rock wall tile. It gives stone and maybe ore, and leaves a floor tile.
    Mine,
    /// Carry an item stack to a stockpile.
    Haul,
    /// Bring material to a blueprint and build the building.
    Construct,
    /// Take a finished building down. Part of its material units come back.
    Deconstruct,
    /// Make one run of a workshop recipe.
    Craft,
}

impl JobKind {
    /// Number of job types.
    pub const COUNT: usize = 6;
    /// All job types, in index order.
    pub const ALL: [JobKind; Self::COUNT] = [
        JobKind::Cut,
        JobKind::Mine,
        JobKind::Haul,
        JobKind::Construct,
        JobKind::Deconstruct,
        JobKind::Craft,
    ];

    /// Index in the priority array of a colonist.
    pub fn index(self) -> usize {
        self as usize
    }

    /// Display name.
    pub fn name(self) -> &'static str {
        match self {
            JobKind::Cut => "Cut",
            JobKind::Mine => "Mine",
            JobKind::Haul => "Haul",
            JobKind::Construct => "Construct",
            JobKind::Deconstruct => "Deconstruct",
            JobKind::Craft => "Craft",
        }
    }

    /// The skill that this job type trains and uses. Hauling has none.
    pub fn skill(self) -> Option<Skill> {
        match self {
            JobKind::Cut => Some(Skill::Woodcutting),
            JobKind::Mine => Some(Skill::Mining),
            JobKind::Haul => None,
            JobKind::Construct | JobKind::Deconstruct => Some(Skill::Construction),
            JobKind::Craft => Some(Skill::Crafting),
        }
    }
}

/// Typed ID of a job.
pub type JobId = Id<Job>;

/// A unit of work on the board.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Job {
    /// The job type.
    pub kind: JobKind,
    /// Target tile column: the tree, the rock, or the tile of the item.
    pub x: u16,
    /// Target tile row.
    pub y: u16,
    /// The item to haul (hauling jobs only).
    pub item: Option<ItemId>,
    /// Extra number of the job: the layer code of a construction or deconstruction job, the
    /// recipe index of a craft job, and 0 for other jobs.
    pub arg: u16,
    /// The colonist who does the job. Only one colonist can.
    pub claimed_by: Option<ColonistId>,
    /// The job is not offered before this tick (after a failure).
    pub blocked_until: u64,
}

/// The step of a job that a colonist does now.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum JobStep {
    /// Walk to the work tile (cut, mine).
    GoToWork,
    /// Work. `progress` grows by the work speed in percent each tick.
    Work {
        /// Work done so far, in tick-percent units.
        progress: u32,
    },
    /// Walk to the item (haul).
    GoToItem,
    /// Carry the item to the stockpile tile (haul).
    GoToDest,
}

/// The job that a colonist does, with its state.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActiveJob {
    /// The job on the board.
    pub job: JobId,
    /// The job type.
    pub kind: JobKind,
    /// Current step.
    pub step: JobStep,
    /// Tile where the colonist works, or the tile of the item.
    pub stand: (u16, u16),
    /// Stockpile tile where the colonist drops the item (haul).
    pub dest: Option<(u16, u16)>,
    /// The reserved item (haul).
    pub item: Option<ItemId>,
}

/// The jobs and the reservations of destination tiles.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct JobBoard {
    jobs: Arena<Job>,
    dest_reservations: BTreeMap<(u16, u16), ColonistId>,
}

impl JobBoard {
    /// One job.
    pub fn get(&self, id: JobId) -> Option<&Job> {
        self.jobs.get(id)
    }

    /// All jobs, in ID order.
    pub fn iter(&self) -> impl Iterator<Item = (JobId, &Job)> {
        self.jobs.iter()
    }

    /// Number of jobs.
    pub fn len(&self) -> usize {
        self.jobs.len()
    }

    /// True if there are no jobs.
    pub fn is_empty(&self) -> bool {
        self.jobs.is_empty()
    }

    /// The job of a type on a tile, if any.
    pub fn find(&self, kind: JobKind, x: u16, y: u16) -> Option<JobId> {
        self.jobs
            .iter()
            .find(|(_, j)| j.kind == kind && j.x == x && j.y == y && j.item.is_none())
            .map(|(id, _)| id)
    }

    /// The hauling job of an item, if any.
    pub fn find_by_item(&self, item: ItemId) -> Option<JobId> {
        self.jobs
            .iter()
            .find(|(_, j)| j.item == Some(item))
            .map(|(id, _)| id)
    }

    /// The colonist who reserves a destination tile.
    pub fn dest_reserved_by(&self, tile: (u16, u16)) -> Option<ColonistId> {
        self.dest_reservations.get(&tile).copied()
    }

    /// All reserved destination tiles with their colonist, in tile order.
    pub fn dest_reservations(&self) -> impl Iterator<Item = (&(u16, u16), &ColonistId)> {
        self.dest_reservations.iter()
    }

    pub(crate) fn add(&mut self, kind: JobKind, x: u16, y: u16, item: Option<ItemId>) -> JobId {
        self.jobs.insert(Job {
            kind,
            x,
            y,
            item,
            arg: 0,
            claimed_by: None,
            blocked_until: 0,
        })
    }

    /// Put a job with an argument on the board (construction, deconstruction, craft).
    pub(crate) fn add_arg(&mut self, kind: JobKind, x: u16, y: u16, arg: u16) -> JobId {
        self.jobs.insert(Job {
            kind,
            x,
            y,
            item: None,
            arg,
            claimed_by: None,
            blocked_until: 0,
        })
    }

    /// The job of a type with this tile and argument, if any.
    pub fn find_arg(&self, kind: JobKind, x: u16, y: u16, arg: u16) -> Option<JobId> {
        self.jobs
            .iter()
            .find(|(_, j)| j.kind == kind && j.x == x && j.y == y && j.arg == arg)
            .map(|(id, _)| id)
    }

    pub(crate) fn remove(&mut self, id: JobId) -> Option<Job> {
        self.jobs.remove(id)
    }

    fn get_mut(&mut self, id: JobId) -> Option<&mut Job> {
        self.jobs.get_mut(id)
    }
}

/// Why a command was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reject {
    /// The tile is outside the map.
    OutOfMap,
    /// A cut designation needs a tree.
    NothingToCut,
    /// A mine designation needs a rock wall.
    NothingToMine,
    /// A haul designation needs an item on the tile.
    NoItems,
    /// The stockpile corners are outside the map.
    BadZone,
    /// The stockpile overlaps another stockpile.
    ZoneOverlap,
    /// The colonist or stockpile does not exist.
    UnknownTarget,
    /// A priority must be 0 (off) or 1 to 4.
    BadPriority,
    /// Only cut, mine, and haul are designations.
    BadDesignation,
    /// The building type does not exist.
    UnknownBuilding,
    /// The material is not in a category that the building accepts.
    BadMaterial,
    /// Water, rock, a tree, or a wall tile is on the tile.
    TileBlocked,
    /// A building or a blueprint is on this layer of the tile.
    TileOccupied,
    /// A roof needs a wall or natural rock within the roof support radius (A-008).
    NoRoofSupport,
    /// There is no blueprint on this layer of the tile.
    NoBlueprint,
    /// There is no building on this layer of the tile.
    NoBuilding,
    /// The building already has a deconstruction order.
    AlreadyOrdered,
    /// The recipe does not exist.
    UnknownRecipe,
    /// A craft order needs a count of 1 or more.
    BadCount,
    /// The tile has no workshop that can make the recipe.
    NoWorkshop,
}

/// Why a job stopped before it was done.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AbortReason {
    /// The job left the board (cancelled or its target is gone).
    JobGone,
    /// The colonist found no path.
    NoPath,
    /// A construction or craft job found no material to bring.
    NoMaterial,
}

/// Job events for the interface. They are not part of the state. `Sim::drain_job_events`
/// gives them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum JobEvent {
    /// A colonist finished a job.
    Completed {
        /// The colonist.
        colonist: ColonistId,
        /// The job type.
        kind: JobKind,
        /// Target tile column.
        x: u16,
        /// Target tile row.
        y: u16,
    },
    /// A colonist stopped a job. The job may return to the board.
    Aborted {
        /// The colonist.
        colonist: ColonistId,
        /// The job type.
        kind: JobKind,
        /// The reason.
        reason: AbortReason,
    },
    /// A command was refused.
    Rejected(Reject),
}

/// A job that a colonist can take now.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Pick {
    job: JobId,
    kind: JobKind,
    stand: (u16, u16),
    dest: Option<(u16, u16)>,
    item: Option<ItemId>,
    key: (u8, u32, u32),
}

/// Estimated walking cost between two tiles (octile, costs 10 and 14).
pub(crate) fn octile(a: (u16, u16), b: (u16, u16)) -> u32 {
    let dx = u32::from(a.0.abs_diff(b.0));
    let dy = u32::from(a.1.abs_diff(b.1));
    14 * dx.min(dy) + 10 * (dx.max(dy) - dx.min(dy))
}

/// The four neighbors of a tile, in a fixed order.
pub(crate) const NEIGHBORS: [(i32, i32); 4] = [(0, -1), (-1, 0), (1, 0), (0, 1)];

pub(crate) enum Leg {
    Arrived,
    Walking,
    Failed,
}

impl Sim {
    // ---- commands ----

    pub(crate) fn apply_job_command(&mut self, command: &Command) {
        match command {
            Command::Designate { kind, x, y } => self.designate(*kind, *x, *y),
            Command::CancelDesignation { x, y } => self.cancel_designation(*x, *y),
            Command::DigDitch { x, y } => self.dig_ditch(*x, *y),
            Command::CreateStockpile {
                x0,
                y0,
                x1,
                y1,
                filter,
            } => self.create_stockpile(*x0, *y0, *x1, *y1, filter.clone()),
            Command::RemoveStockpile { id } => {
                if self.stockpiles.remove(*id).is_none() {
                    self.reject(Reject::UnknownTarget);
                }
            }
            Command::SetStockpileFilter { id, filter } => match self.stockpiles.get_mut(*id) {
                Some(sp) => {
                    sp.filter = filter.normalized();
                    self.requeue_hauls();
                    self.wake_idle();
                }
                None => self.reject(Reject::UnknownTarget),
            },
            Command::SetPriority {
                colonist,
                job,
                priority,
            } => {
                if *priority > 4 {
                    self.reject(Reject::BadPriority);
                } else if let Some(c) = self.colonists.get_mut(*colonist) {
                    c.priorities[job.index()] = *priority;
                    c.next_search = c.next_search.min(self.tick);
                } else {
                    self.reject(Reject::UnknownTarget);
                }
            }
            Command::MoveColonist { .. } => {}
            Command::PlaceBlueprint { .. }
            | Command::CancelBlueprint { .. }
            | Command::Deconstruct { .. }
            | Command::CancelDeconstruct { .. }
            | Command::OrderCraft { .. }
            | Command::CancelCraft { .. } => self.apply_build_command(command),
        }
    }

    /// Put a job on the board. Idle colonists look for work again at once.
    fn add_job(&mut self, kind: JobKind, x: u16, y: u16, item: Option<ItemId>) {
        self.jobs.add(kind, x, y, item);
        self.wake_idle();
    }

    /// Make hauling jobs for flagged items that have none (after a stockpile change).
    fn requeue_hauls(&mut self) {
        let flagged: Vec<ItemId> = self
            .items
            .iter()
            .filter(|(_, i)| i.to_haul && i.reserved_by.is_none())
            .map(|(id, _)| id)
            .collect();
        for id in flagged {
            self.queue_haul(id);
        }
    }

    /// Let idle colonists search the board in this tick.
    pub(crate) fn wake_idle(&mut self) {
        let tick = self.tick;
        let ids: Vec<ColonistId> = self.colonists.iter().map(|(id, _)| id).collect();
        for id in ids {
            if let Some(c) = self.colonists.get_mut(id)
                && c.job.is_none()
            {
                c.next_search = c.next_search.min(tick);
            }
        }
    }

    pub(crate) fn reject(&mut self, reason: Reject) {
        self.job_events.push(JobEvent::Rejected(reason));
    }

    fn designate(&mut self, kind: JobKind, x: u16, y: u16) {
        let (ix, iy) = (i32::from(x), i32::from(y));
        if !self.map.in_bounds(ix, iy) {
            self.reject(Reject::OutOfMap);
            return;
        }
        match kind {
            JobKind::Cut | JobKind::Mine => {
                let ok = if kind == JobKind::Cut {
                    self.map.tree(ix, iy)
                } else {
                    self.map
                        .rock(ix, iy)
                        .is_some_and(|r| r != worldgen::RockKind::None)
                };
                if !ok {
                    self.reject(if kind == JobKind::Cut {
                        Reject::NothingToCut
                    } else {
                        Reject::NothingToMine
                    });
                    return;
                }
                if self.designations.insert((x, y), kind).is_none() {
                    self.add_job(kind, x, y, None);
                }
            }
            JobKind::Haul => {
                let ids: Vec<ItemId> = self.items.at_tile(x, y).to_vec();
                if ids.is_empty() {
                    self.reject(Reject::NoItems);
                    return;
                }
                for id in ids {
                    self.items.set_to_haul(id, true);
                    self.queue_haul(id);
                }
            }
            JobKind::Construct | JobKind::Deconstruct | JobKind::Craft => {
                self.reject(Reject::BadDesignation);
            }
        }
    }

    fn cancel_designation(&mut self, x: u16, y: u16) {
        if let Some(kind) = self.designations.remove(&(x, y))
            && let Some(job) = self.jobs.find(kind, x, y)
        {
            self.jobs.remove(job);
        }
        let ids: Vec<ItemId> = self.items.at_tile(x, y).to_vec();
        for id in ids {
            let loose = self
                .items
                .get(id)
                .is_some_and(|i| i.reserved_by.is_none() && i.to_haul);
            if loose {
                self.items.set_to_haul(id, false);
                if let Some(job) = self.jobs.find_by_item(id) {
                    self.jobs.remove(job);
                }
            }
        }
    }

    fn create_stockpile(&mut self, ax: u16, ay: u16, bx: u16, by: u16, filter: crate::ItemFilter) {
        let (x0, x1) = (ax.min(bx), ax.max(bx));
        let (y0, y1) = (ay.min(by), ay.max(by));
        if !self.map.in_bounds(i32::from(x1), i32::from(y1)) {
            self.reject(Reject::BadZone);
            return;
        }
        let zone = crate::Stockpile {
            x0,
            y0,
            x1,
            y1,
            filter: filter.normalized(),
        };
        if self.stockpiles.iter().any(|(_, s)| s.overlaps(&zone)) {
            self.reject(Reject::ZoneOverlap);
            return;
        }
        self.stockpiles.insert(zone);
        self.requeue_hauls();
        self.wake_idle();
    }

    // ---- items and haul jobs ----

    /// Put an item stack on a tile without a haul order. For scenario tests and debug
    /// tools; the player has no such command.
    pub fn debug_spawn_item(&mut self, material: u16, count: u32, x: u16, y: u16) {
        if self.map.in_bounds(i32::from(x), i32::from(y)) {
            self.spawn_item(material, count, x, y, false);
        }
    }

    /// Put a new item stack on a tile. A stack outside the stockpiles that accept it gets
    /// a hauling job when `to_haul` is true.
    pub(crate) fn spawn_item(&mut self, material: u16, count: u32, x: u16, y: u16, to_haul: bool) {
        if let Some(id) = self.items.spawn(material, count, x, y, to_haul) {
            self.queue_haul(id);
        }
    }

    /// True if the tile of the item lies in a stockpile that accepts its material.
    pub(crate) fn is_stockpiled(&self, item: ItemId) -> bool {
        let Some(i) = self.items.get(item) else {
            return false;
        };
        let ItemLoc::Tile(x, y) = i.loc else {
            return false;
        };
        self.stockpiles
            .iter()
            .any(|(_, s)| s.contains(x, y) && s.filter.accepts(i.material))
    }

    /// Make the hauling job for a flagged item, if it needs one and has none.
    pub(crate) fn queue_haul(&mut self, item: ItemId) {
        let Some(i) = self.items.get(item) else {
            return;
        };
        let ItemLoc::Tile(x, y) = i.loc else {
            return;
        };
        if !i.to_haul || self.jobs.find_by_item(item).is_some() {
            return;
        }
        if self.is_stockpiled(item) {
            self.items.set_to_haul(item, false);
            return;
        }
        let material = i.material;
        if !self
            .stockpiles
            .iter()
            .any(|(_, s)| s.filter.accepts(material))
        {
            return; // No stockpile accepts it: no hauling job. A new zone adds the job.
        }
        self.add_job(JobKind::Haul, x, y, Some(item));
    }

    // ---- why chain for work speed ----

    /// The work priority of a colonist for a job type: 1 to 4, or 0 for "off".
    pub fn priority(&self, id: ColonistId, kind: JobKind) -> Option<u8> {
        Some(self.colonists.get(id)?.priorities[kind.index()])
    }

    /// The level of a skill of a colonist (R-010).
    pub fn skill_level(&self, id: ColonistId, skill: Skill) -> Option<u8> {
        let c = self.colonists.get(id)?;
        Some(skill::level_for_xp(
            c.skill_xp[skill.index()],
            &self.rules.jobs,
        ))
    }

    /// The work speed of a colonist for a job type, as a sum of named contributions
    /// (R-010, D-021). The work system uses this same function.
    pub fn work_speed(&self, id: ColonistId, kind: JobKind) -> Option<WorkSpeed> {
        let skill = match kind.skill() {
            Some(s) => Some((s, self.skill_level(id, s)?)),
            None => {
                self.colonists.get(id)?;
                None
            }
        };
        Some(skill::work_speed(skill, &self.rules.jobs))
    }

    /// Set the experience of a skill. For scenario tests and debug tools.
    pub fn set_skill_xp(&mut self, id: ColonistId, skill: Skill, xp: u32) {
        let cap = skill::max_xp(&self.rules.jobs);
        if let Some(c) = self.colonists.get_mut(id) {
            c.skill_xp[skill.index()] = xp.min(cap);
        }
    }

    // ---- job choice ----

    /// The unclaimed job that the colonist would take now, by the selection rule:
    /// lowest work priority number, then shortest estimated distance, then lowest job ID.
    /// A job in another region or with priority "off" is never chosen. Does not claim.
    pub fn pick_job(&self, id: ColonistId) -> Option<JobId> {
        let mut budget = u32::MAX;
        self.pick(id, &mut budget).0.map(|p| p.job)
    }

    /// Find the best job. Each examined job costs 1 of the budget. Return the best job
    /// found and true if the budget ended the search early.
    fn pick(&self, id: ColonistId, budget: &mut u32) -> (Option<Pick>, bool) {
        let Some(c) = self.colonists.get(id) else {
            return (None, false);
        };
        let from = c.tile();
        let region = self.regions.id_at(i32::from(from.0), i32::from(from.1));
        if region == 0 {
            return (None, false);
        }
        let mut best: Option<Pick> = None;
        for (job_id, job) in self.jobs.iter() {
            let priority = c.priorities[job.kind.index()];
            if priority == 0
                || job.claimed_by.is_some()
                || job.blocked_until > self.tick
                || best.is_some_and(|b| priority > b.key.0)
            {
                continue;
            }
            if *budget == 0 {
                return (best, true);
            }
            *budget -= 1;
            let Some((stand, dest, item)) = self.evaluate(region, from, job) else {
                continue;
            };
            let pick = Pick {
                job: job_id,
                kind: job.kind,
                stand,
                dest,
                item,
                key: (priority, octile(from, stand), job_id.index()),
            };
            if best.is_none_or(|b| pick.key < b.key) {
                best = Some(pick);
            }
        }
        (best, false)
    }

    /// Check that a job is possible in a region. Return the work or item tile, the
    /// destination, and the item.
    #[allow(clippy::type_complexity)]
    fn evaluate(
        &self,
        region: u32,
        from: (u16, u16),
        job: &Job,
    ) -> Option<((u16, u16), Option<(u16, u16)>, Option<ItemId>)> {
        let (x, y) = (i32::from(job.x), i32::from(job.y));
        match job.kind {
            JobKind::Cut => (self.map.tree(x, y) && self.regions.id_at(x, y) == region)
                .then_some(((job.x, job.y), None, None)),
            JobKind::Mine => {
                // The passable neighbor nearest to the colonist; the fixed order breaks ties.
                let mut best: Option<((u16, u16), u32)> = None;
                for (dx, dy) in NEIGHBORS {
                    let (nx, ny) = (x + dx, y + dy);
                    if self.regions.id_at(nx, ny) != region {
                        continue;
                    }
                    let tile = (nx as u16, ny as u16);
                    let cost = octile(from, tile);
                    if best.is_none_or(|(_, c)| cost < c) {
                        best = Some((tile, cost));
                    }
                }
                best.map(|(tile, _)| (tile, None, None))
            }
            JobKind::Haul if job.is_refuel() => self.eval_refuel(region, from, job),
            JobKind::Haul => {
                let item_id = job.item?;
                let item = self.items.get(item_id)?;
                let ItemLoc::Tile(ix, iy) = item.loc else {
                    return None;
                };
                if item.reserved_by.is_some()
                    || !item.to_haul
                    || self.regions.id_at(i32::from(ix), i32::from(iy)) != region
                {
                    return None;
                }
                let dest = self.find_dest(region, item.material, item.count, (ix, iy))?;
                Some(((ix, iy), Some(dest), Some(item_id)))
            }
            JobKind::Construct | JobKind::Deconstruct | JobKind::Craft => {
                self.eval_build(region, from, job)
            }
        }
    }

    /// The first free stockpile tile, in stockpile and row-major order, that accepts the
    /// material, is in the region, and has room.
    fn find_dest(
        &self,
        region: u32,
        material: u16,
        count: u32,
        from: (u16, u16),
    ) -> Option<(u16, u16)> {
        let stack_max = self.rules.jobs.stack_max;
        for (_, sp) in self.stockpiles.iter() {
            if !sp.filter.accepts(material) {
                continue;
            }
            for y in sp.y0..=sp.y1 {
                for x in sp.x0..=sp.x1 {
                    if (x, y) == from
                        || self.regions.id_at(i32::from(x), i32::from(y)) != region
                        || self.jobs.dest_reserved_by((x, y)).is_some()
                    {
                        continue;
                    }
                    let here = self.items.at_tile(x, y);
                    let room = match here {
                        [] => true,
                        [one] => self.items.get(*one).is_some_and(|o| {
                            o.material == material
                                && o.reserved_by.is_none()
                                && o.count.saturating_add(count) <= stack_max
                        }),
                        _ => false,
                    };
                    if room {
                        return Some((x, y));
                    }
                }
            }
        }
        None
    }

    // ---- the job system ----

    /// Run the job system for one tick: clean the board, advance the active jobs, and
    /// give jobs to idle colonists.
    pub(crate) fn update_jobs(&mut self) {
        if self
            .tick
            .is_multiple_of(u64::from(self.rules.jobs.retry_ticks.max(1)))
        {
            self.clean_board();
        }
        let ids: Vec<ColonistId> = self.colonists.iter().map(|(id, _)| id).collect();
        for &id in &ids {
            self.advance_job(id);
        }
        let n = ids.len();
        if n == 0 {
            return;
        }
        let mut budget = self.rules.jobs.search_budget;
        let start = (self.tick % n as u64) as usize;
        for k in 0..n {
            if budget == 0 {
                break;
            }
            let id = ids[(start + k) % n];
            let idle = self.colonists.get(id).is_some_and(|c| {
                c.job.is_none()
                    && c.act.is_none()
                    && c.break_until <= self.tick
                    && c.next_search <= self.tick
            });
            if !idle {
                continue;
            }
            let (pick, cut_short) = self.pick(id, &mut budget);
            match pick {
                Some(p) => self.claim(id, p),
                None if !cut_short => {
                    let wait = u64::from(self.rules.jobs.retry_ticks);
                    if let Some(c) = self.colonists.get_mut(id) {
                        c.next_search = self.tick + wait;
                    }
                }
                None => {}
            }
        }
    }

    /// Remove unclaimed jobs whose target is gone.
    fn clean_board(&mut self) {
        let stale: Vec<JobId> = self
            .jobs
            .iter()
            .filter(|(_, j)| j.claimed_by.is_none())
            .filter(|(_, j)| {
                let (x, y) = (i32::from(j.x), i32::from(j.y));
                match j.kind {
                    JobKind::Cut => !self.map.tree(x, y),
                    JobKind::Mine if j.is_dig() => self.ditch_allowed(j.x, j.y).is_err(),
                    JobKind::Mine => self
                        .map
                        .rock(x, y)
                        .is_none_or(|r| r == worldgen::RockKind::None),
                    JobKind::Haul if j.is_refuel() => !self.refuel_wanted(j.x, j.y),
                    JobKind::Haul => match j.item {
                        Some(item) => {
                            !self.items.get(item).is_some_and(|i| {
                                i.to_haul
                                    && self
                                        .stockpiles
                                        .iter()
                                        .any(|(_, s)| s.filter.accepts(i.material))
                            }) || self.is_stockpiled(item)
                        }
                        None => true,
                    },
                    JobKind::Construct | JobKind::Deconstruct | JobKind::Craft => {
                        self.build_job_stale(j)
                    }
                }
            })
            .map(|(id, _)| id)
            .collect();
        for id in stale {
            let Some(job) = self.jobs.remove(id) else {
                continue;
            };
            if job.is_dig() {
                self.designations.remove(&(job.x, job.y));
            } else if job.kind == JobKind::Haul
                && let Some(item) = job.item
                && self.is_stockpiled(item)
            {
                self.items.set_to_haul(item, false);
            }
        }
    }

    /// The colonist takes the job: reserve the targets and start the first step.
    fn claim(&mut self, id: ColonistId, pick: Pick) {
        if let Some(job) = self.jobs.get_mut(pick.job) {
            job.claimed_by = Some(id);
        }
        if let Some(item) = pick.item {
            self.items.set_reserved(item, Some(id));
        }
        if let Some(dest) = pick.dest
            && pick.kind == JobKind::Haul
            && !self.is_refuel_job(pick.job)
        {
            self.jobs.dest_reservations.insert(dest, id);
        }
        let step = if pick.item.is_some() {
            JobStep::GoToItem
        } else {
            JobStep::GoToWork
        };
        if let Some(c) = self.colonists.get_mut(id) {
            c.job = Some(ActiveJob {
                job: pick.job,
                kind: pick.kind,
                step,
                stand: pick.stand,
                dest: pick.dest,
                item: pick.item,
            });
        }
        self.start_leg(id, pick.stand);
    }

    /// Ask a path to a tile. A colonist who is already there does not move.
    pub(crate) fn start_leg(&mut self, id: ColonistId, goal: (u16, u16)) {
        let Some(c) = self.colonists.get_mut(id) else {
            return;
        };
        if c.tile() == goal && c.path.is_empty() {
            c.target = None;
            return;
        }
        c.target = Some(goal);
        self.path_queue.retain(|r| r.colonist != id);
        self.path_queue
            .push_back(PathRequest { colonist: id, goal });
    }

    pub(crate) fn leg_state(&self, id: ColonistId, goal: (u16, u16)) -> Leg {
        match self.colonists.get(id) {
            Some(c) if c.tile() == goal && c.target.is_none() && c.path.is_empty() => Leg::Arrived,
            Some(c) if c.target.is_some() => Leg::Walking,
            _ => Leg::Failed,
        }
    }

    fn set_step(&mut self, id: ColonistId, step: JobStep) {
        if let Some(a) = self.colonists.get_mut(id).and_then(|c| c.job.as_mut()) {
            a.step = step;
        }
    }

    fn advance_job(&mut self, id: ColonistId) {
        let Some(active) = self.colonists.get(id).and_then(|c| c.job) else {
            return;
        };
        let on_board = self
            .jobs
            .get(active.job)
            .is_some_and(|j| j.claimed_by == Some(id));
        if !on_board {
            self.abort_job(id, AbortReason::JobGone, false);
            return;
        }
        match active.step {
            JobStep::GoToWork => match self.leg_state(id, active.stand) {
                Leg::Arrived => self.set_step(id, JobStep::Work { progress: 0 }),
                Leg::Walking => {}
                Leg::Failed => self.abort_job(id, AbortReason::NoPath, true),
            },
            JobStep::Work { progress } => self.do_work(id, active, progress),
            JobStep::GoToItem => match self.leg_state(id, active.stand) {
                Leg::Arrived => self.pick_up_item(id, active),
                Leg::Walking => {}
                Leg::Failed => self.abort_job(id, AbortReason::NoPath, true),
            },
            JobStep::GoToDest => {
                let Some(dest) = active.dest else {
                    self.abort_job(id, AbortReason::JobGone, false);
                    return;
                };
                match self.leg_state(id, dest) {
                    Leg::Arrived => self.drop_item(id, active, dest),
                    Leg::Walking => {}
                    Leg::Failed => self.abort_job(id, AbortReason::NoPath, true),
                }
            }
        }
    }

    /// Work time of a job in ticks at 100% speed.
    fn work_ticks(&self, job: &Job) -> u32 {
        let rules = &self.rules.jobs;
        match job.kind {
            JobKind::Cut => rules.cut_ticks,
            JobKind::Mine if job.is_dig() => self.rules.drought.dig_ticks,
            JobKind::Mine => {
                let rock = self
                    .map
                    .rock(i32::from(job.x), i32::from(job.y))
                    .unwrap_or(worldgen::RockKind::None);
                let hardness = self.rules.hardness(self.rules.rock_material(rock));
                rules.mine_ticks_base + hardness * rules.mine_ticks_per_10_hardness / 10
            }
            JobKind::Haul => 0,
            JobKind::Construct | JobKind::Deconstruct | JobKind::Craft => {
                self.build_work_ticks(job)
            }
        }
    }

    fn do_work(&mut self, id: ColonistId, active: ActiveJob, progress: u32) {
        let Some(job) = self.jobs.get(active.job).cloned() else {
            return;
        };
        let (x, y) = (i32::from(job.x), i32::from(job.y));
        let target_ok = match job.kind {
            JobKind::Cut => self.map.tree(x, y),
            JobKind::Mine if job.is_dig() => self.ditch_allowed(job.x, job.y).is_ok(),
            JobKind::Mine => self
                .map
                .rock(x, y)
                .is_some_and(|r| r != worldgen::RockKind::None),
            JobKind::Haul => false,
            JobKind::Construct | JobKind::Deconstruct | JobKind::Craft => {
                self.build_target_ok(&job)
            }
        };
        if !target_ok {
            self.abort_job(id, AbortReason::JobGone, false);
            return;
        }
        let pct = self
            .effective_work_speed(id, job.kind)
            .map_or(0, |s| u32::try_from(s.total_pct).unwrap_or(0));
        let progress = progress.saturating_add(pct);
        if u64::from(progress) < u64::from(self.work_ticks(&job)) * 100 {
            self.set_step(id, JobStep::Work { progress });
            return;
        }
        if matches!(
            job.kind,
            JobKind::Construct | JobKind::Deconstruct | JobKind::Craft
        ) {
            self.complete_build_work(id, active, &job);
            return;
        }
        if job.is_dig() {
            // A ditch tile is blocked for walking. It waits until the tile is free.
            if self
                .colonists
                .iter()
                .any(|(_, c)| c.x == job.x && c.y == job.y)
            {
                self.set_step(id, JobStep::Work { progress });
                return;
            }
            self.finish_ditch(job.x, job.y);
            self.designations.remove(&(job.x, job.y));
            self.finish_job(id, active, job.x, job.y);
            return;
        }
        let rules = self.rules.clone();
        // Hook for R-008 (tools): a later sprint checks the tool here and lowers its
        // durability. Sprint 3 colonists work bare-handed.
        match job.kind {
            JobKind::Cut => {
                self.map.clear_tree(x, y);
                self.spawn_item(rules.log, rules.jobs.log_yield, job.x, job.y, true);
            }
            JobKind::Mine => {
                if let Some((rock, ore)) = self.map.mine_rock(x, y) {
                    self.spawn_item(
                        rules.rock_material(rock),
                        rules.jobs.stone_yield,
                        job.x,
                        job.y,
                        true,
                    );
                    if ore != worldgen::Ore::None {
                        self.spawn_item(
                            rules.ore_material(ore),
                            rules.jobs.ore_yield,
                            job.x,
                            job.y,
                            true,
                        );
                    }
                    self.passability_changed(job.x, job.y);
                    self.rooms_dirty = true;
                }
            }
            JobKind::Haul | JobKind::Construct | JobKind::Deconstruct | JobKind::Craft => {}
        }
        self.designations.remove(&(job.x, job.y));
        self.finish_job(id, active, job.x, job.y);
    }

    fn pick_up_item(&mut self, id: ColonistId, active: ActiveJob) {
        let (Some(item), Some(dest)) = (active.item, active.dest) else {
            self.abort_job(id, AbortReason::JobGone, false);
            return;
        };
        let here = self.colonists.get(id).map(|c| c.tile()).unwrap_or_default();
        let valid = self
            .items
            .get(item)
            .is_some_and(|i| i.reserved_by == Some(id) && i.loc == ItemLoc::Tile(here.0, here.1));
        let dest_ok = self.regions.connected(
            (i32::from(here.0), i32::from(here.1)),
            (i32::from(dest.0), i32::from(dest.1)),
        );
        if valid && dest_ok && (active.kind != JobKind::Haul || self.is_refuel_job(active.job)) {
            self.trim_to_need(&active, item);
        }
        if !valid || !dest_ok || !self.items.pick_up(item, id) {
            self.abort_job(id, AbortReason::JobGone, !dest_ok);
            return;
        }
        if let Some(c) = self.colonists.get_mut(id) {
            c.carrying = Some(item);
        }
        self.set_step(id, JobStep::GoToDest);
        self.start_leg(id, dest);
    }

    fn drop_item(&mut self, id: ColonistId, active: ActiveJob, dest: (u16, u16)) {
        if active.kind == JobKind::Haul && self.is_refuel_job(active.job) {
            self.deliver_refuel(id, active);
            return;
        }
        if active.kind != JobKind::Haul {
            self.deliver_item(id, active);
            return;
        }
        let stack_max = self.rules.jobs.stack_max;
        if let Some(item) = active.item
            && let Some(kept) = self.items.put_down(item, dest.0, dest.1, stack_max)
        {
            self.items.set_to_haul(kept, false);
        }
        if let Some(c) = self.colonists.get_mut(id) {
            c.carrying = None;
        }
        self.finish_job(id, active, dest.0, dest.1);
    }

    /// The job is done: free the reservations, remove the job, add experience.
    pub(crate) fn finish_job(&mut self, id: ColonistId, active: ActiveJob, x: u16, y: u16) {
        self.jobs.remove(active.job);
        self.jobs.dest_reservations.retain(|_, who| *who != id);
        let rules = self.rules.jobs;
        if let Some(c) = self.colonists.get_mut(id) {
            c.job = None;
            c.next_search = self.tick;
            if let Some(skill) = active.kind.skill() {
                let cap = skill::max_xp(&rules);
                let xp = &mut c.skill_xp[skill.index()];
                *xp = xp.saturating_add(rules.xp_per_job).min(cap);
            }
        }
        self.notice_job_done(active.kind, x, y);
        self.job_events.push(JobEvent::Completed {
            colonist: id,
            kind: active.kind,
            x,
            y,
        });
    }

    /// Stop the job of a colonist. Free the reservations. A carried stack goes down on the
    /// tile of the colonist and keeps its haul flag. `block` keeps the job off the
    /// board for a while, so the same colonist does not retry at once.
    pub(crate) fn abort_job(&mut self, id: ColonistId, reason: AbortReason, block: bool) {
        let Some(c) = self.colonists.get_mut(id) else {
            return;
        };
        let Some(active) = c.job.take() else {
            return;
        };
        let here = c.tile();
        let carried = c.carrying.take();
        c.path.clear();
        c.target = None;
        c.progress = 0;
        let wait = u64::from(self.rules.jobs.retry_ticks);
        c.next_search = self.tick + wait;
        self.path_queue.retain(|r| r.colonist != id);
        self.jobs.dest_reservations.retain(|_, who| *who != id);
        let until = self.tick + wait;
        if let Some(job) = self.jobs.get_mut(active.job)
            && job.claimed_by == Some(id)
        {
            job.claimed_by = None;
            if block {
                job.blocked_until = until;
            }
        }
        if let Some(item) = active.item {
            if carried == Some(item) {
                let stack_max = self.rules.jobs.stack_max;
                if let Some(kept) = self.items.put_down(item, here.0, here.1, stack_max) {
                    self.items.set_to_haul(kept, true);
                    if active.kind == JobKind::Haul && !self.is_refuel_job(active.job) {
                        if kept != item {
                            self.jobs.remove(active.job);
                        } else if let Some(job) = self.jobs.get_mut(active.job) {
                            job.x = here.0;
                            job.y = here.1;
                        }
                    }
                    self.queue_haul(kept);
                }
            } else if self
                .items
                .get(item)
                .is_some_and(|i| i.reserved_by == Some(id))
            {
                self.items.set_reserved(item, None);
            }
        }
        self.job_events.push(JobEvent::Aborted {
            colonist: id,
            kind: active.kind,
            reason,
        });
    }

    // ---- checks ----

    /// Check the job state of a loaded `Sim`: all tiles are inside the map, stockpiles are
    /// valid, and the invariants hold. Call after the caches are rebuilt.
    pub(crate) fn jobs_state_ok(&self) -> bool {
        let inside = |x: u16, y: u16| self.map.in_bounds(i32::from(x), i32::from(y));
        let tiles_ok = self.designations.keys().all(|&(x, y)| inside(x, y))
            && self.jobs.iter().all(|(_, j)| inside(j.x, j.y))
            && self
                .jobs
                .dest_reservations()
                .all(|(&(x, y), _)| inside(x, y))
            && self.items.iter().all(|(_, i)| match i.loc {
                ItemLoc::Tile(x, y) => inside(x, y),
                ItemLoc::Carried(_) => true,
            })
            && self
                .stockpiles
                .iter()
                .all(|(_, s)| s.x0 <= s.x1 && s.y0 <= s.y1 && inside(s.x1, s.y1))
            && self.colonists.iter().all(|(_, c)| {
                c.job.is_none_or(|a| {
                    inside(a.stand.0, a.stand.1) && a.dest.is_none_or(|d| inside(d.0, d.1))
                })
            });
        tiles_ok && self.check_invariants().is_ok() && self.buildings_state_ok()
    }

    /// Check the job, item, and reservation invariants. Return the first problem.
    /// For tests and the soak tool.
    pub fn check_invariants(&self) -> Result<(), String> {
        let mut claimed_jobs = Vec::new();
        let mut reserved_items = Vec::new();
        let mut dest_tiles = Vec::new();
        for (cid, c) in self.colonists.iter() {
            if let Some(a) = c.job {
                if self
                    .jobs
                    .get(a.job)
                    .is_none_or(|j| j.claimed_by != Some(cid))
                {
                    return Err(format!(
                        "colonist {} has a job that is not claimed by it",
                        cid.index()
                    ));
                }
                claimed_jobs.push(a.job);
                if let Some(item) = a.item {
                    reserved_items.push(item);
                    if self
                        .items
                        .get(item)
                        .is_none_or(|i| i.reserved_by != Some(cid))
                    {
                        return Err(format!(
                            "item of colonist {} is not reserved by it",
                            cid.index()
                        ));
                    }
                }
                if let Some(dest) = a.dest
                    && a.kind == JobKind::Haul
                    && !self.is_refuel_job(a.job)
                {
                    dest_tiles.push(dest);
                    if self.jobs.dest_reserved_by(dest) != Some(cid) {
                        return Err(format!(
                            "destination of colonist {} is not reserved",
                            cid.index()
                        ));
                    }
                }
            }
            if let Some(NeedAct::Eat { item, .. } | NeedAct::Wear { item, .. }) = c.act {
                reserved_items.push(item);
                if c.job.is_some()
                    || self
                        .items
                        .get(item)
                        .is_none_or(|i| i.reserved_by != Some(cid))
                {
                    return Err(format!(
                        "need action of colonist {} has no reserved item",
                        cid.index()
                    ));
                }
            }
            if let Some(item) = c.carrying
                && self
                    .items
                    .get(item)
                    .is_none_or(|i| i.loc != ItemLoc::Carried(cid))
            {
                return Err(format!(
                    "colonist {} carries an item that is not in its hands",
                    cid.index()
                ));
            }
        }
        for list_len in [
            (claimed_jobs.len(), dedup_len(&claimed_jobs)),
            (reserved_items.len(), dedup_len(&reserved_items)),
            (dest_tiles.len(), dedup_len(&dest_tiles)),
        ] {
            if list_len.0 != list_len.1 {
                return Err("two colonists share a reservation".to_string());
            }
        }
        for (item_id, item) in self.items.iter() {
            if item.count == 0 {
                return Err("an item has count 0".to_string());
            }
            if let Some(who) = item.reserved_by
                && !reserved_items.contains(&item_id)
            {
                return Err(format!(
                    "item reserved by colonist {} without a job",
                    who.index()
                ));
            }
            match item.loc {
                ItemLoc::Tile(x, y) => {
                    if !self.items.at_tile(x, y).contains(&item_id) {
                        return Err("tile index misses an item".to_string());
                    }
                }
                ItemLoc::Carried(who) => {
                    if self
                        .colonists
                        .get(who)
                        .is_none_or(|c| c.carrying != Some(item_id))
                    {
                        return Err("a carried item has no carrier".to_string());
                    }
                }
            }
        }
        for (job_id, job) in self.jobs.iter() {
            if let Some(who) = job.claimed_by
                && self
                    .colonists
                    .get(who)
                    .and_then(|c| c.job)
                    .is_none_or(|a| a.job != job_id)
            {
                return Err("a claimed job has no colonist".to_string());
            }
        }
        for (tile, who) in self.jobs.dest_reservations() {
            if !dest_tiles.contains(tile) || self.colonists.get(*who).is_none() {
                return Err("destination reserved without a job".to_string());
            }
        }
        self.check_building_invariants()
    }
}

fn dedup_len<T: Ord + Clone>(list: &[T]) -> usize {
    let mut sorted = list.to_vec();
    sorted.sort();
    sorted.dedup();
    sorted.len()
}
