//! Colony simulation: state, tick, commands, events, save and load.
//!
//! `Sim` owns all colony state. The interface reads `&Sim` and sends [`Command`] values
//! to [`Sim::step`]. `Sim` sends [`Event`] values back through [`Sim::drain_events`].
//! This crate uses no floats, no hash collections, no file I/O, no time, and no threads
//! (TECHNICAL_DESIGN §7).

pub mod alert;
pub mod build;
pub mod chronicle;
pub mod climate;
pub mod colonist;
pub mod combat;
pub mod command;
pub mod creature;
pub mod disease;
pub mod drought;
pub mod event;
pub mod farm;
pub mod fire;
pub mod hearth;
pub mod item;
pub mod job;
pub mod ledger;
pub mod life;
pub mod map;
pub mod migrant;
pub mod needact;
pub mod needs;
pub mod notice;
pub mod path;
pub mod production;
pub mod raid;
pub mod region;
pub mod relations;
pub mod room;
pub mod rules;
pub mod save;
pub mod scenario;
pub mod skill;
pub mod spoilage;
pub mod start;
pub mod stockpile;
pub mod time;
pub mod wants;

mod loader;

pub use alert::{Alert, AlertKind, AlertLevel};
pub use build::{Blueprint, BuildLayer, Building, Buildings, WorkOrder};
pub use chronicle::{
    ChronicleKind, ChronicleRules, ChronicleSeason, ChronicleState, Entry, Legacy, RunInfo,
    RunState,
};
pub use climate::{
    ClimateParams, ClimateRules, DayWeather, FORECAST_DAYS, OutdoorParts, TempCause, TempPart,
    TempPlace, TileTemperature,
};
pub use colonist::{Colonist, ColonistId, Memory, NeedAct, Sex};
pub use combat::melee_damage;
pub use command::Command;
pub use creature::{
    CombatEvent, CombatReject, Creature, CreatureId, CreatureKind, CreatureRules, Mode, Role,
};
pub use disease::{
    DOCTOR_ARG, DiseaseEvent, DiseaseKind, Illness, IllnessCause, IllnessPart, IllnessReport,
};
pub use drought::{DIG_ARG, DroughtEvent, Fire, Growth, GrowthCause, GrowthPart};
pub use event::Event;
pub use farm::{
    CropDef, FarmRules, FarmState, FertilityCause, FertilityPart, FertilityReport, Plant,
    PlantReport, WildDef, WildStatus,
};
pub use fire::EXTINGUISH_ARG;
pub use hearth::REFUEL_ARG;
pub use item::{Item, ItemId, ItemLoc, Items};
pub use job::{AbortReason, ActiveJob, Job, JobBoard, JobEvent, JobId, JobKind, JobStep, Reject};
pub use ledger::{Ledger, LedgerCause, LedgerLine, LedgerPart, LedgerRules, LedgerState};
pub use life::{
    ComfortBand, ComfortCause, ComfortPart, Condition, DeathCause, EffectiveSpeed, Grave,
    HealthCause, HealthPart, HealthReport, LifeEvent, Mood, MoodCause, MoodPart, SpeedMod,
    SpeedModCause,
};
pub use map::LocalMap;
pub use migrant::{MigrantBlocker, MigrantEvent, MigrantOffer, MigrantProfile, MigrantRules};
pub use needs::{ClothesDef, FoodDef, MemoryDef, NeedRules, TraitDef};
pub use notice::{NoticeCause, NoticeEvent, NoticePart, NoticeReport, NoticeSource, Threat};
pub use path::PathFinder;
pub use raid::{
    Hostility, HostilityCause, HostilityPart, Raid, RaidEvent, RaidOutcome, RaidPhase, RaidRecord,
    RaidReject, RaidWarning, RivalColony, TributeGood,
};
pub use region::Regions;
pub use relations::{
    OpinionCause, OpinionPart, RelationEvent, RelationReport, RelationRules, Standing,
};
pub use room::{
    BeautyCause, BeautyPart, BorderKind, BorderTile, InsulationPart, Room, RoomBeauty,
    RoomInsulation, Rooms,
};
pub use rules::SimRules;
pub use save::SaveError;
pub use skill::{Skill, SpeedCause, SpeedPart, WorkSpeed};
pub use spoilage::{RotCause, RotPart, RotReport, SpoilRules};
pub use start::StartRules;
pub use stockpile::{ItemFilter, Stockpile, StockpileId};
pub use time::{GameTime, Season, Speed};
pub use wants::{GoalKind, GoalReport, Social, WantRules, Wants};

