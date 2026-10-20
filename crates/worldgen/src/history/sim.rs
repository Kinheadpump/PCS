//! The yearly history loop. Integer math, fixed iteration order, stateless hash rolls.

use super::road;
use super::{
    Artifact, Civ, Event, EventKind, HISTORY_STAGE_COUNT, History, HistoryTables, LIMIT_COUNT,
    NEVER, NO_EVENT, Settlement, Site, SiteKind, hash_history,
};
use crate::biome::WorldBiome;
use crate::world::{WORLD_SIZE, WorldData, world_grid};
use base::content::{CultureDef, HistoryRules};
use base::rng::mix;
use std::collections::BTreeSet;

/// Kinds of rolls. Each kind makes an independent stream for a (year, id) pair.
#[derive(Clone, Copy)]
enum Roll {
    Expand = 1,
    ExpandPick,
    Mine,
    MinePick,
    MineCell,
    MineLife,
    Road,
    RoadPick,
    Battle,
    BattlePick,
    BattleWin,
    BattleCell,
    Seal,
    SealPick,
    SealThreat,
    ArtifactBattle,
    ArtifactRaze,
    ArtifactSeal,
    ArtifactKind,
    Collapse,
    NewCiv,
    NewCivPick,
    Culture,
    Drive,
    SettleCell,
    Drift,
}

/// An index of `History::skipped`.
#[derive(Clone, Copy)]
enum Limit {
    Civs,
    Settlements,
    Sites,
    Artifacts,
    Events,
}

/// A working mine.
#[derive(Clone, Copy)]
struct Mine {
    site: u32,
    civ: u32,
    region: u16,
    opened: u16,
    life: u16,
}

struct Sim<'a> {
    w: &'a WorldData,
    rules: &'a HistoryRules,
    cultures: &'a [CultureDef],
    seed: u64,
    civs: Vec<Civ>,
    settlements: Vec<Settlement>,
    sites: Vec<Site>,
    artifacts: Vec<Artifact>,
    events: Vec<Event>,
    road: Vec<u8>,
    depletion: Vec<u8>,
    sanctity: Vec<i8>,
    skipped: [u32; LIMIT_COUNT],
    /// Region index → eligible for starts, mines, and roads.
    eligible: Vec<bool>,
    /// Region index → all cells, ascending.
    cells: Vec<Vec<u32>>,
    /// Region index → cells where people live, ascending.
    habitable: Vec<Vec<u32>>,
    /// Region index → deposit cells, ascending.
    deposit_cells: Vec<Vec<u32>>,
    /// Region index → owner civ ID (0 for none).
    region_owner: Vec<u32>,
    /// Region index → standing settlement ID (0 for none).
    region_settlement: Vec<u32>,
    /// Region index → working mine site ID (0 for none).
    region_mine: Vec<u32>,
    /// Working mines, ascending by site ID.
    mines: Vec<Mine>,
    /// Pairs of regions with a road, as (low, high).
    linked: BTreeSet<(u16, u16)>,
    /// Civ index → standing settlements.
    civ_count: Vec<u32>,
    /// Civ index → unrest from lost settlements.
    unrest: Vec<u8>,
}

fn is_habitable(b: WorldBiome) -> bool {
    !matches!(
        b,
        WorldBiome::Ocean
            | WorldBiome::Lake
            | WorldBiome::Glacier
            | WorldBiome::HighMountain
            | WorldBiome::Volcanic
    )
}

fn clamp_u8(v: i32, lo: i32, hi: i32) -> u8 {
    v.clamp(lo, hi) as u8
}

fn to_u16(v: u32) -> u16 {
    u16::try_from(v).unwrap_or(u16::MAX)
}

/// Run the history of a world.
pub(super) fn run(w: &WorldData, tables: &HistoryTables, seed: u64, years: u32) -> History {
    let mut sim = Sim::new(w, tables, seed);
    sim.found_start_civs();
    let mut alive_by_year = Vec::with_capacity(years as usize);
    for year in 0..years {
        sim.year(year);
        alive_by_year.push(sim.alive().min(255) as u8);
    }
    sim.finish(years, alive_by_year)
}

