//! `EmbarkSummary`: the only input of local map generation (TECHNICAL_DESIGN §8.2).
//! In Phase 1 the test presets make it. In Phase 2 the world makes it.

/// Side length of a local map in tiles.
pub const LOCAL_SIZE: u32 = 256;

/// Biome of the embark area.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Biome {
    /// Mild climate, many trees.
    TemperateForest,
    /// Cold climate, rocky hills, few trees.
    ColdHills,
    /// Dry climate, flat, sand, few trees.
    DryPlain,
}

/// Summary of the world data around the embark cell. All values are small integers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EmbarkSummary {
    /// Seed of the local map.
    pub seed: u64,
    /// Biome.
    pub biome: Biome,
    /// Mean temperature in tenths of a degree Celsius.
    pub mean_temperature: i16,
    /// Share of tiles with trees, in per-mille (0 to 1000).
    pub tree_density: u16,
    /// Share of water tiles, in per-mille.
    pub water_amount: u16,
    /// Share of solid rock tiles (mountain), in per-mille.
    pub mountain_amount: u16,
    /// Share of sand among the open ground tiles, in per-mille.
    pub sand_amount: u16,
    /// Share of clay among the open ground tiles, in per-mille.
    pub clay_amount: u16,
    /// Ore richness: 0 (no ore) to 3 (rich).
    pub ore_richness: u8,
    /// Local map width in tiles.
    pub width: u32,
    /// Local map height in tiles.
    pub height: u32,
}

impl EmbarkSummary {
    /// Same summary with another seed.
    pub fn with_seed(mut self, seed: u64) -> Self {
        self.seed = seed;
        self
    }

    /// Test preset: temperate forest, a stream and a hill.
    pub fn temperate_forest() -> Self {
        Self {
            seed: 1,
            biome: Biome::TemperateForest,
            mean_temperature: 110,
            tree_density: 450,
            water_amount: 60,
            mountain_amount: 120,
            sand_amount: 30,
            clay_amount: 60,
            ore_richness: 2,
            width: LOCAL_SIZE,
            height: LOCAL_SIZE,
        }
    }

    /// Test preset: cold hills with a large mountain and few trees.
    pub fn cold_hills() -> Self {
        Self {
            seed: 2,
            biome: Biome::ColdHills,
            mean_temperature: -30,
            tree_density: 150,
            water_amount: 40,
            mountain_amount: 350,
            sand_amount: 10,
            clay_amount: 30,
            ore_richness: 3,
            width: LOCAL_SIZE,
            height: LOCAL_SIZE,
        }
    }

    /// Test preset: dry plain with sand, a small pond, and a small rock outcrop.
    pub fn dry_plain() -> Self {
        Self {
            seed: 3,
            biome: Biome::DryPlain,
            mean_temperature: 230,
            tree_density: 15,
            water_amount: 25,
            mountain_amount: 60,
            sand_amount: 500,
            clay_amount: 40,
            ore_richness: 1,
            width: LOCAL_SIZE,
            height: LOCAL_SIZE,
        }
    }
}

// ---------------------------------------------------------------------------------------------
// S11: embark summary from the world, and the site records of the embark area.
// ---------------------------------------------------------------------------------------------

use crate::biome::WorldBiome;
use crate::history::{EventKind, History, NEVER, SiteKind};
use crate::world::{WORLD_SIZE, WorldData};
use base::content::{ContentError, EmbarkRules, load_embark_rules};
use base::rng::mix;
use std::sync::OnceLock;

const EMBARK_RON: &str = include_str!("../../../assets/content/embark.ron");
const TAG_EMBARK: u64 = 0x454d_4241_524b_5f31;
const TAG_EDGE: u64 = 0x454d_4241_524b_4544;
/// Index of the north road exit in `EmbarkSites::road_exits`.
pub const EXIT_N: usize = 0;
/// Index of the west road exit.
pub const EXIT_W: usize = 1;
/// Index of the east road exit.
pub const EXIT_E: usize = 2;
/// Index of the south road exit.
pub const EXIT_S: usize = 3;

static RULES: OnceLock<Result<EmbarkRules, ContentError>> = OnceLock::new();

/// The embark numbers. Parsed from the embedded file on the first call.
pub fn embark_rules() -> Result<&'static EmbarkRules, &'static ContentError> {
    RULES
        .get_or_init(|| {
            let names = WorldBiome::ALL.map(WorldBiome::name);
            load_embark_rules("embark.ron", EMBARK_RON, &names)
        })
        .as_ref()
}

/// Check the embedded embark table.
pub fn check_content() -> Result<(), ContentError> {
    embark_rules().map(|_| ()).map_err(Clone::clone)
}

/// A site record in the embark area, ready for stamping.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EmbarkSite {
    /// Site ID in the history (0 for a site that has no record).
    pub id: u32,
    /// Kind.
    pub kind: SiteKind,
    /// Age in years at the end of the history. A living settlement has age 0.
    pub age: u32,
    /// Culture of the civilization that made the site.
    pub culture: u8,
    /// Mineral index plus 1 of a mine (0 for other sites).
    pub mineral: u8,
    /// Richness of a mine, or threat of a sealed site (1 to 3).
    pub level: u8,
}

