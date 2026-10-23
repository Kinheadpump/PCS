//! Start conditions of a new colony (GAME_DESIGN §5, `start.ron`).
//!
//! Each colonist gets main skills at a high level and other skills at a low level. The
//! colony gets a stock on the tile of the first colonist and seeds for the crops that grow
//! in the local climate.

use crate::Sim;
use crate::climate::day_mean;
use crate::colonist::ColonistId;
use crate::item::ItemLoc;
use crate::loader::{Issues, parse};
use crate::skill::SKILL_COUNT;
use crate::time::DAYS_PER_YEAR;
use base::Pcg32;
use base::content::{ContentError, MaterialDef};
use serde::Deserialize;

/// Numbers for the start (`start.ron`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StartRules {
    /// Number of main skills of a colonist.
    pub main_skills: u32,
    /// Lowest level of a main skill.
    pub main_level_min: u32,
    /// Highest level of a main skill.
    pub main_level_max: u32,
    /// Lowest level of the other skills.
    pub other_level_min: u32,
    /// Highest level of the other skills.
    pub other_level_max: u32,
    /// The start stock: material index and units.
    pub stock: Vec<(u16, u32)>,
    /// Number of crops with seeds.
    pub seed_crops: u32,
    /// Seed units for each crop.
    pub seed_units: u32,
    /// The start camp: a closed cabin with beds and a lit hearth, and a stockpile.
    pub camp: CampRules,
}

/// Numbers for the start camp (`start.ron`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CampRules {
    /// The camp is built at most this many tiles from the first colonist. 0: no camp.
    pub radius: u32,
    /// Material of the walls, the door, the floor, the roof, and the beds.
    pub wood: u16,
    /// Material of the hearth.
    pub stone: u16,
    /// Fuel in the hearth at the start, in burning hours.
    pub fuel_hours: u32,
    /// Game hours that the hearth has warmed the cabin before the first tick.
    pub preheat_hours: u32,
}

/// Width and height of the cabin, with its walls.
const CABIN: u16 = 6;
/// Rows below the cabin that the camp needs: the stockpile and one free row.
const YARD: u16 = 3;

