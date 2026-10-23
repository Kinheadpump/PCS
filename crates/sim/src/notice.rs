//! Notice and dormant threats (S14, R-030, R-031).
//!
//! Notice is the sum of named contributions (wealth, population, noise, site disturbance). Each
//! contribution has a tile for the Notice overlay. Noise and disturbance are kept for each tile.
//! Each game day they decay and the sim rolls the chance of a beast. A dormant threat sits
//! behind the seal tiles of a sealed site and wakes when a colonist mines one of them. The
//! creatures of the beasts and threats come in a later sprint: this module gives the rolls, the
//! events, and the strength numbers.

use crate::Sim;
use crate::time::TICKS_PER_DAY;
use base::Grid;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use worldgen::history::SiteKind;
use worldgen::{PlacedSite, SiteTile};

/// What a Notice contribution is made of.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoticeCause {
    /// Value of the items and the buildings.
    Wealth,
    /// The colonists.
    Population,
    /// Completed mining and construction jobs, on their tiles.
    Noise,
    /// Excavation, relics, and opened seals, on their tiles.
    Disturbance,
}

/// One contribution to Notice with its tile (R-030).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NoticePart {
    /// What it is.
    pub cause: NoticeCause,
    /// Notice that it gives.
    pub value: u32,
    /// Tile column for the overlay.
    pub x: u16,
    /// Tile row for the overlay.
    pub y: u16,
}

/// The Notice of the colony and its parts. `total` is the sum of the parts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NoticeReport {
    /// Sum of the part values.
    pub total: u32,
    /// The parts: wealth, population, then noise and disturbance in `(y, x)` order.
    pub parts: Vec<NoticePart>,
}

/// An action that disturbs a site (R-030). The value is in `notice.ron`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoticeSource {
    /// A colonist excavates a site.
    Excavation,
    /// A colonist takes a relic from a site.
    Relic,
}

/// A thing that happened with Notice. The caller takes it with `Sim::drain_notice_events`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoticeEvent {
    /// The daily roll for a beast (R-031). The sim does not make the beast yet: a later system
    /// reads `arrived`.
    BeastRoll {
        /// Game day number.
        day: u64,
        /// Notice at the roll.
        notice: u32,
        /// Chance of a beast in percent.
        chance_pct: u32,
        /// True if a beast arrives.
        arrived: bool,
    },
    /// A colonist opened a sealed site and woke its threat.
    ThreatWoke {
        /// Site ID.
        site: u32,
        /// Tile where the seal opened.
        x: u16,
        /// Tile where the seal opened.
        y: u16,
        /// Base level of the threat.
        level: u8,
    },
}

/// A dormant threat of a sealed site.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Threat {
    /// Site ID in the history.
    pub site: u32,
    /// Site center column.
    pub x: u16,
    /// Site center row.
    pub y: u16,
    /// Base strength level (1 to 3).
    pub level: u8,
    /// Seal tiles of the site. Mining one of them wakes the threat. Sorted.
    pub seals: Vec<(u16, u16)>,
    /// True after a colonist opened a seal.
    pub awake: bool,
}

/// Noise and disturbance, by tile. Part of the state.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct NoticeState {
    noise: BTreeMap<(u16, u16), u32>,
    disturbance: BTreeMap<(u16, u16), u32>,
}

/// The threats of a generated map: one for each sealed site. Each seal tile belongs to the
/// nearest sealed site (the first one if two are as near).
pub(crate) fn threats_of(placed: &[PlacedSite], site: &Grid<SiteTile>) -> Vec<Threat> {
    let mut threats: Vec<Threat> = placed
        .iter()
        .filter(|p| p.kind == SiteKind::Sealed)
        .map(|p| Threat {
            site: p.id,
            x: p.x,
            y: p.y,
            level: p.level,
            seals: Vec::new(),
            awake: false,
        })
        .collect();
    if threats.is_empty() {
        return threats;
    }
    for y in 0..site.height() as i32 {
        for x in 0..site.width() as i32 {
            if site.get(x, y) != Some(&SiteTile::Seal) {
                continue;
            }
            let near = |t: &Threat| (x - i32::from(t.x)).abs().max((y - i32::from(t.y)).abs());
            let mut best = 0;
            for (i, t) in threats.iter().enumerate() {
                if near(t) < near(&threats[best]) {
                    best = i;
                }
            }
            threats[best].seals.push((x as u16, y as u16));
        }
    }
    for t in &mut threats {
        t.seals.sort_unstable();
    }
    threats
}