impl<'a> Sim<'a> {
    fn new(w: &'a WorldData, tables: &'a HistoryTables, seed: u64) -> Self {
        let regions = &w.regions;
        let n = regions.count();
        let mut cells = vec![Vec::new(); n];
        let mut habitable = vec![Vec::new(); n];
        let mut deposit_cells = vec![Vec::new(); n];
        for (i, &id) in regions.id.as_slice().iter().enumerate() {
            let Some(r) = usize::from(id).checked_sub(1) else {
                continue;
            };
            cells[r].push(i as u32);
            if is_habitable(w.biome.as_slice()[i]) {
                habitable[r].push(i as u32);
            }
            if regions.deposits.mineral.as_slice()[i] != 0 {
                deposit_cells[r].push(i as u32);
            }
        }
        let min_cells = tables.rules.min_region_cells;
        let eligible = (0..n)
            .map(|r| regions.info[r].size >= min_cells && !habitable[r].is_empty())
            .collect();
        let size = (WORLD_SIZE * WORLD_SIZE) as usize;
        Self {
            w,
            rules: &tables.rules,
            cultures: &tables.cultures,
            seed,
            civs: Vec::new(),
            settlements: Vec::new(),
            sites: Vec::new(),
            artifacts: Vec::new(),
            events: Vec::new(),
            road: vec![0; size],
            depletion: vec![0; size],
            sanctity: vec![0; size],
            skipped: [0; LIMIT_COUNT],
            eligible,
            cells,
            habitable,
            deposit_cells,
            region_owner: vec![0; n],
            region_settlement: vec![0; n],
            region_mine: vec![0; n],
            mines: Vec::new(),
            linked: BTreeSet::new(),
            civ_count: Vec::new(),
            unrest: Vec::new(),
        }
    }

    // ----- rolls -----

    fn hash(&self, year: u32, id: u32, roll: Roll, salt: u32) -> u64 {
        let a = mix(self.seed ^ (u64::from(year) << 32 | u64::from(id)));
        mix(a ^ (roll as u64 + (u64::from(salt) << 8)).wrapping_mul(0x9E37_79B9_7F4A_7C15))
    }

    /// True with a chance of `pm` per-mille.
    fn chance(&self, year: u32, id: u32, roll: Roll, pm: u32) -> bool {
        self.hash(year, id, roll, 0) % 1000 < u64::from(pm)
    }

    /// An index below `n` (`n` must be above 0).
    fn pick(&self, year: u32, id: u32, roll: Roll, salt: u32, n: usize) -> usize {
        (self.hash(year, id, roll, salt) >> 16) as usize % n.max(1)
    }

    // ----- limits -----

    fn skip(&mut self, limit: Limit) {
        self.skipped[limit as usize] += 1;
    }

    /// Check the limits for `sites` new sites and `events` new events. Count a skip if not.
    fn room(&mut self, sites: usize, events: usize) -> bool {
        if self.sites.len() + sites > self.rules.max_sites as usize {
            self.skip(Limit::Sites);
            return false;
        }
        if self.events.len() + events > self.rules.max_events as usize {
            self.skip(Limit::Events);
            return false;
        }
        true
    }

    fn room_for_settlement(&mut self, extra_events: usize) -> bool {
        if self.settlements.len() >= self.rules.max_settlements as usize {
            self.skip(Limit::Settlements);
            return false;
        }
        self.room(1, 1 + extra_events)
    }

    // ----- records -----

    fn push_event(
        &mut self,
        year: u32,
        kind: EventKind,
        actors: [u32; 3],
        place: u32,
        extra: u32,
    ) -> u32 {
        let index = self.events.len() as u32;
        let year = to_u16(year);
        self.events.push(Event {
            year,
            kind,
            actors,
            place,
            extra,
        });
        index
    }

