//! Climate: the season curve, the weather, the outdoor temperature (R-002, R-003), the
//! room temperature (R-022), freezing water (R-004), and the tile temperature why chain.
//!
//! All temperatures are integers in tenths of a degree Celsius. Room temperature has an
//! inner precision of thousandths of a degree, so a slow change is not lost by rounding.
//!
//! The weather is a function of the game seed and the day number only. It does not use the
//! random stream of the `Sim`, so the forecast is the true weather.

use crate::Sim;
use crate::room::{Room, Rooms};
use crate::time::{GameTime, TICKS_PER_DAY, TICKS_PER_HOUR};
use base::Pcg32;
use base::content::{ContentError, ContentIssue};
use base::rng::mix;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Days in a forecast (R-003).
pub const FORECAST_DAYS: usize = 3;
/// Days in one year, the length of the season curve.
const YEAR_DAYS: usize = 40;
/// Hours in one day, the length of the daily curve.
const DAY_HOURS: usize = 24;
/// Inner precision of the room temperature: units in one tenth of a degree.
const MILLI_PER_TENTH: i64 = 100;

// ---- content ----

/// Numbers for the climate (`climate.ron`). All integers.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClimateRules {
    /// Season curve in per-mille, one value for each day of the year (R-002).
    pub season_curve: Vec<i16>,
    /// Daily curve in tenths of a degree, one value for each hour (R-002).
    pub daily_curve: Vec<i16>,
    /// The daily change of the weather offset is from minus this to plus this (R-003).
    pub weather_step_max: u16,
    /// The weather offset stays between minus this and plus this (R-003).
    pub weather_limit: u16,
    /// Annual mean of a colony without embark data, tenths of a degree.
    pub default_mean: i16,
    /// Seasonal amplitude of a colony without embark data, tenths of a degree.
    pub default_amplitude: i16,
    /// Moisture of a colony without embark data (0 to 255).
    pub default_moisture: u8,
    /// Chance of rain at moisture 255, in percent (R-003).
    pub rain_max_pct: u8,
    /// Days that freeze or thaw shallow water (R-004).
    pub freeze_days: u8,
    /// A water tile this many tiles from land or more is deep water.
    pub deep_water_distance: u8,
    /// Heat of one burning hearth in tenths of a degree times tiles for each hour (R-022).
    pub hearth_heat: u32,
    /// Fuel that a hearth holds, in burning hours.
    pub hearth_fuel_capacity_hours: u32,
    /// A hearth with this fuel or less gets a refuel job, in burning hours.
    pub refuel_below_hours: u32,
    /// Fuel materials (key) with the burning hours of one unit (R-022).
    pub fuel: Vec<(String, u32)>,
    /// A hearth with less fuel than this raises the low fuel alert, in burning hours.
    pub low_fuel_alert_hours: u32,
    /// A weather offset at or below this is a cold period, in tenths of a degree (R-003).
    pub cold_period_offset: i16,
}

impl ClimateRules {
    /// Rules with no effect. For the empty rules that `SimRules` uses if the content is
    /// invalid.
    pub fn empty() -> Self {
        Self {
            season_curve: vec![0; YEAR_DAYS],
            daily_curve: vec![0; DAY_HOURS],
            weather_step_max: 0,
            weather_limit: 0,
            default_mean: 100,
            default_amplitude: 0,
            default_moisture: 0,
            rain_max_pct: 0,
            freeze_days: 3,
            deep_water_distance: 3,
            hearth_heat: 0,
            hearth_fuel_capacity_hours: 0,
            refuel_below_hours: 0,
            fuel: Vec::new(),
            low_fuel_alert_hours: 0,
            cold_period_offset: -60,
        }
    }
}

