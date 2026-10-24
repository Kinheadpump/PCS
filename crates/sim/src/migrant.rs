//! Migrants (S15, R-017).
//!
//! Once each game day the sim checks for migrants. They can come when the food lasts
//! `min_food_days`, a bed is free, and the quiet days are over. The chance is `base_pct` plus 1
//! percent for each `wealth_per_pct` of wealth, to `max_pct`. A group has 1 to `group_max`
//! migrants. The offer waits for the player (`Command::AcceptMigrants` or
//! `Command::RefuseMigrants`); the interface pauses the game. A refused offer, or one that the
//! player does not answer in `decision_ticks`, leaves the map. Each migrant has a short
//! background line (Q-014).

use crate::Sim;
use crate::chronicle::RunState;
use crate::colonist::{Colonist, Sex};
use crate::job::JobKind;
use crate::loader::{Issues, parse};
use crate::skill::SKILL_COUNT;
use crate::time::TICKS_PER_DAY;
use base::Pcg32;
use base::content::{BuildingKind, ContentError};
use base::rng::mix;
use serde::{Deserialize, Serialize};

/// Tag of the migrant random stream.
const TAG_MIGRANT: u64 = 0x4d49_4752_414e_5431;

/// Numbers for migrants (`migrants.ron`).
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MigrantRules {
    /// No migrants in this many first days.
    pub quiet_days: u32,
    /// Food days that migrants need.
    pub min_food_days: u32,
    /// Base chance in percent.
    pub base_pct: u32,
    /// Wealth for 1 percent more chance.
    pub wealth_per_pct: u32,
    /// Largest chance in percent.
    pub max_pct: u32,
    /// Largest group.
    pub group_max: u32,
    /// Mood that gives no change of the chance.
    pub mood_ref: i32,
    /// Points of average mood for 1 percent of chance.
    pub mood_points_per_pct: i32,
    /// Largest change of the chance by mood, up or down, in percent.
    pub mood_pct_max: u32,
    /// Largest change of the chance by bonds, in percent.
    pub bond_pct_max: u32,
    /// Ticks that the player has to answer.
    pub decision_ticks: u32,
    /// Lowest level of the main skill.
    pub main_level_min: u32,
    /// Highest level of the main skill.
    pub main_level_max: u32,
    /// The background lines.
    pub backgrounds: Vec<String>,
}

/// Load and check `migrants.ron`.
pub fn load_migrant_rules(file: &str, text: &str) -> Result<MigrantRules, ContentError> {
    let r: MigrantRules = parse(file, text)?;
    let mut issues = Issues::new(file);
    if r.group_max == 0 || r.group_max > 8 {
        issues.add("group_max", "must be from 1 to 8");
    }
    if r.wealth_per_pct == 0 {
        issues.add("wealth_per_pct", "must be 1 or more");
    }
    if r.mood_points_per_pct <= 0 {
        issues.add("mood_points_per_pct", "must be 1 or more");
    }
    if r.decision_ticks == 0 {
        issues.add("decision_ticks", "must be 1 or more");
    }
    if r.max_pct > 100 || r.base_pct > 100 {
        issues.add("max_pct", "must be 100 or less");
    }
    if r.main_level_min > r.main_level_max || r.main_level_max > 10 {
        issues.add("main_level_min", "must satisfy min <= max <= 10");
    }
    if r.backgrounds.is_empty() || r.backgrounds.len() > 200 {
        issues.add("backgrounds", "must hold 1 to 200 lines");
    }
    issues.finish(r)
}

/// Why the migrant chance has a part of its value (D-021).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChanceCause {
    /// The base chance.
    Base,
    /// The wealth of the colony.
    Wealth,
    /// The average mood of the colonists.
    Mood,
    /// The friend and partner pairs.
    Bonds,
}

/// One named part of the migrant chance.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChancePart {
    /// The cause.
    pub cause: ChanceCause,
    /// Percent points.
    pub value: i32,
}

/// The daily migrant chance with its why chain.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MigrantChance {
    /// The named parts.
    pub parts: Vec<ChancePart>,
    /// The chance in percent: the sum of the parts, limited to 0 and `max_pct`.
    pub total: u32,
}

