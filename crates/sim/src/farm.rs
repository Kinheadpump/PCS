//! Crops, fertility, farm zones, and wild plants (R-006, R-007).
//!
//! A farm zone is a set of tiles with a crop. Colonists sow seeds on free zone tiles and
//! harvest ripe plants. A crop grows one hour for each game hour in which the tile
//! temperature is inside the band of the crop. It dies below its kill temperature.
//! Wild plants stand on tiles by a tile hash and the annual mean temperature of the climate.
//! A colonist gathers a ripe wild plant; the plant is ripe again after some game days, but
//! the days do not count in winter.

use crate::Sim;
use crate::colonist::ColonistId;
use crate::job::{AbortReason, ActiveJob, Job, JobKind, Reject};
use crate::loader::{Issues, parse};
use crate::time::{GameTime, Season, TICKS_PER_DAY, TICKS_PER_HOUR};
use crate::{ChronicleKind, Command};
use base::content::{ContentError, MaterialDef};
use base::rng::mix;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use worldgen::Ground;

/// A tile with the item, the work tile, and the destination of a job.
pub(crate) type FarmPlan = ((u16, u16), Option<(u16, u16)>, Option<crate::item::ItemId>);

/// One crop type (`crops.ron`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CropDef {
    /// String ID.
    pub id: String,
    /// Display name.
    pub name: String,
    /// Material index of the seeds.
    pub seed: u16,
    /// Material index of the harvest.
    pub product: u16,
    /// Lowest growth temperature, tenths of a degree.
    pub min_temp: i16,
    /// Highest growth temperature, tenths of a degree.
    pub max_temp: i16,
    /// The plant dies below this temperature, tenths of a degree.
    pub kill_temp: i16,
    /// Growth time in game days.
    pub growth_days: u16,
    /// Base yield in units at fertility 1000.
    pub yield_units: u32,
    /// Seed units that a harvest gives.
    pub seed_yield: u32,
}

/// One wild plant type (`wild_plants.ron`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WildDef {
    /// String ID.
    pub id: String,
    /// Display name.
    pub name: String,
    /// Material index of the harvest.
    pub product: u16,
    /// Allowed grounds: bit 0 soil, bit 1 sand, bit 2 clay.
    pub ground_mask: u8,
    /// Share of the tiles, per-mille.
    pub density: u16,
    /// Lowest annual mean temperature of the climate, tenths of a degree.
    pub mean_min: i16,
    /// Highest annual mean temperature of the climate, tenths of a degree.
    pub mean_max: i16,
    /// Units of one gathering.
    pub yield_units: u32,
    /// Game days until the plant is ripe again.
    pub regrow_days: u16,
}

/// Numbers for farming (`crops.ron`, `wild_plants.ron`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FarmRules {
    /// Work ticks to sow one tile.
    pub sow_ticks: u32,
    /// Work ticks to harvest one tile.
    pub harvest_ticks: u32,
    /// Work ticks to gather one wild plant.
    pub gather_ticks: u32,
    /// Lowest fertility, per-mille.
    pub fertility_min: i32,
    /// Highest fertility, per-mille.
    pub fertility_max: i32,
    /// Fertility of soil, per-mille.
    pub ground_soil: i32,
    /// Fertility of clay, per-mille.
    pub ground_clay: i32,
    /// Fertility of sand, per-mille.
    pub ground_sand: i32,
    /// Tile variation of the fertility: from minus this to plus this, per-mille.
    pub variation: i32,
    /// A tile within this distance of water gets the water bonus.
    pub water_radius: u8,
    /// Fertility bonus near water, per-mille.
    pub water_bonus: i32,
    /// The crops, in file order.
    pub crops: Vec<CropDef>,
    /// The wild plants, in file order.
    pub wild: Vec<WildDef>,
}