/// Load and check a climate file.
pub fn load_climate_rules(file_name: &str, text: &str) -> Result<ClimateRules, ContentError> {
    let fail = |entry: Option<&str>, message: String| ContentError {
        file: file_name.to_string(),
        issues: vec![ContentIssue {
            entry: entry.map(str::to_string),
            message,
        }],
    };
    let rules: ClimateRules =
        ron::from_str(text).map_err(|e| fail(None, format!("parse error: {e}")))?;
    let mut issues = Vec::new();
    let mut bad = |field: &str, message: &str| {
        issues.push(ContentIssue {
            entry: Some(field.to_string()),
            message: message.to_string(),
        });
    };
    if rules.season_curve.len() != YEAR_DAYS {
        bad("season_curve", "must have 40 values");
    }
    if rules
        .season_curve
        .iter()
        .any(|v| !(-1000..=1000).contains(v))
    {
        bad("season_curve", "values must be from -1000 to 1000");
    }
    if rules.daily_curve.len() != DAY_HOURS {
        bad("daily_curve", "must have 24 values");
    }
    if rules.weather_limit > 1000 || rules.weather_step_max > 1000 {
        bad("weather_limit", "weather values must be at most 1000");
    }
    if !(-600..=600).contains(&rules.default_mean) || !(0..=600).contains(&rules.default_amplitude)
    {
        bad("default_mean", "default climate is out of range");
    }
    if rules.rain_max_pct > 100 {
        bad("rain_max_pct", "must be from 0 to 100");
    }
    if rules.freeze_days == 0 {
        bad("freeze_days", "must be 1 or more");
    }
    if rules.hearth_heat > 100_000 {
        bad("hearth_heat", "must be at most 100000");
    }
    if rules.hearth_fuel_capacity_hours > 100_000 {
        bad("hearth_fuel_capacity_hours", "must be at most 100000");
    }
    if rules.fuel.iter().any(|(_, hours)| *hours == 0) {
        bad("fuel", "a fuel unit must burn 1 hour or more");
    }
    if issues.is_empty() {
        Ok(rules)
    } else {
        Err(ContentError {
            file: file_name.to_string(),
            issues,
        })
    }
}

/// A fuel material with its burning time. Made from the climate rules and the materials.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FuelDef {
    /// Material index.
    pub material: u16,
    /// Burning hours of one unit.
    pub hours: u32,
}

// ---- state ----

/// The climate of the colony: the annual mean, the seasonal amplitude, and the moisture.
/// They come from the world cell of the embark.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClimateParams {
    /// Annual mean temperature in tenths of a degree. It is also the ground temperature.
    pub mean: i16,
    /// Seasonal amplitude in tenths of a degree.
    pub amplitude: i16,
    /// Moisture, 0 to 255.
    pub moisture: u8,
}

/// The state of the climate system. It is part of the saved state.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClimateState {
    /// The climate numbers.
    pub params: ClimateParams,
    /// Seed of the weather, from the game seed.
    pub weather_seed: u64,
    /// Weather offset of the current day, tenths of a degree.
    pub offset: i16,
    /// Days in a row with a mean below 0 (R-004).
    pub cold_days: u8,
    /// Days in a row with a mean above 0 (R-004).
    pub warm_days: u8,
    /// True if the shallow water is ice.
    pub ice: bool,
}

impl ClimateState {
    /// The state at tick 0.
    pub(crate) fn new(rules: &ClimateRules, game_seed: u64) -> Self {
        let weather_seed = mix(game_seed ^ 0x5745_4154_4845_5231);
        Self {
            params: ClimateParams {
                mean: rules.default_mean,
                amplitude: rules.default_amplitude,
                moisture: rules.default_moisture,
            },
            weather_seed,
            offset: offset_on_day(rules, weather_seed, 0),
            cold_days: 0,
            warm_days: 0,
            ice: false,
        }
    }
}

// ---- weather ----

fn day_rng(seed: u64, day: u64, salt: u64) -> Pcg32 {
    Pcg32::from_seed(mix(seed ^ mix(day.wrapping_add(salt))))
}

/// The change of the weather offset on a day, in tenths of a degree (R-003).
pub fn weather_step(rules: &ClimateRules, seed: u64, day: u64) -> i16 {
    let m = i32::from(rules.weather_step_max);
    let mut rng = day_rng(seed, day, 0x1000_0000_0001);
    i16::try_from(rng.range(-m, m + 1)).unwrap_or(0)
}

