//! Snow, floods, and storms (S27, R-053, R-054, R-055).
//!
//! Snow falls on a cold day with precipitation and melts on a warm day. The melt and the rain
//! raise the river level. A high river floods the land near the water: crops on it die and
//! walking is slow. A wall line stops the water (a dike). A storm can tear roofs off, wood roofs
//! first. The first days of a run are safe. All numbers are in `weather.ron`.

use crate::climate::DayWeather;
use crate::colonist::WALK_PROGRESS_PER_TICK;
use crate::loader::{Issues, parse};
use crate::needs::NeedRules;
use crate::time::{GameTime, Season, TICKS_PER_DAY};
use crate::{BuildLayer, ChronicleKind, ColonistId, Sim};
use base::content::{ContentError, MaterialCategory};
use base::rng::mix;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, VecDeque};
use worldgen::Ground;

/// Tag of the storm rolls.
const TAG_STORM: u64 = 0x5354_4f52_4d44_4159;
/// Tag of the roof rolls.
const TAG_ROOF: u64 = 0x524f_4f46_5445_4152;

/// The numbers of snow, floods, and storms (`weather.ron`).
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WeatherRules {
    /// No flood and no storm in these first game days of a run.
    pub grace_days: u32,
    /// Snow falls on a rain day with a mean at or below this, in tenths of a degree.
    pub snow_temp_max: i32,
    /// Depth that one snow day adds.
    pub snow_per_day: u32,
    /// Depth limit.
    pub snow_max: u32,
    /// Depth that melts on a day with a mean above 0, before the temperature part.
    pub melt_base: u32,
    /// Depth that melts for each degree of the mean above 0.
    pub melt_per_deg: u32,
    /// Walking speed lost in percent points for each 10 depth.
    pub snow_slow_per_10: u32,
    /// Limit of the speed that snow takes away, in percent points.
    pub snow_slow_max: u32,
    /// Limit of the river level.
    pub level_max: u32,
    /// Percent of the river level that stays after one day.
    pub level_decay_pct: u32,
    /// River level that 1 depth of melted snow adds.
    pub melt_to_level: u32,
    /// River level that a rain day above 0 degrees adds.
    pub rain_to_level: u32,
    /// The flood starts at this level or more.
    pub flood_start: u32,
    /// The flood ends below this level.
    pub flood_end: u32,
    /// Tiles from the water that the flood reaches at `flood_start`.
    pub flood_reach_min: u32,
    /// Tiles that the reach grows for each 10 level above `flood_start`.
    pub flood_reach_per_10: u32,
    /// Limit of the reach.
    pub flood_reach_max: u32,
    /// Walking speed lost on a flooded tile, in percent points.
    pub flood_slow_pct: u32,
    /// Chance in 1000 of a storm on a day that is not in winter.
    pub storm_permille: u32,
    /// Chance in 1000 that a storm of strength 1, 2, or 3 tears off one roof.
    pub storm_roof_permille: [u32; 3],
    /// A stone roof has this percent of that chance.
    pub stone_roof_pct: u32,
    /// The memory of a colonist after a storm tore roofs off. Linked from `needs.ron`.
    #[serde(skip)]
    pub storm_memory: u8,
}

/// Load and check `weather.ron`. `needs` gives the memory key.
pub fn load_weather_rules(
    file: &str,
    text: &str,
    needs: &NeedRules,
) -> Result<WeatherRules, ContentError> {
    let mut r: WeatherRules = parse(file, text)?;
    let mut issues = Issues::new(file);
    if r.flood_end >= r.flood_start || r.flood_start > r.level_max {
        issues.add("flood_end", "need flood_end < flood_start <= level_max");
    }
    if r.snow_slow_max >= 100
        || r.flood_slow_pct >= 100
        || r.snow_slow_max + r.flood_slow_pct >= 100
    {
        issues.add(
            "snow_slow_max",
            "the slow parts must add up to less than 100",
        );
    }
    if r.flood_reach_min > r.flood_reach_max || r.flood_reach_max > 32 {
        issues.add(
            "flood_reach_min",
            "need flood_reach_min <= flood_reach_max <= 32",
        );
    }
    if r.level_decay_pct > 100 || r.stone_roof_pct > 100 || r.storm_permille > 1000 {
        issues.add(
            "level_decay_pct",
            "a percent is over 100 or a chance is over 1000",
        );
    }
    if r.storm_roof_permille.iter().any(|&p| p > 1000) {
        issues.add("storm_roof_permille", "a chance is over 1000");
    }
    if r.snow_per_day == 0 || r.snow_max == 0 || r.level_max == 0 {
        issues.add(
            "snow_per_day",
            "the snow and level limits must be 1 or more",
        );
    }
    r.storm_memory = needs.memory_index("storm_damage").unwrap_or_else(|| {
        issues.add("storm_damage", "the memory is not in needs.ron");
        0
    });
    issues.finish(r)
}

