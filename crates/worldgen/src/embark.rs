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