/// The weather offset of a day from the offset of the day before (R-003).
pub fn next_offset(rules: &ClimateRules, seed: u64, previous: i16, day: u64) -> i16 {
    let limit = i32::from(rules.weather_limit);
    let value =
        (i32::from(previous) + i32::from(weather_step(rules, seed, day))).clamp(-limit, limit);
    i16::try_from(value).unwrap_or(0)
}

/// The weather offset of a day (0 is the first day). It starts from 0 before day 0.
pub fn offset_on_day(rules: &ClimateRules, seed: u64, day: u64) -> i16 {
    let mut offset = 0;
    for d in 0..=day {
        offset = next_offset(rules, seed, offset, d);
    }
    offset
}

/// True if it rains on a day. The chance is the moisture (0 to 255) times
/// `rain_max_pct` percent divided by 255 (R-003).
pub fn rain_on_day(rules: &ClimateRules, seed: u64, moisture: u8, day: u64) -> bool {
    let permille = u32::from(moisture) * u32::from(rules.rain_max_pct) * 10 / 255;
    day_rng(seed, day, 0x2000_0000_0002).below(1000) < permille
}

/// The parts of the outdoor temperature, in tenths of a degree (R-002).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OutdoorParts {
    /// Annual mean.
    pub mean: i32,
    /// Seasonal amplitude times the season curve.
    pub season: i32,
    /// The daily curve at this hour.
    pub daily: i32,
    /// The weather offset of the day.
    pub weather: i32,
}

impl OutdoorParts {
    /// The sum of the parts: the outdoor temperature (R-002).
    pub fn total(&self) -> i32 {
        self.mean + self.season + self.daily + self.weather
    }
}

/// The parts of the outdoor temperature (R-002):
/// `mean + amplitude * season_curve(day) / 1000 + daily_curve(hour) + weather_offset`.
/// `day_of_year` is from 1 to 40. The division rounds toward zero.
pub fn outdoor_parts(
    rules: &ClimateRules,
    params: &ClimateParams,
    day_of_year: u32,
    hour: u32,
    offset: i16,
) -> OutdoorParts {
    let curve = i32::from(
        rules
            .season_curve
            .get((day_of_year as usize).saturating_sub(1))
            .copied()
            .unwrap_or(0),
    );
    OutdoorParts {
        mean: i32::from(params.mean),
        season: i32::from(params.amplitude) * curve / 1000,
        daily: i32::from(rules.daily_curve.get(hour as usize).copied().unwrap_or(0)),
        weather: i32::from(offset),
    }
}

/// The mean outdoor temperature of a day: the integer mean of its 24 hours (R-004).
pub fn day_mean(
    rules: &ClimateRules,
    params: &ClimateParams,
    day_of_year: u32,
    offset: i16,
) -> i16 {
    let sum: i32 = (0..DAY_HOURS as u32)
        .map(|h| outdoor_parts(rules, params, day_of_year, h, offset).total())
        .sum();
    i16::try_from(sum / DAY_HOURS as i32).unwrap_or(0)
}

/// The weather of one game day.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DayWeather {
    /// Day number from 0 at the start of the game.
    pub day: u64,
    /// Year, from 1.
    pub year: u64,
    /// Day in the year, 1 to 40.
    pub day_of_year: u32,
    /// Weather offset in tenths of a degree.
    pub offset: i16,
    /// True if it rains.
    pub rain: bool,
    /// Mean outdoor temperature of the day, tenths of a degree.
    pub mean_temp: i16,
}

// ---- tile temperature why chain ----

/// Where a part of a tile temperature comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TempCause {
    /// The annual mean of the world cell.
    Mean,
    /// The season.
    Season,
    /// The hour of the day.
    Daily,
    /// The weather offset of the day.
    Weather,
    /// The ground temperature, the reference of a rock room (R-022).
    Ground,
    /// The heat of the burning hearths in the room, at the balance (R-022).
    Hearth,
    /// The room is still warming up or cooling down: the rest of the sum (R-022).
    Lag,
}

impl TempCause {
    /// Display name.
    pub fn name(self) -> &'static str {
        match self {
            TempCause::Mean => "Annual mean",
            TempCause::Season => "Season",
            TempCause::Daily => "Time of day",
            TempCause::Weather => "Weather",
            TempCause::Ground => "Ground temperature",
            TempCause::Hearth => "Hearth heat",
            TempCause::Lag => "Heat still arriving or leaving",
        }
    }
}