    fn civ(&self, id: u32) -> &Civ {
        &self.civs[id as usize - 1]
    }

    fn alive(&self) -> u32 {
        self.civs.iter().filter(|c| c.collapsed == NEVER).count() as u32
    }

    fn alive_ids(&self) -> Vec<u32> {
        let alive = self.civs.iter().filter(|c| c.collapsed == NEVER);
        alive.map(|c| c.id).collect()
    }

    fn push_site(
        &mut self,
        kind: SiteKind,
        civ: u32,
        region: u16,
        cell: u32,
        year: u32,
        event: u32,
    ) -> u32 {
        let id = self.sites.len() as u32 + 1;
        self.sites.push(Site {
            id,
            kind,
            civ,
            region,
            x: (cell % WORLD_SIZE) as u16,
            y: (cell / WORLD_SIZE) as u16,
            year: to_u16(year),
            event,
            ended: NEVER,
            cause: NO_EVENT,
            mineral: 0,
            level: 0,
        });
        id
    }

    // ----- overlays -----

    /// Raise the depletion of the cells around a cell. The center gets `strength`.
    fn stamp_depletion(&mut self, cell: u32, strength: u32) {
        let r = self.rules.depletion_radius as i32;
        let (cx, cy) = ((cell % WORLD_SIZE) as i32, (cell / WORLD_SIZE) as i32);
        for dy in -r..=r {
            for dx in -r..=r {
                let d = dx.abs().max(dy.abs()) as u32;
                let Some(i) = cell_index(cx + dx, cy + dy) else {
                    continue;
                };
                let v = (strength * (r as u32 + 1 - d) / (r as u32 + 1)).min(255) as u8;
                if self.depletion[i] < v {
                    self.depletion[i] = v;
                }
            }
        }
    }

    /// Add sanctity around a cell. The center gets `strength` (negative for a battlefield).
    fn stamp_sanctity(&mut self, cell: u32, radius: u32, strength: i32) {
        let r = radius as i32;
        let (cx, cy) = ((cell % WORLD_SIZE) as i32, (cell / WORLD_SIZE) as i32);
        for dy in -r..=r {
            for dx in -r..=r {
                let d = dx.abs().max(dy.abs());
                let Some(i) = cell_index(cx + dx, cy + dy) else {
                    continue;
                };
                let v = strength * (r + 1 - d) / (r + 1);
                self.sanctity[i] = (i32::from(self.sanctity[i]) + v).clamp(-127, 127) as i8;
            }
        }
    }

    // ----- civilizations and settlements -----

    /// Region indices that a new civilization can start in, ascending.
    fn free_regions(&self) -> Vec<usize> {
        let free = |r: &usize| self.eligible[*r] && self.region_owner[*r] == 0;
        (0..self.eligible.len()).filter(free).collect()
    }

    /// Found a settlement of `civ` in region `r`. The caller checked the limits.
    fn add_settlement(&mut self, year: u32, civ: u32, r: usize) {
        let list = &self.habitable[r];
        let count = self.settlements.len() as u32;
        let cell = list[self.pick(year, civ, Roll::SettleCell, count, list.len())];
        let region = r as u16 + 1;
        let sid = self.settlements.len() as u32 + 1;
        let ev = self.events.len() as u32;
        let site = self.push_site(SiteKind::Settlement, civ, region, cell, year, ev);
        self.settlements.push(Settlement {
            id: sid,
            civ,
            region,
            x: (cell % WORLD_SIZE) as u16,
            y: (cell / WORLD_SIZE) as u16,
            founded: to_u16(year),
            ended: NEVER,
            site,
        });
        self.push_event(
            year,
            EventKind::FoundSettlement,
            [civ, sid, 0],
            site,
            u32::from(region),
        );
        self.region_owner[r] = civ;
        self.region_settlement[r] = sid;
        self.civ_count[civ as usize - 1] += 1;
    }