/// A thing that happened with snow, floods, or storms. The caller takes it with
/// `Sim::drain_weather_events`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WeatherEvent {
    /// Snow fell.
    SnowFell {
        /// Game day number.
        day: u64,
        /// The depth after the snow.
        depth: u32,
    },
    /// The river started to flood the land.
    FloodBegan {
        /// Game day number.
        day: u64,
        /// River level.
        level: u32,
    },
    /// The flood ended.
    FloodEnded {
        /// Game day number.
        day: u64,
    },
    /// The flood destroyed crops.
    CropsFlooded {
        /// How many plants.
        count: u32,
    },
    /// A storm hit the colony.
    Storm {
        /// Strength, 1 to 3.
        strength: u8,
        /// Roofs that the storm tore off.
        roofs_lost: u32,
    },
}

/// What a part of the walking speed is made of.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WalkCause {
    /// Normal speed.
    Base,
    /// Snow on an open tile.
    Snow,
    /// Flood water on the tile.
    Flood,
}

/// One contribution to the walking speed, in percent points.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WalkPart {
    /// What it is.
    pub cause: WalkCause,
    /// Percent points that it adds. Can be negative.
    pub value: i32,
}

/// The walking speed on a tile and its parts. `pct` is the sum of the parts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WalkSpeed {
    /// Walking speed in percent.
    pub pct: u32,
    /// The parts: the base, then snow, then flood.
    pub parts: Vec<WalkPart>,
}

/// The forecast of the river for one day.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FloodDay {
    /// Game day number.
    pub day: u64,
    /// River level at the end of the day.
    pub level: u32,
    /// Snow depth at the end of the day.
    pub snow: u32,
    /// True if the level is at or above the flood level.
    pub flood: bool,
}

/// Snow, river level, and flood. Part of the state.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct WeatherState {
    pub(crate) snow: u32,
    pub(crate) level: u32,
    pub(crate) flood: bool,
    pub(crate) flooded: BTreeSet<(u16, u16)>,
    pub(crate) floods: u32,
    pub(crate) storms: u32,
}

/// The snow and the river level after a day. Returns `(snow, level, fell)`.
fn advance(rules: &WeatherRules, snow: u32, level: u32, day: &DayWeather) -> (u32, u32, u32) {
    let mean = i32::from(day.mean_temp);
    let mut snow = snow;
    let mut melted = 0;
    if mean > 0 {
        let melt = rules.melt_base + mean.unsigned_abs() * rules.melt_per_deg / 10;
        melted = melt.min(snow);
        snow -= melted;
    }
    let mut fell = 0;
    if day.rain && mean <= rules.snow_temp_max {
        let after = snow.saturating_add(rules.snow_per_day).min(rules.snow_max);
        fell = after - snow;
        snow = after;
    }
    let rain = if day.rain && mean > 0 {
        rules.rain_to_level
    } else {
        0
    };
    let level = (level * rules.level_decay_pct / 100 + melted * rules.melt_to_level + rain)
        .min(rules.level_max);
    (snow, level, fell)
}

impl Sim {
    /// Snow depth on open ground.
    pub fn snow_depth(&self) -> u32 {
        self.weather.snow
    }

    /// The river level.
    pub fn river_level(&self) -> u32 {
        self.weather.level
    }

    /// True while the river floods the land.
    pub fn flood_active(&self) -> bool {
        self.weather.flood
    }

    /// The flooded tiles, in `(x, y)` order.
    pub fn flood_tiles(&self) -> Vec<(u16, u16)> {
        self.weather.flooded.iter().copied().collect()
    }

    /// True if the tile is flooded.
    pub fn flooded(&self, x: u16, y: u16) -> bool {
        self.weather.flooded.contains(&(x, y))
    }

    /// Tiles from the water that the flood reaches at the river level now.
    pub fn flood_reach(&self) -> u32 {
        let w = &self.rules.weather;
        let above = self.weather.level.saturating_sub(w.flood_start);
        (w.flood_reach_min + above / 10 * w.flood_reach_per_10).min(w.flood_reach_max)
    }

