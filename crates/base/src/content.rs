//! Content loader and validator (D-014, TECHNICAL_DESIGN §10).
//!
//! The functions take file text. They do no file I/O. The caller reads the file.
//! All numbers in content are integers. The loader refuses float literals.
//! The loader collects all errors and does not stop at the first error.

use std::collections::BTreeMap;
use std::fmt;

use serde::Deserialize;

/// Dense identifier of a material. It is the index in the loaded material list.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MaterialId(pub u16);

/// Category of a material.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
pub enum MaterialCategory {
    /// Rock.
    Stone,
    /// Rock that holds metal.
    Ore,
    /// Smelted metal.
    Metal,
    /// Wood.
    Wood,
    /// Soil, sand, and clay.
    Soil,
    /// Plant matter.
    Plant,
    /// Cloth.
    Cloth,
    /// Leather.
    Leather,
    /// Food.
    Food,
    /// Fuel.
    Fuel,
    /// Water and ice.
    Fluid,
}

/// Result of smelting an ore.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SmeltsTo {
    /// The output metal.
    pub metal: MaterialId,
    /// Fuel cost for each unit of ore.
    pub fuel_cost: u16,
}

/// A material after validation and linking.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MaterialDef {
    /// Dense ID. It equals the position in the list.
    pub id: MaterialId,
    /// String ID from the file.
    pub key: String,
    /// Display name.
    pub name: String,
    /// Category.
    pub category: MaterialCategory,
    /// Hardness (whole units).
    pub hardness: u16,
    /// Density (whole units).
    pub density: u16,
    /// Value (whole units).
    pub value: u16,
    /// Insulation in per-mille (0 to 1000).
    pub insulation: u16,
    /// Flammability in per-mille (0 to 1000).
    pub flammability: u16,
    /// Beauty (whole units).
    pub beauty: u16,
    /// Spoilage in per-mille (0 to 1000). Zero for non-food.
    pub rot_rate: u16,
    /// Palette color.
    pub color: [u8; 3],
    /// Melting point in tenths of a degree Celsius.
    pub melts_at: Option<i16>,
    /// Material that this material becomes when it melts.
    pub melts_into: Option<MaterialId>,
    /// Smelting result, for ore.
    pub smelts_to: Option<SmeltsTo>,
    /// Rock materials in which this ore can occur.
    pub ore_in_rock: Vec<MaterialId>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawSmelts {
    metal: String,
    fuel_cost: u16,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawMaterial {
    id: String,
    name: String,
    category: MaterialCategory,
    hardness: u16,
    density: u16,
    value: u16,
    insulation: u16,
    flammability: u16,
    beauty: u16,
    #[serde(default)]
    rot_rate: u16,
    color: [u8; 3],
    #[serde(default)]
    melts_at: Option<i16>,
    #[serde(default)]
    melts_into: Option<String>,
    #[serde(default)]
    smelts_to: Option<RawSmelts>,
    #[serde(default)]
    ore_in_rock: Vec<String>,
}

/// One problem found in a content file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContentIssue {
    /// Entry name or ID, if the problem belongs to one entry.
    pub entry: Option<String>,
    /// Description of the problem.
    pub message: String,
}

/// Error from content loading. It holds the file name and all problems.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContentError {
    /// Name of the content file.
    pub file: String,
    /// All problems found. Never empty.
    pub issues: Vec<ContentIssue>,
}

impl fmt::Display for ContentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, issue) in self.issues.iter().enumerate() {
            if i > 0 {
                writeln!(f)?;
            }
            match &issue.entry {
                Some(entry) => write!(f, "{}: entry '{}': {}", self.file, entry, issue.message)?,
                None => write!(f, "{}: {}", self.file, issue.message)?,
            }
        }
        Ok(())
    }
}

impl std::error::Error for ContentError {}

