//! Life: warmth, health, mood, and death (R-001, R-013 to R-016).
//!
//! Each game hour the sim updates hunger, rest, hypothermia, heatstroke, health, and the
//! mood effects of every colonist. Every player-visible value is a sum of named parts (the
//! why chain): the comfort band, the mood, the health change, and the work speed. The
//! update and the read functions use the same part functions.

use crate::colonist::{Colonist, ColonistId, Memory, NeedAct, Sex};
use crate::job::{AbortReason, JobKind};
use crate::needs::NeedRules;
use crate::rules::SimRules;
use crate::skill::WorkSpeed;
use crate::time::TICKS_PER_HOUR;
use crate::{PathRequest, Sim};
use base::content::BuildingKind;
use serde::{Deserialize, Serialize};

/// Thousandths in one displayed need unit.
pub const MILLI: i32 = 1000;

fn whole(v: u32) -> i32 {
    i32::try_from(v).unwrap_or(i32::MAX)
}

// ---- conditions and health ----

/// A condition of a colonist (R-016).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Condition {
    /// Hunger is 0 (R-011).
    Starving,
    /// Hypothermia is above 0 (R-013).
    Hypothermia,
    /// Heatstroke is above 0 (R-013).
    Heatstroke,
    /// Rest is 0 (R-012).
    Exhausted,
    /// A wound that sleep in a bed heals (R-016).
    Wounded,
}

impl Condition {
    /// Display name.
    pub fn name(self) -> &'static str {
        match self {
            Condition::Starving => "Starving",
            Condition::Hypothermia => "Hypothermia",
            Condition::Heatstroke => "Heatstroke",
            Condition::Exhausted => "Exhausted",
            Condition::Wounded => "Wounded",
        }
    }
}

/// Why health changes in one game hour.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HealthCause {
    /// Starving colonist.
    Starving,
    /// Hypothermia above the limit.
    Hypothermia,
    /// Heatstroke above the limit.
    Heatstroke,
    /// A wound.
    Wounded,
    /// Healing in a bed, with no condition.
    HealingInBed,
    /// Healing elsewhere, with no condition.
    Healing,
}

impl HealthCause {
    /// Display name.
    pub fn name(self) -> &'static str {
        match self {
            HealthCause::Starving => "Starving",
            HealthCause::Hypothermia => "Hypothermia",
            HealthCause::Heatstroke => "Heatstroke",
            HealthCause::Wounded => "Wound",
            HealthCause::HealingInBed => "Healing in a bed",
            HealthCause::Healing => "Healing",
        }
    }
}

/// One named contribution to the health change of one game hour.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HealthPart {
    /// The cause.
    pub cause: HealthCause,
    /// Health points per game hour. Negative for harm.
    pub per_hour: i32,
}

/// The health of a colonist with the why chain (R-016).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HealthReport {
    /// Health now.
    pub health: i32,
    /// Largest health.
    pub max: i32,
    /// The conditions now.
    pub conditions: Vec<Condition>,
    /// The parts of the change in the next game hour, in a fixed order.
    pub parts: Vec<HealthPart>,
    /// Sum of the parts.
    pub per_hour: i32,
}

fn conditions_of(rules: &NeedRules, c: &Colonist) -> Vec<Condition> {
    let _ = rules;
    let mut list = Vec::new();
    if c.hunger <= 0 {
        list.push(Condition::Starving);
    }
    if c.hypothermia > 0 {
        list.push(Condition::Hypothermia);
    }
    if c.heatstroke > 0 {
        list.push(Condition::Heatstroke);
    }
    if c.rest <= 0 {
        list.push(Condition::Exhausted);
    }
    if c.wounded {
        list.push(Condition::Wounded);
    }
    list
}

