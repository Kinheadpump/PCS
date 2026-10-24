//! Families, ages, and homes (S22, R-037, R-038, R-039).
//!
//! Each game day a colonist gets older. A child (below the adult age) does not work and cannot
//! be drafted. An elder can die of old age. Two partners (S21), a woman and a man, can have a
//! child: the woman is pregnant for some days and then the child is born. A birth, and the death
//! of a parent or a child, give named mood memories (R-014). Each game hour every colonist gets an
//! assigned bed. Partners and children live in the same room when a room has enough beds, and a
//! single colonist prefers a room with few beds. A tired colonist sleeps in the assigned bed
//! when it is free.

use crate::Sim;
use crate::colonist::{Colonist, ColonistId, Sex};
use crate::life::DeathCause;
use crate::loader::{Issues, parse};
use crate::needs::NeedRules;
use crate::time::TICKS_PER_DAY;
use base::content::{BuildingKind, ContentError};
use base::rng::mix;
use serde::Deserialize;
use std::collections::BTreeMap;

/// Tag of the family rolls.
const TAG_FAMILY: u64 = 0x4641_4d49_4c59_5f53;

/// The stage of life of a colonist.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LifeStage {
    /// Below the adult age: does not work.
    Child,
    /// An adult.
    Adult,
    /// From the elder age: can die of old age.
    Elder,
}

impl LifeStage {
    /// Display name.
    pub fn name(self) -> &'static str {
        match self {
            LifeStage::Child => "Child",
            LifeStage::Adult => "Adult",
            LifeStage::Elder => "Elder",
        }
    }
}

/// A family event for the caller (`Sim::drain_family_events`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FamilyEvent {
    /// A woman is pregnant.
    Pregnant {
        /// The mother.
        mother: ColonistId,
        /// The father.
        father: ColonistId,
    },
    /// A child was born.
    Born {
        /// The child.
        child: ColonistId,
        /// The mother.
        mother: ColonistId,
        /// The father, if still known.
        father: Option<ColonistId>,
    },
    /// A child became an adult.
    GrewUp {
        /// The colonist.
        colonist: ColonistId,
    },
}

/// Raw `family.ron`, with the linked memory indices.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FamilyRules {
    /// Game days of one age year.
    pub days_per_age_year: u32,
    /// Age at which a child becomes an adult.
    pub adult_age: u8,
    /// Age from which a colonist can die of old age.
    pub elder_age: u8,
    /// Daily chance of death in per mille for each age year from the elder age.
    pub elder_death_permille: u32,
    /// Youngest age of a mother.
    pub fertile_min_age: u8,
    /// Oldest age of a mother.
    pub fertile_max_age: u8,
    /// Daily chance of a pregnancy in percent.
    pub pregnancy_chance_pct: u32,
    /// Game days of a pregnancy.
    pub pregnancy_days: u32,
    /// Game days after a birth without a pregnancy.
    pub birth_cooldown_days: u32,
    /// Most living children below the adult age of one mother.
    pub children_max: u32,
    /// Days of colony food that a pregnancy needs.
    pub min_food_days: u32,
    /// Mood that both partners need for a pregnancy.
    pub min_mood: i32,
    /// Beds that must stay free after each colonist has a bed.
    pub spare_beds: usize,
    /// Linked memory indices.
    #[serde(skip)]
    pub memories: FamilyMemories,
}

/// Indices of the memories of `needs.ron` that families give.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FamilyMemories {
    /// `child_born`.
    pub child_born: u8,
    /// `parent_died`.
    pub parent_died: u8,
    /// `child_died`.
    pub child_died: u8,
}