    /// The river level and snow of the next 3 days (R-054). It is exact.
    pub fn flood_forecast(&self) -> [FloodDay; 3] {
        let (mut snow, mut level) = (self.weather.snow, self.weather.level);
        let days = self.forecast();
        days.map(|d| {
            (snow, level, _) = advance(&self.rules.weather, snow, level, &d);
            FloodDay {
                day: d.day,
                level,
                snow,
                flood: level >= self.rules.weather.flood_start,
            }
        })
    }

    /// The strength of the storm on each of the next 3 days, if one comes (R-055). It is exact.
    pub fn storm_forecast(&self) -> [Option<u8>; 3] {
        let today = self.tick / TICKS_PER_DAY;
        std::array::from_fn(|i| self.storm_on_day(today + 1 + i as u64))
    }

    /// Floods and storms that happened in this run.
    pub fn weather_counts(&self) -> (u32, u32) {
        (self.weather.floods, self.weather.storms)
    }

    /// Take the weather events that the last ticks made.
    pub fn drain_weather_events(&mut self) -> Vec<WeatherEvent> {
        std::mem::take(&mut self.weather_events)
    }

    /// The walking speed on a tile, with its parts (R-053, R-054).
    pub fn walk_speed(&self, x: u16, y: u16) -> WalkSpeed {
        let w = &self.rules.weather;
        let mut parts = vec![WalkPart {
            cause: WalkCause::Base,
            value: 100,
        }];
        if self.weather.snow > 0 && self.buildings.roof_at(x, y).is_none() {
            let loss = (self.weather.snow / 10 * w.snow_slow_per_10).min(w.snow_slow_max);
            if loss > 0 {
                parts.push(WalkPart {
                    cause: WalkCause::Snow,
                    value: -i32::try_from(loss).unwrap_or(0),
                });
            }
        }
        if self.flooded(x, y) {
            parts.push(WalkPart {
                cause: WalkCause::Flood,
                value: -i32::try_from(w.flood_slow_pct).unwrap_or(0),
            });
        }
        let sum: i32 = parts.iter().map(|p| p.value).sum();
        WalkSpeed {
            pct: u32::try_from(sum).unwrap_or(1).max(1),
            parts,
        }
    }

    /// Walk progress that a colonist gets this tick on its tile.
    pub(crate) fn walk_progress(&self, id: ColonistId) -> u16 {
        if self.weather.snow == 0 && !self.weather.flood {
            return WALK_PROGRESS_PER_TICK;
        }
        let Some(c) = self.colonists.get(id) else {
            return WALK_PROGRESS_PER_TICK;
        };
        let pct = self.walk_speed(c.x, c.y).pct.min(100);
        u16::try_from(u32::from(WALK_PROGRESS_PER_TICK) * pct / 100)
            .unwrap_or(WALK_PROGRESS_PER_TICK)
    }

    /// Check a loaded state: all flooded tiles are on the map.
    pub(crate) fn weather_state_ok(&self) -> bool {
        let w = &self.weather;
        w.flooded
            .iter()
            .all(|&(x, y)| self.map.in_bounds(i32::from(x), i32::from(y)))
            && (w.flood || w.flooded.is_empty())
    }

    /// Called when a new game day starts: snow, river, flood, and storm (R-053 to R-055).
    pub(crate) fn weather_day(&mut self) {
        let today = self.weather_today();
        let rules = self.rules.clone();
        let w = &rules.weather;
        let (snow, level, fell) = advance(w, self.weather.snow, self.weather.level, &today);
        self.weather.snow = snow;
        self.weather.level = level;
        if fell > 0 {
            self.weather_events.push(WeatherEvent::SnowFell {
                day: today.day,
                depth: snow,
            });
        }
        if self.weather.flood && level < w.flood_end {
            self.weather.flood = false;
            self.weather.flooded.clear();
            self.crisis_survived();
            self.weather_events
                .push(WeatherEvent::FloodEnded { day: today.day });
        } else if !self.weather.flood
            && today.day > u64::from(w.grace_days)
            && level >= w.flood_start
        {
            self.weather.flood = true;
            self.weather.floods += 1;
            self.chronicle_add(
                ChronicleKind::Flood,
                [i32::try_from(level).unwrap_or(i32::MAX), 0, 0],
            );
            self.weather_events.push(WeatherEvent::FloodBegan {
                day: today.day,
                level,
            });
        }
        if self.weather.flood {
            self.flood_spread();
        }
        if let Some(strength) = self.storm_on_day(today.day) {
            self.storm_hits(today.day, strength);
        }
    }