    /// Found a civilization with its first settlement in region `r`.
    fn found_civ(&mut self, year: u32, r: usize) {
        if self.civs.len() >= self.rules.max_civs as usize {
            self.skip(Limit::Civs);
            return;
        }
        if !self.room_for_settlement(1) {
            return;
        }
        let id = self.civs.len() as u32 + 1;
        let pick = self.pick(year, id, Roll::Culture, 0, self.cultures.len());
        let culture = &self.cultures[pick];
        let jitter =
            |sim: &Self, salt: u32| (sim.hash(year, id, Roll::Drive, salt) % 31) as i32 - 15;
        let expansion = clamp_u8(i32::from(culture.expansion) + jitter(self, 0), 5, 100);
        let aggression = clamp_u8(i32::from(culture.aggression) + jitter(self, 1), 5, 100);
        let prosperity = clamp_u8(i32::from(culture.prosperity) + jitter(self, 2), 10, 100);
        let found_event = self.push_event(
            year,
            EventKind::FoundCiv,
            [id, 0, 0],
            r as u32 + 1,
            pick as u32,
        );
        self.civs.push(Civ {
            id,
            culture: pick as u8,
            founded: to_u16(year),
            collapsed: NEVER,
            capital: r as u16 + 1,
            expansion,
            aggression,
            prosperity,
            found_event,
            end_event: NO_EVENT,
        });
        self.civ_count.push(0);
        self.unrest.push(0);
        self.add_settlement(year, id, r);
    }

    fn found_start_civs(&mut self) {
        for i in 0..self.rules.start_civs {
            let free = self.free_regions();
            if free.is_empty() {
                break;
            }
            let n = self.civs.len() as u32 + 1;
            let r = free[self.pick(0, n, Roll::NewCivPick, i, free.len())];
            self.found_civ(0, r);
        }
    }

    /// Found a migrant civilization. It prefers a region with no owned neighbor.
    fn spawn_civ(&mut self, year: u32) {
        let free = self.free_regions();
        let n = self.civs.len() as u32 + 1;
        let apart: Vec<usize> = free
            .iter()
            .copied()
            .filter(|&r| {
                let links = self.w.regions.neighbors(r as u16 + 1);
                links
                    .iter()
                    .all(|l| self.region_owner[usize::from(l.to) - 1] == 0)
            })
            .collect();
        let list = if apart.is_empty() { &free } else { &apart };
        if list.is_empty() {
            return;
        }
        let r = list[self.pick(year, n, Roll::NewCivPick, 99, list.len())];
        self.found_civ(year, r);
    }

    /// Regions owned by a civilization, ascending.
    fn owned(&self, civ: u32) -> Vec<usize> {
        let mine = |r: &usize| self.region_owner[*r] == civ;
        (0..self.region_owner.len()).filter(mine).collect()
    }

    // ----- the yearly loop -----

    fn year(&mut self, year: u32) {
        if year > 0 && year.is_multiple_of(self.rules.heavy_interval) {
            self.heavy_update(year);
        }
        for civ in self.alive_ids() {
            self.expand(year, civ);
            self.open_mine(year, civ);
            self.build_road(year, civ);
            self.battle(year, civ);
            self.seal(year, civ);
            self.collapse_roll(year, civ);
        }
        self.deplete_mines(year);
        let alive = self.alive();
        if alive < self.rules.min_alive {
            for _ in alive..self.rules.min_alive {
                self.spawn_civ(year);
            }
        } else if alive < self.rules.spawn_below_alive {
            let id = self.civs.len() as u32 + 1;
            if self.chance(year, id, Roll::NewCiv, self.rules.new_civ_pm) {
                self.spawn_civ(year);
            }
        }
    }