/// Load and check `family.ron`. `needs` gives the memory keys.
pub fn load_family_rules(
    file: &str,
    text: &str,
    needs: &NeedRules,
) -> Result<FamilyRules, ContentError> {
    let mut r: FamilyRules = parse(file, text)?;
    let mut issues = Issues::new(file);
    if r.days_per_age_year == 0 || r.pregnancy_days == 0 {
        issues.add(
            "days_per_age_year",
            "days_per_age_year and pregnancy_days must be 1 or more",
        );
    }
    if !(r.adult_age < r.fertile_max_age && r.fertile_max_age < r.elder_age)
        || r.fertile_min_age < r.adult_age
        || r.fertile_min_age > r.fertile_max_age
    {
        issues.add(
            "adult_age",
            "need adult_age <= fertile_min_age <= fertile_max_age < elder_age",
        );
    }
    if r.pregnancy_chance_pct > 100 {
        issues.add("pregnancy_chance_pct", "must be 100 or less");
    }
    let mut find = |key: &str| -> u8 {
        needs.memory_index(key).unwrap_or_else(|| {
            issues.add(key, "the memory is not in needs.ron");
            0
        })
    };
    r.memories = FamilyMemories {
        child_born: find("child_born"),
        parent_died: find("parent_died"),
        child_died: find("child_died"),
    };
    issues.finish(r)
}

/// The family of a colonist, for the UI.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FamilyReport {
    /// Age in age years.
    pub age: u8,
    /// Days since the last birthday.
    pub age_days: u32,
    /// The stage of life.
    pub stage: LifeStage,
    /// Game days left of a pregnancy, if the colonist is pregnant.
    pub pregnant_days_left: Option<u32>,
    /// The mother, if known.
    pub mother: Option<ColonistId>,
    /// The father, if known.
    pub father: Option<ColonistId>,
    /// The living children (below or above the adult age).
    pub children: Vec<ColonistId>,
    /// The partner.
    pub partner: Option<ColonistId>,
    /// The assigned bed.
    pub home: Option<(u16, u16)>,
}

/// The living kin of a colonist, taken before it is removed.
pub(crate) struct Kin {
    parents: Vec<ColonistId>,
    children: Vec<ColonistId>,
}

impl Sim {
    /// The living children of a colonist, in slot order.
    fn children_of(&self, id: ColonistId) -> Vec<ColonistId> {
        self.colonists
            .iter()
            .filter(|(_, c)| c.mother == Some(id) || c.father == Some(id))
            .map(|(i, _)| i)
            .collect()
    }

    /// The family of a colonist (R-037, R-038).
    pub fn family(&self, id: ColonistId) -> Option<FamilyReport> {
        let c = self.colonists.get(id)?;
        let f = &self.rules.family;
        let stage = if c.age < f.adult_age {
            LifeStage::Child
        } else if c.age >= f.elder_age {
            LifeStage::Elder
        } else {
            LifeStage::Adult
        };
        let pregnant_days_left = (c.pregnant_until > 0).then(|| {
            let left = c.pregnant_until.saturating_sub(self.tick);
            u32::try_from(left.div_ceil(TICKS_PER_DAY)).unwrap_or(u32::MAX)
        });
        Some(FamilyReport {
            age: c.age,
            age_days: u32::from(c.age_days),
            stage,
            pregnant_days_left,
            mother: c.mother,
            father: c.father,
            children: self.children_of(id),
            partner: self.partner_of(id),
            home: c.bed,
        })
    }

    /// The assigned bed of a colonist (R-039).
    pub fn home_of(&self, id: ColonistId) -> Option<(u16, u16)> {
        self.colonists.get(id)?.bed
    }

    /// Take the family events that the last ticks made.
    pub fn drain_family_events(&mut self) -> Vec<FamilyEvent> {
        std::mem::take(&mut self.family_events)
    }

    /// A number from 0 to 999 for a roll of the day. The same state gives the same number.
    fn family_roll(&self, salt: u64, a: ColonistId, b: u32) -> u32 {
        let day = self.tick / TICKS_PER_DAY;
        let key = (u64::from(a.index()) << 32) | u64::from(b);
        let h = mix(self.relation_state.seed ^ TAG_FAMILY ^ mix(salt ^ mix(key ^ day)));
        u32::try_from(h % 1000).unwrap_or(0)
    }