/// Find float literals in RON text. Strings and comments are ignored.
/// Returns the 1-based line and the text of each float literal.
pub fn find_float_literals(text: &str) -> Vec<(usize, String)> {
    let chars: Vec<char> = text.chars().collect();
    let mut found = Vec::new();
    let mut line = 1usize;
    let mut i = 0usize;
    while i < chars.len() {
        let c = chars[i];
        let next = chars.get(i + 1).copied();
        if c == '\n' {
            line += 1;
            i += 1;
        } else if c == '"' {
            i += 1;
            while i < chars.len() && chars[i] != '"' {
                if chars[i] == '\\' {
                    i += 1;
                }
                if chars.get(i) == Some(&'\n') {
                    line += 1;
                }
                i += 1;
            }
            i += 1;
        } else if c == '/' && next == Some('/') {
            while i < chars.len() && chars[i] != '\n' {
                i += 1;
            }
        } else if c == '/' && next == Some('*') {
            i += 2;
            while i < chars.len() && !(chars[i] == '*' && chars.get(i + 1) == Some(&'/')) {
                if chars[i] == '\n' {
                    line += 1;
                }
                i += 1;
            }
            i += 2;
        } else if c.is_ascii_alphabetic() || c == '_' {
            while i < chars.len() && (chars[i].is_ascii_alphanumeric() || chars[i] == '_') {
                i += 1;
            }
        } else if c.is_ascii_digit() {
            let start = i;
            while i < chars.len()
                && (chars[i].is_ascii_alphanumeric() || chars[i] == '_' || chars[i] == '.')
            {
                i += 1;
            }
            let token: String = chars[start..i].iter().collect();
            let is_hex = token.starts_with("0x") || token.starts_with("0X");
            let has_exponent = !is_hex && token.contains(['e', 'E']);
            if token.contains('.') || has_exponent {
                found.push((line, token));
            }
        } else {
            i += 1;
        }
    }
    found
}

/// Load and validate a materials file.
///
/// `file_name` is used in error messages. `text` is the RON text.
/// Returns materials in file order. The dense ID is the position.
pub fn load_materials(file_name: &str, text: &str) -> Result<Vec<MaterialDef>, ContentError> {
    let floats = find_float_literals(text);
    if !floats.is_empty() {
        let issues = floats
            .into_iter()
            .map(|(line, token)| ContentIssue {
                entry: None,
                message: format!(
                    "line {line}: float literal '{token}' is not allowed; use integers"
                ),
            })
            .collect();
        return Err(ContentError {
            file: file_name.to_string(),
            issues,
        });
    }
    let raw: Vec<RawMaterial> = ron::from_str(text).map_err(|e| ContentError {
        file: file_name.to_string(),
        issues: vec![ContentIssue {
            entry: None,
            message: format!("parse error: {e}"),
        }],
    })?;
    link_materials(file_name, &raw)
}

fn link_materials(file_name: &str, raw: &[RawMaterial]) -> Result<Vec<MaterialDef>, ContentError> {
    let mut issues: Vec<ContentIssue> = Vec::new();
    let mut add = |entry: &str, message: String| {
        issues.push(ContentIssue {
            entry: Some(entry.to_string()),
            message,
        });
    };

    if raw.len() >= usize::from(u16::MAX) {
        add("(file)", "too many materials".to_string());
        return Err(ContentError {
            file: file_name.to_string(),
            issues,
        });
    }

    let mut index: BTreeMap<&str, u16> = BTreeMap::new();
    for (i, m) in raw.iter().enumerate() {
        let id = u16::try_from(i).unwrap_or(u16::MAX);
        if index.insert(m.id.as_str(), id).is_some() {
            add(&m.id, "duplicate ID".to_string());
        }
    }

    let mut out = Vec::with_capacity(raw.len());
    for (i, m) in raw.iter().enumerate() {
        let name = m.id.as_str();
        if name.is_empty() {
            add(&format!("#{i}"), "empty ID".to_string());
        }
        for (field, value) in [
            ("insulation", m.insulation),
            ("flammability", m.flammability),
            ("rot_rate", m.rot_rate),
        ] {
            if value > 1000 {
                add(
                    name,
                    format!("{field} is {value}; the maximum is 1000 per-mille"),
                );
            }
        }
        let mut resolve = |field: &str, target: &str| -> MaterialId {
            if let Some(&id) = index.get(target) {
                MaterialId(id)
            } else {
                add(
                    name,
                    format!("{field} refers to unknown material '{target}'"),
                );
                MaterialId(u16::MAX)
            }
        };
        let melts_into = m.melts_into.as_deref().map(|t| resolve("melts_into", t));
        let smelts_to = m.smelts_to.as_ref().map(|s| SmeltsTo {
            metal: resolve("smelts_to.metal", &s.metal),
            fuel_cost: s.fuel_cost,
        });
        let ore_in_rock = m
            .ore_in_rock
            .iter()
            .map(|t| resolve("ore_in_rock", t))
            .collect();
        out.push(MaterialDef {
            id: MaterialId(u16::try_from(i).unwrap_or(u16::MAX)),
            key: m.id.clone(),
            name: m.name.clone(),
            category: m.category,
            hardness: m.hardness,
            density: m.density,
            value: m.value,
            insulation: m.insulation,
            flammability: m.flammability,
            beauty: m.beauty,
            rot_rate: m.rot_rate,
            color: m.color,
            melts_at: m.melts_at,
            melts_into,
            smelts_to,
            ore_in_rock,
        });
    }

    if issues.is_empty() {
        Ok(out)
    } else {
        Err(ContentError {
            file: file_name.to_string(),
            issues,
        })
    }
}