    /// Find the flooded tiles at the river level now and destroy the crops on them. The water
    /// spreads from the water tiles over the land, stops at walls and solid buildings, and
    /// goes as far as the reach.
    fn flood_spread(&mut self) {
        let reach = self.flood_reach();
        let (w, h) = (self.map.width() as i32, self.map.height() as i32);
        let mut seen: BTreeSet<(u16, u16)> = BTreeSet::new();
        let mut queue: VecDeque<(u16, u16, u32)> = VecDeque::new();
        for y in 0..h {
            for x in 0..w {
                if self.map.ground(x, y) == Some(Ground::Water) {
                    queue.push_back((x as u16, y as u16, 0));
                }
            }
        }
        let mut flooded = BTreeSet::new();
        while let Some((x, y, depth)) = queue.pop_front() {
            if depth >= reach {
                continue;
            }
            for (dx, dy) in crate::job::NEIGHBORS {
                let (nx, ny) = (i32::from(x) + dx, i32::from(y) + dy);
                if !self.map.in_bounds(nx, ny)
                    || self.map.ground(nx, ny) == Some(Ground::Water)
                    || self.map.wall(nx, ny)
                    || self.map.solid(nx, ny)
                {
                    continue;
                }
                let tile = (nx as u16, ny as u16);
                if seen.insert(tile) {
                    flooded.insert(tile);
                    queue.push_back((tile.0, tile.1, depth + 1));
                }
            }
        }
        let lost: Vec<(u16, u16)> = self
            .farm
            .plants
            .keys()
            .filter(|t| flooded.contains(t))
            .copied()
            .collect();
        for t in &lost {
            self.farm.plants.remove(t);
        }
        self.weather.flooded = flooded;
        if let Ok(count) = u32::try_from(lost.len())
            && count > 0
        {
            self.weather_events
                .push(WeatherEvent::CropsFlooded { count });
        }
    }

    /// The strength of the storm on a game day, or `None` (R-055). Roll from the weather seed.
    fn storm_on_day(&self, day: u64) -> Option<u8> {
        let w = &self.rules.weather;
        if day <= u64::from(w.grace_days)
            || GameTime::from_tick(day * TICKS_PER_DAY).season == Season::Winter
        {
            return None;
        }
        let roll = mix(self.climate.weather_seed ^ TAG_STORM ^ mix(day));
        if roll % 1000 >= u64::from(w.storm_permille) {
            return None;
        }
        Some(1 + ((roll >> 20) % 3) as u8)
    }

    /// A storm hits: each roof can be torn off. Wood roofs go first.
    fn storm_hits(&mut self, day: u64, strength: u8) {
        let w = self.rules.weather.clone();
        let base = w.storm_roof_permille[usize::from(strength - 1)];
        let seed = mix(self.climate.weather_seed ^ TAG_ROOF ^ mix(day));
        let mut gone: Vec<(u16, u16)> = Vec::new();
        for (x, y, layer, b) in self.buildings.iter() {
            if layer != BuildLayer::Roof {
                continue;
            }
            let stone = self
                .rules
                .materials
                .get(usize::from(b.material))
                .is_some_and(|m| m.category == MaterialCategory::Stone);
            let permille = if stone {
                base * w.stone_roof_pct / 100
            } else {
                base
            };
            let roll = mix(seed ^ (u64::from(x) << 16) ^ u64::from(y));
            if roll % 1000 < u64::from(permille) {
                gone.push((x, y));
            }
        }
        for &(x, y) in &gone {
            self.remove_building(x, y, BuildLayer::Roof);
        }
        let lost = u32::try_from(gone.len()).unwrap_or(u32::MAX);
        self.weather.storms += 1;
        if lost > 0 {
            let ids: Vec<ColonistId> = self.colonists.iter().map(|(id, _)| id).collect();
            for id in ids {
                self.give_memory(id, w.storm_memory);
            }
        }
        self.chronicle_add(
            ChronicleKind::Storm,
            [
                i32::from(strength),
                i32::try_from(lost).unwrap_or(i32::MAX),
                0,
            ],
        );
        self.weather_events.push(WeatherEvent::Storm {
            strength,
            roofs_lost: lost,
        });
    }

    // ---- debug hooks (scenario tests) ----

    /// Set the snow depth. For tests.
    pub fn debug_set_snow(&mut self, depth: u32) {
        self.weather.snow = depth;
    }

    /// Set the river level. For tests.
    pub fn debug_set_river_level(&mut self, level: u32) {
        self.weather.level = level;
    }
}