fn health_parts(rules: &NeedRules, c: &Colonist) -> Vec<HealthPart> {
    let mut parts = Vec::new();
    let mut harm = |cause: HealthCause, loss: u32| {
        parts.push(HealthPart {
            cause,
            per_hour: -whole(loss),
        });
    };
    if c.hunger <= 0 {
        harm(HealthCause::Starving, rules.starving_health_loss);
    }
    if c.hypothermia > whole(rules.hazard_limit) {
        harm(HealthCause::Hypothermia, rules.hazard_health_loss);
    }
    if c.heatstroke > whole(rules.hazard_limit) {
        harm(HealthCause::Heatstroke, rules.hazard_health_loss);
    }
    if c.wounded {
        harm(HealthCause::Wounded, rules.wounded_health_loss);
    }
    if conditions_of(rules, c).is_empty() {
        let in_bed = matches!(
            c.act,
            Some(NeedAct::Sleep {
                bed: Some(_),
                asleep: true
            })
        );
        if in_bed {
            parts.push(HealthPart {
                cause: HealthCause::HealingInBed,
                per_hour: whole(rules.heal_in_bed),
            });
        } else {
            parts.push(HealthPart {
                cause: HealthCause::Healing,
                per_hour: whole(rules.heal_elsewhere),
            });
        }
    }
    parts
}

/// Why a colonist died.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeathCause {
    /// Hunger.
    Starvation,
    /// Cold.
    Hypothermia,
    /// Heat.
    Heatstroke,
    /// A wound.
    Wounds,
    /// No health part explains it (a debug command).
    Unknown,
}

impl DeathCause {
    /// Display name.
    pub fn name(self) -> &'static str {
        match self {
            DeathCause::Starvation => "Starvation",
            DeathCause::Hypothermia => "Hypothermia",
            DeathCause::Heatstroke => "Heatstroke",
            DeathCause::Wounds => "Wounds",
            DeathCause::Unknown => "Unknown",
        }
    }
}

/// A dead colonist (the graveyard keeps these).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Grave {
    /// Tile column of the death.
    pub x: u16,
    /// Tile row of the death.
    pub y: u16,
    /// The tick of the death.
    pub tick: u64,
    /// Age in years.
    pub age: u8,
    /// Sex.
    pub sex: Sex,
    /// The cause.
    pub cause: DeathCause,
}

/// Life events for the interface. They are not part of the state.
/// `Sim::drain_life_events` gives them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LifeEvent {
    /// A colonist died.
    Died {
        /// Where.
        x: u16,
        /// Where.
        y: u16,
        /// The cause.
        cause: DeathCause,
    },
    /// A colonist left the colony after a long breakdown (R-015).
    Left {
        /// Where.
        x: u16,
        /// Where.
        y: u16,
    },
}

// ---- comfort band ----

/// Where a part of the comfort band lower limit comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ComfortCause {
    /// The base limit of the band.
    Base,
    /// The worn clothes.
    Clothes {
        /// Material index of the clothes.
        material: u16,
    },
    /// A trait.
    Trait {
        /// Trait index.
        index: u8,
    },
}

/// One named contribution to the lower limit of the comfort band.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ComfortPart {
    /// The cause.
    pub cause: ComfortCause,
    /// Tenths of a degree.
    pub tenths: i32,
}

/// The comfort band of a colonist (R-001).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ComfortBand {
    /// Parts of the lower limit.
    pub parts: Vec<ComfortPart>,
    /// Lower limit in tenths of a degree: the sum of the parts.
    pub low: i32,
    /// Upper limit in tenths of a degree.
    pub high: i32,
}

fn comfort_parts(rules: &SimRules, c: &Colonist) -> Vec<ComfortPart> {
    let n = &rules.needs;
    let mut parts = vec![ComfortPart {
        cause: ComfortCause::Base,
        tenths: i32::from(n.comfort_low),
    }];
    if let Some(material) = c.clothes
        && let Some(def) = n.clothes_def(material)
    {
        parts.push(ComfortPart {
            cause: ComfortCause::Clothes { material },
            tenths: -i32::from(def.insulation),
        });
    }
    for &index in &c.traits {
        if let Some(def) = rules.traits.get(usize::from(index))
            && def.comfort_low_delta != 0
        {
            parts.push(ComfortPart {
                cause: ComfortCause::Trait { index },
                tenths: i32::from(def.comfort_low_delta),
            });
        }
    }
    parts
}

