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

/// Parse RON text into a raw value. Refuse float literals. Return a clear error with the
/// file name.
fn parse_ron<T: serde::de::DeserializeOwned>(
    file_name: &str,
    text: &str,
) -> Result<T, ContentError> {
    let fail = |message: String| ContentError {
        file: file_name.to_string(),
        issues: vec![ContentIssue {
            entry: None,
            message,
        }],
    };
    let floats = find_float_literals(text);
    if !floats.is_empty() {
        return Err(ContentError {
            file: file_name.to_string(),
            issues: floats
                .into_iter()
                .map(|(line, token)| ContentIssue {
                    entry: None,
                    message: format!(
                        "line {line}: float literal '{token}' is not allowed; use integers"
                    ),
                })
                .collect(),
        });
    }
    ron::from_str(text).map_err(|e| fail(format!("parse error: {e}")))
}

/// Collect the issues of a file and turn them into a result.
fn finish<T>(file_name: &str, value: T, issues: Vec<ContentIssue>) -> Result<T, ContentError> {
    if issues.is_empty() {
        Ok(value)
    } else {
        Err(ContentError {
            file: file_name.to_string(),
            issues,
        })
    }
}

fn issue(entry: &str, message: impl Into<String>) -> ContentIssue {
    ContentIssue {
        entry: Some(entry.to_string()),
        message: message.into(),
    }
}

/// What a building does (`buildings.ron`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
pub enum BuildingKind {
    /// Closes a room. Blocks walking.
    Wall,
    /// Closes a room. Colonists walk through it.
    Door,
    /// A floor covering.
    Floor,
    /// A roof.
    Roof,
    /// A bed.
    Bed,
    /// A hearth.
    Hearth,
    /// A workshop where colonists make items with recipes.
    Workshop,
}

impl BuildingKind {
    /// All kinds, in declaration order.
    pub const ALL: [BuildingKind; 7] = [
        BuildingKind::Wall,
        BuildingKind::Door,
        BuildingKind::Floor,
        BuildingKind::Roof,
        BuildingKind::Bed,
        BuildingKind::Hearth,
        BuildingKind::Workshop,
    ];
}

/// A building type after validation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BuildingDef {
    /// String ID from the file.
    pub key: String,
    /// Display name.
    pub name: String,
    /// What the building does.
    pub kind: BuildingKind,
    /// Material categories that the building can be made from.
    pub categories: Vec<MaterialCategory>,
    /// Units of material that one building needs.
    pub cost: u32,
    /// Work time to build, in ticks at 100% work speed, before the material hardness.
    pub work_ticks: u32,
    /// Work time that each 10 hardness units of the material add, in ticks.
    pub work_ticks_per_10_hardness: u32,
    /// Work time to take the building apart, in percent of the build time.
    pub deconstruct_pct: u32,
    /// Percent of the material units that deconstruction gives back (rounded down).
    pub return_pct: u32,
    /// True if colonists cannot walk through the building.
    pub blocks_walking: bool,
    /// Beauty of the building itself, added to the beauty of its material.
    pub beauty: i16,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawBuilding {
    id: String,
    name: String,
    kind: BuildingKind,
    categories: Vec<MaterialCategory>,
    cost: u32,
    work_ticks: u32,
    work_ticks_per_10_hardness: u32,
    deconstruct_pct: u32,
    return_pct: u32,
    blocks_walking: bool,
    beauty: i16,
}

/// Load and validate a buildings file. Buildings are in file order; the position is the
/// building index.
pub fn load_buildings(file_name: &str, text: &str) -> Result<Vec<BuildingDef>, ContentError> {
    let raw: Vec<RawBuilding> = parse_ron(file_name, text)?;
    let mut issues = Vec::new();
    let mut seen: Vec<&str> = Vec::new();
    let mut out = Vec::with_capacity(raw.len());
    if raw.len() >= usize::from(u16::MAX) {
        issues.push(issue("(file)", "too many buildings"));
    }
    for (i, b) in raw.iter().enumerate() {
        let name = if b.id.is_empty() {
            issues.push(issue(&format!("#{i}"), "empty ID"));
            format!("#{i}")
        } else {
            b.id.clone()
        };
        if seen.contains(&b.id.as_str()) {
            issues.push(issue(&name, "duplicate ID"));
        }
        seen.push(b.id.as_str());
        if b.categories.is_empty() {
            issues.push(issue(&name, "categories must name at least one category"));
        }
        for (k, c) in b.categories.iter().enumerate() {
            if b.categories[..k].contains(c) {
                issues.push(issue(&name, format!("category {c:?} is listed twice")));
            }
        }
        if !(1..=1000).contains(&b.cost) {
            issues.push(issue(&name, "cost must be from 1 to 1000"));
        }
        if b.work_ticks == 0 || b.work_ticks > 1_000_000 {
            issues.push(issue(&name, "work_ticks must be from 1 to 1000000"));
        }
        if b.work_ticks_per_10_hardness > 100_000 {
            issues.push(issue(&name, "work_ticks_per_10_hardness is too large"));
        }
        if b.deconstruct_pct == 0 || b.deconstruct_pct > 1000 {
            issues.push(issue(&name, "deconstruct_pct must be from 1 to 1000"));
        }
        if b.return_pct > 100 {
            issues.push(issue(&name, "return_pct must be from 0 to 100"));
        }
        match b.kind {
            BuildingKind::Wall if !b.blocks_walking => {
                issues.push(issue(&name, "a wall must have blocks_walking: true"));
            }
            BuildingKind::Door | BuildingKind::Floor | BuildingKind::Roof if b.blocks_walking => {
                issues.push(issue(
                    &name,
                    "a door, floor, or roof must have blocks_walking: false",
                ));
            }
            _ => {}
        }
        out.push(BuildingDef {
            key: b.id.clone(),
            name: b.name.clone(),
            kind: b.kind,
            categories: b.categories.clone(),
            cost: b.cost,
            work_ticks: b.work_ticks,
            work_ticks_per_10_hardness: b.work_ticks_per_10_hardness,
            deconstruct_pct: b.deconstruct_pct,
            return_pct: b.return_pct,
            blocks_walking: b.blocks_walking,
            beauty: b.beauty,
        });
    }
    for kind in BuildingKind::ALL {
        if !out.iter().any(|b| b.kind == kind) {
            issues.push(issue("(file)", format!("no building of kind {kind:?}")));
        }
    }
    finish(file_name, out, issues)
}

