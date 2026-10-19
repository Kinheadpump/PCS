//! World data and the generation pipeline for steps 1 to 9 (TECHNICAL_DESIGN §8).
//!
//! Each stage writes integer outputs. The next stage reads only these outputs. Each stage
//! has a seed (mix of the world seed and a fixed tag) and a stage hash of its outputs.

use crate::biome::{self, WorldBiome};
use crate::{climate, height, hydro, plates};
use base::Grid;
use base::hash::Fnv1a;
use base::rng::mix;

/// Side length of the world in cells (GAME_DESIGN §9.1).
pub const WORLD_SIZE: u32 = 256;

/// Number of generation stages in this file.
pub const STAGE_COUNT: usize = 9;

/// A generation stage. The number is the index in the seed and hash arrays.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    /// Step 1: stage seeds.
    Seeds,
    /// Step 2: uplift and volcanic flags (ridged noise in place of plates).
    Uplift,
    /// Step 3: height.
    Height,
    /// Step 4: sea level.
    SeaLevel,
    /// Step 5: lakes and drainage.
    Lakes,
    /// Step 6: temperature.
    Temperature,
    /// Step 7: moisture.
    Moisture,
    /// Step 8: rivers.
    Rivers,
    /// Step 9: biomes.
    Biomes,
}

impl Stage {
    /// All stages in generation order.
    pub const ALL: [Stage; STAGE_COUNT] = [
        Stage::Seeds,
        Stage::Uplift,
        Stage::Height,
        Stage::SeaLevel,
        Stage::Lakes,
        Stage::Temperature,
        Stage::Moisture,
        Stage::Rivers,
        Stage::Biomes,
    ];

    /// Short stable name, used in golden files and tool output.
    pub fn name(self) -> &'static str {
        match self {
            Stage::Seeds => "seeds",
            Stage::Uplift => "uplift",
            Stage::Height => "height",
            Stage::SeaLevel => "sea_level",
            Stage::Lakes => "lakes",
            Stage::Temperature => "temperature",
            Stage::Moisture => "moisture",
            Stage::Rivers => "rivers",
            Stage::Biomes => "biomes",
        }
    }

    /// Fixed tag constant of the stage. Never change a tag: it changes all worlds.
    fn tag(self) -> u64 {
        0x574F_524C_4400_0000 + self as u64
    }
}

/// Tunable numbers of the world generator. All values are integers.
/// A stage reads only its own fields, so a change in one stage keeps the earlier stages.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorldParams {
    /// Uplift: maximum mountain height added by a ridge, in height units.
    pub ridge_height: i32,
    /// Uplift: lattice spacing of the ridge noise in cells.
    pub ridge_cell: u32,
    /// Uplift: share of the highest ridge cells that are volcanic, in per-mille.
    pub volcano_per_mille: u32,
    /// Height: amplitude of the fBm term in height units.
    pub height_amp: i32,
    /// Height: lattice spacing of the first fBm octave in cells.
    pub height_cell: u32,
    /// Height: width of the ocean falloff at the map edges in cells.
    pub edge_width: i32,
    /// Sea level: land share in per-mille (default 400 = 40%).
    pub land_per_mille: u32,
    /// Lakes: minimum depth of a filled depression to become a lake, in height units.
    pub lake_depth_min: i16,
    /// Temperature: drop in degrees Celsius for each 10,000 height units above sea level.
    pub lapse_per_10k: i32,
    /// Moisture: share of the carried moisture that falls on a land cell, per-mille.
    pub rain_base: i32,
    /// Moisture: extra share that falls for each height unit of climb, per-mille.
    pub rain_climb: i32,
    /// Rivers: flow accumulation for a small river.
    pub river_small: u32,
    /// Rivers: flow accumulation for a medium river.
    pub river_medium: u32,
    /// Rivers: flow accumulation for a large river.
    pub river_large: u32,
    /// Biomes: mean temperature (degrees Celsius) at or below which land is glacier.
    pub glacier_max_temp: i8,
    /// Biomes: height above sea level of high mountains.
    pub mountain_min: i16,
    /// Biomes: maximum height above sea level of a floodplain.
    pub floodplain_max: i16,
}

impl Default for WorldParams {
    fn default() -> Self {
        Self {
            ridge_height: 2600,
            ridge_cell: 40,
            volcano_per_mille: 40,
            height_amp: 3200,
            height_cell: 72,
            edge_width: 22,
            land_per_mille: 400,
            lake_depth_min: 30,
            lapse_per_10k: 65,
            rain_base: 30,
            rain_climb: 4,
            river_small: 150,
            river_medium: 600,
            river_large: 1800,
            glacier_max_temp: -14,
            mountain_min: 1800,
            floodplain_max: 300,
        }
    }
}

/// The generated world terrain. Immutable after generation.
#[derive(Clone, Debug)]
pub struct WorldData {
    /// World seed.
    pub seed: u64,
    /// Stage seeds, indexed by stage number.
    pub stage_seeds: [u64; STAGE_COUNT],
    /// Hash of the outputs of each stage, indexed by stage number.
    pub stage_hashes: [u64; STAGE_COUNT],
    /// Uplift from mountain ridges, in height units.
    pub uplift: Grid<i16>,
    /// Volcanic flag.
    pub volcanic: Grid<bool>,
    /// Height in height units (about 1 m).
    pub height: Grid<i16>,
    /// Sea level. Cells below it are ocean.
    pub sea_level: i16,
    /// Height after depression filling.
    pub filled: Grid<i16>,
    /// Lake ID of the cell (0 for none).
    pub lake: Grid<u16>,
    /// Direction index into the D8 table of the next cell downstream (255 for none).
    pub drain: Grid<u8>,
    /// Mean temperature in degrees Celsius.
    pub temperature: Grid<i8>,
    /// Half of the yearly temperature range in degrees Celsius.
    pub temp_amp: Grid<u8>,
    /// Moisture 0 to 255.
    pub moisture: Grid<u8>,
    /// Weighted flow accumulation.
    pub flow: Grid<u32>,
    /// River size: 0 none, 1 small, 2 medium, 3 large.
    pub river: Grid<u8>,
    /// Biome of each cell.
    pub biome: Grid<WorldBiome>,
}