use base::hash::fnv1a;
use base::{Arena, Pcg32};
use colonist::{START_COLONISTS, WALK_PROGRESS_PER_TICK, step_progress_needed};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, VecDeque};
use std::sync::Arc;
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
    /// All item stacks.
    items: Items,
    /// The jobs and their reservations.
    jobs: JobBoard,
    /// Cut and mine designations, by tile.
    designations: BTreeMap<(u16, u16), JobKind>,
    /// Stockpile zones.
    stockpiles: Arena<Stockpile>,
    /// Buildings and blueprints.
    buildings: Buildings,
    /// Climate numbers, weather, and ice (Sprint 5).
    climate: climate::ClimateState,
    /// Room temperatures in thousandths of a degree. The key is the first tile of the room.
    room_temps: BTreeMap<(u16, u16), i32>,
    /// The dead, oldest first (R-016).
    graveyard: Vec<life::Grave>,
    /// Crops, farm zones, and wild plants (Sprint 7).
    farm: FarmState,
    /// "Keep N" orders by workshop tile and recipe.
    keep: BTreeMap<(u16, u16, u16), u32>,
    /// The ledger history.
    ledger: LedgerState,
    /// The chronicle.
    chronicle: ChronicleState,
    /// The run: its state and the end numbers (R-050).
    run: RunInfo,
    /// Noise and site disturbance by tile (S14).
    notice: notice::NoticeState,
    /// Dormant threats of the sealed sites (S14).
    threats: Vec<Threat>,
    /// Dry days, ditches, and fires (S16).
    drought: drought::DroughtState,
    /// Diseases, treatments, and sick rooms (S17).
    disease: disease::DiseaseState,
    /// Rival colonies and raids (S18).
    raid: raid::RaidState,
    /// Migrant offer and counts (S15).
    migrants: migrant::MigrantState,
    /// Creatures, lairs, and kill counts (S13).
    creature: creature::CreatureState,
    /// Relations between colonists (S21).
    relation_state: relations::RelationState,
    /// Events for the caller. Not part of the state.
    #[serde(skip)]
    events: Vec<Event>,
    /// Standing changes for the caller. Not part of the state.
    #[serde(skip)]
    relation_events: Vec<RelationEvent>,
    /// Life events for the caller. Not part of the state.
    #[serde(skip)]
    life_events: Vec<LifeEvent>,
    /// Notice events for the caller. Not part of the state.
    #[serde(skip)]
    notice_events: Vec<notice::NoticeEvent>,
    /// Drought and fire events for the caller. Not part of the state.
    #[serde(skip)]
    drought_events: Vec<DroughtEvent>,
    /// Disease events for the caller. Not part of the state.
    #[serde(skip)]
    disease_events: Vec<DiseaseEvent>,
    /// Raid events for the caller. Not part of the state.
    #[serde(skip)]
    raid_events: Vec<RaidEvent>,
    /// Migrant events for the caller. Not part of the state.
    #[serde(skip)]
    migrant_events: Vec<MigrantEvent>,
    /// Combat and creature events for the caller. Not part of the state.
    #[serde(skip)]
    combat_events: Vec<CombatEvent>,
    /// True: the needs system is off. Tests only. Not part of the state.
    #[serde(skip)]
    needs_off: bool,
    /// Job events for the caller. Not part of the state.
    #[serde(skip)]
    job_events: Vec<JobEvent>,
    /// Content for the job systems. Not part of the state.
    #[serde(skip)]
    rules: Arc<SimRules>,
    /// Derived cache: fertility and wild plants.
    #[serde(skip)]
    farm_cache: farm::FarmCache,
    /// Derived cache: connectivity regions.
    #[serde(skip)]
    regions: Regions,
    /// Derived cache: the rooms.
    #[serde(skip)]
    rooms: Rooms,
    /// A structural change waits for the room detection in the next tick. Derived.
    #[serde(skip)]
    rooms_dirty: bool,
    /// How many times the room detection ran. Not part of the state.
    #[serde(skip)]
    room_detections: u64,
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
        let mut sim = Self::from_map(map, seed, content_hash, &spawns);
        sim.threats = notice::threats_of(&data.placed, &data.site);
        sim.raid = raid::RaidState::with_rivals(seed, &data.rivals);
        sim.roll_colonists(seed);
        sim.roll_start_skills(seed);
        sim.roll_all_wants(seed);
        sim.place_start_stock();
        sim.creature_start();
        sim
    }

    /// Start a colony on a map with colonists on the given tiles. Tiles that are not
    /// passable are skipped.
    pub fn from_map(map: LocalMap, seed: u64, content_hash: u64, spawns: &[(u16, u16)]) -> Self {
        let rules = SimRules::builtin();
        let mut colonists = Arena::new();
        for &(x, y) in spawns {
            if map.passable(i32::from(x), i32::from(y)) {
                let mut colonist = Colonist::new(x, y);
                colonist.priorities = [rules.jobs.default_priority; JobKind::COUNT];
                rules.needs.init_colonist(&mut colonist);
                colonists.insert(colonist);
            }
        }
        let mut sim = Self {
            content_hash,
            tick: 0,
            rng: Pcg32::from_seed(seed),
            map,
            colonists,
            path_queue: VecDeque::new(),
            items: Items::default(),
            jobs: JobBoard::default(),
            designations: BTreeMap::new(),
            stockpiles: Arena::new(),
            buildings: Buildings::default(),
            climate: climate::ClimateState::new(&rules.climate, seed),
            room_temps: BTreeMap::new(),
            graveyard: Vec::new(),
            farm: FarmState::with_seed(base::rng::mix(seed ^ 0x4641_524d_5345_4544)),
            keep: BTreeMap::new(),
            ledger: LedgerState::default(),
            chronicle: ChronicleState::default(),
            run: RunInfo::default(),
            notice: notice::NoticeState::default(),
            threats: Vec::new(),
            drought: drought::DroughtState::default(),
            disease: disease::DiseaseState::new(seed),
            disease_events: Vec::new(),
            raid: raid::RaidState::new(seed),
            raid_events: Vec::new(),
            migrants: migrant::MigrantState::new(seed),
            migrant_events: Vec::new(),
            creature: creature::CreatureState::new(seed),
            relation_state: relations::RelationState::new(seed),
            relation_events: Vec::new(),
            combat_events: Vec::new(),
            notice_events: Vec::new(),
            drought_events: Vec::new(),
            events: Vec::new(),
            life_events: Vec::new(),
            needs_off: false,
            job_events: Vec::new(),
            rules,
            farm_cache: farm::FarmCache::default(),
            rooms: Rooms::default(),
            rooms_dirty: false,
            room_detections: 0,
            regions: Regions::default(),
            finder: PathFinder::default(),
            scratch_path: Vec::new(),
            path_stats: PathStats::default(),
        };
        sim.rebuild_caches();
        sim.refresh_rooms();
        sim
    }

    /// Rebuild the derived caches except the rooms. Call after a load.
    pub(crate) fn rebuild_caches(&mut self) {
        let mut blocking = self
            .buildings
            .object_tiles(|def| self.rules.building(def).is_some_and(|d| d.blocks_walking));
        blocking.extend(self.drought.ditches.iter().copied());
        self.map.reset_solid(blocking.into_iter());
        self.map
            .rebuild_deep(u32::from(self.rules.climate.deep_water_distance));
        self.map.set_ice(self.climate.ice);
        self.regions.rebuild(&self.map);
        self.items.rebuild_index();
        self.rebuild_ditches();
        self.rebuild_farm_cache();
    }

    /// Replace the content rules. For tests that use their own content. Call before the
    /// first tick; the buildings must fit the new rules.
    pub fn debug_set_rules(&mut self, rules: Arc<SimRules>) {
        self.rules = rules;
        self.resync_weather();
        self.rebuild_caches();
        self.refresh_rooms();
    }

    /// Run one tick. Order: room detection (only after a structural change), commands, jobs,
    /// combat, path requests, creatures, movement, the tick number increases, then the climate
    /// (each game hour).
    pub fn step(&mut self, commands: &[Command]) {
        self.detect_rooms();
        for command in commands {
            self.apply_command(command);
        }
        self.update_needs();
        self.update_jobs();
        self.combat_tick();
        self.process_path_queue();
        self.creature_tick();
        self.move_colonists();
        self.tick += 1;
        self.climate_tick();
        self.life_tick();
        self.spoil_tick();
        self.farm_tick();
        self.production_tick();
        self.colony_daily_tick();
        self.notice_tick();
        self.drought_tick();
        self.disease_tick();
        self.raid_tick();
        self.migrant_tick();
    }

    /// Take the job events that the last ticks made.
    pub fn drain_job_events(&mut self) -> Vec<JobEvent> {
        std::mem::take(&mut self.job_events)
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
        self.rooms_dirty = true;
        self.passability_changed(x, y);
        true
    }

    /// Passability of a tile changed: rebuild the regions and ask a new path for each
    /// colonist whose path contains the tile.
    pub(crate) fn passability_changed(&mut self, x: u16, y: u16) {
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
            _ => self.apply_job_command(command),
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

    /// All item stacks.
    pub fn items(&self) -> &Items {
        &self.items
    }

    /// The job board.
    pub fn jobs(&self) -> &JobBoard {
        &self.jobs
    }

    /// The cut and mine designation on a tile, if any.
    pub fn designation(&self, x: u16, y: u16) -> Option<JobKind> {
        self.designations.get(&(x, y)).copied()
    }

    /// All cut and mine designations, in `(x, y)` order.
    pub fn designations(&self) -> impl Iterator<Item = ((u16, u16), JobKind)> + '_ {
        self.designations.iter().map(|(&t, &k)| (t, k))
    }

    /// All stockpile zones, in ID order.
    pub fn stockpiles(&self) -> &Arena<Stockpile> {
        &self.stockpiles
    }

    /// The content that the job systems use.
    pub fn rules(&self) -> &SimRules {
        &self.rules
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
        if !sim.buildings_state_ok()
            || !sim.drought_state_ok()
            || !sim.disease_state_ok()
            || !sim.raid_state_ok()
            || !sim.migrant_state_ok()
            || !sim.creature_state_ok()
        {
            return None;
        }
        sim.rebuild_caches();
        if !sim.climate_state_ok() {
            return None;
        }
        if !sim.life_state_ok() || !sim.jobs_state_ok() {
            return None;
        }
        if !sim.farm_state_ok() || !sim.keep_state_ok() || !sim.chronicle_state_ok() {
            return None;
        }
        sim.refresh_rooms();
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