/// One named part of a tile temperature, in tenths of a degree.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TempPart {
    /// The cause.
    pub cause: TempCause,
    /// The value in tenths of a degree (it can be negative).
    pub tenths: i32,
}

/// What kind of place the tile is for its temperature.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TempPlace {
    /// Not in a room: the outdoor temperature.
    Outdoor,
    /// In a room that follows the outdoor temperature.
    Room,
    /// In a room with 75% or more rock border: it follows the ground temperature.
    RockRoom,
}

/// The temperature of a tile as a sum of named parts (D-021).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TileTemperature {
    /// Kind of place.
    pub place: TempPlace,
    /// The named parts, in a fixed order. Their sum is `total`.
    pub parts: Vec<TempPart>,
    /// The temperature in tenths of a degree.
    pub total: i16,
    /// Burning hearths in the room, 0 outdoors.
    pub burning_hearths: u32,
    /// Insulation of the room, 0 outdoors.
    pub insulation: u32,
}

// ---- Sim ----

impl Room {
    /// True if 75% or more of the border tiles are natural rock (R-022).
    pub fn is_rock_room(&self) -> bool {
        !self.border.is_empty()
            && u64::from(self.rock_border) * 100 >= 75 * self.border.len() as u64
    }
}

impl Sim {
    // ---- queries ----

    /// The climate numbers of the colony.
    pub fn climate_params(&self) -> ClimateParams {
        self.climate.params
    }

    /// The parts of the outdoor temperature now (R-002).
    pub fn outdoor_parts(&self) -> OutdoorParts {
        let t = GameTime::from_tick(self.tick);
        outdoor_parts(
            &self.rules.climate,
            &self.climate.params,
            t.day_of_year,
            t.hour,
            self.climate.offset,
        )
    }

    /// The outdoor temperature now, in tenths of a degree (R-002).
    pub fn outdoor_temp(&self) -> i16 {
        i16::try_from(self.outdoor_parts().total()).unwrap_or(0)
    }

    /// The weather of the current day.
    pub fn weather_today(&self) -> DayWeather {
        let day = self.tick / TICKS_PER_DAY;
        self.day_weather(day, self.climate.offset)
    }

    fn day_weather(&self, day: u64, offset: i16) -> DayWeather {
        let t = GameTime::from_tick(day * TICKS_PER_DAY);
        DayWeather {
            day,
            year: t.year,
            day_of_year: t.day_of_year,
            offset,
            rain: rain_on_day(
                &self.rules.climate,
                self.climate.weather_seed,
                self.climate.params.moisture,
                day,
            ),
            mean_temp: day_mean(
                &self.rules.climate,
                &self.climate.params,
                t.day_of_year,
                offset,
            ),
        }
    }

    /// The forecast: the true weather of the next 3 days (R-003).
    pub fn forecast(&self) -> [DayWeather; FORECAST_DAYS] {
        let today = self.tick / TICKS_PER_DAY;
        let mut offset = self.climate.offset;
        std::array::from_fn(|i| {
            let day = today + 1 + i as u64;
            offset = next_offset(&self.rules.climate, self.climate.weather_seed, offset, day);
            self.day_weather(day, offset)
        })
    }

    /// True if the shallow water is ice (R-004).
    pub fn ice(&self) -> bool {
        self.climate.ice
    }

    /// Days in a row with a cold mean and with a warm mean (R-004).
    pub fn ice_counters(&self) -> (u8, u8) {
        (self.climate.cold_days, self.climate.warm_days)
    }

    /// True if the tile is shallow water that is ice now.
    pub fn ice_at(&self, x: u16, y: u16) -> bool {
        self.climate.ice && self.map.is_shallow_water(i32::from(x), i32::from(y))
    }

    /// True if the tile is deep water. Deep water does not freeze (R-004).
    pub fn is_deep_water(&self, x: u16, y: u16) -> bool {
        self.map.is_deep_water(i32::from(x), i32::from(y))
    }

    /// The temperature of the room that contains the tile, in tenths of a degree.
    pub fn room_temp(&self, x: u16, y: u16) -> Option<i16> {
        let room = self.rooms.at(x, y)?;
        Some(to_tenths(self.room_temp_milli(room)))
    }

