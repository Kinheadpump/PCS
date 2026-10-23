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
}

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
    issues.finish(StartRules {
        main_skills: raw.main_skills,
        main_level_min: raw.main_level_min,
        main_level_max: raw.main_level_max,
        other_level_min: raw.other_level_min,
        other_level_max: raw.other_level_max,
        stock,
        seed_crops: raw.seed_crops,
        seed_units: raw.seed_units,
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
        self.farm.start_tile = Some((x, y));
        for (material, count) in self.rules.start.stock.clone() {
            self.spawn_units(material, count, x, y);
        }
        self.place_start_seeds();
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