fn comfort_band(rules: &SimRules, c: &Colonist) -> ComfortBand {
    let parts = comfort_parts(rules, c);
    let low = parts.iter().map(|p| p.tenths).sum();
    ComfortBand {
        parts,
        low,
        high: i32::from(rules.needs.comfort_high),
    }
}

// ---- mood ----

/// Where a mood modifier comes from. The list is open: later sprints add social sources.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum MoodCause {
    /// The base mood.
    Base,
    /// Hunger at or below the eating limit.
    Hungry,
    /// Hunger 0.
    Starving,
    /// Rest at or below the sleeping limit.
    Tired,
    /// Rest 0.
    Exhausted,
    /// Temperature below the lower limit of the comfort band.
    Cold,
    /// Temperature above the upper limit of the comfort band.
    Hot,
    /// A wound.
    Wounded,
    /// The beauty of the room.
    RoomBeauty {
        /// Beauty of a room tile on average.
        per_tile: i32,
    },
    /// A timed memory from `needs.ron`.
    Memory {
        /// Index in the memory list.
        key: u8,
    },
}

/// One mood modifier with its value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MoodPart {
    /// The cause.
    pub cause: MoodCause,
    /// Mood points.
    pub value: i32,
}

/// The mood of a colonist with the why chain (R-014).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Mood {
    /// All modifiers; the first is the base.
    pub parts: Vec<MoodPart>,
    /// Sum of the parts, not limited.
    pub sum: i32,
    /// The mood: `sum` limited to the mood range.
    pub total: i32,
}

/// Inputs for the mood that depend on the place of the colonist.
struct Place {
    /// Tile temperature in tenths, if the tile is on the map.
    temp: Option<i32>,
    /// Beauty of a room tile on average, if the tile is in a room.
    beauty: Option<i32>,
}

fn mood_of(rules: &SimRules, c: &Colonist, place: &Place, tick: u64) -> Mood {
    let n = &rules.needs;
    let mut parts = vec![MoodPart {
        cause: MoodCause::Base,
        value: i32::from(n.mood_base),
    }];
    let mut add = |cause: MoodCause, value: i16| {
        if value != 0 {
            parts.push(MoodPart {
                cause,
                value: i32::from(value),
            });
        }
    };
    if c.hunger <= 0 {
        add(MoodCause::Starving, n.mood_starving);
    } else if c.hunger <= whole(n.hunger_eat_at) * MILLI {
        add(MoodCause::Hungry, n.mood_hungry);
    }
    if c.rest <= 0 {
        add(MoodCause::Exhausted, n.mood_exhausted);
    } else if c.rest <= whole(n.rest_sleep_at) * MILLI {
        add(MoodCause::Tired, n.mood_tired);
    }
    if let Some(temp) = place.temp {
        let band = comfort_band(rules, c);
        if temp < band.low {
            add(MoodCause::Cold, n.mood_cold);
        } else if temp > band.high {
            add(MoodCause::Hot, n.mood_hot);
        }
    }
    if c.wounded {
        add(MoodCause::Wounded, n.mood_wounded);
    }
    if let Some(per_tile) = place.beauty {
        let cap = i32::from(n.beauty_mood_max);
        let value = (per_tile / n.beauty_divisor.max(1)).clamp(-cap, cap);
        if value != 0 {
            parts.push(MoodPart {
                cause: MoodCause::RoomBeauty { per_tile },
                value,
            });
        }
    }
    let danger_pct: i32 = c
        .traits
        .iter()
        .filter_map(|&t| rules.traits.get(usize::from(t)))
        .map(|t| i32::from(t.danger_mood_pct))
        .fold(100, |acc, pct| acc * pct / 100);
    for m in &c.memories {
        if m.until <= tick {
            continue;
        }
        let Some(def) = n.memories.get(usize::from(m.key)) else {
            continue;
        };
        let mut value = i32::from(def.value);
        if def.danger {
            value = value * danger_pct / 100;
        }
        if value != 0 {
            parts.push(MoodPart {
                cause: MoodCause::Memory { key: m.key },
                value,
            });
        }
    }
    let sum: i32 = parts.iter().map(|p| p.value).sum();
    let total = sum.clamp(i32::from(n.mood_min), i32::from(n.mood_max));
    Mood { parts, sum, total }
}