impl WorldData {
    /// Land share in per-mille (cells at or above sea level).
    pub fn land_per_mille(&self) -> u32 {
        let cells = self.height.as_slice();
        let land = cells.iter().filter(|&&h| h >= self.sea_level).count();
        (land * 1000 / cells.len().max(1)) as u32
    }

    /// Height above sea level of a cell (negative in the ocean).
    pub fn height_above_sea(&self, x: i32, y: i32) -> i32 {
        i32::from(self.height.get(x, y).copied().unwrap_or(0)) - i32::from(self.sea_level)
    }
}

/// Feed a slice into a hash. Each item becomes a fixed little-endian byte array.
fn feed<T: Copy, const N: usize>(h: &mut Fnv1a, items: &[T], bytes: impl Fn(T) -> [u8; N]) {
    for &item in items {
        h.write(&bytes(item));
    }
}

/// Generate the world terrain (steps 1 to 9) for a seed.
pub fn generate_world(seed: u64, params: &WorldParams) -> WorldData {
    let mut stage_seeds = [0u64; STAGE_COUNT];
    for stage in Stage::ALL {
        stage_seeds[stage as usize] = mix(seed ^ stage.tag());
    }
    let s = |stage: Stage| stage_seeds[stage as usize];
    let mut hashes = [0u64; STAGE_COUNT];

    let mut h = Fnv1a::new();
    feed(&mut h, &stage_seeds, u64::to_le_bytes);
    hashes[Stage::Seeds as usize] = h.finish();

    let (uplift, volcanic) = plates::make_uplift(s(Stage::Uplift), params);
    let mut h = Fnv1a::new();
    feed(&mut h, uplift.as_slice(), i16::to_le_bytes);
    feed(&mut h, volcanic.as_slice(), |v| [u8::from(v)]);
    hashes[Stage::Uplift as usize] = h.finish();

    let height = height::make_height(s(Stage::Height), &uplift, params);
    let mut h = Fnv1a::new();
    feed(&mut h, height.as_slice(), i16::to_le_bytes);
    hashes[Stage::Height as usize] = h.finish();

    let sea_level = height::sea_level(&height, params);
    hashes[Stage::SeaLevel as usize] = base::hash::fnv1a(&sea_level.to_le_bytes());

    let lakes = hydro::fill_lakes(&height, sea_level, params);
    let mut h = Fnv1a::new();
    feed(&mut h, lakes.filled.as_slice(), i16::to_le_bytes);
    feed(&mut h, lakes.lake.as_slice(), u16::to_le_bytes);
    feed(&mut h, lakes.drain.as_slice(), |v| [v]);
    hashes[Stage::Lakes as usize] = h.finish();

    let (temperature, temp_amp) =
        climate::make_temperature(s(Stage::Temperature), &height, sea_level, params);
    let mut h = Fnv1a::new();
    feed(&mut h, temperature.as_slice(), i8::to_le_bytes);
    feed(&mut h, temp_amp.as_slice(), |v| [v]);
    hashes[Stage::Temperature as usize] = h.finish();

    let moisture =
        climate::make_moisture(s(Stage::Moisture), &height, sea_level, &temperature, params);
    let mut h = Fnv1a::new();
    feed(&mut h, moisture.as_slice(), |v| [v]);
    hashes[Stage::Moisture as usize] = h.finish();

    let rivers = hydro::make_rivers(&height, sea_level, &lakes, &moisture, params);
    let mut h = Fnv1a::new();
    feed(&mut h, rivers.flow.as_slice(), u32::to_le_bytes);
    feed(&mut h, rivers.river.as_slice(), |v| [v]);
    hashes[Stage::Rivers as usize] = h.finish();

    let inputs = biome::BiomeInputs {
        height: &height,
        sea_level,
        lake: &lakes.lake,
        temperature: &temperature,
        moisture: &moisture,
        river: &rivers.river,
        volcanic: &volcanic,
    };
    let biome = biome::make_biomes(&inputs, params);
    let mut h = Fnv1a::new();
    feed(&mut h, biome.as_slice(), |v| [v as u8]);
    hashes[Stage::Biomes as usize] = h.finish();

    WorldData {
        seed,
        stage_seeds,
        stage_hashes: hashes,
        uplift,
        volcanic,
        height,
        sea_level,
        filled: lakes.filled,
        lake: lakes.lake,
        drain: lakes.drain,
        temperature,
        temp_amp,
        moisture,
        flow: rivers.flow,
        river: rivers.river,
        biome,
    }
}

/// Make a world-size grid from a row-major vector. A wrong length gives a grid of the fill value.
pub(crate) fn world_grid<T: Clone>(cells: Vec<T>, fill: T) -> Grid<T> {
    match Grid::from_vec(WORLD_SIZE, WORLD_SIZE, cells) {
        Some(grid) => grid,
        None => Grid::new(WORLD_SIZE, WORLD_SIZE, fill),
    }
}