/// A living settlement near the embark cell that can raid the colony (R-070).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EmbarkRival {
    /// Site ID of the settlement in the history.
    pub site: u32,
    /// Owner civilization ID.
    pub civ: u32,
    /// Culture of the owner.
    pub culture: u8,
    /// Fighters (3 to 12 with the numbers of `embark.ron`).
    pub strength: u8,
    /// Hostility from the history alone (0 to 100): war with the owner of the embark cell.
    pub base_hostility: u8,
    /// The map edge that the raiders come from: `EXIT_N`, `EXIT_W`, `EXIT_E`, or `EXIT_S`.
    pub edge: u8,
    /// Distance from the embark cell in world cells.
    pub distance: u8,
}

/// The history of the embark area (the 3 by 3 world cells around the selected cell).
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct EmbarkSites {
    /// Sites, nearest first. At most `max_map_sites`.
    pub sites: Vec<EmbarkSite>,
    /// Road exits on the north, west, east, and south edge: the tile offset along the edge.
    pub road_exits: [Option<u16>; 4],
    /// True when a road crosses the selected cell.
    pub road_here: bool,
    /// True when a road lies in the embark area.
    pub road_near: bool,
    /// Largest depletion (0 to 255) in the embark area.
    pub depletion: u8,
    /// Number of site records in the embark area before the limit.
    pub site_count: u32,
    /// The rival colonies, nearest first (R-070).
    pub rivals: Vec<EmbarkRival>,
}

impl EmbarkSites {
    /// True when the area has a site record or a road (Rule 9).
    pub fn has_history(&self) -> bool {
        self.site_count > 0 || self.road_near
    }
}

/// True when a player can embark on the cell: land that is not ocean, lake, or glacier.
pub fn is_habitable(world: &WorldData, x: i32, y: i32) -> bool {
    world.biome.get(x, y).is_some_and(|b| {
        !matches!(
            b,
            WorldBiome::Ocean | WorldBiome::Lake | WorldBiome::Glacier
        )
    })
}

/// Offset of a road exit along the edge between two world cells. Both cells get the same
/// value, so roads join across a cell border.
fn edge_offset(world_seed: u64, a: (i32, i32), b: (i32, i32), margin: u32) -> u16 {
    let key = (u64::from(a.0.min(b.0) as u32) << 40)
        ^ (u64::from(a.1.min(b.1) as u32) << 20)
        ^ u64::from((a.0.max(b.0) + a.1.max(b.1)) as u32);
    let span = LOCAL_SIZE.saturating_sub(2 * margin).max(1);
    (margin + (mix(world_seed ^ TAG_EDGE ^ key) % u64::from(span)) as u32) as u16
}

/// The rival colonies of an embark cell (R-070): living settlements of living civilizations
/// within `rival_radius` world cells, nearest first, at most `max_rivals`. Same input gives
/// the same list.
fn rivals_of(history: &History, rules: &EmbarkRules, wx: i32, wy: i32) -> Vec<EmbarkRival> {
    let radius = rules.rival_radius as i32;
    let mut found: Vec<(i32, u32, EmbarkRival)> = Vec::new();
    for site in &history.sites {
        if site.kind != SiteKind::Settlement || site.ended != NEVER {
            continue;
        }
        let (dx, dy) = (i32::from(site.x) - wx, i32::from(site.y) - wy);
        let distance = dx.abs().max(dy.abs());
        if distance > radius {
            continue;
        }
        let Some(civ) = history.civ(site.civ).filter(|c| c.collapsed == NEVER) else {
            continue;
        };
        // Wars over the embark area: battles and razed settlements of this civilization in the 3
        // by 3 cells around the embark cell.
        let war: u32 = history
            .events
            .iter()
            .filter(|e| e.actors[0] == civ.id || e.actors[1] == civ.id)
            .filter(|e| {
                history.site(e.place).is_some_and(|s| {
                    (i32::from(s.x) - wx).abs() <= 1 && (i32::from(s.y) - wy).abs() <= 1
                })
            })
            .map(|e| match e.kind {
                EventKind::Battle => rules.rival_battle_hostility,
                EventKind::Raze => rules.rival_raze_hostility,
                _ => 0,
            })
            .sum();
        let base = (u32::from(civ.aggression) / rules.rival_aggression_div.max(1) + war).min(100);
        let span = rules.rival_strength_max - rules.rival_strength_min;
        let strength = rules.rival_strength_min + span * u32::from(civ.prosperity.min(100)) / 100;
        let edge = if dx == 0 && dy == 0 {
            [EXIT_N, EXIT_W, EXIT_E, EXIT_S][(site.id % 4) as usize]
        } else if dx.abs() >= dy.abs() {
            if dx > 0 { EXIT_E } else { EXIT_W }
        } else if dy > 0 {
            EXIT_S
        } else {
            EXIT_N
        };
        found.push((
            distance,
            site.id,
            EmbarkRival {
                site: site.id,
                civ: civ.id,
                culture: civ.culture,
                strength: strength.min(255) as u8,
                base_hostility: base as u8,
                edge: edge as u8,
                distance: distance.min(255) as u8,
            },
        ));
    }
    found.sort_by_key(|&(distance, id, _)| (distance, id));
    found
        .into_iter()
        .take(rules.max_rivals as usize)
        .map(|(_, _, r)| r)
        .collect()
}