impl FarmRules {
    /// Rules without crops and wild plants. For the empty default rules.
    pub fn empty() -> Self {
        Self {
            sow_ticks: 100,
            harvest_ticks: 100,
            gather_ticks: 100,
            fertility_min: 500,
            fertility_max: 1500,
            ground_soil: 1000,
            ground_clay: 1000,
            ground_sand: 1000,
            variation: 0,
            water_radius: 0,
            water_bonus: 0,
            crops: Vec::new(),
            wild: Vec::new(),
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawCrop {
    id: String,
    name: String,
    seed: String,
    product: String,
    min_temp: i16,
    max_temp: i16,
    kill_temp: i16,
    growth_days: u16,
    yield_units: u32,
    seed_yield: u32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawCrops {
    sow_ticks: u32,
    harvest_ticks: u32,
    gather_ticks: u32,
    fertility_min: i32,
    fertility_max: i32,
    ground_soil: i32,
    ground_clay: i32,
    ground_sand: i32,
    variation: i32,
    water_radius: u8,
    water_bonus: i32,
    crops: Vec<RawCrop>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawWild {
    id: String,
    name: String,
    product: String,
    ground: String,
    density: u16,
    mean_min: i16,
    mean_max: i16,
    yield_units: u32,
    regrow_days: u16,
}

/// Load and check `crops.ron` and `wild_plants.ron`.
pub fn load_farm_rules(
    crops_text: &str,
    wild_text: &str,
    materials: &[MaterialDef],
) -> Result<FarmRules, ContentError> {
    let raw: RawCrops = parse("crops.ron", crops_text)?;
    let raw_wild: Vec<RawWild> = parse("wild_plants.ron", wild_text)?;
    let mut issues = Issues::new("crops.ron");
    if raw.fertility_min <= 0 || raw.fertility_min > raw.fertility_max || raw.fertility_max > 5000 {
        issues.add("fertility_min", "need 0 < min <= max <= 5000");
    }
    if raw.variation < 0 || raw.variation > 1000 || raw.water_radius > 8 {
        issues.add(
            "variation",
            "variation at most 1000, water_radius at most 8",
        );
    }
    if raw.sow_ticks == 0 || raw.harvest_ticks == 0 || raw.gather_ticks == 0 {
        issues.add("sow_ticks", "work times must be 1 tick or more");
    }
    let mut crops = Vec::new();
    for c in &raw.crops {
        if c.min_temp >= c.max_temp || c.kill_temp >= c.min_temp {
            issues.add(&c.id, "need kill_temp < min_temp < max_temp");
        }
        if c.growth_days == 0 || c.growth_days > 200 || c.yield_units == 0 {
            issues.add(&c.id, "growth_days from 1 to 200, yield_units 1 or more");
        }
        if crops.iter().any(|d: &CropDef| d.id == c.id) {
            issues.add(&c.id, "duplicate crop id");
        }
        crops.push(CropDef {
            id: c.id.clone(),
            name: c.name.clone(),
            seed: issues.material(materials, &c.seed, "crop seed"),
            product: issues.material(materials, &c.product, "crop product"),
            min_temp: c.min_temp,
            max_temp: c.max_temp,
            kill_temp: c.kill_temp,
            growth_days: c.growth_days,
            yield_units: c.yield_units,
            seed_yield: c.seed_yield,
        });
    }
    let mut wild = Vec::new();
    let mut wild_issues = Issues::new("wild_plants.ron");
    for w in &raw_wild {
        let mut mask = 0u8;
        for letter in w.ground.chars() {
            match letter {
                'S' => mask |= 1,
                'D' => mask |= 2,
                'C' => mask |= 4,
                _ => wild_issues.add(&w.id, "ground letters are S, C, and D"),
            }
        }
        if w.density == 0 || w.density > 1000 || w.regrow_days == 0 || w.yield_units == 0 {
            wild_issues.add(
                &w.id,
                "density 1 to 1000, regrow_days and yield_units 1 or more",
            );
        }
        if w.mean_min > w.mean_max {
            wild_issues.add(&w.id, "mean_min must not be above mean_max");
        }
        wild.push(WildDef {
            id: w.id.clone(),
            name: w.name.clone(),
            product: wild_issues.material(materials, &w.product, "wild plant product"),
            ground_mask: mask,
            density: w.density,
            mean_min: w.mean_min,
            mean_max: w.mean_max,
            yield_units: w.yield_units,
            regrow_days: w.regrow_days,
        });
    }
    wild_issues.finish(())?;
    issues.finish(FarmRules {
        sow_ticks: raw.sow_ticks,
        harvest_ticks: raw.harvest_ticks,
        gather_ticks: raw.gather_ticks,
        fertility_min: raw.fertility_min,
        fertility_max: raw.fertility_max,
        ground_soil: raw.ground_soil,
        ground_clay: raw.ground_clay,
        ground_sand: raw.ground_sand,
        variation: raw.variation,
        water_radius: raw.water_radius,
        water_bonus: raw.water_bonus,
        crops,
        wild,
    })
}

// ---- state ----

/// A crop plant on a tile.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Plant {
    /// Crop index (`crops.ron` order).
    pub crop: u16,
    /// Growing hours so far.
    pub hours: u32,
}

/// The farm state. It is part of the saved state.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FarmState {
    /// Seed of the tile hashes (variation, wild plants).
    pub(crate) seed: u64,
    /// Farm zone tiles with their crop.
    pub(crate) zones: BTreeMap<(u16, u16), u16>,
    /// The crop plants.
    pub(crate) plants: BTreeMap<(u16, u16), Plant>,
    /// Gathered wild plants with the game days until they are ripe again.
    pub(crate) regrow: BTreeMap<(u16, u16), u16>,
    /// Crop plants that the frost killed since the last chronicle entry.
    pub(crate) killed_today: u32,
    /// True after the first harvest.
    pub(crate) harvested: bool,
    /// The tile with the start stock (the seeds change with the climate before tick 0).
    pub(crate) start_tile: Option<(u16, u16)>,
}

impl FarmState {
    /// An empty state with the seed of the tile hashes.
    pub(crate) fn with_seed(seed: u64) -> Self {
        Self {
            seed,
            ..Self::default()
        }
    }
}

/// Derived caches of the farm system.
#[derive(Clone, Debug, Default)]
pub struct FarmCache {
    /// Fertility for each tile (per-mille), 0 if the tile has no farmable ground.
    pub(crate) fertility: Vec<i16>,
    /// Wild plant type + 1 for each tile, 0 for none.
    pub(crate) wild: Vec<u8>,
}

/// Why a tile has its fertility: the named parts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FertilityCause {
    /// The base of the ground type.
    Ground,
    /// The variation of this tile.
    Variation,
    /// Water within the water radius.
    Water,
    /// The change that the lowest and highest limits make.
    Limit,
}

/// One part of a fertility value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FertilityPart {
    /// What the part is.
    pub cause: FertilityCause,
    /// Its value in per-mille.
    pub value: i32,
}

/// The fertility of a tile as a sum of named parts (D-021).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FertilityReport {
    /// The parts. Their sum is `total`.
    pub parts: Vec<FertilityPart>,
    /// Fertility in per-mille.
    pub total: i32,
}