// ---- work speed ----

/// Where a work speed change from life comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum SpeedModCause {
    /// Mood at or above the high limit (R-015).
    HighMood,
    /// Mood below the low limit (R-015).
    LowMood,
    /// Rest 0 (R-012).
    Exhausted,
    /// A trait.
    Trait {
        /// Trait index.
        index: u8,
    },
}

/// One named change of the work speed, in percent points.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SpeedMod {
    /// The cause.
    pub cause: SpeedModCause,
    /// Percent points.
    pub pct: i32,
}

/// The work speed of a colonist with the skill parts and the life parts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EffectiveSpeed {
    /// The base and skill parts (R-010).
    pub base: WorkSpeed,
    /// Changes from mood, exhaustion, and traits.
    pub mods: Vec<SpeedMod>,
    /// Sum of all parts in percent, not below 0. 100 is normal speed.
    pub total_pct: i32,
}

// ---- Sim ----

impl Sim {
    fn place_of(&self, c: &Colonist) -> Place {
        let (x, y) = c.tile();
        Place {
            temp: self.tile_temperature(x, y).map(|t| i32::from(t.total)),
            beauty: self.room_beauty(x, y).map(|b| b.per_tile),
        }
    }

    /// The comfort band of a colonist as a sum of named parts (R-001).
    pub fn comfort_band(&self, id: ColonistId) -> Option<ComfortBand> {
        Some(comfort_band(&self.rules, self.colonists.get(id)?))
    }

    /// The mood of a colonist as a sum of named modifiers (R-014).
    pub fn mood(&self, id: ColonistId) -> Option<Mood> {
        let c = self.colonists.get(id)?;
        Some(mood_of(&self.rules, c, &self.place_of(c), self.tick))
    }

    /// The conditions of a colonist (R-016).
    pub fn conditions(&self, id: ColonistId) -> Option<Vec<Condition>> {
        Some(conditions_of(&self.rules.needs, self.colonists.get(id)?))
    }

    /// The health of a colonist with the parts of the next hourly change (R-016).
    pub fn health_report(&self, id: ColonistId) -> Option<HealthReport> {
        let c = self.colonists.get(id)?;
        let rules = &self.rules.needs;
        let parts = health_parts(rules, c);
        let per_hour = parts.iter().map(|p| p.per_hour).sum();
        Some(HealthReport {
            health: c.health,
            max: whole(rules.health_max),
            conditions: conditions_of(rules, c),
            parts,
            per_hour,
        })
    }

    /// The work speed of a colonist for a job type with all parts: base, skill level, mood,
    /// exhaustion, and traits (R-010, R-012, R-015). The work system uses this function.
    pub fn effective_work_speed(&self, id: ColonistId, kind: JobKind) -> Option<EffectiveSpeed> {
        let base = self.work_speed(id, kind)?;
        let c = self.colonists.get(id)?;
        let n = &self.rules.needs;
        let mut mods = Vec::new();
        if n.enabled && !self.needs_off {
            let mood = mood_of(&self.rules, c, &self.place_of(c), self.tick).total;
            if mood >= i32::from(n.mood_high_from) {
                mods.push(SpeedMod {
                    cause: SpeedModCause::HighMood,
                    pct: i32::from(n.mood_high_speed_pct),
                });
            } else if mood < i32::from(n.mood_low_below) {
                mods.push(SpeedMod {
                    cause: SpeedModCause::LowMood,
                    pct: i32::from(n.mood_low_speed_pct),
                });
            }
            if c.rest <= 0 {
                mods.push(SpeedMod {
                    cause: SpeedModCause::Exhausted,
                    pct: i32::from(n.exhausted_speed_pct),
                });
            }
            for &index in &c.traits {
                if let Some(def) = self.rules.traits.get(usize::from(index))
                    && def.work_speed_pct != 0
                {
                    mods.push(SpeedMod {
                        cause: SpeedModCause::Trait { index },
                        pct: i32::from(def.work_speed_pct),
                    });
                }
            }
        }
        let total = base.total_pct + mods.iter().map(|m| m.pct).sum::<i32>();
        Some(EffectiveSpeed {
            base,
            mods,
            total_pct: total.max(0),
        })
    }