impl Sim {
    /// The Notice of the colony with each contribution (R-030).
    pub fn notice(&self) -> NoticeReport {
        let n = &self.rules.notice;
        let mut parts = Vec::new();
        // Wealth: items, then buildings. The tile is the one with the most valuable thing.
        let mut wealth: u64 = 0;
        let mut top: Option<(u64, (u16, u16))> = None;
        let mut add = |value: u64, tile: (u16, u16)| {
            wealth += value;
            if value > 0 && top.is_none_or(|(v, _)| value > v) {
                top = Some((value, tile));
            }
        };
        for (_, item) in self.items.iter() {
            if let crate::ItemLoc::Tile(x, y) = item.loc {
                add(
                    u64::from(item.count) * self.material_value(item.material),
                    (x, y),
                );
            } else if let crate::ItemLoc::Carried(id) = item.loc
                && let Some(c) = self.colonists.get(id)
            {
                add(
                    u64::from(item.count) * self.material_value(item.material),
                    (c.x, c.y),
                );
            }
        }
        for (x, y, _, b) in self.buildings.iter() {
            add(u64::from(b.units) * self.material_value(b.material), (x, y));
        }
        let home = self.notice_home();
        let (wx, wy) = top.map_or(home, |(_, t)| t);
        let wealth_notice =
            u32::try_from(wealth / u64::from(n.wealth_div.max(1))).unwrap_or(u32::MAX);
        parts.push(NoticePart {
            cause: NoticeCause::Wealth,
            value: wealth_notice,
            x: wx,
            y: wy,
        });
        let people = u32::try_from(self.colonists.len()).unwrap_or(u32::MAX);
        parts.push(NoticePart {
            cause: NoticeCause::Population,
            value: people.saturating_mul(n.per_colonist),
            x: home.0,
            y: home.1,
        });
        for (cause, map) in [
            (NoticeCause::Noise, &self.notice.noise),
            (NoticeCause::Disturbance, &self.notice.disturbance),
        ] {
            let mut tiles: Vec<_> = map.iter().map(|(&t, &v)| (t, v)).collect();
            tiles.sort_unstable_by_key(|&((x, y), _)| (y, x));
            for ((x, y), value) in tiles {
                parts.push(NoticePart { cause, value, x, y });
            }
        }
        let total = parts.iter().fold(0u32, |a, p| a.saturating_add(p.value));
        NoticeReport { total, parts }
    }

    fn material_value(&self, material: u16) -> u64 {
        self.rules
            .materials
            .get(usize::from(material))
            .map_or(0, |m| u64::from(m.value))
    }

    /// The average tile of the colonists, or the map center if there are none.
    fn notice_home(&self) -> (u16, u16) {
        let count = self.colonists.len() as u64;
        if count == 0 {
            return (
                (self.map.width() / 2).min(u32::from(u16::MAX)) as u16,
                (self.map.height() / 2).min(u32::from(u16::MAX)) as u16,
            );
        }
        let (mut sx, mut sy) = (0u64, 0u64);
        for (_, c) in self.colonists.iter() {
            sx += u64::from(c.x);
            sy += u64::from(c.y);
        }
        ((sx / count) as u16, (sy / count) as u16)
    }

    /// Notice now, for the daily rolls and the strength of threats.
    fn notice_total(&self) -> u32 {
        self.notice().total
    }

    /// The chance of a beast each game day, in percent (R-031).
    pub fn beast_chance_pct(&self) -> u32 {
        let n = &self.rules.notice;
        (self.notice_total() / n.beast_div.max(1)).min(n.beast_max_pct)
    }

    /// True when Notice is high enough to wake the beasts in lairs (R-031).
    pub fn lairs_active(&self) -> bool {
        self.notice_total() >= self.rules.notice.lair_active
    }

    /// The dormant threats of the map, in site order.
    pub fn threats(&self) -> &[Threat] {
        &self.threats
    }

    /// Strength level of an awake threat: its base level plus 1 for each `threat_per_level`
    /// Notice (R-031). `None` while it is dormant or if the index is wrong.
    pub fn threat_strength(&self, index: usize) -> Option<u32> {
        let t = self.threats.get(index)?;
        t.awake.then(|| {
            u32::from(t.level) + self.notice_total() / self.rules.notice.threat_per_level.max(1)
        })
    }