    /// Each game day: ages, growth, old age, births, and new pregnancies. Runs at the first hour
    /// of the day.
    pub(crate) fn family_day(&mut self) {
        if self.tick == 0 || !self.tick.is_multiple_of(TICKS_PER_DAY) {
            return;
        }
        let rules = self.rules.clone();
        let f = &rules.family;
        let ids: Vec<ColonistId> = self.colonists.iter().map(|(i, _)| i).collect();
        // Ages and growth.
        let mut grown: Vec<ColonistId> = Vec::new();
        for &id in &ids {
            let Some(c) = self.colonists.get_mut(id) else {
                continue;
            };
            c.age_days = c.age_days.saturating_add(1);
            if u32::from(c.age_days) >= f.days_per_age_year {
                c.age_days = 0;
                c.age = c.age.saturating_add(1);
                if c.age == f.adult_age {
                    c.priorities = [rules.jobs.default_priority; crate::job::JobKind::COUNT];
                    c.next_search = c.next_search.min(self.tick);
                    grown.push(id);
                }
            }
        }
        for id in grown {
            self.family_events
                .push(FamilyEvent::GrewUp { colonist: id });
        }
        // Old age.
        let old: Vec<ColonistId> = ids
            .iter()
            .copied()
            .filter(|&id| {
                self.colonists.get(id).is_some_and(|c| {
                    if c.age < f.elder_age {
                        return false;
                    }
                    let years = u32::from(c.age - f.elder_age) + 1;
                    self.family_roll(1, id, 0) < years.saturating_mul(f.elder_death_permille)
                })
            })
            .collect();
        for id in old {
            self.kill(id, DeathCause::OldAge);
        }
        self.births();
        self.pregnancies();
    }

    /// A mother whose pregnancy ends gives birth to a child.
    fn births(&mut self) {
        let due: Vec<ColonistId> = self
            .colonists
            .iter()
            .filter(|(_, c)| c.pregnant_until > 0 && c.pregnant_until <= self.tick)
            .map(|(i, _)| i)
            .collect();
        for mother in due {
            let Some(m) = self.colonists.get_mut(mother) else {
                continue;
            };
            let (x, y, race) = (m.x, m.y, m.race);
            let father = m
                .pregnancy_father
                .take()
                .filter(|&f| self.colonists.get(f).is_some());
            let Some(m) = self.colonists.get_mut(mother) else {
                continue;
            };
            m.pregnant_until = 0;
            m.last_birth = self.tick;
            let mut c = Colonist::new(x, y);
            self.rules.needs.init_colonist(&mut c);
            c.age = 0;
            c.race = race;
            c.mother = Some(mother);
            c.father = father;
            let roll = self.family_roll(2, mother, father.map_or(0, |f| f.index()));
            c.sex = if roll.is_multiple_of(2) {
                Sex::Female
            } else {
                Sex::Male
            };
            let trait_count = self.rules.traits.len();
            if trait_count > 0 {
                let inherited = father
                    .and_then(|f| self.colonists.get(f))
                    .and_then(|f| f.traits.first().copied());
                let own = u8::try_from((roll as usize / 2) % trait_count).unwrap_or(0);
                c.traits = vec![if roll.is_multiple_of(3) {
                    inherited.unwrap_or(own)
                } else {
                    own
                }];
            }
            let child = self.colonists.insert(c);
            self.roll_wants(child, u64::from(roll) ^ self.tick);
            if let Some(c) = self.colonists.get_mut(child) {
                c.age_days = 0;
            }
            let key = self.rules.family.memories.child_born;
            self.give_memory(mother, key);
            if let Some(f) = father {
                self.give_memory(f, key);
            }
            let n = i32::try_from(self.colonists.len()).unwrap_or(i32::MAX);
            self.chronicle_add(crate::ChronicleKind::Birth, [n, 0, 0]);
            self.wake_idle();
            self.family_events.push(FamilyEvent::Born {
                child,
                mother,
                father,
            });
        }
    }