    /// The units that a colonist carries in one stack: the stack size with the carry
    /// capacity of the traits (GAME_DESIGN 7.2).
    pub fn carry_capacity(&self, id: ColonistId) -> Option<u32> {
        let c = self.colonists.get(id)?;
        let pct = c
            .traits
            .iter()
            .filter_map(|&t| self.rules.traits.get(usize::from(t)))
            .fold(100u64, |acc, t| acc * u64::from(t.carry_pct) / 100);
        let units = u64::from(self.rules.jobs.stack_max) * pct / 100;
        Some(u32::try_from(units).unwrap_or(u32::MAX))
    }

    /// The graves, oldest first.
    pub fn graveyard(&self) -> &[Grave] {
        &self.graveyard
    }

    /// Take the life events that the last ticks made.
    pub fn drain_life_events(&mut self) -> Vec<LifeEvent> {
        std::mem::take(&mut self.life_events)
    }

    // ---- debug hooks (scenario tests and tools) ----

    /// Switch the needs system on or off. Off: no hunger, rest, warmth, health, mood
    /// effects, or need actions. For tests. Not part of the state.
    pub fn debug_set_needs_enabled(&mut self, on: bool) {
        self.needs_off = !on;
    }

    /// Set hunger in whole units.
    pub fn debug_set_hunger(&mut self, id: ColonistId, units: u32) {
        let max = self.rules.needs.hunger_max;
        if let Some(c) = self.colonists.get_mut(id) {
            c.hunger = whole(units.min(max)) * MILLI;
        }
    }

    /// Set rest in whole units.
    pub fn debug_set_rest(&mut self, id: ColonistId, units: u32) {
        let max = self.rules.needs.rest_max;
        if let Some(c) = self.colonists.get_mut(id) {
            c.rest = whole(units.min(max)) * MILLI;
        }
    }

    /// Set health.
    pub fn debug_set_health(&mut self, id: ColonistId, health: u32) {
        let max = self.rules.needs.health_max;
        if let Some(c) = self.colonists.get_mut(id) {
            c.health = whole(health.min(max));
        }
    }

    /// Give a wound to a colonist.
    pub fn debug_wound(&mut self, id: ColonistId) {
        if let Some(c) = self.colonists.get_mut(id) {
            c.wounded = true;
            c.wound_bed_hours = 0;
        }
    }

    /// Set the traits of a colonist (trait indices in `traits.ron` order).
    pub fn debug_set_traits(&mut self, id: ColonistId, traits: &[u8]) {
        let count = self.rules.traits.len();
        if let Some(c) = self.colonists.get_mut(id) {
            c.traits = traits
                .iter()
                .copied()
                .filter(|&t| usize::from(t) < count)
                .collect();
        }
    }

    /// Set the worn clothes of a colonist (a material index) or take them off.
    pub fn debug_set_clothes(&mut self, id: ColonistId, material: Option<u16>) {
        if let Some(c) = self.colonists.get_mut(id) {
            c.clothes = material;
        }
    }

    /// Give a colonist a memory by its key. It lasts for the hours in `needs.ron`.
    /// Return false if the key is unknown.
    pub fn debug_add_memory(&mut self, id: ColonistId, key: &str) -> bool {
        let Some(index) = self.rules.needs.memory_index(key) else {
            return false;
        };
        self.give_memory(id, index);
        true
    }

    /// Kill a colonist now.
    pub fn debug_kill(&mut self, id: ColonistId) {
        self.kill(id, DeathCause::Unknown);
    }

    // ---- the hourly update ----