    /// The temperature of a tile as a sum of named parts (R-002, R-022). `None` outside
    /// the map.
    pub fn tile_temperature(&self, x: u16, y: u16) -> Option<TileTemperature> {
        if !self.map.in_bounds(i32::from(x), i32::from(y)) {
            return None;
        }
        let outdoor = self.outdoor_parts();
        let outdoor_parts = |cause_total: &mut Vec<TempPart>| {
            cause_total.extend([
                TempPart {
                    cause: TempCause::Mean,
                    tenths: outdoor.mean,
                },
                TempPart {
                    cause: TempCause::Season,
                    tenths: outdoor.season,
                },
                TempPart {
                    cause: TempCause::Daily,
                    tenths: outdoor.daily,
                },
                TempPart {
                    cause: TempCause::Weather,
                    tenths: outdoor.weather,
                },
            ]);
        };
        let mut parts = Vec::new();
        let Some(index) = self.rooms.index_at(x, y) else {
            outdoor_parts(&mut parts);
            return Some(TileTemperature {
                place: TempPlace::Outdoor,
                total: i16::try_from(outdoor.total()).unwrap_or(0),
                parts,
                burning_hearths: 0,
                insulation: 0,
            });
        };
        let room = self.rooms.get(index)?;
        let total = to_tenths(self.room_temp_milli(room));
        let rock = room.is_rock_room();
        let reference = if rock {
            parts.push(TempPart {
                cause: TempCause::Ground,
                tenths: i32::from(self.climate.params.mean),
            });
            i32::from(self.climate.params.mean)
        } else {
            outdoor_parts(&mut parts);
            outdoor.total()
        };
        let burning = self.burning_hearths_in(index);
        let insulation = room.insulation.total.min(100);
        let tiles = room.tiles.len().max(1) as i64;
        // At the balance the exchange with the reference equals the heat of the hearths:
        // (reference - room) x (100 - insulation) / 400 = -heat, in tenths of a degree.
        let hearth = if burning > 0 && insulation < 100 {
            let heat = i64::from(burning) * i64::from(self.rules.climate.hearth_heat);
            let eq = heat * 400 / (tiles * i64::from(100 - insulation));
            i32::try_from(eq).unwrap_or(0)
        } else {
            0
        };
        parts.push(TempPart {
            cause: TempCause::Hearth,
            tenths: hearth,
        });
        let lag = i32::from(total) - reference - hearth;
        parts.push(TempPart {
            cause: TempCause::Lag,
            tenths: lag,
        });
        Some(TileTemperature {
            place: if rock {
                TempPlace::RockRoom
            } else {
                TempPlace::Room
            },
            parts,
            total,
            burning_hearths: burning,
            insulation,
        })
    }

    // ---- commands for setup ----

    /// Set the climate of the colony: the annual mean, the seasonal amplitude (tenths of a
    /// degree), and the moisture (0 to 255). The embark gives them. Call before the first
    /// tick.
    pub fn set_climate(&mut self, mean: i16, amplitude: i16, moisture: u8) {
        self.climate.params = ClimateParams {
            mean: mean.clamp(-600, 600),
            amplitude: amplitude.clamp(0, 600),
            moisture,
        };
        self.reset_room_temps();
        self.rebuild_wild_cache();
        if self.tick == 0 {
            self.place_start_seeds();
            self.preheat_start_camp();
        }
    }

    // ---- the hourly update ----

    /// Called after the tick number increased. Each game hour: new day (weather, ice), room
    /// temperature, hearth fuel.
    pub(crate) fn climate_tick(&mut self) {
        if !self.tick.is_multiple_of(TICKS_PER_HOUR) {
            return;
        }
        if self.tick.is_multiple_of(TICKS_PER_DAY) {
            self.advance_day();
        }
        self.update_room_temperatures();
        self.burn_hearths();
    }