    fn heavy_update(&mut self, year: u32) {
        let mines = self.mines.clone();
        for civ in self.alive_ids() {
            let i = civ as usize - 1;
            let count = self.civ_count[i] as i32;
            let working = mines.iter().filter(|m| m.civ == civ).count() as i32;
            let base = i32::from(self.cultures[usize::from(self.civs[i].culture)].prosperity);
            let target = base + count * 4 + working * 3 - i32::from(self.unrest[i]);
            let old = i32::from(self.civs[i].prosperity);
            self.civs[i].prosperity = clamp_u8((old * 3 + target) / 4, 5, 100);
            let drift =
                |sim: &Self, salt: u32| (sim.hash(year, civ, Roll::Drift, salt) % 7) as i32 - 3;
            let a = i32::from(self.civs[i].aggression) + drift(self, 0);
            let e = i32::from(self.civs[i].expansion) + drift(self, 1);
            self.civs[i].aggression = clamp_u8(a, 5, 100);
            self.civs[i].expansion = clamp_u8(e, 5, 100);
            self.unrest[i] = self.unrest[i].saturating_sub(5);
        }
    }

    fn expand(&mut self, year: u32, civ: u32) {
        let cap = self.rules.max_civ_settlements;
        let count = self.civ_count[civ as usize - 1];
        if count >= cap {
            return;
        }
        let drive = u32::from(self.civ(civ).expansion) + 20;
        let pm = self.rules.settle_pm * drive * (cap - count) / (120 * cap);
        if !self.chance(year, civ, Roll::Expand, pm) {
            return;
        }
        let mut frontier = BTreeSet::new();
        for r in self.owned(civ) {
            for link in self.w.regions.neighbors(r as u16 + 1) {
                let n = usize::from(link.to) - 1;
                if self.eligible[n] && self.region_owner[n] == 0 {
                    frontier.insert(n);
                }
            }
        }
        if frontier.is_empty() || !self.room_for_settlement(0) {
            return;
        }
        let list: Vec<usize> = frontier.into_iter().collect();
        let r = list[self.pick(year, civ, Roll::ExpandPick, 0, list.len())];
        self.add_settlement(year, civ, r);
    }

    fn open_mine(&mut self, year: u32, civ: u32) {
        let mining = u32::from(self.cultures[usize::from(self.civ(civ).culture)].mining) + 20;
        let pm = self.rules.mine_pm * mining / 120;
        if !self.chance(year, civ, Roll::Mine, pm) {
            return;
        }
        let ok = |sim: &Self, r: usize| {
            sim.region_mine[r] == 0
                && sim.deposit_cells[r]
                    .iter()
                    .any(|&c| sim.depletion[c as usize] < 100)
        };
        let list: Vec<usize> = self
            .owned(civ)
            .into_iter()
            .filter(|&r| ok(self, r))
            .collect();
        if list.is_empty() || !self.room(1, 1) {
            return;
        }
        let r = list[self.pick(year, civ, Roll::MinePick, 0, list.len())];
        let fresh: Vec<u32> = self.deposit_cells[r]
            .iter()
            .copied()
            .filter(|&c| self.depletion[c as usize] < 100)
            .collect();
        let cell = fresh[self.pick(year, civ, Roll::MineCell, 0, fresh.len())];
        let mineral = self.w.regions.deposits.mineral.as_slice()[cell as usize];
        let richness = self.w.regions.deposits.richness.as_slice()[cell as usize];
        let region = r as u16 + 1;
        let ev = self.events.len() as u32;
        let site = self.push_site(SiteKind::Mine, civ, region, cell, year, ev);
        let s = &mut self.sites[site as usize - 1];
        s.mineral = mineral;
        s.level = richness;
        let extra = u32::from(mineral) | u32::from(richness) << 8;
        self.push_event(year, EventKind::OpenMine, [civ, 0, 0], site, extra);
        let life = self.rules.mine_life_base
            + u32::from(richness) * self.rules.mine_life_per_richness
            + (self.hash(year, civ, Roll::MineLife, 0) % u64::from(self.rules.mine_life_jitter + 1))
                as u32;
        self.mines.push(Mine {
            site,
            civ,
            region,
            opened: to_u16(year),
            life: to_u16(life),
        });
        self.region_mine[r] = site;
    }