/// A recipe after validation and linking.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecipeDef {
    /// String ID from the file.
    pub key: String,
    /// Display name.
    pub name: String,
    /// Building index (`buildings.ron` order) of the workshop that makes it.
    pub workshop: u16,
    /// Input material indices (`materials.ron` order) and unit counts.
    pub inputs: Vec<(u16, u32)>,
    /// Output material indices and unit counts.
    pub outputs: Vec<(u16, u32)>,
    /// Work time of one run, in ticks at 100% work speed.
    pub work_ticks: u32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawRecipe {
    id: String,
    name: String,
    workshop: String,
    inputs: Vec<(String, u32)>,
    outputs: Vec<(String, u32)>,
    work_ticks: u32,
}

/// Load and validate a recipes file. The materials and buildings must be loaded first.
pub fn load_recipes(
    file_name: &str,
    text: &str,
    materials: &[MaterialDef],
    buildings: &[BuildingDef],
) -> Result<Vec<RecipeDef>, ContentError> {
    let raw: Vec<RawRecipe> = parse_ron(file_name, text)?;
    let mut issues = Vec::new();
    let mut out = Vec::with_capacity(raw.len());
    let mut seen: Vec<&str> = Vec::new();
    if raw.len() >= usize::from(u16::MAX) {
        issues.push(issue("(file)", "too many recipes"));
    }
    for (i, r) in raw.iter().enumerate() {
        let name = if r.id.is_empty() {
            issues.push(issue(&format!("#{i}"), "empty ID"));
            format!("#{i}")
        } else {
            r.id.clone()
        };
        if seen.contains(&r.id.as_str()) {
            issues.push(issue(&name, "duplicate ID"));
        }
        seen.push(r.id.as_str());
        let workshop = match buildings.iter().position(|b| b.key == r.workshop) {
            Some(w) if buildings[w].kind == BuildingKind::Workshop => w as u16,
            Some(_) => {
                issues.push(issue(
                    &name,
                    format!(
                        "workshop '{}' is not a building of kind Workshop",
                        r.workshop
                    ),
                ));
                u16::MAX
            }
            None => {
                issues.push(issue(
                    &name,
                    format!("workshop refers to unknown building '{}'", r.workshop),
                ));
                u16::MAX
            }
        };
        let mut link = |field: &str, list: &[(String, u32)]| -> Vec<(u16, u32)> {
            if list.is_empty() {
                issues.push(issue(&name, format!("{field} must not be empty")));
            }
            list.iter()
                .map(|(key, count)| {
                    if !(1..=1000).contains(count) {
                        issues.push(issue(
                            &name,
                            format!("{field} count for '{key}' must be from 1 to 1000"),
                        ));
                    }
                    match materials.iter().position(|m| &m.key == key) {
                        Some(m) => m as u16,
                        None => {
                            issues.push(issue(
                                &name,
                                format!("{field} refers to unknown material '{key}'"),
                            ));
                            u16::MAX
                        }
                    }
                })
                .zip(list.iter().map(|(_, c)| *c))
                .collect()
        };
        let inputs = link("inputs", &r.inputs);
        let outputs = link("outputs", &r.outputs);
        if r.work_ticks == 0 || r.work_ticks > 1_000_000 {
            issues.push(issue(&name, "work_ticks must be from 1 to 1000000"));
        }
        out.push(RecipeDef {
            key: r.id.clone(),
            name: r.name.clone(),
            workshop,
            inputs,
            outputs,
            work_ticks: r.work_ticks,
        });
    }
    finish(file_name, out, issues)
}

/// Numbers for rooms (`rooms.ron`). All integers.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RoomRules {
    /// Largest number of tiles in a room (R-020).
    pub max_tiles: u32,
    /// A roof tile needs a wall or natural rock within this many tiles (A-008). The
    /// distance is the larger of the column and row distance.
    pub roof_support_radius: u32,
    /// Material (key) of a wall tile that has no building, for example a wall of a
    /// scenario map. It gives the insulation and the beauty of that tile.
    pub bare_wall_material: String,
}

/// Load and validate a rooms file.
pub fn load_room_rules(file_name: &str, text: &str) -> Result<RoomRules, ContentError> {
    let rules: RoomRules = parse_ron(file_name, text)?;
    let mut issues = Vec::new();
    if !(1..=65_536).contains(&rules.max_tiles) {
        issues.push(issue("max_tiles", "must be from 1 to 65536"));
    }
    if !(1..=64).contains(&rules.roof_support_radius) {
        issues.push(issue("roof_support_radius", "must be from 1 to 64"));
    }
    if rules.bare_wall_material.is_empty() {
        issues.push(issue("bare_wall_material", "must name a material"));
    }
    finish(file_name, rules, issues)
}