    /// Each partner pair with a woman and a man can start a pregnancy.
    fn pregnancies(&mut self) {
        let f = self.rules.family.clone();
        let food_ok = self
            .ledger()
            .food
            .days_remaining_tenths
            .is_none_or(|d| d >= u64::from(f.min_food_days) * 10);
        let beds = self.bed_count();
        if !food_ok || beds < self.colonists.len() + f.spare_beds {
            return;
        }
        let pairs: Vec<(ColonistId, ColonistId)> = self
            .relation_state
            .map
            .iter()
            .filter(|(_, r)| r.partner)
            .map(|(k, _)| *k)
            .collect();
        for (a, b) in pairs {
            let (Some(ca), Some(cb)) = (self.colonists.get(a), self.colonists.get(b)) else {
                continue;
            };
            let (mother, father) = match (ca.sex, cb.sex) {
                (Sex::Female, Sex::Male) => (a, b),
                (Sex::Male, Sex::Female) => (b, a),
                _ => continue,
            };
            let Some(m) = self.colonists.get(mother) else {
                continue;
            };
            let cooldown = u64::from(f.birth_cooldown_days) * TICKS_PER_DAY;
            let minors = self
                .children_of(mother)
                .iter()
                .filter(|&&k| self.colonists.get(k).is_some_and(|c| c.age < f.adult_age))
                .count();
            let mood_ok = |id| self.mood(id).is_some_and(|x| x.total >= f.min_mood);
            if m.pregnant_until > 0
                || m.age < f.fertile_min_age
                || m.age > f.fertile_max_age
                || (m.last_birth > 0 && self.tick < m.last_birth + cooldown)
                || u32::try_from(minors).unwrap_or(u32::MAX) >= f.children_max
                || !mood_ok(mother)
                || !mood_ok(father)
                || self.family_roll(3, mother, father.index()) >= f.pregnancy_chance_pct * 10
            {
                continue;
            }
            if let Some(m) = self.colonists.get_mut(mother) {
                m.pregnant_until = self.tick + u64::from(f.pregnancy_days) * TICKS_PER_DAY;
                m.pregnancy_father = Some(father);
            }
            self.family_events
                .push(FamilyEvent::Pregnant { mother, father });
        }
    }

    /// The living parents and children of a colonist, taken before it is removed.
    pub(crate) fn family_kin(&self, id: ColonistId) -> Kin {
        let parents = self
            .colonists
            .get(id)
            .map(|c| [c.mother, c.father])
            .unwrap_or([None, None])
            .into_iter()
            .flatten()
            .filter(|&p| self.colonists.get(p).is_some())
            .collect();
        Kin {
            parents,
            children: self.children_of(id),
        }
    }

    /// The memories that a death gives to the parents and the children of the dead.
    pub(crate) fn family_mourn(&mut self, kin: &Kin) {
        let m = self.rules.family.memories;
        for &p in &kin.parents {
            self.give_memory(p, m.child_died);
        }
        for &c in &kin.children {
            self.give_memory(c, m.parent_died);
        }
    }

