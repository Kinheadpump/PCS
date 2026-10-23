//! Content that the sim reads: materials, job rules, buildings, recipes, and room rules.
//!
//! The text of the content files is compiled into the crate with `include_str!`. This is
//! not file I/O at run time, so the sim stays deterministic. Item materials are the
//! dense indices (`u16`) of `materials.ron`. Buildings and recipes are the dense indices
//! of `buildings.ron` and `recipes.ron`.

use crate::chronicle::{ChronicleRules, load_chronicle_rules};
use crate::climate::{ClimateRules, FuelDef, load_climate_rules};
use crate::farm::{FarmRules, load_farm_rules};
use crate::ledger::{LedgerRules, load_ledger_rules};
use crate::needs::{NeedRules, TraitDef, load_need_rules, load_traits};
use crate::spoilage::{SpoilRules, load_spoil_rules};
use crate::start::{StartRules, load_start_rules};
use base::content::{
    BuildingDef, BuildingKind, ContentError, ContentIssue, DroughtRules, JobRules, MaterialDef,
    NoticeRules, RaidRules, RecipeDef, RoomRules, load_buildings, load_drought_rules,
    load_job_rules, load_materials, load_notice_rules, load_raid_rules, load_recipes,
    load_room_rules,
};
use std::sync::Arc;
use worldgen::{Ore, RockKind};

const MATERIALS_TEXT: &str = include_str!("../../../assets/content/materials.ron");
const JOBS_TEXT: &str = include_str!("../../../assets/content/jobs.ron");
const BUILDINGS_TEXT: &str = include_str!("../../../assets/content/buildings.ron");
const RECIPES_TEXT: &str = include_str!("../../../assets/content/recipes.ron");
const ROOMS_TEXT: &str = include_str!("../../../assets/content/rooms.ron");
const CLIMATE_TEXT: &str = include_str!("../../../assets/content/climate.ron");
const NEEDS_TEXT: &str = include_str!("../../../assets/content/needs.ron");
const TRAITS_TEXT: &str = include_str!("../../../assets/content/traits.ron");
const SPOILAGE_TEXT: &str = include_str!("../../../assets/content/spoilage.ron");
const CROPS_TEXT: &str = include_str!("../../../assets/content/crops.ron");
const WILD_TEXT: &str = include_str!("../../../assets/content/wild_plants.ron");
const START_TEXT: &str = include_str!("../../../assets/content/start.ron");
const LEDGER_TEXT: &str = include_str!("../../../assets/content/ledger.ron");
const CHRONICLE_TEXT: &str = include_str!("../../../assets/content/chronicle_templates.ron");
const NOTICE_TEXT: &str = include_str!("../../../assets/content/notice.ron");
const DROUGHT_TEXT: &str = include_str!("../../../assets/content/drought.ron");
const RAID_TEXT: &str = include_str!("../../../assets/content/raid.ron");

/// Material index used when a material key is not in the content. It never happens
/// with valid content.
pub const NO_MATERIAL: u16 = u16::MAX;

/// The text of the content files of the repository, for tests that change a file.
#[derive(Clone, Copy, Debug)]
pub struct ContentTexts {
    /// `materials.ron`.
    pub materials: &'static str,
    /// `jobs.ron`.
    pub jobs: &'static str,
    /// `buildings.ron`.
    pub buildings: &'static str,
    /// `recipes.ron`.
    pub recipes: &'static str,
    /// `rooms.ron`.
    pub rooms: &'static str,
    /// `climate.ron`.
    pub climate: &'static str,
    /// `needs.ron`.
    pub needs: &'static str,
    /// `traits.ron`.
    pub traits: &'static str,
    /// `spoilage.ron`.
    pub spoilage: &'static str,
    /// `crops.ron`.
    pub crops: &'static str,
    /// `wild_plants.ron`.
    pub wild_plants: &'static str,
    /// `start.ron`.
    pub start: &'static str,
    /// `ledger.ron`.
    pub ledger: &'static str,
    /// `chronicle_templates.ron`.
    pub chronicle: &'static str,
}

/// The content texts of the repository.
pub const BUILTIN_TEXTS: ContentTexts = ContentTexts {
    materials: MATERIALS_TEXT,
    jobs: JOBS_TEXT,
    buildings: BUILDINGS_TEXT,
    recipes: RECIPES_TEXT,
    rooms: ROOMS_TEXT,
    climate: CLIMATE_TEXT,
    needs: NEEDS_TEXT,
    traits: TRAITS_TEXT,
    spoilage: SPOILAGE_TEXT,
    crops: CROPS_TEXT,
    wild_plants: WILD_TEXT,
    start: START_TEXT,
    ledger: LEDGER_TEXT,
    chronicle: CHRONICLE_TEXT,
};