/// Numbers for skills, work times, and the job board (`jobs.ron`). All integers.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JobRules {
    /// Experience points that one completed job of a skill type adds (R-010).
    pub xp_per_job: u32,
    /// Experience points for one skill level (R-010).
    pub xp_per_level: u32,
    /// Highest skill level (R-010).
    pub max_level: u8,
    /// Work speed in percent at skill level 0.
    pub speed_base_pct: i32,
    /// Work speed in percent that each skill level adds.
    pub speed_per_level_pct: i32,
    /// Work time of a cut job, in ticks at 100% speed.
    pub cut_ticks: u32,
    /// Base work time of a mine job, in ticks at 100% speed.
    pub mine_ticks_base: u32,
    /// Work time that a mine job adds for each 10 hardness units, in ticks.
    pub mine_ticks_per_10_hardness: u32,
    /// Logs that a cut tree gives.
    pub log_yield: u32,
    /// Stone items that a mined rock tile gives.
    pub stone_yield: u32,
    /// Ore items that a mined ore tile gives in addition to the stone.
    pub ore_yield: u32,
    /// Largest item stack.
    pub stack_max: u32,
    /// Largest number of jobs that the board examines for all colonists in one tick.
    pub search_budget: u32,
    /// Ticks that a colonist waits after a search that found no job. Also the wait
    /// before a job that failed is offered again.
    pub retry_ticks: u32,
    /// Work priority that a new colonist has for each job type (1 to 4).
    pub default_priority: u8,
}

/// Load and validate a job rules file.
pub fn load_job_rules(file_name: &str, text: &str) -> Result<JobRules, ContentError> {
    let fail = |entry: Option<&str>, message: String| ContentError {
        file: file_name.to_string(),
        issues: vec![ContentIssue {
            entry: entry.map(str::to_string),
            message,
        }],
    };
    if let Some((line, token)) = find_float_literals(text).into_iter().next() {
        return Err(fail(
            None,
            format!("line {line}: float literal '{token}' is not allowed; use integers"),
        ));
    }
    let rules: JobRules =
        ron::from_str(text).map_err(|e| fail(None, format!("parse error: {e}")))?;
    let mut issues = Vec::new();
    let mut check = |ok: bool, field: &str, message: &str| {
        if !ok {
            issues.push(ContentIssue {
                entry: Some(field.to_string()),
                message: message.to_string(),
            });
        }
    };
    check(rules.xp_per_level > 0, "xp_per_level", "must be 1 or more");
    check(rules.xp_per_job > 0, "xp_per_job", "must be 1 or more");
    check(
        (1..=100).contains(&rules.max_level),
        "max_level",
        "must be from 1 to 100",
    );
    check(
        rules.speed_base_pct > 0 && rules.speed_base_pct <= 1000,
        "speed_base_pct",
        "must be from 1 to 1000",
    );
    check(
        (0..=100).contains(&rules.speed_per_level_pct),
        "speed_per_level_pct",
        "must be from 0 to 100",
    );
    check(rules.cut_ticks > 0, "cut_ticks", "must be 1 or more");
    check(
        rules.mine_ticks_base > 0,
        "mine_ticks_base",
        "must be 1 or more",
    );
    check(
        (1..=1_000_000).contains(&rules.stack_max),
        "stack_max",
        "must be from 1 to 1000000",
    );
    for (field, value) in [
        ("log_yield", rules.log_yield),
        ("stone_yield", rules.stone_yield),
        ("ore_yield", rules.ore_yield),
    ] {
        check(
            value >= 1 && value <= rules.stack_max,
            field,
            "must be from 1 to stack_max",
        );
    }
    check(
        rules.search_budget > 0,
        "search_budget",
        "must be 1 or more",
    );
    check(rules.retry_ticks > 0, "retry_ticks", "must be 1 or more");
    check(
        (1..=4).contains(&rules.default_priority),
        "default_priority",
        "must be from 1 to 4",
    );
    if issues.is_empty() {
        Ok(rules)
    } else {
        Err(ContentError {
            file: file_name.to_string(),
            issues,
        })
    }
}

