//! Content that the sim reads: materials and job rules.
//!
//! The text of the content files is compiled into the crate with `include_str!`. This is
//! not file I/O at run time, so the sim stays deterministic. Item materials are the
//! dense indices (`u16`) of `materials.ron`.

use base::content::{
    ContentError, ContentIssue, JobRules, MaterialDef, load_job_rules, load_materials,
};
use std::sync::Arc;
use worldgen::{Ore, RockKind};

const MATERIALS_TEXT: &str = include_str!("../../../assets/content/materials.ron");
const JOBS_TEXT: &str = include_str!("../../../assets/content/jobs.ron");

/// Material index used when a material key is not in the content. It never happens
/// with valid content.
pub const NO_MATERIAL: u16 = u16::MAX;

/// Linked content for the job systems.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SimRules {
    /// Skill, work time, and job board numbers.
    pub jobs: JobRules,
    /// All materials, in file order.
    pub materials: Vec<MaterialDef>,
    /// Material of a log.
    pub log: u16,
    /// Stone item material for each `RockKind` code (index 0 is unused).
    pub rock_item: [u16; 4],
    /// Ore item material for each `Ore` code (index 0 is unused).
    pub ore_item: [u16; 4],
}

impl Default for SimRules {
    /// The rules from the content files of the repository. If they are invalid (a test
    /// checks that they are not), the rules are empty and no job gives any item.
    fn default() -> Self {
        Self::load(MATERIALS_TEXT, JOBS_TEXT).unwrap_or_else(|_| Self {
            jobs: JobRules::default(),
            materials: Vec::new(),
            log: NO_MATERIAL,
            rock_item: [NO_MATERIAL; 4],
            ore_item: [NO_MATERIAL; 4],
        })
    }
}

impl SimRules {
    /// Link the rules from file text. All required materials must exist.
    pub fn load(materials_text: &str, jobs_text: &str) -> Result<Self, ContentError> {
        let materials = load_materials("materials.ron", materials_text)?;
        let jobs = load_job_rules("jobs.ron", jobs_text)?;
        let mut issues = Vec::new();
        let mut find = |key: &str| -> u16 {
            match materials.iter().position(|m| m.key == key) {
                Some(i) => u16::try_from(i).unwrap_or(NO_MATERIAL),
                None => {
                    issues.push(ContentIssue {
                        entry: Some(key.to_string()),
                        message: "the job rules need this material".to_string(),
                    });
                    NO_MATERIAL
                }
            }
        };
        let log = find("log");
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
        if !issues.is_empty() {
            return Err(ContentError {
                file: "materials.ron".to_string(),
                issues,
            });
        }
        Ok(Self {
            jobs,
            materials,
            log,
            rock_item,
            ore_item,
        })
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

    /// Display name of a material, empty if unknown.
    pub fn material_name(&self, material: u16) -> &str {
        self.materials
            .get(usize::from(material))
            .map_or("", |m| m.name.as_str())
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
}