    /// Give a colonist a memory. The same memory again renews it.
    pub(crate) fn give_memory(&mut self, id: ColonistId, key: u8) {
        let Some(def) = self.rules.needs.memories.get(usize::from(key)) else {
            return;
        };
        let until = self.tick + u64::from(def.hours) * TICKS_PER_HOUR;
        if let Some(c) = self.colonists.get_mut(id) {
            c.memories.retain(|m| m.key != key);
            c.memories.push(Memory { key, until });
        }
    }

    /// Each game hour (when the tick number is a multiple of 100): hunger, rest, warmth,
    /// wounds, health, death, and the mood effects. Colonists run in slot order.
    pub(crate) fn life_tick(&mut self) {
        if !self.tick.is_multiple_of(TICKS_PER_HOUR) || !self.needs_active() {
            return;
        }
        let rules = self.rules.clone();
        let n = &rules.needs;
        let hour = (self.tick / TICKS_PER_HOUR) % 24;
        let ids: Vec<ColonistId> = self.colonists.iter().map(|(id, _)| id).collect();
        let mut dead: Vec<(ColonistId, DeathCause)> = Vec::new();
        let mut breaks: Vec<(ColonistId, bool)> = Vec::new();
        let mut leaving: Vec<ColonistId> = Vec::new();
        for id in ids {
            let Some(c) = self.colonists.get(id) else {
                continue;
            };
            let place = self.place_of(c);
            let band = comfort_band(&rules, c);
            let hunger_pct: u64 = c
                .traits
                .iter()
                .filter_map(|&t| rules.traits.get(usize::from(t)))
                .fold(100u64, |acc, t| acc * u64::from(t.hunger_rate_pct) / 100);
            let Some(c) = self.colonists.get_mut(id) else {
                continue;
            };
            // R-011: the day total is spread over the hours without rounding drift.
            let day = u64::from(n.hunger_per_day) * 1000;
            let share = |h: u64| day * h / 24;
            let loss = (share(hour + 1) - share(hour)) * hunger_pct / 100;
            c.hunger = (c.hunger - i32::try_from(loss).unwrap_or(i32::MAX)).max(0);
            // R-012
            let asleep = matches!(c.act, Some(NeedAct::Sleep { asleep: true, .. }));
            let in_bed = matches!(
                c.act,
                Some(NeedAct::Sleep {
                    bed: Some(_),
                    asleep: true
                })
            );
            let rest_max = whole(n.rest_max) * MILLI;
            if asleep {
                let gain = if in_bed {
                    n.sleep_per_hour_bed
                } else {
                    n.sleep_per_hour_ground
                };
                c.rest = (c.rest + whole(gain) * MILLI).min(rest_max);
                if in_bed && c.wounded {
                    c.wound_bed_hours += 1;
                    if c.wound_bed_hours >= n.wounded_bed_hours {
                        c.wounded = false;
                        c.wound_bed_hours = 0;
                    }
                }
            } else {
                c.rest = (c.rest - whole(n.rest_per_awake_hour) * MILLI).max(0);
            }
            // R-013
            if let Some(temp) = place.temp {
                let margin = i32::from(n.hazard_margin);
                let max = whole(n.hazard_max);
                if temp < band.low - margin {
                    c.hypothermia = (c.hypothermia + whole(n.hazard_step)).min(max);
                } else if temp > band.high + margin {
                    c.heatstroke = (c.heatstroke + whole(n.hazard_step)).min(max);
                } else if temp >= band.low && temp <= band.high {
                    c.hypothermia = (c.hypothermia - whole(n.hazard_recovery)).max(0);
                    c.heatstroke = (c.heatstroke - whole(n.hazard_recovery)).max(0);
                }
            }
            // R-016
            let parts = health_parts(n, c);
            let change: i32 = parts.iter().map(|p| p.per_hour).sum();
            c.health = (c.health + change).clamp(0, whole(n.health_max));
            c.memories.retain(|m| m.until > self.tick);
            let woke = asleep && c.rest >= rest_max;
            let was_in_bed = in_bed;
            if woke {
                c.act = None;
                c.need_next = self.tick;
            }
            if c.health <= 0 {
                dead.push((id, death_cause(&parts)));
                continue;
            }
            if woke {
                let key = if was_in_bed {
                    "slept_bed"
                } else {
                    "slept_ground"
                };
                if let Some(index) = n.memory_index(key) {
                    self.give_memory(id, index);
                }
            }
            // R-014, R-015
            let Some(c) = self.colonists.get(id) else {
                continue;
            };
            let place = self.place_of(c);
            let mood = mood_of(&rules, c, &place, self.tick).total;
            let on_break = c.break_until > self.tick;
            let hours = c.low_mood_hours;
            let mut next_hours = 0;
            if mood < i32::from(n.mood_break_below) {
                next_hours = hours + 1;
                if !on_break {
                    breaks.push((id, false));
                }
                if next_hours >= n.leave_after_hours {
                    leaving.push(id);
                }
            } else if mood < i32::from(n.mood_low_below)
                && !on_break
                && self.rng.chance(n.wander_chance_pct, 100)
            {
                breaks.push((id, true));
            }
            if let Some(c) = self.colonists.get_mut(id) {
                c.low_mood_hours = next_hours;
            }
        }
        for (id, wander) in breaks {
            if leaving.contains(&id) {
                continue;
            }
            self.begin_break(id, wander);
        }
        for id in leaving {
            self.leave(id);
        }
        for (id, cause) in dead {
            self.kill(id, cause);
        }
    }