    /// Close a working mine. The cells around it keep a part of the depletion.
    fn close_mine(&mut self, index: usize, year: u32, full: bool, cause: u32) {
        let mine = self.mines.remove(index);
        let site = self.sites[mine.site as usize - 1];
        let worked = year.saturating_sub(u32::from(mine.opened));
        let strength = if full {
            255
        } else {
            (worked * 255 / u32::from(mine.life).max(1)).min(240)
        };
        self.stamp_depletion(u32::from(site.y) * WORLD_SIZE + u32::from(site.x), strength);
        let s = &mut self.sites[mine.site as usize - 1];
        s.ended = to_u16(year);
        s.cause = cause;
        self.region_mine[usize::from(mine.region) - 1] = 0;
    }

    fn close_mines_in(&mut self, region: u16, year: u32) {
        while let Some(i) = self.mines.iter().position(|m| m.region == region) {
            self.close_mine(i, year, false, NO_EVENT);
        }
    }

    fn deplete_mines(&mut self, year: u32) {
        let mut i = 0;
        while i < self.mines.len() {
            let m = self.mines[i];
            if u32::from(m.opened) + u32::from(m.life) > year || !self.room(0, 1) {
                i += 1;
                continue;
            }
            let ev = self.events.len() as u32;
            let level = u32::from(self.sites[m.site as usize - 1].level);
            self.push_event(year, EventKind::MineDepleted, [m.civ, 0, 0], m.site, level);
            self.close_mine(i, year, true, ev);
        }
    }

    fn build_road(&mut self, year: u32, civ: u32) {
        if self.civ_count[civ as usize - 1] < 2
            || !self.chance(year, civ, Roll::Road, self.rules.road_pm)
        {
            return;
        }
        let owned = self.owned(civ);
        let mut pairs = Vec::new();
        for &a in &owned {
            for link in self.w.regions.neighbors(a as u16 + 1) {
                let b = usize::from(link.to) - 1;
                let key = (a as u16 + 1, link.to);
                if b > a && self.region_owner[b] == civ && !self.linked.contains(&key) {
                    pairs.push(key);
                }
            }
        }
        if pairs.is_empty() || !self.room(0, 1) {
            return;
        }
        let (ra, rb) = pairs[self.pick(year, civ, Roll::RoadPick, 0, pairs.len())];
        self.linked.insert((ra, rb));
        let sa = self.settlements[self.region_settlement[usize::from(ra) - 1] as usize - 1];
        let sb = self.settlements[self.region_settlement[usize::from(rb) - 1] as usize - 1];
        let from = u32::from(sa.y) * WORLD_SIZE + u32::from(sa.x);
        let to = u32::from(sb.y) * WORLD_SIZE + u32::from(sb.x);
        let Some(path) = road::find_path(self.w, from, to, self.rules.road_margin) else {
            return;
        };
        for &c in &path {
            let v = &mut self.road[c as usize];
            *v = v.saturating_add(1);
        }
        self.push_event(
            year,
            EventKind::BuildRoad,
            [civ, sa.id, sb.id],
            u32::from(ra),
            path.len() as u32,
        );
    }

    fn lose_artifact(&mut self, year: u32, civ: u32, site: u32, roll: Roll, pm: u32) {
        if !self.chance(year, civ, roll, pm) {
            return;
        }
        if self.artifacts.len() >= self.rules.max_artifacts as usize {
            self.skip(Limit::Artifacts);
            return;
        }
        if !self.room(0, 1) {
            return;
        }
        let id = self.artifacts.len() as u32 + 1;
        let kind = (self.hash(year, civ, Roll::ArtifactKind, id) % 4) as u8;
        let event = self.events.len() as u32;
        self.artifacts.push(Artifact {
            id,
            civ,
            year: to_u16(year),
            site,
            kind,
            event,
        });
        self.push_event(
            year,
            EventKind::ArtifactLost,
            [civ, id, 0],
            site,
            u32::from(kind),
        );
    }