fn single_issue(file_name: &str, entry: Option<&str>, message: String) -> ContentError {
    ContentError {
        file: file_name.to_string(),
        issues: vec![ContentIssue {
            entry: entry.map(str::to_string),
            message,
        }],
    }
}

/// Refuse float literals, then parse RON text into a value. No file I/O.
fn parse_integer_ron<T: serde::de::DeserializeOwned>(
    file_name: &str,
    text: &str,
) -> Result<T, ContentError> {
    let floats = find_float_literals(text);
    if !floats.is_empty() {
        let issues = floats
            .into_iter()
            .map(|(line, token)| ContentIssue {
                entry: None,
                message: format!(
                    "line {line}: float literal '{token}' is not allowed; use integers"
                ),
            })
            .collect();
        return Err(ContentError {
            file: file_name.to_string(),
            issues,
        });
    }
    ron::from_str(text).map_err(|e| single_issue(file_name, None, format!("parse error: {e}")))
}

/// A geological setting with its pool of rocks (rocks.ron).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SettingDef {
    /// String ID from the file.
    pub key: String,
    /// Rock indices (into RockTable::rocks) that can appear in a province of this setting.
    pub pool: Vec<usize>,
}

/// The rock set and the setting pools after validation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RockTable {
    /// Rock IDs in file order. The position is the rock index.
    pub rocks: Vec<String>,
    /// Settings in file order. The position is the setting index.
    pub settings: Vec<SettingDef>,
}