    /// A colonist stops work for a breakdown or a walk-around (R-015). A walk-around also
    /// picks a tile near the colonist.
    fn begin_break(&mut self, id: ColonistId, wander: bool) {
        let n = &self.rules.needs;
        let hours = if wander {
            n.wander_hours
        } else {
            n.break_hours
        };
        let radius = i32::from(n.wander_radius);
        let until = self.tick + u64::from(hours) * TICKS_PER_HOUR;
        self.abort_job(id, AbortReason::JobGone, false);
        let Some(c) = self.colonists.get_mut(id) else {
            return;
        };
        c.break_until = until;
        if !wander || c.act.is_some() {
            return;
        }
        let (x, y) = (i32::from(c.x), i32::from(c.y));
        let tx = x + self.rng.range(-radius, radius + 1);
        let ty = y + self.rng.range(-radius, radius + 1);
        if self.map.passable(tx, ty) && self.regions.connected((x, y), (tx, ty)) {
            self.start_leg(id, (tx as u16, ty as u16));
        }
    }

    /// Stop everything that a colonist holds: the job, the need action, the path.
    fn release_colonist(&mut self, id: ColonistId) {
        self.abort_job(id, AbortReason::JobGone, false);
        self.cancel_act(id, 0);
        self.path_queue.retain(|r: &PathRequest| r.colonist != id);
    }

    /// A colonist leaves the colony (R-015).
    fn leave(&mut self, id: ColonistId) {
        let Some(c) = self.colonists.get(id) else {
            return;
        };
        let (x, y) = c.tile();
        let age = c.age;
        self.release_colonist(id);
        self.colonists.remove(id);
        self.life_events.push(LifeEvent::Left { x, y });
        self.chronicle_gone(crate::ChronicleKind::Left, age);
    }

    /// A colonist dies (R-016). The others get the memory "colonist_died".
    pub(crate) fn kill(&mut self, id: ColonistId, cause: DeathCause) {
        let Some(c) = self.colonists.get(id) else {
            return;
        };
        let grave = Grave {
            x: c.x,
            y: c.y,
            tick: self.tick,
            age: c.age,
            sex: c.sex,
            cause,
        };
        self.release_colonist(id);
        self.colonists.remove(id);
        self.graveyard.push(grave);
        let cap = self.rules.needs.graveyard_max as usize;
        if self.graveyard.len() > cap {
            let extra = self.graveyard.len() - cap;
            self.graveyard.drain(..extra);
        }
        self.life_events.push(LifeEvent::Died {
            x: grave.x,
            y: grave.y,
            cause,
        });
        self.chronicle_gone(crate::ChronicleKind::of_death(cause), grave.age);
        if let Some(index) = self.rules.needs.memory_index("colonist_died") {
            let others: Vec<ColonistId> = self.colonists.iter().map(|(i, _)| i).collect();
            for other in others {
                self.give_memory(other, index);
            }
        }
    }