    /// Give every colonist without a valid bed a bed. A partner, a child, and a parent prefer a
    /// room that holds the family. A single colonist prefers the room with the fewest beds.
    pub(crate) fn assign_homes(&mut self) {
        let mut beds = self
            .buildings
            .object_tiles(|def| self.rules.is_kind(def, BuildingKind::Bed));
        beds.sort_by_key(|t| (t.1, t.0));
        let ids: Vec<ColonistId> = self.colonists.iter().map(|(i, _)| i).collect();
        let mut owner: BTreeMap<(u16, u16), ColonistId> = BTreeMap::new();
        for &id in &ids {
            let Some(c) = self.colonists.get_mut(id) else {
                continue;
            };
            if let Some(b) = c.bed {
                if beds.contains(&b) && !owner.contains_key(&b) {
                    owner.insert(b, id);
                } else {
                    c.bed = None;
                }
            }
        }
        let room_of =
            |s: &Self, t: (u16, u16)| s.room_at(t.0, t.1).and_then(|r| r.tiles.first().copied());
        for &id in &ids {
            let Some(c) = self.colonists.get(id) else {
                continue;
            };
            if c.bed.is_some() {
                continue;
            }
            let free: Vec<(u16, u16)> = beds
                .iter()
                .copied()
                .filter(|t| !owner.contains_key(t))
                .collect();
            if free.is_empty() {
                break;
            }
            // The family: partner, parents, and children.
            let mut kin: Vec<ColonistId> = self.children_of(id);
            kin.extend([c.mother, c.father].into_iter().flatten());
            kin.extend(self.partner_of(id));
            let kin: Vec<ColonistId> = kin
                .into_iter()
                .filter(|&k| self.colonists.get(k).is_some())
                .collect();
            let family_rooms: Vec<(u16, u16)> = kin
                .iter()
                .filter_map(|&k| self.colonists.get(k).and_then(|o| o.bed))
                .filter_map(|b| room_of(self, b))
                .collect();
            let needed = 1 + kin
                .iter()
                .filter(|&&k| self.colonists.get(k).is_some_and(|o| o.bed.is_none()))
                .count();
            let used_rooms: Vec<(u16, u16)> =
                owner.keys().filter_map(|&b| room_of(self, b)).collect();
            let from = c.tile();
            let best = free.iter().copied().min_by_key(|&t| {
                let room = room_of(self, t);
                let room_beds = room.map_or(usize::MAX, |_| self.beds_in_room(t.0, t.1));
                let class = match room {
                    Some(r) if family_rooms.contains(&r) => 0u8,
                    Some(r) if !used_rooms.contains(&r) => 1,
                    _ => 2,
                };
                let fits = room_beds >= needed;
                let size_key = if fits {
                    room_beds
                } else {
                    usize::MAX - room_beds.min(usize::MAX - 1)
                };
                let dist = usize::from(t.0.abs_diff(from.0)) + usize::from(t.1.abs_diff(from.1));
                (class, !fits, size_key, dist, t.1, t.0)
            });
            if let Some(b) = best {
                owner.insert(b, id);
                if let Some(c) = self.colonists.get_mut(id) {
                    c.bed = Some(b);
                }
            }
        }
    }

    /// Set the age in age days since the last birthday. For scenario tests and debug tools.
    pub fn debug_set_age_days(&mut self, id: ColonistId, days: u32) {
        if let Some(c) = self.colonists.get_mut(id) {
            c.age_days = u16::try_from(days).unwrap_or(u16::MAX);
        }
    }

    /// Set the sex of a colonist. For scenario tests and debug tools.
    pub fn debug_set_sex(&mut self, id: ColonistId, sex: Sex) {
        if let Some(c) = self.colonists.get_mut(id) {
            c.sex = sex;
        }
    }

    /// Set all work priorities of a colonist to off. For scenario tests and debug tools.
    pub fn debug_zero_priorities(&mut self, id: ColonistId) {
        if let Some(c) = self.colonists.get_mut(id) {
            c.priorities = [0; crate::job::JobKind::COUNT];
        }
    }

    /// Make a colonist pregnant, with the days left. For scenario tests and debug tools.
    pub fn debug_make_pregnant(&mut self, mother: ColonistId, father: ColonistId, days: u32) {
        let until = self.tick + u64::from(days) * TICKS_PER_DAY;
        if let Some(c) = self.colonists.get_mut(mother) {
            c.pregnant_until = until;
            c.pregnancy_father = Some(father);
        }
    }

    /// Add a child of two colonists on the tile of the mother. For scenario tests and debug tools.
    pub fn debug_add_child(&mut self, mother: ColonistId, father: ColonistId) -> ColonistId {
        let (x, y) = self.colonists.get(mother).map_or((0, 0), |c| c.tile());
        let mut c = Colonist::new(x, y);
        self.rules.needs.init_colonist(&mut c);
        c.age = 3;
        c.mother = Some(mother);
        c.father = Some(father);
        self.colonists.insert(c)
    }
}