/// Numbers for Notice, beasts, and dormant threats (`notice.ron`, R-030 and R-031). All integers.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NoticeRules {
    /// Wealth units that give 1 Notice.
    pub wealth_div: u32,
    /// Notice of each colonist.
    pub per_colonist: u32,
    /// Notice of each completed mining or construction job.
    pub noise_per_job: u32,
    /// Share of the noise that goes away each game day, in percent. The loss rounds up.
    pub noise_decay_pct: u32,
    /// Notice of the excavation of a site.
    pub excavation: u32,
    /// Notice of each relic that a colonist takes from a site.
    pub relic: u32,
    /// Notice of opening a sealed site.
    pub sealed_open: u32,
    /// Site disturbance that goes away each game day.
    pub disturbance_decay: u32,
    /// Divisor of the daily beast chance: the chance in percent is Notice / `beast_div`.
    pub beast_div: u32,
    /// Largest daily beast chance, in percent.
    pub beast_max_pct: u32,
    /// Notice at which the beasts in lairs become active.
    pub lair_active: u32,
    /// Notice that adds 1 strength level to an awake dormant threat.
    pub threat_per_level: u32,
}

/// Load and validate the Notice numbers.
pub fn load_notice_rules(file_name: &str, text: &str) -> Result<NoticeRules, ContentError> {
    let r: NoticeRules = parse_ron(file_name, text)?;
    let mut issues = Vec::new();
    for (value, name) in [
        (r.wealth_div, "wealth_div"),
        (r.beast_div, "beast_div"),
        (r.threat_per_level, "threat_per_level"),
    ] {
        if value == 0 {
            issues.push(issue(name, "must be 1 or more"));
        }
    }
    if r.noise_decay_pct > 100 {
        issues.push(issue("noise_decay_pct", "must be 100 or less"));
    }
    if r.beast_max_pct > 100 {
        issues.push(issue("beast_max_pct", "must be 100 or less"));
    }
    finish(file_name, r, issues)
}

/// Numbers for drought, irrigation, wildfire, and fire spread (`drought.ron`, R-024 to R-026).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DroughtRules {
    /// Days in a row without rain that start a drought.
    pub drought_days: u32,
    /// Growth speed of a crop on a tile that is not irrigated during a drought, in percent.
    pub dry_growth_pct: u32,
    /// A tile this near a water tile or a connected ditch tile is irrigated.
    pub irrigation_radius: u32,
    /// Fire spread chance during a drought, in percent of the normal chance.
    pub fire_drought_pct: u32,
    /// Chance of lightning on a summer day with a drought, in percent.
    pub lightning_pct: u32,
    /// Intensity of a fire that lightning starts (1 to 10).
    pub lightning_intensity: u32,
    /// Ticks between two fire steps.
    pub fire_step_ticks: u32,
    /// Flammability of a tree tile: the spread chance in percent is this times the intensity
    /// divided by 10.
    pub tree_flammability: u32,
    /// Fire steps that a burning tree lasts.
    pub tree_fuel: u32,
    /// Work time to dig one ditch tile, in ticks at 100% work speed.
    pub dig_ticks: u32,
    /// Chance in percent each game hour that one flammable item or object within 1 tile of a
    /// burning hearth starts to burn (R-023).
    pub hearth_fire_pct: u32,
    /// No hearth fire starts in this many first game days of a run.
    pub hearth_fire_grace_days: u32,
    /// Intensity of a fire that a hearth starts (1 to 10).
    pub hearth_fire_intensity: u32,
    /// Fire steps that a burning item or building lasts.
    pub thing_fuel: u32,
    /// Work time of one extinguish job, in ticks at 100% work speed (one game hour).
    pub extinguish_ticks: u32,
    /// Intensity that one extinguish job removes (R-024).
    pub extinguish_drop: u32,
    /// Colonists put out a fire only if an item or a building is within this many tiles.
    pub extinguish_radius: u32,
    /// Health that a colonist on a burning tile loses each game hour.
    pub fire_health_loss: u32,
}

/// Load and validate the drought numbers.
pub fn load_drought_rules(file_name: &str, text: &str) -> Result<DroughtRules, ContentError> {
    let r: DroughtRules = parse_ron(file_name, text)?;
    let mut issues = Vec::new();
    for (value, name) in [
        (r.drought_days, "drought_days"),
        (r.fire_step_ticks, "fire_step_ticks"),
        (r.tree_fuel, "tree_fuel"),
        (r.dig_ticks, "dig_ticks"),
        (r.thing_fuel, "thing_fuel"),
        (r.extinguish_ticks, "extinguish_ticks"),
        (r.extinguish_drop, "extinguish_drop"),
    ] {
        if value == 0 {
            issues.push(issue(name, "must be 1 or more"));
        }
    }
    for (value, name) in [
        (r.dry_growth_pct, "dry_growth_pct"),
        (r.lightning_pct, "lightning_pct"),
        (r.tree_flammability, "tree_flammability"),
        (r.hearth_fire_pct, "hearth_fire_pct"),
    ] {
        if value > 100 {
            issues.push(issue(name, "must be 100 or less"));
        }
    }
    if !(100..=1000).contains(&r.fire_drought_pct) {
        issues.push(issue("fire_drought_pct", "must be from 100 to 1000"));
    }
    if !(1..=10).contains(&r.lightning_intensity) {
        issues.push(issue("lightning_intensity", "must be from 1 to 10"));
    }
    if !(1..=10).contains(&r.hearth_fire_intensity) {
        issues.push(issue("hearth_fire_intensity", "must be from 1 to 10"));
    }
    if r.extinguish_radius > 32 {
        issues.push(issue("extinguish_radius", "must be 32 or less"));
    }
    if r.irrigation_radius > 32 {
        issues.push(issue("irrigation_radius", "must be 32 or less"));
    }
    finish(file_name, r, issues)
}
/// Numbers for rival colonies and raids (`raid.ron`, R-070 to R-072).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RaidRules {
    /// A raid can start on a day after this one.
    pub quiet_days: u32,
    /// The daily raid chance in percent is hostility divided by this.
    pub chance_div: u32,
    /// Largest daily raid chance, in percent.
    pub max_pct: u32,
    /// Days that a rival colony waits after a raid.
    pub cooldown_days: u32,
    /// Fewest raiders.
    pub min_fighters: u32,
    /// Ticks between the scout and the raiders.
    pub scout_lead_ticks: u32,
    /// Ticks that the player has to answer when the raiders arrive.
    pub decision_ticks: u32,
    /// Value of the tribute for each raider.
    pub tribute_per_raider: u32,
    /// Hostility that a tribute removes.
    pub pay_hostility: u32,
    /// Hostility that a won fight removes.
    pub win_hostility: u32,
    /// Hostility that a raid adds when the raiders carry away items.
    pub loot_hostility: u32,
    /// Wealth units for 1 hostility.
    pub wealth_div: u32,
    /// Notice is divided by this for the hostility.
    pub notice_div: u32,
    /// Most stacks that one raider carries away.
    pub loot_per_raider: u32,
    /// Ticks that the raiders stay in a fight.
    pub stay_ticks: u32,
    /// Ticks to break a door or a wall, for each unit of hardness.
    pub break_ticks_per_hardness: u32,
}