/// One migrant of an offer.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MigrantProfile {
    /// Race index (`races.ron` order).
    pub race: u8,
    /// Sex.
    pub sex: Sex,
    /// Age in years.
    pub age: u8,
    /// Trait index (`traits.ron` order), if the content has traits.
    pub trait_index: Option<u8>,
    /// Background line index plus 1 (`migrants.ron` order).
    pub background: u8,
    /// Index of the main skill (`Skill::index` order).
    pub skill: u8,
    /// Level of the main skill.
    pub level: u32,
}

/// A group of migrants that waits for the answer of the player.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MigrantOffer {
    /// The migrants.
    pub profiles: Vec<MigrantProfile>,
    /// Map-edge tile where they come in.
    pub entry: (u16, u16),
    /// Tick when the offer ends and the migrants leave.
    pub expires_tick: u64,
}

/// Migrant offers and counts. Part of the state.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct MigrantState {
    rng: Pcg32,
    pub(crate) offer: Option<MigrantOffer>,
    pub(crate) joined: u32,
    pub(crate) refused: u32,
}

impl MigrantState {
    pub(crate) fn new(seed: u64) -> Self {
        Self {
            rng: Pcg32::from_seed(mix(seed ^ TAG_MIGRANT)),
            offer: None,
            joined: 0,
            refused: 0,
        }
    }
}

/// A thing that happened with migrants. The caller takes it with `Sim::drain_migrant_events`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MigrantEvent {
    /// A group came to the map edge. The interface pauses the game and asks the player.
    Offered {
        /// Number of migrants.
        count: u32,
        /// Entry tile column.
        x: u16,
        /// Entry tile row.
        y: u16,
    },
    /// The player accepted: the migrants are colonists now.
    Joined {
        /// Number of migrants.
        count: u32,
    },
    /// The migrants left the map.
    Left {
        /// Number of migrants.
        count: u32,
        /// True if the player did not answer in time.
        timed_out: bool,
    },
    /// An accept or refuse command came when no offer waited.
    NoOffer,
}

impl Sim {
    /// The offer that waits for an answer, if any.
    pub fn migrant_offer(&self) -> Option<&MigrantOffer> {
        self.migrants.offer.as_ref()
    }

    /// Migrants that joined, and migrants that left unasked or refused.
    pub fn migrant_counts(&self) -> (u32, u32) {
        (self.migrants.joined, self.migrants.refused)
    }

    /// The background line of a migrant colonist. `None` for a colonist with no line.
    pub fn background_line(&self, id: crate::ColonistId) -> Option<&str> {
        let index = usize::from(self.colonists.get(id)?.background).checked_sub(1)?;
        self.rules
            .migrant
            .backgrounds
            .get(index)
            .map(String::as_str)
    }

    /// Take the migrant events that the last ticks made.
    pub fn drain_migrant_events(&mut self) -> Vec<MigrantEvent> {
        std::mem::take(&mut self.migrant_events)
    }

    /// Number of beds in the colony.
    pub fn bed_count(&self) -> usize {
        self.buildings
            .object_tiles(|def| self.rules.is_kind(def, BuildingKind::Bed))
            .len()
    }

    /// Why migrants cannot come now, or `None` if they can (R-017): the food lasts too few days
    /// or no bed is free.
    pub fn migrant_blocker(&self) -> Option<MigrantBlocker> {
        let days = self.ledger().food.days_remaining_tenths;
        let enough = days.is_none_or(|d| d >= u64::from(self.rules.migrant.min_food_days) * 10);
        if !enough {
            return Some(MigrantBlocker::LittleFood);
        }
        if self.bed_count() <= self.colonists.len() {
            return Some(MigrantBlocker::NoFreeBed);
        }
        None
    }

    /// The daily chance of migrants in percent (R-017, R-040).
    pub fn migrant_chance_pct(&self) -> u32 {
        self.migrant_chance().total
    }