    /// The day ended: count it for the ice, then roll the weather forward.
    fn advance_day(&mut self) {
        let ended = GameTime::from_tick(self.tick.saturating_sub(1));
        let mean = day_mean(
            &self.rules.climate,
            &self.climate.params,
            ended.day_of_year,
            self.climate.offset,
        );
        let c = &mut self.climate;
        if mean < 0 {
            c.cold_days = c.cold_days.saturating_add(1);
            c.warm_days = 0;
        } else if mean > 0 {
            c.warm_days = c.warm_days.saturating_add(1);
            c.cold_days = 0;
        } else {
            c.cold_days = 0;
            c.warm_days = 0;
        }
        let need = self.rules.climate.freeze_days;
        let change = if !c.ice && c.cold_days >= need {
            Some(true)
        } else if c.ice && c.warm_days >= need {
            Some(false)
        } else {
            None
        };
        let day = self.tick / TICKS_PER_DAY;
        self.climate.offset = next_offset(
            &self.rules.climate,
            self.climate.weather_seed,
            self.climate.offset,
            day,
        );
        if let Some(ice) = change {
            self.climate.ice = ice;
            self.apply_ice();
        }
    }

    /// The ice state changed: update the passability, the regions, and the paths.
    pub(crate) fn apply_ice(&mut self) {
        if self.map.set_ice(self.climate.ice) {
            self.regions.rebuild(&self.map);
            if !self.climate.ice {
                self.move_off_water();
            }
            let affected: Vec<crate::ColonistId> = self
                .colonists
                .iter()
                .filter(|(_, c)| c.target.is_some())
                .map(|(id, _)| id)
                .collect();
            for id in affected {
                if let Some(c) = self.colonists.get_mut(id)
                    && let Some(goal) = c.target
                {
                    c.path.clear();
                    c.progress = 0;
                    self.path_queue.retain(|r| r.colonist != id);
                    self.path_queue
                        .push_back(crate::PathRequest { colonist: id, goal });
                }
            }
        }
    }

    /// Ice thawed: colonists and items on water tiles move to the nearest land.
    fn move_off_water(&mut self) {
        let stuck: Vec<crate::ColonistId> = self
            .colonists
            .iter()
            .filter(|(_, c)| !self.map.passable(i32::from(c.x), i32::from(c.y)))
            .map(|(id, _)| id)
            .collect();
        for id in stuck {
            let Some(from) = self.colonists.get(id).map(|c| c.tile()) else {
                continue;
            };
            if let Some(to) = self.nearest_free_tile(from.0, from.1)
                && let Some(c) = self.colonists.get_mut(id)
            {
                c.x = to.0;
                c.y = to.1;
                c.path.clear();
                c.progress = 0;
                c.target = None;
            }
        }
        let lost: Vec<crate::ItemId> = self
            .items
            .iter()
            .filter_map(|(id, i)| match i.loc {
                crate::ItemLoc::Tile(x, y)
                    if !self.map.passable(i32::from(x), i32::from(y))
                        && self.map.ground(i32::from(x), i32::from(y))
                            == Some(worldgen::Ground::Water) =>
                {
                    Some(id)
                }
                _ => None,
            })
            .collect();
        for id in lost {
            let Some(crate::ItemLoc::Tile(x, y)) = self.items.get(id).map(|i| i.loc) else {
                continue;
            };
            if let Some(to) = self.nearest_free_tile(x, y) {
                self.items.relocate(id, to.0, to.1);
            }
        }
    }

    // ---- room temperature ----

    /// The inner temperature of a room, in thousandths of a degree.
    pub(crate) fn room_temp_milli(&self, room: &Room) -> i32 {
        room.tiles
            .first()
            .and_then(|t| self.room_temps.get(t))
            .copied()
            .unwrap_or_else(|| i32::from(self.outdoor_temp()) * MILLI_PER_TENTH as i32)
    }

    /// Set every room to the outdoor temperature.
    fn reset_room_temps(&mut self) {
        let outdoor = i32::from(self.outdoor_temp()) * MILLI_PER_TENTH as i32;
        self.room_temps = self
            .rooms
            .iter()
            .filter_map(|r| r.tiles.first().map(|&t| (t, outdoor)))
            .collect();
    }

