//! Colony simulation: state, tick, commands, events, save and load.
//!
//! `Sim` owns all colony state. The interface reads `&Sim` and sends [`Command`] values
//! to [`Sim::step`]. `Sim` sends [`Event`] values back through [`Sim::drain_events`].
//! This crate uses no floats, no hash collections, no file I/O, no time, and no threads
//! (TECHNICAL_DESIGN §7).

pub mod colonist;
pub mod command;
pub mod event;
pub mod map;
pub mod path;
pub mod region;
pub mod save;
pub mod scenario;
pub mod time;

pub use colonist::{Colonist, ColonistId};
pub use command::Command;
pub use event::Event;
pub use map::LocalMap;
pub use path::PathFinder;
pub use region::Regions;
pub use save::SaveError;
pub use time::{GameTime, Season, Speed};

use base::hash::fnv1a;
use base::{Arena, Pcg32};
use colonist::{START_COLONISTS, WALK_PROGRESS_PER_TICK, step_progress_needed};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use worldgen::LocalMapData;

/// Path request budget for each tick: maximum searches (TECHNICAL_DESIGN §6.3).
pub const PATH_BUDGET_SEARCHES: u32 = 20;
/// Path request budget for each tick: maximum expanded nodes.
pub const PATH_BUDGET_NODES: u32 = 50_000;

/// A queued request for a path from a colonist to its target.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct PathRequest {
    colonist: ColonistId,
    goal: (u16, u16),
}

/// Work that the path system did in the last tick. For the debug overlay and benchmarks.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PathStats {
    /// Searches done.
    pub searches: u32,
    /// Nodes expanded.
    pub expanded: u32,
}

/// The colony simulation state.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Sim {
    /// Hash of the content that the colony uses. The save header holds it.
    content_hash: u64,
    tick: u64,
    rng: Pcg32,
    map: LocalMap,
    colonists: Arena<Colonist>,
    path_queue: VecDeque<PathRequest>,
    /// Events for the caller. Not part of the state.
    #[serde(skip)]
    events: Vec<Event>,
    /// Derived cache: connectivity regions.
    #[serde(skip)]
    regions: Regions,
    /// Derived cache: A* buffers.
    #[serde(skip)]
    finder: PathFinder,
    /// Derived cache: scratch list of path results.
    #[serde(skip)]
    scratch_path: Vec<(u16, u16)>,
    /// Work of the last tick. Not part of the state.
    #[serde(skip)]
    path_stats: PathStats,
}

impl Sim {
    /// Start a colony on a generated local map. `seed` starts the gameplay random
    /// stream. `content_hash` identifies the content (use 0 if there is none).
    /// The colony starts with 7 colonists on passable tiles near the map center.
    pub fn new(data: &LocalMapData, seed: u64, content_hash: u64) -> Self {
        let map = LocalMap::from_data(data);
        let regions = Regions::build(&map);
        let spawns = center_spawns(&map, &regions, START_COLONISTS);
        Self::from_map(map, seed, content_hash, &spawns)
    }

    /// Start a colony on a map with colonists on the given tiles. Tiles that are not
    /// passable are skipped.
    pub fn from_map(map: LocalMap, seed: u64, content_hash: u64, spawns: &[(u16, u16)]) -> Self {
        let mut colonists = Arena::new();
        for &(x, y) in spawns {
            if map.passable(i32::from(x), i32::from(y)) {
                colonists.insert(Colonist::new(x, y));
            }
        }
        let mut sim = Self {
            content_hash,
            tick: 0,
            rng: Pcg32::from_seed(seed),
            map,
            colonists,
            path_queue: VecDeque::new(),
            events: Vec::new(),
            regions: Regions::default(),
            finder: PathFinder::default(),
            scratch_path: Vec::new(),
            path_stats: PathStats::default(),
        };
        sim.rebuild_caches();
        sim
    }

    /// Rebuild all derived caches. Call after a load.
    fn rebuild_caches(&mut self) {
        self.map.rebuild_passable();
        self.regions.rebuild(&self.map);
    }

    /// Run one tick. Order: commands, path requests, movement, then the tick number
    /// increases.
    pub fn step(&mut self, commands: &[Command]) {
        for command in commands {
            self.apply_command(command);
        }
        self.process_path_queue();
        self.move_colonists();
        self.tick += 1;
    }

    /// Take the events that the last ticks made.
    pub fn drain_events(&mut self) -> Vec<Event> {
        std::mem::take(&mut self.events)
    }