/// Load and validate the raid numbers.
pub fn load_raid_rules(file_name: &str, text: &str) -> Result<RaidRules, ContentError> {
    let r: RaidRules = parse_ron(file_name, text)?;
    let mut issues = Vec::new();
    for (value, name) in [
        (r.chance_div, "chance_div"),
        (r.min_fighters, "min_fighters"),
        (r.scout_lead_ticks, "scout_lead_ticks"),
        (r.decision_ticks, "decision_ticks"),
        (r.wealth_div, "wealth_div"),
        (r.notice_div, "notice_div"),
        (r.stay_ticks, "stay_ticks"),
    ] {
        if value == 0 {
            issues.push(issue(name, "must be 1 or more"));
        }
    }
    if r.max_pct > 100 {
        issues.push(issue("max_pct", "must be 100 or less"));
    }
    for (value, name) in [
        (r.pay_hostility, "pay_hostility"),
        (r.win_hostility, "win_hostility"),
        (r.loot_hostility, "loot_hostility"),
    ] {
        if value > 100 {
            issues.push(issue(name, "must be 100 or less"));
        }
    }
    finish(file_name, r, issues)
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

/// Load and validate a rocks file: the rock set and the rock pool of each setting.
pub fn load_rocks(file_name: &str, text: &str) -> Result<RockTable, ContentError> {
    let raw: RawRockFile = parse_ron(file_name, text)?;
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
    finish(file_name, RockTable { rocks, settings }, issues)
}

/// Load and validate a deposits file. Rock references must name rocks in the rock table.
pub fn load_minerals(
    file_name: &str,
    text: &str,
    rocks: &RockTable,
) -> Result<Vec<MineralDef>, ContentError> {
    let raw: RawMineralFile = parse_ron(file_name, text)?;
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
    finish(file_name, out, issues)
}

/// Numbers of the history simulation (`history.ron`). Chances are per-mille per year.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HistoryRules {
    /// Default history length in years.
    pub years_default: u32,
    /// Shortest history in years.
    pub years_min: u32,
    /// Longest history in years.
    pub years_max: u32,
    /// Regions with fewer cells are ignored for starts, mines, and roads.
    pub min_region_cells: u32,
    /// Civilizations at year 0.
    pub start_civs: u32,
    /// Minimum alive civilizations (homeostasis).
    pub min_alive: u32,
    /// Maximum alive civilizations (homeostasis).
    pub max_alive: u32,
    /// Migrants appear by chance only below this many alive civilizations.
    pub spawn_below_alive: u32,
    /// Chance of a migrant civilization.
    pub new_civ_pm: u32,
    /// Limit of civilizations.
    pub max_civs: u32,
    /// Limit of settlements.
    pub max_settlements: u32,
    /// Limit of artifacts.
    pub max_artifacts: u32,
    /// Limit of sites.
    pub max_sites: u32,
    /// Limit of events.
    pub max_events: u32,
    /// Years between heavy updates.
    pub heavy_interval: u32,
    /// Most settlements of one civilization at the same time.
    pub max_civ_settlements: u32,
    /// Chance of a new settlement.
    pub settle_pm: u32,
    /// Chance of a new mine.
    pub mine_pm: u32,
    /// Chance of a new road.
    pub road_pm: u32,
    /// Chance of a battle.
    pub battle_pm: u32,
    /// Chance of a seal.
    pub seal_pm: u32,
    /// Chance of an artifact after a battle, a raze, or a seal.
    pub artifact_pm: u32,
    /// Base chance of a collapse.
    pub collapse_pm: u32,
    /// Extra collapse chance at zero prosperity.
    pub collapse_poor_pm: u32,
    /// Extra collapse chance above the maximum alive civilizations.
    pub collapse_crowd_pm: u32,
    /// Base chance that an attacker wins.
    pub attack_win_pm: u32,
    /// Base mine life in years.
    pub mine_life_base: u32,
    /// Extra mine life for each richness step.
    pub mine_life_per_richness: u32,
    /// Range of the random mine life term.
    pub mine_life_jitter: u32,
    /// Radius of the depletion overlay.
    pub depletion_radius: u32,
    /// Radius of the battlefield sanctity.
    pub battle_radius: u32,
    /// Radius of the seal sanctity.
    pub seal_radius: u32,
    /// Margin of the road search box.
    pub road_margin: u32,
}