impl StartRules {
    /// Rules without stock. For the empty default rules.
    pub fn empty() -> Self {
        Self {
            main_skills: 0,
            main_level_min: 0,
            main_level_max: 0,
            other_level_min: 0,
            other_level_max: 0,
            stock: Vec::new(),
            seed_crops: 0,
            seed_units: 0,
            camp: CampRules {
                radius: 0,
                wood: 0,
                stone: 0,
                fuel_hours: 0,
                preheat_hours: 0,
            },
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawStart {
    main_skills: u32,
    main_level_min: u32,
    main_level_max: u32,
    other_level_min: u32,
    other_level_max: u32,
    stock: Vec<(String, u32)>,
    seed_crops: u32,
    seed_units: u32,
    camp: RawCamp,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawCamp {
    radius: u32,
    wood: String,
    stone: String,
    fuel_hours: u32,
    preheat_hours: u32,
}

/// Load and check `start.ron`.
pub fn load_start_rules(
    file: &str,
    text: &str,
    materials: &[MaterialDef],
) -> Result<StartRules, ContentError> {
    let raw: RawStart = parse(file, text)?;
    let mut issues = Issues::new(file);
    if raw.main_skills as usize > SKILL_COUNT {
        issues.add("main_skills", "more than the number of skills");
    }
    if raw.main_level_min > raw.main_level_max || raw.other_level_min > raw.other_level_max {
        issues.add("levels", "the lowest level is above the highest");
    }
    if raw.main_level_max > 20 || raw.other_level_max > 20 {
        issues.add("levels", "at most 20");
    }
    let stock = raw
        .stock
        .iter()
        .map(|(key, n)| (issues.material(materials, key, "start stock"), *n))
        .collect();
    let camp = CampRules {
        radius: raw.camp.radius.min(200),
        wood: issues.material(materials, &raw.camp.wood, "start camp"),
        stone: issues.material(materials, &raw.camp.stone, "start camp"),
        fuel_hours: raw.camp.fuel_hours.min(100_000),
        preheat_hours: raw.camp.preheat_hours.min(1000),
    };
    issues.finish(StartRules {
        main_skills: raw.main_skills,
        main_level_min: raw.main_level_min,
        main_level_max: raw.main_level_max,
        other_level_min: raw.other_level_min,
        other_level_max: raw.other_level_max,
        stock,
        seed_crops: raw.seed_crops,
        seed_units: raw.seed_units,
        camp,
    })
}

impl Sim {
    /// Roll the start skills of the colonists (GAME_DESIGN §5). Own random stream.
    pub(crate) fn roll_start_skills(&mut self, seed: u64) {
        let mut rng = Pcg32::from_seed(seed ^ 0x534b_494c_4c53_5441);
        let r = self.rules.start.clone();
        let xp_per_level = self.rules.jobs.xp_per_level;
        let ids: Vec<ColonistId> = self.colonists.iter().map(|(id, _)| id).collect();
        for id in ids {
            let mut order: Vec<usize> = (0..SKILL_COUNT).collect();
            for i in 0..SKILL_COUNT.saturating_sub(1) {
                let j = i + rng.below((SKILL_COUNT - i) as u32) as usize;
                order.swap(i, j);
            }
            let mut levels = [0u32; SKILL_COUNT];
            for (k, &skill) in order.iter().enumerate() {
                let (lo, hi) = if k < r.main_skills as usize {
                    (r.main_level_min, r.main_level_max)
                } else {
                    (r.other_level_min, r.other_level_max)
                };
                levels[skill] = lo + rng.below(hi - lo + 1);
            }
            if let Some(c) = self.colonists.get_mut(id) {
                for (skill, level) in levels.iter().enumerate() {
                    c.skill_xp[skill] = level * xp_per_level;
                }
            }
        }
    }

    /// Put the start stock on the tile of the first colonist, and the seeds.
    pub(crate) fn place_start_stock(&mut self) {
        let Some((x, y)) = self.colonists.iter().next().map(|(_, c)| c.tile()) else {
            return;
        };
        let (x, y) = self.place_start_camp((x, y)).unwrap_or((x, y));
        self.farm.start_tile = Some((x, y));
        for (material, count) in self.rules.start.stock.clone() {
            self.spawn_units(material, count, x, y);
        }
        self.place_start_seeds();
        self.preheat_start_camp();
    }

    /// Warm the rooms of a new colony: the hearth of the start camp has burned for
    /// `preheat_hours` before the first tick. Call it again when the climate changes.
    pub(crate) fn preheat_start_camp(&mut self) {
        if self.tick != 0 || self.rules.start.camp.radius == 0 {
            return;
        }
        self.refresh_rooms();
        for _ in 0..self.rules.start.camp.preheat_hours {
            self.update_room_temperatures();
        }
    }

    /// Build the start camp near a tile: a closed cabin with stone walls, a floor, a roof, seven
    /// beds, and a burning hearth, and a stockpile in front of its door. Return the first tile of the
    /// stockpile. Return `None` if there is no free place or the rules have no camp.
    fn place_start_camp(&mut self, near: (u16, u16)) -> Option<(u16, u16)> {
        let camp = self.rules.start.camp.clone();
        if camp.radius == 0 {
            return None;
        }
        let door = self.rules.building_index("door")?;
        let (wall, floor, roof, bed, hearth) = (
            self.rules.building_index("wall")?,
            self.rules.building_index("floor")?,
            self.rules.building_index("roof")?,
            self.rules.building_index("bed")?,
            self.rules.building_index("hearth")?,
        );
        let (cx, cy) = (i32::from(near.0), i32::from(near.1));
        let radius = camp.radius as i32;
        let mut best: Option<(i32, i32, i32)> = None;
        for sy in (cy - radius)..=(cy + radius) {
            for sx in (cx - radius)..=(cx + radius) {
                if !self.camp_site_free(sx, sy) {
                    continue;
                }
                let dx = sx + i32::from(CABIN) / 2 - cx;
                let dy = sy + i32::from(CABIN) / 2 - cy;
                let key = (dx * dx + dy * dy, sy, sx);
                if best.is_none_or(|b| key < b) {
                    best = Some(key);
                }
            }
        }
        let (_, sy, sx) = best?;
        let (sx, sy) = (u16::try_from(sx).ok()?, u16::try_from(sy).ok()?);
        for dy in 0..CABIN + YARD {
            for dx in 0..CABIN {
                self.map.clear_tree(i32::from(sx + dx), i32::from(sy + dy));
            }
        }
        let last = CABIN - 1;
        for dy in 0..CABIN {
            for dx in 0..CABIN {
                let (x, y) = (sx + dx, sy + dy);
                let edge = dx == 0 || dy == 0 || dx == last || dy == last;
                if edge {
                    if (dx, dy) == (CABIN / 2, last) {
                        self.debug_build(door, x, y, camp.wood);
                    } else {
                        self.debug_build(wall, x, y, camp.stone);
                    }
                } else {
                    self.debug_build(floor, x, y, camp.wood);
                    self.debug_build(roof, x, y, camp.wood);
                }
            }
        }
        for dx in 1..last {
            self.debug_build(bed, sx + dx, sy + 2, camp.wood);
        }
        self.debug_build(bed, sx + 1, sy + 3, camp.wood);
        self.debug_build(bed, sx + last - 1, sy + 3, camp.wood);
        self.debug_build(bed, sx + 1, sy + 4, camp.wood);
        let (hx, hy) = (sx + 2, sy + 1);
        self.debug_build(hearth, hx, hy, camp.stone);
        self.debug_set_hearth_fuel(hx, hy, camp.fuel_hours);
        let zone = crate::Stockpile {
            x0: sx + 2,
            y0: sy + CABIN + 1,
            x1: sx + 4,
            y1: sy + CABIN + 2,
            filter: crate::stockpile::ItemFilter::All,
        };
        let first = (zone.x0, zone.y0);
        self.stockpiles.insert(zone);
        self.map.rebuild_passable();
        self.rebuild_caches();
        Some(first)
    }

    /// True if the camp fits with its top left corner on a tile: the cabin and the yard in
    /// front of it are inside the map, on free ground.
    fn camp_site_free(&self, sx: i32, sy: i32) -> bool {
        let (w, h) = (i32::from(CABIN), i32::from(CABIN + YARD));
        (sy..sy + h).all(|y| {
            (sx..sx + w).all(|x| {
                self.map.in_bounds(x, y)
                    && x > 0
                    && y > 0
                    && x < self.map.width() as i32 - 1
                    && y < self.map.height() as i32 - 1
                    && self.map.ground(x, y) != Some(worldgen::Ground::Water)
                    && self.map.rock(x, y) == Some(worldgen::RockKind::None)
                    && !self.map.wall(x, y)
            })
        })
    }

    /// The crops that grow in the climate: the season curve gives at least `growth_days`
    /// days with a day mean temperature inside the band of the crop. The first
    /// `seed_crops` crops in file order.
    pub fn suitable_crops(&self) -> Vec<u16> {
        let rules = &self.rules;
        let means: Vec<i16> = (1..=DAYS_PER_YEAR as u32)
            .map(|d| day_mean(&rules.climate, &self.climate.params, d, 0))
            .collect();
        rules
            .farm
            .crops
            .iter()
            .enumerate()
            .filter(|(_, c)| {
                let days = means
                    .iter()
                    .filter(|&&m| (c.min_temp..=c.max_temp).contains(&m))
                    .count();
                days >= usize::from(c.growth_days)
            })
            .filter_map(|(i, _)| u16::try_from(i).ok())
            .collect()
    }

    /// Put seed items for the suitable crops on the start tile. Existing start seeds go
    /// first, so a change of the climate before the first tick gives the right seeds.
    pub(crate) fn place_start_seeds(&mut self) {
        let Some((x, y)) = self.farm.start_tile else {
            return;
        };
        let seed_materials: Vec<u16> = self.rules.farm.crops.iter().map(|c| c.seed).collect();
        let old: Vec<_> = self
            .items
            .iter()
            .filter(|(_, i)| seed_materials.contains(&i.material) && i.loc == ItemLoc::Tile(x, y))
            .map(|(id, _)| id)
            .collect();
        for id in old {
            self.items.remove(id);
        }
        let want = self.rules.start.seed_crops as usize;
        let units = self.rules.start.seed_units;
        for crop in self.suitable_crops().into_iter().take(want) {
            if let Some(def) = self.rules.farm.crops.get(usize::from(crop)) {
                let seed = def.seed;
                self.spawn_units(seed, units, x, y);
            }
        }
    }
}