    /// Put a wall on a tile or remove it. Update the regions, and ask a new path for each
    /// colonist whose path contains the tile. Return true if passability changed.
    pub fn set_wall(&mut self, x: u16, y: u16, wall: bool) -> bool {
        if !self.map.set_wall(i32::from(x), i32::from(y), wall) {
            return false;
        }
        self.regions.rebuild(&self.map);
        let affected: Vec<ColonistId> = self
            .colonists
            .iter()
            .filter(|(_, c)| c.path.contains(&(x, y)))
            .map(|(id, _)| id)
            .collect();
        for id in affected {
            if let Some(c) = self.colonists.get_mut(id)
                && let Some(goal) = c.target
            {
                self.path_queue.retain(|r| r.colonist != id);
                self.path_queue
                    .push_back(PathRequest { colonist: id, goal });
            }
        }
        true
    }

    fn apply_command(&mut self, command: &Command) {
        match *command {
            Command::MoveColonist { id, x, y } => {
                let Some(c) = self.colonists.get(id) else {
                    return;
                };
                let (sx, sy) = (i32::from(c.x), i32::from(c.y));
                let (gx, gy) = (i32::from(x), i32::from(y));
                if !self.regions.connected((sx, sy), (gx, gy)) {
                    self.events.push(Event::PathFailed { colonist: id, x, y });
                    return;
                }
                self.path_queue.retain(|r| r.colonist != id);
                self.path_queue.push_back(PathRequest {
                    colonist: id,
                    goal: (x, y),
                });
                if let Some(c) = self.colonists.get_mut(id) {
                    c.target = Some((x, y));
                }
            }
        }
    }

    /// Answer path requests in queue order until the budget is used.
    fn process_path_queue(&mut self) {
        let mut stats = PathStats::default();
        while stats.searches < PATH_BUDGET_SEARCHES && stats.expanded < PATH_BUDGET_NODES {
            let Some(req) = self.path_queue.pop_front() else {
                break;
            };
            let Some(c) = self.colonists.get_mut(req.colonist) else {
                continue;
            };
            if c.path.is_empty() {
                c.progress = 0;
            }
            // A colonist in the middle of a step finishes the step first.
            let carried = if c.progress > 0 {
                c.path.last().copied()
            } else {
                None
            };
            let start = carried.unwrap_or_else(|| c.tile());
            let result = self
                .finder
                .find(&self.map, start, req.goal, &mut self.scratch_path);
            stats.searches += 1;
            stats.expanded += result.expanded;
            if !result.found {
                c.path.clear();
                c.progress = 0;
                c.target = None;
                self.events.push(Event::PathFailed {
                    colonist: req.colonist,
                    x: req.goal.0,
                    y: req.goal.1,
                });
                continue;
            }
            c.path.clear();
            c.path.extend_from_slice(&self.scratch_path);
            if let Some(tile) = carried {
                c.path.push(tile);
            }
            c.target = Some(req.goal);
            if c.path.is_empty() {
                c.target = None;
                self.events.push(Event::ColonistArrived {
                    colonist: req.colonist,
                    x: c.x,
                    y: c.y,
                });
            }
        }
        self.path_stats = stats;
    }

    /// Move each colonist along its path, in slot order.
    fn move_colonists(&mut self) {
        let ids: Vec<ColonistId> = self.colonists.iter().map(|(id, _)| id).collect();
        for id in ids {
            let Some(c) = self.colonists.get_mut(id) else {
                continue;
            };
            let Some(&next) = c.path.last() else {
                continue;
            };
            if !self.map.passable(i32::from(next.0), i32::from(next.1)) {
                c.path.clear();
                c.progress = 0;
                if let Some(goal) = c.target {
                    self.path_queue.retain(|r| r.colonist != id);
                    self.path_queue
                        .push_back(PathRequest { colonist: id, goal });
                }
                continue;
            }
            c.progress = c.progress.saturating_add(WALK_PROGRESS_PER_TICK);
            let need = step_progress_needed(c.tile(), next);
            if c.progress >= need {
                c.progress -= need;
                c.x = next.0;
                c.y = next.1;
                c.path.pop();
                if c.path.is_empty() {
                    c.progress = 0;
                    c.target = None;
                    self.events.push(Event::ColonistArrived {
                        colonist: id,
                        x: c.x,
                        y: c.y,
                    });
                }
            }
        }
    }

    /// The tick number. It is 0 at the start and increases by 1 in each `step`.
    pub fn tick(&self) -> u64 {
        self.tick
    }

    /// The tick number as calendar time.
    pub fn time(&self) -> GameTime {
        GameTime::from_tick(self.tick)
    }

    /// The local map.
    pub fn map(&self) -> &LocalMap {
        &self.map
    }

    /// All colonists, in slot order.
    pub fn colonists(&self) -> &Arena<Colonist> {
        &self.colonists
    }

    /// One colonist.
    pub fn colonist(&self, id: ColonistId) -> Option<&Colonist> {
        self.colonists.get(id)
    }

    /// The connectivity regions.
    pub fn regions(&self) -> &Regions {
        &self.regions
    }