/// Linked content for the sim systems.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SimRules {
    /// Skill, work time, and job board numbers.
    pub jobs: JobRules,
    /// All materials, in file order.
    pub materials: Vec<MaterialDef>,
    /// All building types, in file order.
    pub buildings: Vec<BuildingDef>,
    /// All recipes, in file order.
    pub recipes: Vec<RecipeDef>,
    /// Room numbers.
    pub rooms: RoomRules,
    /// Climate, weather, water, hearth, and alert numbers.
    pub climate: ClimateRules,
    /// Needs, comfort, warmth, health, and mood numbers.
    pub needs: NeedRules,
    /// Traits, in file order.
    pub traits: Vec<TraitDef>,
    /// Notice, beast, and threat numbers.
    pub notice: NoticeRules,
    /// Drought, irrigation, and fire numbers.
    pub drought: DroughtRules,
    /// Rival colony and raid numbers.
    pub raid: RaidRules,
    /// Fuel materials of the hearth, in file order.
    pub fuel: Vec<FuelDef>,
    /// Food spoilage numbers (R-005).
    pub spoil: SpoilRules,
    /// Crops, wild plants, and fertility numbers (R-006, R-007).
    pub farm: FarmRules,
    /// Start conditions.
    pub start: StartRules,
    /// Ledger numbers.
    pub ledger: LedgerRules,
    /// Chronicle numbers and templates.
    pub chronicle: ChronicleRules,
    /// Material of a log.
    pub log: u16,
    /// Material of a wall tile without a building (`rooms.ron`).
    pub bare_wall: u16,
    /// Stone item material for each `RockKind` code (index 0 is unused).
    pub rock_item: [u16; 4],
    /// Ore item material for each `Ore` code (index 0 is unused).
    pub ore_item: [u16; 4],
}

impl Default for SimRules {
    /// The rules from the content files of the repository. If they are invalid (a test
    /// checks that they are not), the rules are empty and no job gives any item.
    fn default() -> Self {
        Self::load_all(&BUILTIN_TEXTS).unwrap_or_else(|_| Self {
            jobs: JobRules::default(),
            materials: Vec::new(),
            buildings: Vec::new(),
            recipes: Vec::new(),
            rooms: RoomRules {
                max_tiles: 400,
                roof_support_radius: 6,
                bare_wall_material: String::new(),
            },
            climate: ClimateRules::empty(),
            needs: NeedRules::empty(),
            traits: Vec::new(),
            notice: NoticeRules::default(),
            drought: DroughtRules::default(),
            raid: RaidRules::default(),
            fuel: Vec::new(),
            spoil: SpoilRules::empty(),
            farm: FarmRules::empty(),
            start: StartRules::empty(),
            ledger: LedgerRules::empty(),
            chronicle: ChronicleRules::empty(),
            log: NO_MATERIAL,
            bare_wall: NO_MATERIAL,
            rock_item: [NO_MATERIAL; 4],
            ore_item: [NO_MATERIAL; 4],
        })
    }
}

impl SimRules {
    /// Link the rules from the repository files, but with another climate file.
    pub fn load_with_climate(climate_text: &str) -> Result<Self, ContentError> {
        Self::load_texts_climate(
            MATERIALS_TEXT,
            JOBS_TEXT,
            BUILDINGS_TEXT,
            RECIPES_TEXT,
            ROOMS_TEXT,
            climate_text,
        )
    }

    /// Link the rules from the text of the materials file and the jobs file. The other
    /// files come from the repository. All required materials must exist.
    pub fn load(materials_text: &str, jobs_text: &str) -> Result<Self, ContentError> {
        Self::load_texts(
            materials_text,
            jobs_text,
            BUILDINGS_TEXT,
            RECIPES_TEXT,
            ROOMS_TEXT,
        )
    }

    /// Link the rules from a set of file texts.
    pub fn load_all(texts: &ContentTexts) -> Result<Self, ContentError> {
        Self::load_full(texts)
    }

    /// Link the rules from the repository files, but with other needs and traits files.
    pub fn load_with_needs(needs_text: &str, traits_text: &str) -> Result<Self, ContentError> {
        Self::load_full_with(
            BUILTIN_TEXTS.materials,
            BUILTIN_TEXTS.jobs,
            BUILTIN_TEXTS.buildings,
            BUILTIN_TEXTS.recipes,
            BUILTIN_TEXTS.rooms,
            BUILTIN_TEXTS.climate,
            needs_text,
            traits_text,
        )
    }

    fn load_full(texts: &ContentTexts) -> Result<Self, ContentError> {
        Self::load_full_with(
            texts.materials,
            texts.jobs,
            texts.buildings,
            texts.recipes,
            texts.rooms,
            texts.climate,
            texts.needs,
            texts.traits,
        )
        .and_then(|mut rules| {
            rules.load_sprint7(texts)?;
            Ok(rules)
        })
    }