    /// Check the life state of a loaded `Sim`: indices, ranges, and tiles.
    pub(crate) fn life_state_ok(&self) -> bool {
        let n = &self.rules.needs;
        let inside = |t: (u16, u16)| self.map.in_bounds(i32::from(t.0), i32::from(t.1));
        let max_hunger = whole(n.hunger_max) * MILLI;
        let max_rest = whole(n.rest_max) * MILLI;
        let materials = self.rules.materials.len();
        self.colonists.iter().all(|(_, c)| {
            c.traits
                .iter()
                .all(|&t| usize::from(t) < self.rules.traits.len())
                && c.memories
                    .iter()
                    .all(|m| usize::from(m.key) < n.memories.len())
                && c.clothes.is_none_or(|m| usize::from(m) < materials)
                && (0..=max_hunger).contains(&c.hunger)
                && (0..=max_rest).contains(&c.rest)
                && (0..=whole(n.health_max)).contains(&c.health)
                && (0..=whole(n.hazard_max)).contains(&c.hypothermia)
                && (0..=whole(n.hazard_max)).contains(&c.heatstroke)
                && match c.act {
                    Some(NeedAct::Eat { stand, .. } | NeedAct::Wear { stand, .. }) => inside(stand),
                    Some(NeedAct::Sleep { bed, .. }) => bed.is_none_or(inside),
                    None => true,
                }
        })
    }

    /// True if the tile holds a finished bed.
    pub(crate) fn is_bed_tile(&self, x: u16, y: u16) -> bool {
        self.buildings.object_at(x, y).is_some_and(|b| {
            self.rules
                .building(b.def)
                .is_some_and(|d| d.kind == BuildingKind::Bed)
        })
    }
}

/// The main cause of the harm in an hour: the largest loss, ties in the order of the list.
fn death_cause(parts: &[HealthPart]) -> DeathCause {
    let mut best: Option<(&HealthPart, DeathCause)> = None;
    for p in parts {
        let cause = match p.cause {
            HealthCause::Starving => DeathCause::Starvation,
            HealthCause::Hypothermia => DeathCause::Hypothermia,
            HealthCause::Heatstroke => DeathCause::Heatstroke,
            HealthCause::Wounded => DeathCause::Wounds,
            HealthCause::HealingInBed | HealthCause::Healing => continue,
        };
        if best.is_none_or(|(b, _)| p.per_hour < b.per_hour) {
            best = Some((p, cause));
        }
    }
    best.map_or(DeathCause::Unknown, |(_, c)| c)
}

impl Sim {
    /// Give the colonists of a new colony an age, a sex, and a trait (GAME_DESIGN 7.2).
    /// Uses its own random stream, so the main stream does not change.
    pub(crate) fn roll_colonists(&mut self, seed: u64) {
        let mut rng = base::Pcg32::from_seed(seed ^ 0x7241_4954_5345_4544);
        let n = &self.rules.needs;
        let (lo, hi) = (u32::from(n.age_min), u32::from(n.age_max));
        let trait_count = u32::try_from(self.rules.traits.len()).unwrap_or(0);
        let ids: Vec<ColonistId> = self.colonists.iter().map(|(id, _)| id).collect();
        for id in ids {
            let age = u8::try_from(lo + rng.below(hi - lo + 1)).unwrap_or(n.age_min);
            let sex = if rng.below(2) == 0 {
                Sex::Female
            } else {
                Sex::Male
            };
            let picked = rng.below(trait_count.max(1));
            if let Some(c) = self.colonists.get_mut(id) {
                c.age = age;
                c.sex = sex;
                if trait_count > 0 {
                    c.traits = vec![u8::try_from(picked).unwrap_or(0)];
                }
            }
        }
    }
}