    fn battle(&mut self, year: u32, civ: u32) {
        let aggression = u32::from(self.civ(civ).aggression) + 20;
        let pm = self.rules.battle_pm * aggression / 120;
        if !self.chance(year, civ, Roll::Battle, pm) {
            return;
        }
        let mut fronts = Vec::new();
        for r in self.owned(civ) {
            for link in self.w.regions.neighbors(r as u16 + 1) {
                let n = usize::from(link.to) - 1;
                let owner = self.region_owner[n];
                if owner != 0 && owner != civ {
                    fronts.push(n);
                }
            }
        }
        if fronts.is_empty() || !self.room(1, 3) {
            return;
        }
        let n = fronts[self.pick(year, civ, Roll::BattlePick, 0, fronts.len())];
        let defender = self.region_owner[n];
        let list = &self.cells[n];
        let cell = list[self.pick(year, civ, Roll::BattleCell, 0, list.len())];
        let region = n as u16 + 1;
        let ev = self.events.len() as u32;
        let site = self.push_site(SiteKind::Battlefield, civ, region, cell, year, ev);
        self.push_event(
            year,
            EventKind::Battle,
            [civ, defender, 0],
            site,
            u32::from(region),
        );
        self.stamp_sanctity(cell, self.rules.battle_radius, -40);
        let a = self.civ(civ);
        let d = self.civ(defender);
        let win = self.rules.attack_win_pm as i32
            + (i32::from(a.aggression) - i32::from(d.aggression)) * 5
            + (i32::from(a.prosperity) - i32::from(d.prosperity)) * 2;
        let win = win.clamp(100, 900) as u64;
        let won = self.hash(year, civ, Roll::BattleWin, 0) % 1000 < win;
        let can_raze =
            self.civ_count[defender as usize - 1] > 1 || self.alive() > self.rules.min_alive;
        if won && can_raze {
            self.raze(year, civ, defender, n);
        } else {
            let i = civ as usize - 1;
            self.unrest[i] = self.unrest[i].saturating_add(5);
            let pm = self.rules.artifact_pm;
            self.lose_artifact(year, civ, site, Roll::ArtifactBattle, pm);
        }
    }

    fn raze(&mut self, year: u32, attacker: u32, defender: u32, n: usize) {
        let sid = self.region_settlement[n];
        let settlement = self.settlements[sid as usize - 1];
        let ev = self.push_event(
            year,
            EventKind::Raze,
            [attacker, defender, sid],
            settlement.site,
            u32::from(settlement.region),
        );
        self.settlements[sid as usize - 1].ended = to_u16(year);
        let site = &mut self.sites[settlement.site as usize - 1];
        site.kind = SiteKind::Ruin;
        site.ended = to_u16(year);
        site.cause = ev;
        self.region_owner[n] = 0;
        self.region_settlement[n] = 0;
        self.close_mines_in(settlement.region, year);
        let di = defender as usize - 1;
        self.civ_count[di] -= 1;
        self.unrest[di] = self.unrest[di].saturating_add(15);
        let pm = self.rules.artifact_pm;
        self.lose_artifact(year, attacker, settlement.site, Roll::ArtifactRaze, pm);
        if self.civ_count[di] == 0 {
            self.collapse(year, defender);
        }
    }