    /// The daily chance of migrants as a sum of named parts (R-017, R-040): `base_pct`, the
    /// wealth, the average mood, and the friend and partner pairs. The total is limited to
    /// 0 and `max_pct`.
    pub fn migrant_chance(&self) -> MigrantChance {
        let r = &self.rules.migrant;
        let wealth = self.wealth_value() / u64::from(r.wealth_per_pct.max(1));
        let count = i64::try_from(self.colonists.len())
            .unwrap_or(i64::MAX)
            .max(1);
        let mood_sum: i64 = self
            .colonists
            .iter()
            .map(|(id, _)| i64::from(self.mood(id).map_or(r.mood_ref, |m| m.total)))
            .sum();
        let average = i32::try_from(mood_sum / count).unwrap_or(r.mood_ref);
        let limit = i32::try_from(r.mood_pct_max).unwrap_or(0);
        let mood = ((average - r.mood_ref) / r.mood_points_per_pct.max(1)).clamp(-limit, limit);
        let pairs = self
            .relation_state
            .map
            .keys()
            .filter(|k| {
                matches!(
                    self.relation(k.0, k.1).map(|x| x.standing),
                    Some(crate::Standing::Friend | crate::Standing::Partner)
                )
            })
            .count();
        let bonds = i32::try_from(pairs)
            .unwrap_or(i32::MAX)
            .min(i32::try_from(r.bond_pct_max).unwrap_or(0));
        let parts = vec![
            ChancePart {
                cause: ChanceCause::Base,
                value: i32::try_from(r.base_pct).unwrap_or(0),
            },
            ChancePart {
                cause: ChanceCause::Wealth,
                value: i32::try_from(wealth).unwrap_or(i32::MAX),
            },
            ChancePart {
                cause: ChanceCause::Mood,
                value: mood,
            },
            ChancePart {
                cause: ChanceCause::Bonds,
                value: bonds,
            },
        ];
        let sum = parts.iter().fold(0i64, |a, p| a + i64::from(p.value));
        let total = sum.clamp(0, i64::from(r.max_pct));
        MigrantChance {
            parts,
            total: u32::try_from(total).unwrap_or(0),
        }
    }

    /// Called every tick after the tick number increased.
    pub(crate) fn migrant_tick(&mut self) {
        if let Some(offer) = &self.migrants.offer
            && self.tick >= offer.expires_tick
        {
            self.migrant_leave(true);
        }
        if self.tick > 0 && self.tick.is_multiple_of(TICKS_PER_DAY) {
            self.migrant_daily();
        }
    }

    fn migrant_daily(&mut self) {
        let r = self.rules.migrant.clone();
        let day = self.tick / TICKS_PER_DAY;
        if day <= u64::from(r.quiet_days)
            || self.migrants.offer.is_some()
            || self.run.state != RunState::Running
            || self.colonists.is_empty()
            || self.migrant_blocker().is_some()
        {
            return;
        }
        let pct = self.migrant_chance_pct();
        if !self.migrants.rng.chance(pct, 100) {
            return;
        }
        let edge = self.migrants.rng.below(4) as u8;
        let Some(entry) = self.raid_entry_tile(edge) else {
            return;
        };
        let count = 1 + self.migrants.rng.below(r.group_max);
        let profiles: Vec<MigrantProfile> = (0..count).map(|_| self.roll_migrant()).collect();
        self.migrants.offer = Some(MigrantOffer {
            profiles,
            entry,
            expires_tick: self.tick + u64::from(r.decision_ticks),
        });
        self.migrant_events.push(MigrantEvent::Offered {
            count,
            x: entry.0,
            y: entry.1,
        });
    }

    fn roll_migrant(&mut self) -> MigrantProfile {
        let r = self.rules.migrant.clone();
        let n = &self.rules.needs;
        let (lo, hi) = (u32::from(n.age_min), u32::from(n.age_max));
        let trait_count = u32::try_from(self.rules.traits.len()).unwrap_or(0);
        let rng = &mut self.migrants.rng;
        let age = u8::try_from(lo + rng.below(hi - lo + 1)).unwrap_or(n.age_min);
        let sex = if rng.below(2) == 0 {
            Sex::Female
        } else {
            Sex::Male
        };
        let trait_index =
            (trait_count > 0).then(|| u8::try_from(rng.below(trait_count)).unwrap_or(0));
        let lines = u32::try_from(r.backgrounds.len()).unwrap_or(1);
        let background = u8::try_from(1 + rng.below(lines)).unwrap_or(1);
        let skill = u8::try_from(rng.below(SKILL_COUNT as u32)).unwrap_or(0);
        let level = r.main_level_min + rng.below(r.main_level_max - r.main_level_min + 1);
        // The race comes from a copy of the stream, so that the numbers of the stream stay.
        let race_roll = u64::from(rng.clone().below(1 << 20));
        let race = self.rules.races.pick(race_roll);
        MigrantProfile {
            race,
            sex,
            age,
            trait_index,
            background,
            skill,
            level,
        }
    }