/// The state of a wild plant tile.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WildStatus {
    /// Wild plant type index (`wild_plants.ron` order).
    pub kind: u16,
    /// True if a colonist can gather it now.
    pub ripe: bool,
    /// Game days until the plant is ripe again. 0 if it is ripe.
    pub days_left: u16,
}

/// The growth of a crop plant.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlantReport {
    /// The plant.
    pub plant: Plant,
    /// Growing hours needed.
    pub hours_needed: u32,
    /// Temperature of the tile, tenths of a degree.
    pub temperature: i16,
    /// True if the temperature is inside the growth band now.
    pub growing: bool,
    /// True if the plant is ripe.
    pub ripe: bool,
}

const GROUND_SOIL: u8 = 1;
const GROUND_SAND: u8 = 2;
const GROUND_CLAY: u8 = 4;

fn ground_bit(ground: Ground) -> u8 {
    match ground {
        Ground::Soil => GROUND_SOIL,
        Ground::Sand => GROUND_SAND,
        Ground::Clay => GROUND_CLAY,
        Ground::Water | Ground::Stone => 0,
    }
}

impl Sim {
    // ---- queries ----

    /// The crop of the farm zone on a tile, if any.
    pub fn farm_zone(&self, x: u16, y: u16) -> Option<u16> {
        self.farm.zones.get(&(x, y)).copied()
    }