    /// New rooms after a detection: a new room starts at the mean of the temperatures that
    /// its tiles had. A tile that was in no room had the outdoor temperature.
    pub(crate) fn carry_room_temps(&mut self, old: &Rooms, old_temps: &BTreeMap<(u16, u16), i32>) {
        let outdoor = i64::from(self.outdoor_temp()) * MILLI_PER_TENTH;
        let mut temps = BTreeMap::new();
        for room in self.rooms.iter() {
            let Some(&first) = room.tiles.first() else {
                continue;
            };
            let sum: i64 = room
                .tiles
                .iter()
                .map(|&(x, y)| {
                    old.at(x, y)
                        .and_then(|r| r.tiles.first())
                        .and_then(|t| old_temps.get(t))
                        .map_or(outdoor, |&v| i64::from(v))
                })
                .sum();
            let mean = sum / room.tiles.len().max(1) as i64;
            temps.insert(first, i32::try_from(mean).unwrap_or(0));
        }
        self.room_temps = temps;
    }

    /// Read the room temperatures after a load or a new colony: a room without a saved
    /// value starts at the outdoor temperature.
    pub(crate) fn fill_room_temps(&mut self) {
        let outdoor = i32::from(self.outdoor_temp()) * MILLI_PER_TENTH as i32;
        let temps: BTreeMap<(u16, u16), i32> = self
            .rooms
            .iter()
            .filter_map(|r| r.tiles.first().copied())
            .map(|t| (t, self.room_temps.get(&t).copied().unwrap_or(outdoor)))
            .collect();
        self.room_temps = temps;
    }

    /// Each game hour (R-022):
    /// `room += (reference - room) x (100 - insulation) / 400 + hearth_heat / room_tiles`.
    pub(crate) fn update_room_temperatures(&mut self) {
        let outdoor = i64::from(self.outdoor_temp()) * MILLI_PER_TENTH;
        let ground = i64::from(self.climate.params.mean) * MILLI_PER_TENTH;
        let heat = i64::from(self.rules.climate.hearth_heat) * MILLI_PER_TENTH;
        let mut burning = vec![0i64; self.rooms.len()];
        for (x, y) in self.hearth_tiles() {
            if self.hearth_lit(x, y)
                && let Some(i) = self.rooms.index_at(x, y)
            {
                burning[i] += 1;
            }
        }
        let mut next = BTreeMap::new();
        for (i, room) in self.rooms.iter().enumerate() {
            let Some(&first) = room.tiles.first() else {
                continue;
            };
            let current = i64::from(self.room_temp_milli(room));
            let reference = if room.is_rock_room() { ground } else { outdoor };
            let open = 100 - i64::from(room.insulation.total.min(100));
            let tiles = room.tiles.len().max(1) as i64;
            let value = current + (reference - current) * open / 400 + burning[i] * heat / tiles;
            next.insert(first, i32::try_from(value).unwrap_or(0));
        }
        self.room_temps = next;
    }

    /// Number of burning hearths in the room with this index.
    pub(crate) fn burning_hearths_in(&self, room: usize) -> u32 {
        self.hearth_tiles()
            .into_iter()
            .filter(|&(x, y)| self.hearth_lit(x, y) && self.rooms.index_at(x, y) == Some(room))
            .count() as u32
    }

    /// Check the climate state of a loaded `Sim`.
    pub(crate) fn climate_state_ok(&self) -> bool {
        let c = &self.climate;
        let limit = i32::from(self.rules.climate.weather_limit);
        i32::from(c.offset).abs() <= limit
            && (-600..=600).contains(&c.params.mean)
            && (0..=600).contains(&c.params.amplitude)
            && self.room_temps.iter().all(|(&(x, y), &v)| {
                self.map.in_bounds(i32::from(x), i32::from(y)) && v.abs() <= 1_000_000
            })
    }

    /// Recompute the weather offset from the seed. For a change of the rules.
    pub(crate) fn resync_weather(&mut self) {
        let day = self.tick / TICKS_PER_DAY;
        self.climate.offset = offset_on_day(&self.rules.climate, self.climate.weather_seed, day);
    }
}

/// Thousandths of a degree to tenths, rounded down.
fn to_tenths(milli: i32) -> i16 {
    i16::try_from(i64::from(milli).div_euclid(MILLI_PER_TENTH)).unwrap_or(0)
}