    fn seal(&mut self, year: u32, civ: u32) {
        if !self.chance(year, civ, Roll::Seal, self.rules.seal_pm) {
            return;
        }
        let working: Vec<usize> = (0..self.mines.len())
            .filter(|&i| self.mines[i].civ == civ)
            .collect();
        if working.is_empty() || !self.room(0, 2) {
            return;
        }
        let index = working[self.pick(year, civ, Roll::SealPick, 0, working.len())];
        let site_id = self.mines[index].site;
        let threat = 1 + (self.hash(year, civ, Roll::SealThreat, 0) % 3) as u8;
        let ev = self.events.len() as u32;
        self.push_event(
            year,
            EventKind::Seal,
            [civ, 0, 0],
            site_id,
            u32::from(threat),
        );
        self.close_mine(index, year, false, ev);
        let site = &mut self.sites[site_id as usize - 1];
        site.kind = SiteKind::Sealed;
        site.level = threat;
        let cell = u32::from(site.y) * WORLD_SIZE + u32::from(site.x);
        self.stamp_sanctity(cell, self.rules.seal_radius, 100);
        let pm = (self.rules.artifact_pm * 2).min(1000);
        self.lose_artifact(year, civ, site_id, Roll::ArtifactSeal, pm);
    }

    fn collapse_roll(&mut self, year: u32, civ: u32) {
        if self.civ(civ).collapsed != NEVER || self.alive() <= self.rules.min_alive {
            return;
        }
        let c = self.civ(civ);
        let poor = (100 - u32::from(c.prosperity)) * self.rules.collapse_poor_pm / 100;
        let crowd = if self.alive() > self.rules.max_alive {
            self.rules.collapse_crowd_pm
        } else {
            0
        };
        let pm = self.rules.collapse_pm + poor + crowd;
        if self.chance(year, civ, Roll::Collapse, pm) && self.room(0, 1) {
            self.collapse(year, civ);
        }
    }

    /// End a civilization. Its standing settlements become ruins.
    fn collapse(&mut self, year: u32, civ: u32) {
        let standing: Vec<u32> = self
            .settlements
            .iter()
            .filter(|s| s.civ == civ && s.ended == NEVER)
            .map(|s| s.id)
            .collect();
        let capital = u32::from(self.civ(civ).capital);
        let ev = self.push_event(
            year,
            EventKind::CivCollapse,
            [civ, 0, 0],
            capital,
            standing.len() as u32,
        );
        for sid in standing {
            let s = self.settlements[sid as usize - 1];
            self.settlements[sid as usize - 1].ended = to_u16(year);
            let site = &mut self.sites[s.site as usize - 1];
            site.kind = SiteKind::Ruin;
            site.ended = to_u16(year);
            site.cause = ev;
            let r = usize::from(s.region) - 1;
            self.region_owner[r] = 0;
            self.region_settlement[r] = 0;
            self.close_mines_in(s.region, year);
            self.civ_count[civ as usize - 1] -= 1;
        }
        let c = &mut self.civs[civ as usize - 1];
        c.collapsed = to_u16(year);
        c.end_event = ev;
    }

    fn finish(self, years: u32, alive_by_year: Vec<u8>) -> History {
        let regions = &self.w.regions;
        let owner = regions.id.as_slice().iter().map(|&id| {
            usize::from(id)
                .checked_sub(1)
                .map_or(0, |r| to_u16(self.region_owner[r]))
        });
        let mut history = History {
            seed: self.seed,
            years,
            hashes: [0; HISTORY_STAGE_COUNT],
            civs: self.civs,
            settlements: self.settlements,
            sites: self.sites,
            artifacts: self.artifacts,
            events: self.events,
            owner: world_grid(owner.collect(), 0),
            road: world_grid(self.road, 0),
            depletion: world_grid(self.depletion, 0),
            sanctity: world_grid(self.sanctity, 0),
            alive_by_year,
            skipped: self.skipped,
        };
        history.hashes = hash_history(&history);
        history
    }
}

/// Index of a cell, or `None` outside the world.
fn cell_index(x: i32, y: i32) -> Option<usize> {
    let size = WORLD_SIZE as i32;
    if x < 0 || y < 0 || x >= size || y >= size {
        return None;
    }
    Some((y * size + x) as usize)
}