    /// Load the files of Sprint 7: spoilage, crops, wild plants, start, ledger, chronicle.
    fn load_sprint7(&mut self, texts: &ContentTexts) -> Result<(), ContentError> {
        self.spoil = load_spoil_rules("spoilage.ron", texts.spoilage)?;
        self.farm = load_farm_rules(texts.crops, texts.wild_plants, &self.materials)?;
        self.start = load_start_rules("start.ron", texts.start, &self.materials)?;
        self.ledger = load_ledger_rules("ledger.ron", texts.ledger)?;
        self.chronicle = load_chronicle_rules("chronicle_templates.ron", texts.chronicle)?;
        Ok(())
    }

    /// Link the rules from the text of the five content files. The climate file comes from
    /// the repository.
    pub fn load_texts(
        materials_text: &str,
        jobs_text: &str,
        buildings_text: &str,
        recipes_text: &str,
        rooms_text: &str,
    ) -> Result<Self, ContentError> {
        Self::load_texts_climate(
            materials_text,
            jobs_text,
            buildings_text,
            recipes_text,
            rooms_text,
            CLIMATE_TEXT,
        )
    }

    /// Link the rules from the text of the six content files.
    pub fn load_texts_climate(
        materials_text: &str,
        jobs_text: &str,
        buildings_text: &str,
        recipes_text: &str,
        rooms_text: &str,
        climate_text: &str,
    ) -> Result<Self, ContentError> {
        Self::load_full_with(
            materials_text,
            jobs_text,
            buildings_text,
            recipes_text,
            rooms_text,
            climate_text,
            NEEDS_TEXT,
            TRAITS_TEXT,
        )
    }

    /// Link the rules from the text of all eight content files.
    #[allow(clippy::too_many_arguments)]
    pub fn load_full_with(
        materials_text: &str,
        jobs_text: &str,
        buildings_text: &str,
        recipes_text: &str,
        rooms_text: &str,
        climate_text: &str,
        needs_text: &str,
        traits_text: &str,
    ) -> Result<Self, ContentError> {
        let materials = load_materials("materials.ron", materials_text)?;
        let jobs = load_job_rules("jobs.ron", jobs_text)?;
        let rooms = load_room_rules("rooms.ron", rooms_text)?;
        let climate = load_climate_rules("climate.ron", climate_text)?;
        let needs = load_need_rules("needs.ron", needs_text, &materials)?;
        let traits = load_traits("traits.ron", traits_text)?;
        let notice = load_notice_rules("notice.ron", NOTICE_TEXT)?;
        let drought = load_drought_rules("drought.ron", DROUGHT_TEXT)?;
        let raid = load_raid_rules("raid.ron", RAID_TEXT)?;
        let mut issues = Vec::new();
        let mut find = |key: &str| -> u16 {
            match materials.iter().position(|m| m.key == key) {
                Some(i) => u16::try_from(i).unwrap_or(NO_MATERIAL),
                None => {
                    issues.push(ContentIssue {
                        entry: Some(key.to_string()),
                        message: "the sim rules need this material".to_string(),
                    });
                    NO_MATERIAL
                }
            }
        };
        let log = find("log");
        let bare_wall = find(&rooms.bare_wall_material);
        let rock_item = [
            NO_MATERIAL,
            find("sandstone"),
            find("limestone"),
            find("granite"),
        ];
        let ore_item = [
            NO_MATERIAL,
            find("coal"),
            find("copper_ore"),
            find("iron_ore"),
        ];
        let fuel: Vec<FuelDef> = climate
            .fuel
            .iter()
            .map(|(key, hours)| FuelDef {
                material: find(key),
                hours: *hours,
            })
            .collect();
        if !issues.is_empty() {
            return Err(ContentError {
                file: "materials.ron".to_string(),
                issues,
            });
        }
        let buildings = load_buildings("buildings.ron", buildings_text)?;
        let recipes = load_recipes("recipes.ron", recipes_text, &materials, &buildings)?;
        let mut rules = Self {
            jobs,
            materials,
            buildings,
            recipes,
            rooms,
            climate,
            needs,
            traits,
            notice,
            drought,
            raid,
            fuel,
            spoil: SpoilRules::empty(),
            farm: FarmRules::empty(),
            start: StartRules::empty(),
            ledger: LedgerRules::empty(),
            chronicle: ChronicleRules::empty(),
            log,
            bare_wall,
            rock_item,
            ore_item,
        };
        rules.load_sprint7(&BUILTIN_TEXTS)?;
        Ok(rules)
    }

    /// The rules from the content files of the repository.
    pub fn builtin() -> Arc<Self> {
        Arc::new(Self::default())
    }