/// Load and validate the history numbers.
pub fn load_history_rules(file_name: &str, text: &str) -> Result<HistoryRules, ContentError> {
    let r: HistoryRules = parse_ron(file_name, text)?;
    let mut issues = Vec::new();
    let mut check = |ok: bool, field: &str, message: &str| {
        if !ok {
            issues.push(issue(field, message));
        }
    };
    check(
        1 <= r.years_min && r.years_min <= r.years_default && r.years_default <= r.years_max,
        "years_default",
        "must satisfy 1 <= years_min <= years_default <= years_max",
    );
    check(r.years_max <= 60_000, "years_max", "must be 60000 or less");
    check(
        r.min_region_cells >= 1,
        "min_region_cells",
        "must be 1 or more",
    );
    check(
        1 <= r.min_alive && r.min_alive <= r.start_civs && r.start_civs <= r.max_alive,
        "start_civs",
        "must satisfy 1 <= min_alive <= start_civs <= max_alive",
    );
    check(
        r.min_alive < r.spawn_below_alive && r.spawn_below_alive <= r.max_alive,
        "spawn_below_alive",
        "must satisfy min_alive < spawn_below_alive <= max_alive",
    );
    check(
        r.max_alive <= r.max_civs && r.max_civs <= 60_000,
        "max_civs",
        "must satisfy max_alive <= max_civs <= 60000",
    );
    check(
        (1..=60_000).contains(&r.max_settlements),
        "max_settlements",
        "must be from 1 to 60000",
    );
    check(
        r.max_artifacts <= 60_000,
        "max_artifacts",
        "must be 60000 or less",
    );
    check(
        (1..=60_000).contains(&r.max_sites),
        "max_sites",
        "must be from 1 to 60000",
    );
    check(
        (1..=1_000_000).contains(&r.max_events),
        "max_events",
        "must be from 1 to 1000000",
    );
    check(r.heavy_interval >= 1, "heavy_interval", "must be 1 or more");
    check(
        r.max_civ_settlements >= 1,
        "max_civ_settlements",
        "must be 1 or more",
    );
    for (value, name) in [
        (r.new_civ_pm, "new_civ_pm"),
        (r.settle_pm, "settle_pm"),
        (r.mine_pm, "mine_pm"),
        (r.road_pm, "road_pm"),
        (r.battle_pm, "battle_pm"),
        (r.seal_pm, "seal_pm"),
        (r.artifact_pm, "artifact_pm"),
        (r.collapse_pm, "collapse_pm"),
        (r.collapse_poor_pm, "collapse_poor_pm"),
        (r.collapse_crowd_pm, "collapse_crowd_pm"),
        (r.attack_win_pm, "attack_win_pm"),
    ] {
        check(value <= 1000, name, "must be from 0 to 1000");
    }
    check(r.mine_life_base >= 1, "mine_life_base", "must be 1 or more");
    for (value, name) in [
        (r.depletion_radius, "depletion_radius"),
        (r.battle_radius, "battle_radius"),
        (r.seal_radius, "seal_radius"),
        (r.road_margin, "road_margin"),
    ] {
        check(value <= 32, name, "must be 32 or less");
    }
    finish(file_name, r, issues)
}

/// A culture (`cultures.ron`). Drives are 0 to 100.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CultureDef {
    /// String ID from the file.
    pub id: String,
    /// Display name.
    pub name: String,
    /// Base expansion drive.
    pub expansion: u8,
    /// Base aggression drive.
    pub aggression: u8,
    /// Base prosperity.
    pub prosperity: u8,
    /// Preference for mines.
    pub mining: u8,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawCultureFile {
    cultures: Vec<CultureDef>,
}