    /// All farm zone tiles with their crop, in `(x, y)` order.
    pub fn farm_zones(&self) -> impl Iterator<Item = ((u16, u16), u16)> + '_ {
        self.farm.zones.iter().map(|(&t, &c)| (t, c))
    }

    /// The crop plant on a tile, if any.
    pub fn plant_at(&self, x: u16, y: u16) -> Option<&Plant> {
        self.farm.plants.get(&(x, y))
    }

    /// All crop plants, in `(x, y)` order.
    pub fn plants(&self) -> impl Iterator<Item = ((u16, u16), &Plant)> + '_ {
        self.farm.plants.iter().map(|(&t, p)| (t, p))
    }

    /// True if the plant is ripe.
    pub fn plant_ripe(&self, plant: &Plant) -> bool {
        self.rules
            .farm
            .crops
            .get(usize::from(plant.crop))
            .is_some_and(|c| plant.hours >= u32::from(c.growth_days) * 24)
    }

    /// The growth of the plant on a tile, with the tile temperature (R-006).
    pub fn plant_report(&self, x: u16, y: u16) -> Option<PlantReport> {
        let plant = *self.farm.plants.get(&(x, y))?;
        let crop = self.rules.farm.crops.get(usize::from(plant.crop))?;
        let temperature = self.tile_temp(x, y);
        Some(PlantReport {
            plant,
            hours_needed: u32::from(crop.growth_days) * 24,
            temperature,
            growing: (crop.min_temp..=crop.max_temp).contains(&temperature),
            ripe: self.plant_ripe(&plant),
        })
    }

    /// The temperature of a tile in tenths of a degree: the room or the outdoor.
    pub(crate) fn tile_temp(&self, x: u16, y: u16) -> i16 {
        self.room_temp(x, y).unwrap_or_else(|| self.outdoor_temp())
    }

    /// The fertility of a tile in per-mille (D-021). 0 if the ground cannot be farmed.
    pub fn fertility(&self, x: u16, y: u16) -> i16 {
        self.map
            .index(i32::from(x), i32::from(y))
            .and_then(|i| self.farm_cache.fertility.get(i).copied())
            .unwrap_or(0)
    }

    /// The fertility of all tiles, row by row. For the fertility overlay.
    pub fn fertility_map(&self) -> &[i16] {
        &self.farm_cache.fertility
    }

    /// The fertility of a tile as a sum of named parts. `None` if the tile is outside the
    /// map or its ground cannot be farmed.
    pub fn fertility_why(&self, x: u16, y: u16) -> Option<FertilityReport> {
        let f = &self.rules.farm;
        let ground = self.map.ground(i32::from(x), i32::from(y))?;
        let base = match ground {
            Ground::Soil => f.ground_soil,
            Ground::Clay => f.ground_clay,
            Ground::Sand => f.ground_sand,
            Ground::Water | Ground::Stone => return None,
        };
        let span = u64::try_from(2 * f.variation + 1).unwrap_or(1);
        let h = mix(self.farm.seed ^ mix(tile_key(x, y) ^ 0x4645_5254));
        let variation = i32::try_from(h % span).unwrap_or(0) - f.variation;
        let water = if self.water_near(x, y) {
            f.water_bonus
        } else {
            0
        };
        let raw = base + variation + water;
        let total = raw.clamp(f.fertility_min, f.fertility_max);
        Some(FertilityReport {
            parts: vec![
                FertilityPart {
                    cause: FertilityCause::Ground,
                    value: base,
                },
                FertilityPart {
                    cause: FertilityCause::Variation,
                    value: variation,
                },
                FertilityPart {
                    cause: FertilityCause::Water,
                    value: water,
                },
                FertilityPart {
                    cause: FertilityCause::Limit,
                    value: total - raw,
                },
            ],
            total,
        })
    }

    fn water_near(&self, x: u16, y: u16) -> bool {
        let r = i32::from(self.rules.farm.water_radius);
        if r == 0 {
            return false;
        }
        let (cx, cy) = (i32::from(x), i32::from(y));
        (cy - r..=cy + r)
            .any(|ty| (cx - r..=cx + r).any(|tx| self.map.ground(tx, ty) == Some(Ground::Water)))
    }

    /// True if a crop can grow on the tile: soil, clay, or sand without a tree, a rock wall,
    /// or a building.
    pub fn farmable(&self, x: u16, y: u16) -> bool {
        let (ix, iy) = (i32::from(x), i32::from(y));
        self.map.ground(ix, iy).is_some_and(|g| ground_bit(g) != 0)
            && !self.map.tree(ix, iy)
            && !self.map.wall(ix, iy)
            && self
                .map
                .rock(ix, iy)
                .is_none_or(|r| r == worldgen::RockKind::None)
            && self.buildings.object_at(x, y).is_none()
    }

    /// The wild plant on a tile and its state. A tree on the tile hides the plant for the
    /// gather command, but this query still shows it.
    pub fn wild_plant_at(&self, x: u16, y: u16) -> Option<WildStatus> {
        let i = self.map.index(i32::from(x), i32::from(y))?;
        let code = *self.farm_cache.wild.get(i)?;
        if code == 0 {
            return None;
        }
        let days_left = self.farm.regrow.get(&(x, y)).copied().unwrap_or(0);
        Some(WildStatus {
            kind: u16::from(code) - 1,
            ripe: days_left == 0,
            days_left,
        })
    }

    /// True if a colonist can gather the wild plant on the tile now.
    pub(crate) fn wild_ripe(&self, x: u16, y: u16) -> bool {
        self.wild_plant_at(x, y).is_some_and(|w| w.ripe)
            && !self.map.tree(i32::from(x), i32::from(y))
    }

    // ---- caches ----

    /// Rebuild the fertility and wild plant caches from the map, the climate, and the seed.
    pub(crate) fn rebuild_farm_cache(&mut self) {
        let (w, h) = (self.map.width(), self.map.height());
        let mut fertility = vec![0i16; (w * h) as usize];
        for y in 0..h {
            for x in 0..w {
                let (tx, ty) = (x as u16, y as u16);
                if let Some(report) = self.fertility_why(tx, ty)
                    && let Some(i) = self.map.index(x as i32, y as i32)
                {
                    fertility[i] = i16::try_from(report.total).unwrap_or(0);
                }
            }
        }
        self.farm_cache.fertility = fertility;
        self.rebuild_wild_cache();
    }

    /// Rebuild the wild plant cache (after a change of the climate).
    pub(crate) fn rebuild_wild_cache(&mut self) {
        let (w, h) = (self.map.width(), self.map.height());
        let mean = self.climate.params.mean;
        let mut wild = vec![0u8; (w * h) as usize];
        for y in 0..h {
            for x in 0..w {
                let Some(ground) = self.map.ground(x as i32, y as i32) else {
                    continue;
                };
                let bit = ground_bit(ground);
                if bit == 0 {
                    continue;
                }
                for (k, def) in self.rules.farm.wild.iter().enumerate() {
                    if def.ground_mask & bit == 0 || mean < def.mean_min || mean > def.mean_max {
                        continue;
                    }
                    let key = tile_key(x as u16, y as u16) ^ ((k as u64 + 1) << 48);
                    let roll = mix(self.farm.seed ^ mix(key ^ 0x5749_4c44)) % 1000;
                    if roll < u64::from(def.density)
                        && let Some(i) = self.map.index(x as i32, y as i32)
                    {
                        wild[i] = u8::try_from(k + 1).unwrap_or(0);
                        break;
                    }
                }
            }
        }
        self.farm_cache.wild = wild;
    }

    // ---- commands ----

    pub(crate) fn apply_farm_command(&mut self, command: &Command) {
        match *command {
            Command::SetFarmZone {
                x0,
                y0,
                x1,
                y1,
                crop,
            } => self.set_farm_zone(x0, y0, x1, y1, crop),
            Command::ClearFarmZone { x0, y0, x1, y1 } => {
                let (ax, bx, ay, by) = (x0.min(x1), x0.max(x1), y0.min(y1), y0.max(y1));
                self.farm
                    .zones
                    .retain(|&(x, y), _| !(x >= ax && x <= bx && y >= ay && y <= by));
            }
            _ => {}
        }
    }

    fn set_farm_zone(&mut self, x0: u16, y0: u16, x1: u16, y1: u16, crop: u16) {
        let (ax, bx, ay, by) = (x0.min(x1), x0.max(x1), y0.min(y1), y0.max(y1));
        if !self.map.in_bounds(i32::from(bx), i32::from(by)) {
            self.reject(Reject::BadZone);
            return;
        }
        if usize::from(crop) >= self.rules.farm.crops.len() {
            self.reject(Reject::UnknownCrop);
            return;
        }
        let mut any = false;
        for y in ay..=by {
            for x in ax..=bx {
                if self.farmable(x, y) {
                    self.farm.zones.insert((x, y), crop);
                    any = true;
                }
            }
        }
        if !any {
            self.reject(Reject::NotFarmable);
        } else {
            self.wake_idle();
        }
    }

    // ---- the hourly and daily update ----

    /// Each game hour: plants grow or die, then sow and harvest jobs appear.
    pub(crate) fn farm_tick(&mut self) {
        if !self.tick.is_multiple_of(TICKS_PER_HOUR) {
            return;
        }
        self.grow_plants();
        self.make_farm_jobs();
        if self.tick.is_multiple_of(TICKS_PER_DAY) {
            self.farm_daily();
        }
    }

    fn grow_plants(&mut self) {
        let outdoor = self.outdoor_temp();
        let tiles: Vec<(u16, u16)> = self.farm.plants.keys().copied().collect();
        for tile in tiles {
            let Some(plant) = self.farm.plants.get(&tile).copied() else {
                continue;
            };
            let Some(crop) = self.rules.farm.crops.get(usize::from(plant.crop)) else {
                continue;
            };
            let temp = self.room_temp(tile.0, tile.1).unwrap_or(outdoor);
            if temp < crop.kill_temp {
                self.farm.plants.remove(&tile);
                self.farm.killed_today += 1;
            } else if (crop.min_temp..=crop.max_temp).contains(&temp) {
                let needed = u32::from(crop.growth_days) * 24;
                if let Some(p) = self.farm.plants.get_mut(&tile) {
                    p.hours = (p.hours + 1).min(needed);
                }
            }
        }
    }

    /// Put sow and harvest jobs on the board for the zone tiles and the ripe plants.
    fn make_farm_jobs(&mut self) {
        let outdoor = self.outdoor_temp();
        let ripe: Vec<((u16, u16), u16)> = self
            .farm
            .plants
            .iter()
            .filter(|(_, p)| self.plant_ripe(p))
            .map(|(&t, p)| (t, p.crop))
            .collect();
        let mut added = false;
        for ((x, y), crop) in ripe {
            if self.jobs.find_arg(JobKind::Harvest, x, y, crop).is_none() {
                self.jobs.add_arg(JobKind::Harvest, x, y, crop);
                added = true;
            }
        }
        let mut seeds: Vec<u64> = self
            .rules
            .farm
            .crops
            .iter()
            .map(|c| self.items.total_units(c.seed))
            .collect();
        for (_, job) in self.jobs.iter() {
            if job.kind == JobKind::Sow
                && let Some(s) = seeds.get_mut(usize::from(job.arg))
            {
                *s = s.saturating_sub(1);
            }
        }
        let zones: Vec<((u16, u16), u16)> = self.farm.zones.iter().map(|(&t, &c)| (t, c)).collect();
        for ((x, y), crop) in zones {
            let Some(def) = self.rules.farm.crops.get(usize::from(crop)) else {
                continue;
            };
            let temp = self.room_temp(x, y).unwrap_or(outdoor);
            if self.farm.plants.contains_key(&(x, y))
                || !(def.min_temp..=def.max_temp).contains(&temp)
                || seeds.get(usize::from(crop)).copied().unwrap_or(0) == 0
                || self.jobs.find(JobKind::Sow, x, y).is_some()
                || !self.farmable(x, y)
            {
                continue;
            }
            self.jobs.add_arg(JobKind::Sow, x, y, crop);
            if let Some(s) = seeds.get_mut(usize::from(crop)) {
                *s -= 1;
            }
            added = true;
        }
        if added {
            self.wake_idle();
        }
    }

    /// Each game day: wild plants grow again (not in winter), and the frost report.
    fn farm_daily(&mut self) {
        let ended = GameTime::from_tick(self.tick.saturating_sub(1));
        if ended.season != Season::Winter {
            for days in self.farm.regrow.values_mut() {
                *days = days.saturating_sub(1);
            }
            self.farm.regrow.retain(|_, d| *d > 0);
        }
        if self.farm.killed_today > 0 {
            let n = i32::try_from(self.farm.killed_today).unwrap_or(i32::MAX);
            self.farm.killed_today = 0;
            self.chronicle_add(ChronicleKind::CropsKilled, [n, 0, 0]);
        }
    }

    // ---- jobs ----

    /// True if an unclaimed sow, harvest, or gather job has nothing to do any more.
    pub(crate) fn farm_job_stale(&self, job: &Job) -> bool {
        match job.kind {
            JobKind::Sow => {
                !self.farm.zones.contains_key(&(job.x, job.y))
                    || self.farm.plants.contains_key(&(job.x, job.y))
            }
            JobKind::Harvest => !self
                .farm
                .plants
                .get(&(job.x, job.y))
                .is_some_and(|p| self.plant_ripe(p)),
            JobKind::Gather => !self.wild_ripe(job.x, job.y),
            _ => false,
        }
    }

    /// True if the work of a farm job can go on now.
    pub(crate) fn farm_target_ok(&self, job: &Job) -> bool {
        match job.kind {
            JobKind::Sow => {
                let Some(crop) = self.rules.farm.crops.get(usize::from(job.arg)) else {
                    return false;
                };
                !self.farm_job_stale(job)
                    && self.farmable(job.x, job.y)
                    && (crop.min_temp..=crop.max_temp).contains(&self.tile_temp(job.x, job.y))
            }
            _ => !self.farm_job_stale(job),
        }
    }

    /// Check a farm job in a region. The colonist works on the tile itself. A sow job
    /// needs a seed stack in the region.
    pub(crate) fn eval_farm(&self, region: u32, from: (u16, u16), job: &Job) -> Option<FarmPlan> {
        if self.regions.id_at(i32::from(job.x), i32::from(job.y)) != region
            || !self.farm_target_ok(job)
        {
            return None;
        }
        if job.kind == JobKind::Sow {
            let crop = self.rules.farm.crops.get(usize::from(job.arg))?;
            self.find_material(region, from, crop.seed)?;
        }
        Some(((job.x, job.y), None, None))
    }

    /// Work ticks of a farm job at 100% speed.
    pub(crate) fn farm_work_ticks(&self, job: &Job) -> u32 {
        match job.kind {
            JobKind::Sow => self.rules.farm.sow_ticks,
            JobKind::Harvest => self.rules.farm.harvest_ticks,
            _ => self.rules.farm.gather_ticks,
        }
    }

    /// The work of a sow, harvest, or gather job is done.
    pub(crate) fn complete_farm_work(&mut self, id: ColonistId, active: ActiveJob, job: &Job) {
        let rules = self.rules.clone();
        match job.kind {
            JobKind::Sow => {
                let Some(crop) = rules.farm.crops.get(usize::from(job.arg)) else {
                    return;
                };
                let from = self.colonists.get(id).map(|c| c.tile()).unwrap_or_default();
                let region = self.regions.id_at(i32::from(from.0), i32::from(from.1));
                // The seed unit leaves the stack at once (a simple model: no seed carrying).
                let Some(stack) = self.find_material(region, from, crop.seed) else {
                    self.abort_job(id, AbortReason::NoMaterial, true);
                    return;
                };
                self.items.take_one(stack);
                self.farm.plants.insert(
                    (job.x, job.y),
                    Plant {
                        crop: job.arg,
                        hours: 0,
                    },
                );
            }
            JobKind::Harvest => {
                let Some(plant) = self.farm.plants.remove(&(job.x, job.y)) else {
                    return;
                };
                let Some(crop) = rules.farm.crops.get(usize::from(plant.crop)) else {
                    return;
                };
                let fertility = u32::try_from(self.fertility(job.x, job.y)).unwrap_or(1000);
                let units = (crop.yield_units * fertility / 1000).max(1);
                self.spawn_units(crop.product, units, job.x, job.y);
                self.spawn_units(crop.seed, crop.seed_yield, job.x, job.y);
                if !self.farm.harvested {
                    self.farm.harvested = true;
                    let n = i32::try_from(units).unwrap_or(i32::MAX);
                    self.chronicle_add(ChronicleKind::FirstHarvest, [n, 0, 0]);
                }
            }
            JobKind::Gather => {
                if let Some(w) = self.wild_plant_at(job.x, job.y)
                    && let Some(def) = rules.farm.wild.get(usize::from(w.kind))
                {
                    self.spawn_units(def.product, def.yield_units, job.x, job.y);
                    self.farm.regrow.insert((job.x, job.y), def.regrow_days);
                }
                self.designations.remove(&(job.x, job.y));
            }
            _ => {}
        }
        self.finish_job(id, active, job.x, job.y);
    }

    /// Check a loaded farm state against the map and the content.
    pub(crate) fn farm_state_ok(&self) -> bool {
        let inside = |t: &(u16, u16)| self.map.in_bounds(i32::from(t.0), i32::from(t.1));
        let crops = self.rules.farm.crops.len();
        self.farm
            .zones
            .iter()
            .all(|(t, &c)| inside(t) && usize::from(c) < crops)
            && self
                .farm
                .plants
                .iter()
                .all(|(t, p)| inside(t) && usize::from(p.crop) < crops)
            && self.farm.regrow.keys().all(inside)
    }

    /// Put a crop plant on a tile at once, ripe if `ripe`. For scenario tests and debug
    /// tools. Return false if the crop or the tile is not valid.
    pub fn debug_plant(&mut self, x: u16, y: u16, crop: u16, ripe: bool) -> bool {
        let Some(def) = self.rules.farm.crops.get(usize::from(crop)) else {
            return false;
        };
        if !self.farmable(x, y) {
            return false;
        }
        let hours = if ripe {
            u32::from(def.growth_days) * 24
        } else {
            0
        };
        self.farm.plants.insert((x, y), Plant { crop, hours });
        true
    }
}

/// A number for a tile, to hash.
fn tile_key(x: u16, y: u16) -> u64 {
    (u64::from(x) << 20) | u64::from(y)
}