    /// Take the Notice events that the last ticks made.
    pub fn drain_notice_events(&mut self) -> Vec<NoticeEvent> {
        std::mem::take(&mut self.notice_events)
    }

    /// Add the disturbance of an action on a site tile (R-030). A later system calls this when
    /// a colonist excavates a site or takes a relic.
    pub fn notice_disturb(&mut self, x: u16, y: u16, source: NoticeSource) {
        let n = &self.rules.notice;
        let value = match source {
            NoticeSource::Excavation => n.excavation,
            NoticeSource::Relic => n.relic,
        };
        self.add_disturbance(x, y, value);
    }

    fn add_disturbance(&mut self, x: u16, y: u16, value: u32) {
        if value > 0 {
            let v = self.notice.disturbance.entry((x, y)).or_insert(0);
            *v = v.saturating_add(value);
        }
    }

    fn add_noise(&mut self, x: u16, y: u16, value: u32) {
        if value > 0 {
            let v = self.notice.noise.entry((x, y)).or_insert(0);
            *v = v.saturating_add(value);
        }
    }

    /// A job is done on a tile: mining and construction make noise, and mining a seal tile
    /// wakes the threat behind it.
    pub(crate) fn notice_job_done(&mut self, kind: crate::JobKind, x: u16, y: u16) {
        match kind {
            crate::JobKind::Mine => {
                self.add_noise(x, y, self.rules.notice.noise_per_job);
                self.open_seal(x, y);
            }
            crate::JobKind::Construct => {
                self.add_noise(x, y, self.rules.notice.noise_per_job);
            }
            _ => {}
        }
    }

    fn open_seal(&mut self, x: u16, y: u16) {
        let Some(i) = self
            .threats
            .iter()
            .position(|t| !t.awake && t.seals.binary_search(&(x, y)).is_ok())
        else {
            return;
        };
        self.wake_threat(i, x, y);
    }

    fn wake_threat(&mut self, index: usize, x: u16, y: u16) {
        let Some(t) = self.threats.get_mut(index) else {
            return;
        };
        if t.awake {
            return;
        }
        t.awake = true;
        let (site, level) = (t.site, t.level);
        self.add_disturbance(x, y, self.rules.notice.sealed_open);
        self.notice_events
            .push(NoticeEvent::ThreatWoke { site, x, y, level });
    }

    /// Once each game day: noise and disturbance decay, then the beast roll.
    pub(crate) fn notice_tick(&mut self) {
        if self.tick == 0 || !self.tick.is_multiple_of(TICKS_PER_DAY) {
            return;
        }
        let n = self.rules.notice;
        let pct = u64::from(n.noise_decay_pct);
        self.notice.noise.retain(|_, v| {
            let loss = (u64::from(*v) * pct).div_ceil(100) as u32;
            *v = v.saturating_sub(loss);
            *v > 0
        });
        self.notice.disturbance.retain(|_, v| {
            *v = v.saturating_sub(n.disturbance_decay);
            *v > 0
        });
        let chance_pct = self.beast_chance_pct();
        let arrived = self.rng.chance(chance_pct, 100);
        let notice = self.notice_total();
        self.notice_events.push(NoticeEvent::BeastRoll {
            day: self.tick / TICKS_PER_DAY,
            notice,
            chance_pct,
            arrived,
        });
    }

    // ---- debug hooks (scenario tests) ----

    /// Add noise on a tile. For tests.
    pub fn debug_add_noise(&mut self, x: u16, y: u16, value: u32) {
        self.add_noise(x, y, value);
    }

    /// Add site disturbance on a tile. For tests.
    pub fn debug_add_disturbance(&mut self, x: u16, y: u16, value: u32) {
        self.add_disturbance(x, y, value);
    }

    /// Add a dormant threat. For tests.
    pub fn debug_add_threat(&mut self, site: u32, level: u8, x: u16, y: u16, seals: &[(u16, u16)]) {
        let mut seals = seals.to_vec();
        seals.sort_unstable();
        self.threats.push(Threat {
            site,
            x,
            y,
            level,
            seals,
            awake: false,
        });
    }

    /// Wake a threat as if a colonist opened its first seal tile. For tests.
    pub fn debug_wake_threat(&mut self, index: usize) {
        let tile = self
            .threats
            .get(index)
            .map(|t| t.seals.first().copied().unwrap_or((t.x, t.y)));
        if let Some((x, y)) = tile {
            self.wake_threat(index, x, y);
        }
    }
}