/// Load and validate the cultures. The list holds 1 to 255 cultures.
pub fn load_cultures(file_name: &str, text: &str) -> Result<Vec<CultureDef>, ContentError> {
    let raw: RawCultureFile = parse_ron(file_name, text)?;
    let mut issues = Vec::new();
    if raw.cultures.is_empty() || raw.cultures.len() > 255 {
        issues.push(issue("cultures", "must hold 1 to 255 cultures"));
    }
    let mut seen: BTreeMap<&str, ()> = BTreeMap::new();
    for c in &raw.cultures {
        if seen.insert(c.id.as_str(), ()).is_some() {
            issues.push(issue(&c.id, "duplicate ID"));
        }
        if c.name.is_empty() {
            issues.push(issue(&c.id, "name must not be empty"));
        }
        let drives = [
            (c.expansion, "expansion"),
            (c.aggression, "aggression"),
            (c.prosperity, "prosperity"),
            (c.mining, "mining"),
        ];
        for (v, n) in drives {
            if v > 100 {
                issues.push(issue(&c.id, format!("{n} must be from 0 to 100")));
            }
        }
    }
    finish(file_name, raw.cultures, issues)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawTemplate {
    kind: String,
    text: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawTemplateFile {
    templates: Vec<RawTemplate>,
}

/// Names that a history event template can use in braces.
pub const TEMPLATE_NAMES: [&str; 7] = ["year", "civ", "culture", "civ2", "site", "region", "extra"];

/// Load the event templates. `kinds` lists the event type names in order. Each type needs
/// exactly one template, in the same order. Returns the texts in that order.
pub fn load_event_templates(
    file_name: &str,
    text: &str,
    kinds: &[&str],
) -> Result<Vec<String>, ContentError> {
    let raw: RawTemplateFile = parse_ron(file_name, text)?;
    let mut issues = Vec::new();
    let found = raw.templates.iter().map(|t| t.kind.as_str());
    if !found.eq(kinds.iter().copied()) {
        issues.push(issue(
            "templates",
            "must hold one template for each event type, in the order of the EventKind type",
        ));
    }
    for t in &raw.templates {
        let mut rest = t.text.as_str();
        while let Some((_, after)) = rest.split_once('{') {
            match after.split_once('}') {
                Some((name, tail)) if TEMPLATE_NAMES.contains(&name) => rest = tail,
                _ => {
                    issues.push(issue(&t.kind, "text has a bad or unknown name in braces"));
                    break;
                }
            }
        }
    }
    let texts = raw.templates.into_iter().map(|t| t.text).collect();
    finish(file_name, texts, issues)
}

/// One row of the embark biome table (`embark.ron`).
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EmbarkBiomeRow {
    /// Name of the world biome. The row order follows the world biome type.
    pub biome: String,
    /// Local map biome: `TemperateForest`, `ColdHills`, or `DryPlain`.
    pub local: String,
    /// Share of tiles with trees, in per-mille.
    pub tree_density: u32,
    /// Share of water tiles, in per-mille.
    pub water: u32,
    /// Share of rock tiles, in per-mille.
    pub mountain: u32,
    /// Share of sand, in per-mille.
    pub sand: u32,
    /// Share of clay, in per-mille.
    pub clay: u32,
}

/// Numbers of the embark summary and the site stamping (`embark.ron`).
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EmbarkRules {
    /// Biome rows.
    pub biomes: Vec<EmbarkBiomeRow>,
    /// Height above sea level where extra mountain starts.
    pub mountain_h0: u32,
    /// Extra mountain per-mille for each height unit above `mountain_h0`.
    pub mountain_per_height: u32,
    /// Cap of the mountain share.
    pub mountain_cap: u32,
    /// Extra water per-mille for each river size.
    pub river_water: u32,
    /// Extra water per-mille on a lake cell.
    pub lake_water: u32,
    /// Extra water per-mille on a coast cell.
    pub coast_water: u32,
    /// Distance between a site center and the map edge.
    pub site_margin: u32,
    /// Most sites placed on one local map.
    pub max_map_sites: u32,
    /// Smallest building width.
    pub ruin_w_min: u32,
    /// Largest building width.
    pub ruin_w_max: u32,
    /// Smallest building height.
    pub ruin_h_min: u32,
    /// Largest building height.
    pub ruin_h_max: u32,
    /// Decay constant of walls.
    pub decay_k: u32,
    /// Floor tiles decay this many times slower than walls.
    pub floor_rubble_div: u32,
    /// Age where trees grow in ruins.
    pub overgrow_age: u32,
    /// Chance of a tree on an open ruin tile, in per-mille.
    pub overgrow_pm: u32,
    /// Salvage spots of a new ruin.
    pub salvage_base: u32,
    /// Years that remove one salvage spot.
    pub salvage_age_div: u32,
    /// Age of the small ruin that R-041 adds.
    pub fallback_age: u32,
    /// Length of an old tunnel.
    pub tunnel_len: u32,
    /// Radius of battlefield debris.
    pub debris_radius: u32,
    /// Debris tiles of a new battlefield.
    pub debris_count: u32,
    /// Decay constant of battlefield debris.
    pub debris_decay_k: u32,
    /// Radius of the ring around a sealed site.
    pub seal_radius: u32,
    /// Largest sideways road shift.
    pub road_jitter: u32,
    /// Share of the legends that are "certain" (R-044), in percent.
    pub legend_certain_pct: u32,
    /// Share of the "rumor" legends that point to a real site (R-044), in percent.
    pub legend_rumor_true_pct: u32,
    /// A false rumor points to a tile at least this far from every site, in tiles.
    pub legend_min_gap: u32,
    /// A rival colony is a living settlement within this many world cells (R-070).
    pub rival_radius: u32,
    /// Most rival colonies of one embark.
    pub max_rivals: u32,
    /// Fewest fighters of a rival colony.
    pub rival_strength_min: u32,
    /// Most fighters of a rival colony.
    pub rival_strength_max: u32,
    /// The aggression of the civilization is divided by this for the base hostility.
    pub rival_aggression_div: u32,
    /// Base hostility for each battle of the rival civilization in the embark area.
    pub rival_battle_hostility: u32,
    /// Base hostility for each settlement razed in the embark area in which the rival civilization took part.
    pub rival_raze_hostility: u32,
}

/// Load and validate the embark numbers. `biome_names` holds the world biome names in order.
pub fn load_embark_rules(
    file_name: &str,
    text: &str,
    biome_names: &[&str],
) -> Result<EmbarkRules, ContentError> {
    let r: EmbarkRules = parse_ron(file_name, text)?;
    let mut issues = Vec::new();
    if r.biomes.len() != biome_names.len() {
        issues.push(issue("biomes", "must hold one row for each world biome"));
    }
    for (i, row) in r.biomes.iter().enumerate() {
        if biome_names.get(i).is_some_and(|&n| n != row.biome) {
            issues.push(issue(
                &row.biome,
                "row is not in the order of the world biomes",
            ));
        }
        if !["TemperateForest", "ColdHills", "DryPlain"].contains(&row.local.as_str()) {
            issues.push(issue(
                &row.biome,
                "local must be TemperateForest, ColdHills, or DryPlain",
            ));
        }
        let shares = [
            row.tree_density,
            row.water,
            row.mountain,
            row.sand,
            row.clay,
        ];
        if shares.iter().any(|&v| v > 1000) {
            issues.push(issue(&row.biome, "shares must be from 0 to 1000"));
        }
    }
    let mut check = |ok: bool, field: &str, message: &str| {
        if !ok {
            issues.push(issue(field, message));
        }
    };
    check(
        r.mountain_cap <= 1000,
        "mountain_cap",
        "must be 1000 or less",
    );
    check(r.overgrow_pm <= 1000, "overgrow_pm", "must be 1000 or less");
    check(
        1 <= r.ruin_w_min && r.ruin_w_min <= r.ruin_w_max && r.ruin_w_max <= 40,
        "ruin_w_min",
        "must satisfy 1 <= ruin_w_min <= ruin_w_max <= 40",
    );
    check(
        1 <= r.ruin_h_min && r.ruin_h_min <= r.ruin_h_max && r.ruin_h_max <= 40,
        "ruin_h_min",
        "must satisfy 1 <= ruin_h_min <= ruin_h_max <= 40",
    );
    check(r.decay_k >= 1, "decay_k", "must be 1 or more");
    check(r.debris_decay_k >= 1, "debris_decay_k", "must be 1 or more");
    check(
        r.floor_rubble_div >= 1,
        "floor_rubble_div",
        "must be 1 or more",
    );
    check(
        r.salvage_age_div >= 1,
        "salvage_age_div",
        "must be 1 or more",
    );
    check(r.salvage_base >= 1, "salvage_base", "must be 1 or more");
    check(
        r.fallback_age <= 60_000,
        "fallback_age",
        "must be 60000 or less",
    );
    check(
        r.site_margin >= 1 && r.site_margin <= 100,
        "site_margin",
        "must be from 1 to 100",
    );
    check(
        r.max_map_sites >= 1 && r.max_map_sites <= 64,
        "max_map_sites",
        "must be from 1 to 64",
    );
    check(r.tunnel_len <= 200, "tunnel_len", "must be 200 or less");
    check(r.debris_radius <= 40, "debris_radius", "must be 40 or less");
    check(r.seal_radius <= 20, "seal_radius", "must be 20 or less");
    check(r.road_jitter <= 100, "road_jitter", "must be 100 or less");
    check(
        r.legend_certain_pct <= 100,
        "legend_certain_pct",
        "must be 100 or less",
    );
    check(
        r.legend_rumor_true_pct <= 100,
        "legend_rumor_true_pct",
        "must be 100 or less",
    );
    check(
        r.legend_min_gap <= 64,
        "legend_min_gap",
        "must be 64 or less",
    );
    check(r.rival_radius <= 20, "rival_radius", "must be 20 or less");
    check(r.max_rivals <= 8, "max_rivals", "must be 8 or less");
    check(
        1 <= r.rival_strength_min && r.rival_strength_min <= r.rival_strength_max,
        "rival_strength_min",
        "must satisfy 1 <= rival_strength_min <= rival_strength_max",
    );
    check(
        r.rival_strength_max <= 100,
        "rival_strength_max",
        "must be 100 or less",
    );
    check(
        r.rival_aggression_div >= 1,
        "rival_aggression_div",
        "must be 1 or more",
    );
    check(
        r.rival_battle_hostility <= 100 && r.rival_raze_hostility <= 100,
        "rival_battle_hostility",
        "hostility values must be 100 or less",
    );
    finish(file_name, r, issues)
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

    // ---- buildings, recipes, rooms ----

    const BUILDINGS: &str = include_str!("../../../assets/content/buildings.ron");
    const RECIPES: &str = include_str!("../../../assets/content/recipes.ron");
    const ROOMS: &str = include_str!("../../../assets/content/rooms.ron");
    const MATERIALS: &str = include_str!("../../../assets/content/materials.ron");

    fn building_error(text: &str) -> String {
        load_buildings("buildings.ron", text)
            .err()
            .map(|e| e.to_string())
            .unwrap_or_default()
    }

    #[test]
    fn real_building_recipe_and_room_files_are_valid() {
        let materials = load_materials("materials.ron", MATERIALS).unwrap_or_default();
        let buildings = load_buildings("buildings.ron", BUILDINGS).unwrap_or_default();
        // The kitchen is a second building of kind Workshop.
        assert_eq!(buildings.len(), BuildingKind::ALL.len() + 1);
        let recipes =
            load_recipes("recipes.ron", RECIPES, &materials, &buildings).unwrap_or_default();
        assert_eq!(recipes.len(), 10);
        let rooms = load_room_rules("rooms.ron", ROOMS);
        assert_eq!(rooms.map(|r| r.max_tiles).unwrap_or(0), 400);
    }

    #[test]
    fn building_float_literal_and_unknown_field_are_refused() {
        let msg = building_error(&BUILDINGS.replace("cost: 4", "cost: 4.5"));
        assert!(
            msg.contains("buildings.ron") && msg.contains("float literal"),
            "{msg}"
        );
        let msg = building_error(&BUILDINGS.replace("(id: \"wall\"", "(extra: 1, id: \"wall\""));
        assert!(msg.contains("buildings.ron"), "{msg}");
    }

    #[test]
    fn building_numbers_are_checked_and_errors_name_the_entry() {
        let msg = building_error(&BUILDINGS.replace("cost: 4", "cost: 0"));
        assert!(msg.contains("'wall'") && msg.contains("cost"), "{msg}");
        let msg = building_error(&BUILDINGS.replace(
            "return_pct: 50, blocks_walking: true, beauty: 0),
    (id: \"door\"",
            "return_pct: 150, blocks_walking: true, beauty: 0),
    (id: \"door\"",
        ));
        assert!(
            msg.contains("'wall'") && msg.contains("return_pct"),
            "{msg}"
        );
        let msg = building_error(&BUILDINGS.replace("id: \"door\"", "id: \"wall\""));
        assert!(
            msg.contains("duplicate ID") && msg.contains("'wall'"),
            "{msg}"
        );
        let msg = building_error(
            &BUILDINGS.replace("categories: [Wood], cost: 2", "categories: [], cost: 2"),
        );
        assert!(
            msg.contains("'door'") && msg.contains("categories"),
            "{msg}"
        );
    }

    #[test]
    fn a_wall_must_block_and_a_door_must_not() {
        let wall = BUILDINGS.replace(
            "cost: 4, work_ticks: 300, work_ticks_per_10_hardness: 40, deconstruct_pct: 50, return_pct: 50, blocks_walking: true",
            "cost: 4, work_ticks: 300, work_ticks_per_10_hardness: 40, deconstruct_pct: 50, return_pct: 50, blocks_walking: false",
        );
        assert!(building_error(&wall).contains("wall must"));
        let door = BUILDINGS.replace(
            "categories: [Wood], cost: 2, work_ticks: 250, work_ticks_per_10_hardness: 40, deconstruct_pct: 50, return_pct: 50, blocks_walking: false",
            "categories: [Wood], cost: 2, work_ticks: 250, work_ticks_per_10_hardness: 40, deconstruct_pct: 50, return_pct: 50, blocks_walking: true",
        );
        assert!(building_error(&door).contains("door"));
    }

    #[test]
    fn every_building_kind_must_exist() {
        let without_bed: String = BUILDINGS
            .lines()
            .filter(|l| !l.contains("id: \"bed\""))
            .collect::<Vec<_>>()
            .join(
                "
",
            );
        let msg = building_error(&without_bed);
        assert!(msg.contains("Bed"), "{msg}");
    }

    #[test]
    fn recipes_are_linked_to_materials_and_workshops() {
        let materials = load_materials("materials.ron", MATERIALS).unwrap_or_default();
        let buildings = load_buildings("buildings.ron", BUILDINGS).unwrap_or_default();
        let err = |text: &str| {
            load_recipes("recipes.ron", text, &materials, &buildings)
                .err()
                .map(|e| e.to_string())
                .unwrap_or_default()
        };
        let msg = err(&RECIPES.replace("(\"plank\", 2)", "(\"board\", 2)"));
        assert!(
            msg.contains("recipes.ron") && msg.contains("board"),
            "{msg}"
        );
        let msg = err(&RECIPES.replace(
            "workshop: \"workshop\", inputs: [(\"log\", 1)], outputs: [(\"plank\"",
            "workshop: \"wall\", inputs: [(\"log\", 1)], outputs: [(\"plank\"",
        ));
        assert!(msg.contains("'plank'") && msg.contains("workshop"), "{msg}");
        let msg = err(&RECIPES.replace(
            "[(\"log\", 1)], outputs: [(\"plank\", 2)]",
            "[(\"log\", 0)], outputs: [(\"plank\", 2)]",
        ));
        assert!(msg.contains("'plank'"), "{msg}");
        let msg = err(&RECIPES.replace("work_ticks: 200", "work_ticks: 2.5"));
        assert!(msg.contains("float literal"), "{msg}");
    }

    #[test]
    fn room_rules_are_checked() {
        let err = |text: &str| {
            load_room_rules("rooms.ron", text)
                .err()
                .map(|e| e.to_string())
                .unwrap_or_default()
        };
        assert!(err(&ROOMS.replace("max_tiles: 400", "max_tiles: 0")).contains("max_tiles"));
        assert!(
            err(&ROOMS.replace("roof_support_radius: 6", "roof_support_radius: 99"))
                .contains("roof_support_radius")
        );
        assert!(err(&ROOMS.replace("\"granite\"", "\"\"")).contains("bare_wall_material"));
        assert!(err(&ROOMS.replace("max_tiles: 400", "max_tiles: 4.5")).contains("float literal"));
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

    fn history_text(years_default: &str) -> String {
        std::fs::read_to_string("../../assets/content/history.ron")
            .unwrap_or_default()
            .replace("years_default: 3000", years_default)
    }

    #[test]
    fn history_rules_load_and_bad_values_are_refused() {
        let ok = history_text("years_default: 3000");
        assert!(load_history_rules("history.ron", &ok).is_ok());
        let bad = history_text("years_default: 99999");
        let err = load_history_rules("history.ron", &bad).map_err(|e| e.to_string());
        assert!(err.is_err_and(|e| e.contains("years_default")));
        let float = history_text("years_default: 3000.5");
        assert!(load_history_rules("history.ron", &float).is_err());
    }

    #[test]
    fn embark_rules_load_and_bad_rows_are_refused() {
        let text = std::fs::read_to_string("../../assets/content/embark.ron")
            .map_err(|e| e.to_string())
            .unwrap_or_default();
        let names: Vec<&str> = (0..19).map(|_| "x").collect();
        // The names do not match, so the order check names a row.
        let err = load_embark_rules("embark.ron", &text, &names).map_err(|e| e.to_string());
        assert!(err.is_err_and(|e| e.contains("order")));
        let real: Vec<String> = load_embark_rules("embark.ron", &text, &names)
            .err()
            .map(|e| e.issues.iter().filter_map(|i| i.entry.clone()).collect())
            .unwrap_or_default();
        assert_eq!(real.len(), 19);
        let refs: Vec<&str> = real.iter().map(String::as_str).collect();
        assert!(load_embark_rules("embark.ron", &text, &refs).is_ok());
        let short = &refs[..18];
        assert!(load_embark_rules("embark.ron", &text, short).is_err());
        let bad = text.replace("decay_k: 400", "decay_k: 0");
        let err = load_embark_rules("embark.ron", &bad, &refs).map_err(|e| e.to_string());
        assert!(err.is_err_and(|e| e.contains("decay_k")));
        let float = text.replace("decay_k: 400", "decay_k: 400.5");
        assert!(load_embark_rules("embark.ron", &float, &refs).is_err());
    }

    #[test]
    fn cultures_and_templates_are_checked() {
        let one = "(cultures: [(id: \"a\", name: \"A\", expansion: 50, aggression: 50, prosperity: 50, mining: 50)])";
        assert!(load_cultures("c.ron", one).is_ok());
        let high = one.replace("mining: 50", "mining: 101");
        assert!(load_cultures("c.ron", &high).is_err());
        let dup = "(cultures: [(id: \"a\", name: \"A\", expansion: 1, aggression: 1, prosperity: 1, mining: 1), (id: \"a\", name: \"B\", expansion: 1, aggression: 1, prosperity: 1, mining: 1)])";
        assert!(load_cultures("c.ron", dup).is_err());
        let t = "(templates: [(kind: \"X\", text: \"{year} {civ}\")])";
        assert!(load_event_templates("t.ron", t, &["X"]).is_ok());
        assert!(load_event_templates("t.ron", t, &["Y"]).is_err());
        let bad = "(templates: [(kind: \"X\", text: \"{nope}\")])";
        assert!(load_event_templates("t.ron", bad, &["X"]).is_err());
    }
}