    /// Stone item material of a rock kind.
    pub fn rock_material(&self, rock: RockKind) -> u16 {
        self.rock_item
            .get(rock as usize)
            .copied()
            .unwrap_or(NO_MATERIAL)
    }

    /// Ore item material of an ore.
    pub fn ore_material(&self, ore: Ore) -> u16 {
        self.ore_item
            .get(ore as usize)
            .copied()
            .unwrap_or(NO_MATERIAL)
    }

    /// Hardness of a material, 0 if unknown.
    pub fn hardness(&self, material: u16) -> u32 {
        self.materials
            .get(usize::from(material))
            .map_or(0, |m| u32::from(m.hardness))
    }

    /// Insulation of a material in per-mille, 0 if unknown.
    pub fn insulation(&self, material: u16) -> u32 {
        self.materials
            .get(usize::from(material))
            .map_or(0, |m| u32::from(m.insulation))
    }

    /// Beauty of a material, 0 if unknown.
    pub fn beauty(&self, material: u16) -> i32 {
        self.materials
            .get(usize::from(material))
            .map_or(0, |m| i32::from(m.beauty))
    }

    /// Display name of a material, empty if unknown.
    pub fn material_name(&self, material: u16) -> &str {
        self.materials
            .get(usize::from(material))
            .map_or("", |m| m.name.as_str())
    }

    /// Index of a material by its string ID.
    pub fn material_index(&self, key: &str) -> Option<u16> {
        self.materials
            .iter()
            .position(|m| m.key == key)
            .and_then(|i| u16::try_from(i).ok())
    }

    /// Index of a building type by its string ID.
    pub fn building_index(&self, key: &str) -> Option<u16> {
        self.buildings
            .iter()
            .position(|b| b.key == key)
            .and_then(|i| u16::try_from(i).ok())
    }

    /// Index of a recipe by its string ID.
    pub fn recipe_index(&self, key: &str) -> Option<u16> {
        self.recipes
            .iter()
            .position(|r| r.key == key)
            .and_then(|i| u16::try_from(i).ok())
    }

    /// The building type with this index.
    pub fn building(&self, index: u16) -> Option<&BuildingDef> {
        self.buildings.get(usize::from(index))
    }

    /// The recipe with this index.
    pub fn recipe(&self, index: u16) -> Option<&RecipeDef> {
        self.recipes.get(usize::from(index))
    }

    /// True if the building type can be made from the material.
    pub fn material_fits(&self, building: u16, material: u16) -> bool {
        match (
            self.building(building),
            self.materials.get(usize::from(material)),
        ) {
            (Some(b), Some(m)) => b.categories.contains(&m.category),
            _ => false,
        }
    }

    /// The work time of a building type made from a material, in ticks at 100% speed.
    pub fn build_ticks(&self, building: u16, material: u16) -> u32 {
        self.building(building).map_or(0, |b| {
            b.work_ticks + self.hardness(material) * b.work_ticks_per_10_hardness / 10
        })
    }

    /// True if the building type has this kind.
    pub fn is_kind(&self, building: u16, kind: BuildingKind) -> bool {
        self.building(building).is_some_and(|b| b.kind == kind)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_content_loads() {
        let rules = SimRules::load(MATERIALS_TEXT, JOBS_TEXT).unwrap();
        assert_eq!(rules.jobs.xp_per_level, 1000);
        assert_ne!(rules.log, NO_MATERIAL);
        assert_eq!(rules.materials[usize::from(rules.log)].key, "log");
        assert_eq!(SimRules::builtin().jobs, rules.jobs);
        assert_eq!(rules.buildings.len(), 8);
        assert_eq!(rules.recipes.len(), 7);
        assert_eq!(rules.rooms.max_tiles, 400);
        let wall = rules.building_index("wall").unwrap();
        let granite = rules.material_index("granite").unwrap();
        assert!(rules.material_fits(wall, granite));
        assert!(!rules.material_fits(wall, rules.material_index("iron_ore").unwrap()));
    }

    #[test]
    fn missing_material_names_file_and_entry() {
        let text = MATERIALS_TEXT.replace("id: \"log\"", "id: \"timber\"");
        let msg = SimRules::load(&text, JOBS_TEXT).unwrap_err().to_string();
        assert!(
            msg.contains("materials.ron") && msg.contains("'log'"),
            "{msg}"
        );
    }

    #[test]
    fn recipe_with_unknown_material_names_file_and_entry() {
        let text = RECIPES_TEXT.replace("(\"plank\", 2)", "(\"board\", 2)");
        let msg =
            SimRules::load_texts(MATERIALS_TEXT, JOBS_TEXT, BUILDINGS_TEXT, &text, ROOMS_TEXT)
                .unwrap_err()
                .to_string();
        assert!(
            msg.contains("recipes.ron") && msg.contains("'plank'") && msg.contains("board"),
            "{msg}"
        );
    }
}
