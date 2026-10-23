//! Scenario test helper: maps from ASCII text (TECHNICAL_DESIGN §11.1).
//!
//! Legend: `.` open soil, `#` wall, `~` water, `T` tree (walkable), `R` granite rock wall (mineable), `c` open soil with a colonist.
//! Empty lines are ignored. All rows must have the same length.

use crate::Sim;
use crate::map::LocalMap;
use std::fmt;

/// Error from reading an ASCII map.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ScenarioError {
    /// The text has no rows.
    Empty,
    /// A row has a different length than the first row (line number from 1).
    RaggedRow(usize),
    /// A character is not in the legend (line number from 1).
    UnknownChar(usize, char),
    /// The map is larger than 4096 tiles on one side.
    TooLarge,
}

impl fmt::Display for ScenarioError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ScenarioError::Empty => write!(f, "scenario map has no rows"),
            ScenarioError::RaggedRow(n) => write!(f, "row {n} has a different length"),
            ScenarioError::UnknownChar(n, c) => write!(f, "row {n} has unknown character {c:?}"),
            ScenarioError::TooLarge => write!(f, "scenario map is too large"),
        }
    }
}

impl std::error::Error for ScenarioError {}

/// Read an ASCII map. Return the map and the colonist spawn tiles in row-major order.
pub fn map_from_ascii(text: &str) -> Result<(LocalMap, Vec<(u16, u16)>), ScenarioError> {
    let rows: Vec<&str> = text
        .lines()
        .map(str::trim_end)
        .filter(|l| !l.trim().is_empty())
        .map(str::trim_start)
        .collect();
    let first = rows.first().ok_or(ScenarioError::Empty)?;
    let width = first.chars().count();
    let height = rows.len();
    if width > crate::map::MAX_MAP_SIDE as usize || height > crate::map::MAX_MAP_SIDE as usize {
        return Err(ScenarioError::TooLarge);
    }
    let len = width * height;
    let (mut water, mut tree, mut wall) = (vec![false; len], vec![false; len], vec![false; len]);
    let mut rock = vec![false; len];
    let mut spawns = Vec::new();
    for (y, row) in rows.iter().enumerate() {
        if row.chars().count() != width {
            return Err(ScenarioError::RaggedRow(y + 1));
        }
        for (x, ch) in row.chars().enumerate() {
            let i = y * width + x;
            match ch {
                '.' => {}
                '#' => wall[i] = true,
                '~' => water[i] = true,
                'T' => tree[i] = true,
                'R' => rock[i] = true,
                'c' => spawns.push((x as u16, y as u16)),
                other => return Err(ScenarioError::UnknownChar(y + 1, other)),
            }
        }
    }
    let map = LocalMap::from_cells(width as u32, height as u32, &water, &tree, &wall, &rock);
    Ok((map, spawns))
}

/// Make a `Sim` from an ASCII map. It has one colonist for each `c` and content hash 0.
pub fn sim_from_ascii(text: &str, seed: u64) -> Result<Sim, ScenarioError> {
    let (map, spawns) = map_from_ascii(text)?;
    let mut sim = Sim::from_map(map, seed, 0, &spawns);
    // Scenario colonists start with a stone axe and a stone pick. Tests of R-008 clear them.
    let (axe, pick) = (
        sim.rules().material_index("stone_axe"),
        sim.rules().material_index("stone_pick"),
    );
    let ids: Vec<_> = sim.colonists().iter().map(|(id, _)| id).collect();
    for id in ids {
        for material in [axe, pick].into_iter().flatten() {
            sim.debug_set_tool(id, material, 400);
        }
    }
    Ok(sim)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_layers_and_spawns() {
        let (map, spawns) = map_from_ascii("\n  .#~T\n  c...\n").unwrap();
        assert_eq!((map.width(), map.height()), (4, 2));
        assert!(map.passable(0, 0));
        assert!(map.wall(1, 0) && !map.passable(1, 0));
        assert!(!map.passable(2, 0));
        assert!(map.tree(3, 0) && map.passable(3, 0), "trees do not block");
        assert_eq!(spawns, vec![(0, 1)]);
    }

    #[test]
    fn rejects_bad_text() {
        assert!(matches!(map_from_ascii("\n\n"), Err(ScenarioError::Empty)));
        assert!(matches!(
            map_from_ascii("..\n."),
            Err(ScenarioError::RaggedRow(2))
        ));
        assert!(matches!(
            map_from_ascii("..\n.x"),
            Err(ScenarioError::UnknownChar(2, 'x'))
        ));
    }
}