    /// The player accepts the migrants (R-017).
    pub(crate) fn migrants_accept(&mut self) {
        let Some(offer) = self.migrants.offer.take() else {
            self.migrant_events.push(MigrantEvent::NoOffer);
            return;
        };
        let xp_per_level = self.rules.jobs.xp_per_level;
        for p in &offer.profiles {
            let mut c = Colonist::new(offer.entry.0, offer.entry.1);
            c.priorities = [self.rules.jobs.default_priority; JobKind::COUNT];
            self.rules.needs.init_colonist(&mut c);
            c.age = p.age;
            c.sex = p.sex;
            c.race = p.race;
            c.traits = p.trait_index.into_iter().collect();
            c.background = p.background;
            if let Some(xp) = c.skill_xp.get_mut(usize::from(p.skill)) {
                *xp = p.level * xp_per_level;
            }
            let id = self.colonists.insert(c);
            let salt =
                self.tick ^ (u64::from(self.migrants.joined) << 40) ^ ((id.index() as u64) << 20);
            self.roll_wants(id, salt);
        }
        let count = u32::try_from(offer.profiles.len()).unwrap_or(u32::MAX);
        self.migrants.joined += count;
        self.run.peak_population = self
            .run
            .peak_population
            .max(u32::try_from(self.colonists.len()).unwrap_or(u32::MAX));
        self.wake_idle();
        self.migrant_events.push(MigrantEvent::Joined { count });
    }

    /// The player refuses the migrants, or the time ran out.
    pub(crate) fn migrants_refuse(&mut self) {
        if self.migrants.offer.is_none() {
            self.migrant_events.push(MigrantEvent::NoOffer);
            return;
        }
        self.migrant_leave(false);
    }

    fn migrant_leave(&mut self, timed_out: bool) {
        let Some(offer) = self.migrants.offer.take() else {
            return;
        };
        let count = u32::try_from(offer.profiles.len()).unwrap_or(u32::MAX);
        self.migrants.refused += count;
        self.migrant_events
            .push(MigrantEvent::Left { count, timed_out });
    }

    /// True if the migrant state fits the map. Called after a load.
    pub(crate) fn migrant_state_ok(&self) -> bool {
        self.migrants.offer.as_ref().is_none_or(|o| {
            self.map
                .in_bounds(i32::from(o.entry.0), i32::from(o.entry.1))
                && !o.profiles.is_empty()
                && o.profiles.len() <= 8
                && o.profiles.iter().all(|p| {
                    usize::from(p.skill) < SKILL_COUNT
                        && usize::from(p.background) <= self.rules.migrant.backgrounds.len()
                })
        })
    }

    // ---- debug hooks (scenario tests) ----

    /// Make an offer of one migrant now. For tests.
    pub fn debug_offer_migrants(&mut self, count: u32) -> bool {
        if self.migrants.offer.is_some() {
            return false;
        }
        let Some(entry) = self.raid_entry_tile(0) else {
            return false;
        };
        let profiles: Vec<MigrantProfile> = (0..count.clamp(1, 8))
            .map(|_| self.roll_migrant())
            .collect();
        let n = u32::try_from(profiles.len()).unwrap_or(1);
        self.migrants.offer = Some(MigrantOffer {
            profiles,
            entry,
            expires_tick: self.tick + u64::from(self.rules.migrant.decision_ticks),
        });
        self.migrant_events.push(MigrantEvent::Offered {
            count: n,
            x: entry.0,
            y: entry.1,
        });
        true
    }
}

/// Why migrants cannot come.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MigrantBlocker {
    /// The food lasts fewer days than `min_food_days`.
    LittleFood,
    /// Every bed has an owner.
    NoFreeBed,
}