/// A mineral with its affinity to each rock (deposits.ron).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MineralDef {
    /// String ID from the file.
    pub key: String,
    /// Affinity (0 to 100) for each rock, in the rock index order. 0 if the file omits the rock.
    pub affinity: Vec<u8>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawRock {
    id: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawSetting {
    id: String,
    pool: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawRockFile {
    rocks: Vec<RawRock>,
    settings: Vec<RawSetting>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawMineral {
    id: String,
    affinity: Vec<(String, u8)>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawMineralFile {
    minerals: Vec<RawMineral>,
}

fn finish<T>(file_name: &str, issues: Vec<ContentIssue>, ok: T) -> Result<T, ContentError> {
    if issues.is_empty() {
        Ok(ok)
    } else {
        Err(ContentError {
            file: file_name.to_string(),
            issues,
        })
    }
}

/// Load and validate a rocks file: the rock set and the rock pool of each setting.
pub fn load_rocks(file_name: &str, text: &str) -> Result<RockTable, ContentError> {
    let raw: RawRockFile = parse_integer_ron(file_name, text)?;
    let mut issues: Vec<ContentIssue> = Vec::new();
    let mut add = |entry: &str, message: String| {
        issues.push(ContentIssue {
            entry: Some(entry.to_string()),
            message,
        });
    };
    let mut index: BTreeMap<&str, usize> = BTreeMap::new();
    for (i, r) in raw.rocks.iter().enumerate() {
        if index.insert(r.id.as_str(), i).is_some() {
            add(&r.id, String::from("duplicate ID"));
        }
    }
    let mut seen: BTreeMap<&str, ()> = BTreeMap::new();
    let mut settings = Vec::with_capacity(raw.settings.len());
    for s in &raw.settings {
        if seen.insert(s.id.as_str(), ()).is_some() {
            add(&s.id, String::from("duplicate ID"));
        }
        let mut pool = Vec::with_capacity(s.pool.len());
        for target in &s.pool {
            let found = index.get(target.as_str()).copied();
            let Some(r) = found else {
                add(&s.id, format!("pool refers to unknown rock '{target}'"));
                continue;
            };
            if pool.contains(&r) {
                add(&s.id, format!("pool repeats rock {target}"));
            } else {
                pool.push(r);
            }
        }
        settings.push(SettingDef {
            key: s.id.clone(),
            pool,
        });
    }
    let rocks = raw.rocks.iter().map(|r| r.id.clone()).collect();
    finish(file_name, issues, RockTable { rocks, settings })
}

/// Load and validate a deposits file. Rock references must name rocks in the rock table.
pub fn load_minerals(
    file_name: &str,
    text: &str,
    rocks: &RockTable,
) -> Result<Vec<MineralDef>, ContentError> {
    let raw: RawMineralFile = parse_integer_ron(file_name, text)?;
    let mut issues: Vec<ContentIssue> = Vec::new();
    let mut add = |entry: &str, message: String| {
        issues.push(ContentIssue {
            entry: Some(entry.to_string()),
            message,
        });
    };
    let mut seen: BTreeMap<&str, ()> = BTreeMap::new();
    let mut out = Vec::with_capacity(raw.minerals.len());
    for m in &raw.minerals {
        if seen.insert(m.id.as_str(), ()).is_some() {
            add(&m.id, String::from("duplicate ID"));
        }
        let mut affinity = vec![0u8; rocks.rocks.len()];
        let mut given = vec![false; rocks.rocks.len()];
        for (target, value) in &m.affinity {
            let found = rocks.rocks.iter().position(|k| k == target);
            let Some(r) = found else {
                add(&m.id, format!("affinity names unknown rock {target}"));
                continue;
            };
            if given.get(r) == Some(&true) {
                add(&m.id, format!("affinity repeats rock {target}"));
            }
            if let Some(slot) = given.get_mut(r) {
                *slot = true;
            }
            if *value > 100 {
                add(
                    &m.id,
                    format!("affinity for {target} is {value}, above 100"),
                );
            }
            if let Some(slot) = affinity.get_mut(r) {
                *slot = *value;
            }
        }
        out.push(MineralDef {
            key: m.id.clone(),
            affinity,
        });
    }
    finish(file_name, issues, out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID_JOBS: &str = "(xp_per_job: 100, xp_per_level: 1000, max_level: 10, speed_base_pct: 100, speed_per_level_pct: 5, cut_ticks: 300, mine_ticks_base: 200, mine_ticks_per_10_hardness: 100, log_yield: 4, stone_yield: 2, ore_yield: 1, stack_max: 50, search_budget: 200, retry_ticks: 10, default_priority: 3)";

    #[test]
    fn job_rules_load_and_validate() {
        let rules = load_job_rules("jobs.ron", VALID_JOBS).unwrap_or_default();
        assert_eq!(rules.xp_per_level, 1000);
        let bad = VALID_JOBS.replace("max_level: 10", "max_level: 0");
        let msg = load_job_rules("jobs.ron", &bad)
            .err()
            .map(|e| e.to_string())
            .unwrap_or_default();
        assert!(
            msg.contains("jobs.ron") && msg.contains("max_level"),
            "{msg}"
        );
        let float = VALID_JOBS.replace("cut_ticks: 300", "cut_ticks: 3.5");
        assert!(load_job_rules("jobs.ron", &float).is_err());
        let unknown = VALID_JOBS.replace("(xp_per_job", "(extra: 1, xp_per_job");
        assert!(load_job_rules("jobs.ron", &unknown).is_err());
    }

    const VALID: &str = r#"#![enable(implicit_some)]
[
    (id: "water", name: "Water", category: Fluid, hardness: 0, density: 100, value: 0, insulation: 0, flammability: 0, beauty: 5, color: (50, 100, 190)),
    (id: "ice", name: "Ice 1.5", category: Fluid, hardness: 10, density: 92, value: 0, insulation: 100, flammability: 0, beauty: 6, color: (190, 225, 240), melts_at: 0, melts_into: "water"),
]"#;

    fn error_text(text: &str) -> String {
        load_materials("m.ron", text)
            .err()
            .map(|e| e.to_string())
            .unwrap_or_default()
    }

    #[test]
    fn valid_file_loads_and_links() {
        let list = load_materials("m.ron", VALID).unwrap_or_default();
        assert_eq!(list.len(), 2);
        assert_eq!(list[1].melts_into, Some(MaterialId(0)));
        assert_eq!(list[1].name, "Ice 1.5"); // digits inside strings are ignored
    }

    #[test]
    fn missing_reference_names_file_and_entry() {
        let msg = error_text(&VALID.replace("melts_into: \"water\"", "melts_into: \"lava\""));
        assert!(msg.contains("m.ron"), "{msg}");
        assert!(msg.contains("'ice'"), "{msg}");
        assert!(msg.contains("lava"), "{msg}");
    }

    #[test]
    fn duplicate_id_is_refused() {
        let msg = error_text(&VALID.replace("id: \"ice\"", "id: \"water\""));
        assert!(
            msg.contains("duplicate ID") && msg.contains("'water'"),
            "{msg}"
        );
    }

    #[test]
    fn float_literal_is_refused() {
        let msg = error_text(&VALID.replace("density: 100", "density: 1.5"));
        assert!(
            msg.contains("m.ron") && msg.contains("float literal"),
            "{msg}"
        );
        assert_eq!(
            find_float_literals("a: 1e3, b: 0x1F, c: \"2.5\", // 3.5\n d: 7").len(),
            1
        );
    }

    #[test]
    fn out_of_range_per_mille_is_refused() {
        let msg = error_text(&VALID.replace("insulation: 100", "insulation: 1001"));
        assert!(msg.contains("'ice'") && msg.contains("insulation"), "{msg}");
    }

    const ROCKS: &str =
        r#"(rocks: [(id: "a"), (id: "b")], settings: [(id: "plain", pool: ["a", "b"])])"#;
    const MINERALS: &str = r#"(minerals: [(id: "coal", affinity: [("a", 30)])])"#;

    fn empty_rocks() -> RockTable {
        RockTable {
            rocks: vec![],
            settings: vec![],
        }
    }

    fn rock_error(text: &str) -> String {
        let r = load_rocks("rocks.ron", text);
        r.err().map(|e| e.to_string()).unwrap_or_default()
    }

    fn mineral_error(text: &str) -> String {
        let rocks = load_rocks("rocks.ron", ROCKS).unwrap_or_else(|_| empty_rocks());
        let r = load_minerals("deposits.ron", text, &rocks);
        r.err().map(|e| e.to_string()).unwrap_or_default()
    }

    #[test]
    fn rock_and_mineral_tables_load() {
        let rocks = load_rocks("rocks.ron", ROCKS).unwrap_or_else(|_| empty_rocks());
        assert_eq!(rocks.rocks.len(), 2);
        let first = rocks.settings.first().map(|s| s.pool.clone());
        assert_eq!(first, Some(vec![0, 1]));
        let m = load_minerals("deposits.ron", MINERALS, &rocks).unwrap_or_default();
        assert_eq!(m.first().map(|d| d.affinity.clone()), Some(vec![30, 0]));
    }

    const ROCKS_MISSING: &str =
        r#"(rocks: [(id: "a")], settings: [(id: "plain", pool: ["a", "zinc"])])"#;
    const ROCKS_DUP: &str = r#"(rocks: [(id: "a"), (id: "a")], settings: [])"#;
    const ROCKS_FLOAT: &str = r#"(rocks: [(id: "a")], settings: [], x: 1.5)"#;

    #[test]
    fn rock_table_missing_reference_names_file_and_entry() {
        let msg = rock_error(ROCKS_MISSING);
        assert!(msg.contains("rocks.ron"), "{msg}");
        assert!(msg.contains("'plain'"), "{msg}");
        assert!(msg.contains("zinc"), "{msg}");
    }

    #[test]
    fn rock_table_duplicate_id_and_float_are_refused() {
        assert!(rock_error(ROCKS_DUP).contains("duplicate ID"));
        assert!(rock_error(ROCKS_FLOAT).contains("float literal"));
    }

    const MIN_MISSING: &str = r#"(minerals: [(id: "coal", affinity: [("zinc", 30)])])"#;
    const MIN_DUP: &str = r#"(minerals: [(id: "coal", affinity: []), (id: "coal", affinity: [])])"#;
    const MIN_FLOAT: &str = r#"(minerals: [(id: "coal", affinity: [("a", 30.5)])])"#;
    const MIN_RANGE: &str = r#"(minerals: [(id: "coal", affinity: [("a", 101)])])"#;

    #[test]
    fn mineral_table_missing_reference_names_file_and_entry() {
        let msg = mineral_error(MIN_MISSING);
        assert!(msg.contains("deposits.ron"), "{msg}");
        assert!(msg.contains("'coal'"), "{msg}");
        assert!(msg.contains("zinc"), "{msg}");
    }

    #[test]
    fn mineral_table_duplicate_float_and_range_are_refused() {
        assert!(mineral_error(MIN_DUP).contains("duplicate ID"));
        assert!(mineral_error(MIN_FLOAT).contains("float literal"));
        assert!(mineral_error(MIN_RANGE).contains("above 100"));
    }
}
