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

#[cfg(test)]
mod tests {
    use super::*;

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
}