    /// Hash of the content that this colony was made with.
    pub fn content_hash(&self) -> u64 {
        self.content_hash
    }

    /// Number of path requests that wait in the queue.
    pub fn path_queue_len(&self) -> usize {
        self.path_queue.len()
    }

    /// Work of the path system in the last tick.
    pub fn path_stats(&self) -> PathStats {
        self.path_stats
    }

    /// Stable 64-bit hash of the state: FNV-1a over the `postcard` bytes.
    pub fn state_hash(&self) -> u64 {
        fnv1a(&self.encode_state())
    }

    /// The `postcard` bytes of the state. Derived caches and events are not included.
    pub(crate) fn encode_state(&self) -> Vec<u8> {
        // Writing to a `Vec` cannot fail for this state; an empty result would only
        // make the hash and the save fail their own checks.
        postcard::to_allocvec(self).unwrap_or_default()
    }

    /// Read a state from `postcard` bytes. Check the data, then rebuild the caches.
    /// Return `None` if the bytes are not a valid state.
    pub(crate) fn decode_state(bytes: &[u8]) -> Option<Self> {
        let (mut sim, rest) = postcard::take_from_bytes::<Sim>(bytes).ok()?;
        if !rest.is_empty() || !sim.map.is_valid() {
            return None;
        }
        let inside = |t: (u16, u16)| sim.map.in_bounds(i32::from(t.0), i32::from(t.1));
        let colonists_ok = sim.colonists.iter().all(|(_, c)| {
            inside(c.tile()) && c.path.iter().all(|&t| inside(t)) && c.target.is_none_or(inside)
        });
        let queue_ok = sim.path_queue.iter().all(|r| inside(r.goal));
        if !colonists_ok || !queue_ok {
            return None;
        }
        sim.rebuild_caches();
        Some(sim)
    }
}

/// Pick `count` passable tiles near the map center, all in the region of the first one.
/// Search in rings of growing size; inside a ring, in row-major order.
fn center_spawns(map: &LocalMap, regions: &Regions, count: usize) -> Vec<(u16, u16)> {
    let (cx, cy) = (map.width() as i32 / 2, map.height() as i32 / 2);
    let max_ring = map.width().max(map.height()) as i32;
    let mut found = Vec::new();
    let mut region = 0;
    for ring in 0..=max_ring {
        for y in cy - ring..=cy + ring {
            for x in cx - ring..=cx + ring {
                if (x - cx).abs().max((y - cy).abs()) != ring || !map.passable(x, y) {
                    continue;
                }
                let id = regions.id_at(x, y);
                if region == 0 {
                    region = id;
                }
                if id == region {
                    found.push((x as u16, y as u16));
                    if found.len() == count {
                        return found;
                    }
                }
            }
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scenario::sim_from_ascii;

    #[test]
    fn new_colony_has_seven_colonists_near_the_center_in_one_region() {
        let data = worldgen::generate_local(&worldgen::EmbarkSummary::temperate_forest());
        let sim = Sim::new(&data, 1, 0);
        assert_eq!(sim.colonists().len(), 7);
        let regions = sim.regions();
        let mut ids = Vec::new();
        for (_, c) in sim.colonists().iter() {
            assert!(sim.map().passable(i32::from(c.x), i32::from(c.y)));
            assert!((i32::from(c.x) - 128).abs() < 20 && (i32::from(c.y) - 128).abs() < 20);
            ids.push(regions.id_at(i32::from(c.x), i32::from(c.y)));
        }
        assert!(ids.iter().all(|&i| i == ids[0] && i != 0));
        let again = Sim::new(&data, 1, 0);
        assert_eq!(sim.state_hash(), again.state_hash());
    }

    #[test]
    fn tick_counts_and_time_follows() {
        let mut sim = sim_from_ascii("c..", 1).unwrap();
        for _ in 0..2_450 {
            sim.step(&[]);
        }
        assert_eq!(sim.tick(), 2_450);
        let t = sim.time();
        assert_eq!((t.day_of_year, t.hour), (2, 0));
    }

    #[test]
    fn wall_change_gives_a_new_path_to_a_walking_colonist() {
        let mut sim = sim_from_ascii("c.....\n......\n......", 1).unwrap();
        let id = sim.colonists().iter().next().unwrap().0;
        sim.step(&[Command::MoveColonist { id, x: 5, y: 0 }]);
        assert!(sim.colonist(id).unwrap().path.contains(&(3, 0)));
        assert!(sim.set_wall(3, 0, true));
        for _ in 0..200 {
            sim.step(&[]);
        }
        let c = sim.colonist(id).unwrap();
        assert_eq!(c.tile(), (5, 0));
        assert!(!sim.set_wall(3, 0, true), "no change the second time");
    }
}