/// Make the embark summary and the site records for a world cell. Returns `None` when the cell
/// is outside the world, is not habitable, or the embedded tables are not valid.
pub fn embark_from_world(
    world: &WorldData,
    history: &History,
    wx: i32,
    wy: i32,
) -> Option<(EmbarkSummary, EmbarkSites)> {
    let rules = embark_rules().ok()?;
    if !is_habitable(world, wx, wy) {
        return None;
    }
    let biome = *world.biome.get(wx, wy)?;
    let row = rules.biomes.get(biome as usize)?;
    let local_biome = match row.local.as_str() {
        "ColdHills" => Biome::ColdHills,
        "DryPlain" => Biome::DryPlain,
        _ => Biome::TemperateForest,
    };
    let river = u32::from(*world.river.get(wx, wy)?);
    let mut water = row.water + river * rules.river_water;
    if world.lake.get(wx, wy).is_some_and(|&l| l != 0) {
        water += rules.lake_water;
    }
    let near_ocean = [(0, -1), (-1, 0), (1, 0), (0, 1)].iter().any(|&(dx, dy)| {
        world
            .biome
            .get(wx + dx, wy + dy)
            .is_some_and(|b| matches!(b, WorldBiome::Ocean))
    });
    if near_ocean {
        water += rules.coast_water;
    }
    let high = world.height_above_sea(wx, wy).max(0) as u32;
    let mountain = (row.mountain
        + high.saturating_sub(rules.mountain_h0) * rules.mountain_per_height)
        .min(rules.mountain_cap);

    let mut richness = 0u8;
    for dy in -1..=1 {
        for dx in -1..=1 {
            if let Some(&r) = world.regions.deposits.richness.get(wx + dx, wy + dy) {
                richness = richness.max(r);
            }
        }
    }
    let temperature = i16::from(*world.temperature.get(wx, wy)?) * 10;
    let cell = (u64::from(wx as u32) << 16) | u64::from(wy as u32);
    let summary = EmbarkSummary {
        seed: mix(world.seed ^ TAG_EMBARK ^ cell),
        biome: local_biome,
        mean_temperature: temperature,
        tree_density: row.tree_density.min(1000) as u16,
        water_amount: water.min(1000) as u16,
        mountain_amount: mountain.min(1000) as u16,
        sand_amount: row.sand.min(1000) as u16,
        clay_amount: row.clay.min(1000) as u16,
        ore_richness: richness.min(3),
        width: LOCAL_SIZE,
        height: LOCAL_SIZE,
    };

    let mut sites = EmbarkSites::default();
    let mut found: Vec<(i32, u32, EmbarkSite)> = Vec::new();
    for site in &history.sites {
        let (dx, dy) = (i32::from(site.x) - wx, i32::from(site.y) - wy);
        if dx.abs() > 1 || dy.abs() > 1 {
            continue;
        }
        let since = if site.ended != NEVER {
            u32::from(site.ended)
        } else {
            u32::from(site.year)
        };
        let age = match site.kind {
            SiteKind::Settlement => 0,
            _ => history.years.saturating_sub(since),
        };
        let culture = history.civ(site.civ).map_or(0, |c| c.culture);
        found.push((
            dx.abs().max(dy.abs()),
            site.id,
            EmbarkSite {
                id: site.id,
                kind: site.kind,
                age,
                culture,
                mineral: site.mineral,
                level: site.level,
            },
        ));
    }
    sites.rivals = rivals_of(history, rules, wx, wy);
    found.sort_by_key(|&(distance, id, _)| (distance, id));
    sites.site_count = found.len() as u32;
    sites.sites = found
        .into_iter()
        .take(rules.max_map_sites as usize)
        .map(|(_, _, s)| s)
        .collect();

    let road = |x: i32, y: i32| history.road.get(x, y).copied().unwrap_or(0) > 0;
    sites.road_here = road(wx, wy);
    for dy in -1..=1 {
        for dx in -1..=1 {
            if road(wx + dx, wy + dy) {
                sites.road_near = true;
            }
            if let Some(&d) = history.depletion.get(wx + dx, wy + dy) {
                sites.depletion = sites.depletion.max(d);
            }
        }
    }
    if sites.road_here {
        let exits = [(0, -1), (-1, 0), (1, 0), (0, 1)];
        for (i, (dx, dy)) in exits.into_iter().enumerate() {
            let (nx, ny) = (wx + dx, wy + dy);
            let in_world =
                (0..WORLD_SIZE as i32).contains(&nx) && (0..WORLD_SIZE as i32).contains(&ny);
            if in_world && road(nx, ny) {
                sites.road_exits[i] = Some(edge_offset(
                    world.seed,
                    (wx, wy),
                    (nx, ny),
                    rules.site_margin,
                ));
            }
        }
    }
    Some((summary, sites))
}
