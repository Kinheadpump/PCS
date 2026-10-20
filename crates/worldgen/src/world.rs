//! World data and the generation pipeline for steps 1 to 12 (TECHNICAL_DESIGN §8).
//!
//! Each stage writes integer outputs. The next stage reads only these outputs. Each stage
//! has a seed (mix of the world seed and a fixed tag) and a stage hash of its outputs.

use crate::biome::{self, WorldBiome};
use crate::deposit::{self, Deposits};
use crate::geology::{self, GeoInputs, Geology};
use crate::region::{self, RegionInputs, Regions};
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

/// Number of geology and deposit stages (steps 10 and 11). They have their own seed and
/// hash arrays so that the hashes of steps 1 to 9 stay as they were.
pub const LAYER_STAGE_COUNT: usize = 2;

/// A generation stage after step 9. The number is the index in the layer arrays.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LayerStage {
    /// Step 10: geology.
    Geology,
    /// Step 11: deposits.
    Deposits,
}

impl LayerStage {
    /// All layer stages in generation order.
    pub const ALL: [LayerStage; LAYER_STAGE_COUNT] = [LayerStage::Geology, LayerStage::Deposits];

    /// Short stable name, used in golden files and tool output.
    pub fn name(self) -> &'static str {
        const NAMES: [&str; LAYER_STAGE_COUNT] = ["geology", "deposits"];
        NAMES[self as usize]
    }

    /// Fixed tag constant of the stage. It continues the tags of the first 9 stages.
    fn tag(self) -> u64 {
        0x574F_524C_4400_0000 + STAGE_COUNT as u64 + self as u64
    }
}

/// Number of region stages (step 12). They have their own seed and hash arrays so that the
/// hashes of steps 1 to 11 stay as they were.
pub const REGION_STAGE_COUNT: usize = 1;

/// A generation stage after step 11. The number is the index in the region arrays.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RegionStage {
    /// Step 12: regions, region graph, and deposit repair.
    Regions,
}

impl RegionStage {
    /// All region stages in generation order.
    pub const ALL: [RegionStage; REGION_STAGE_COUNT] = [RegionStage::Regions];

    /// Short stable name, used in golden files and tool output.
    pub fn name(self) -> &'static str {
        const NAMES: [&str; REGION_STAGE_COUNT] = ["regions"];
        NAMES[self as usize]
    }

    /// Fixed tag constant of the stage. It continues the tags of steps 1 to 11.
    fn tag(self) -> u64 {
        0x574F_524C_4400_0000 + STAGE_COUNT as u64 + LAYER_STAGE_COUNT as u64 + self as u64
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
    /// Geology: lattice spacing of the province points in cells.
    pub province_cell: u32,
    /// Geology: height above sea level that counts as mountain rock.
    pub rock_mountain_min: i16,
    /// Geology: share of volcanic land cells that makes a volcanic province, per-mille.
    pub prov_volcanic_pm: i32,
    /// Geology: share of mountain land cells that makes a mountain province, per-mille.
    pub prov_mountain_pm: i32,
    /// Geology: share of coast land cells that makes a coast province, per-mille.
    pub prov_coast_pm: i32,
    /// Geology: chance of a third rock layer, per-mille.
    pub layer3_per_mille: u32,
    /// Geology: soil depth of dry flat land in decimeters.
    pub soil_flat: i32,
    /// Geology: extra soil depth at full moisture in decimeters.
    pub soil_wet: i32,
    /// Geology: soil lost for each 1,000 height units above sea level, in decimeters.
    pub soil_slope: i32,
    /// Geology: extra soil depth on rivers and floodplains in decimeters.
    pub soil_river: i32,
    /// Geology: extra soil depth on volcanic ash in decimeters.
    pub soil_ash: i32,
    /// Geology: range of the random soil term in decimeters.
    pub soil_jitter: i32,
    /// Geology: maximum soil depth in decimeters.
    pub soil_max: i32,
    /// Deposits: chance for each 100 points of rock affinity, in 1/10,000 per cell.
    pub deposit_base_per_10k: u32,
    /// Deposits: share of richness 2, per-mille.
    pub rich2_per_mille: u32,
    /// Deposits: share of richness 3, per-mille.
    pub rich3_per_mille: u32,
    /// Regions: side of the seed blocks in cells. Each block with land has one seed.
    pub region_cell: u32,
    /// Regions: a region with fewer cells is merged into a neighbor.
    pub region_min_cells: u32,
    /// Regions: extra flood fill cost to enter a high mountain cell.
    pub region_ridge_cost: u32,
    /// Regions: extra cost for each river size step (also the cost to cross a river).
    pub region_river_cost: u32,
    /// Regions: cost for each 16 height units of climb.
    pub region_slope_cost: u32,
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
            province_cell: 16,
            rock_mountain_min: 1200,
            prov_volcanic_pm: 150,
            prov_mountain_pm: 300,
            prov_coast_pm: 250,
            layer3_per_mille: 500,
            soil_flat: 20,
            soil_wet: 30,
            soil_slope: 20,
            soil_river: 15,
            soil_ash: 10,
            soil_jitter: 8,
            soil_max: 60,
            deposit_base_per_10k: 100,
            rich2_per_mille: 300,
            rich3_per_mille: 100,
            region_cell: 8,
            region_min_cells: 24,
            region_ridge_cost: 60,
            region_river_cost: 12,
            region_slope_cost: 16,
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
    /// Seeds of steps 10 and 11, indexed by `LayerStage`.
    pub layer_seeds: [u64; LAYER_STAGE_COUNT],
    /// Hashes of the outputs of steps 10 and 11, indexed by `LayerStage`.
    pub layer_hashes: [u64; LAYER_STAGE_COUNT],
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
    /// Provinces, rock layers, and soil (step 10).
    pub geology: Geology,
    /// Mineral deposits (step 11).
    pub deposits: Deposits,
    /// Seeds of step 12, indexed by `RegionStage`.
    pub region_seeds: [u64; REGION_STAGE_COUNT],
    /// Hashes of the outputs of step 12, indexed by `RegionStage`.
    pub region_hashes: [u64; REGION_STAGE_COUNT],
    /// Regions, region graph, and the repaired deposits (step 12).
    pub regions: Regions,
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

/// Check the embedded content tables, then generate the world (steps 1 to 12).
pub fn try_generate_world(
    seed: u64,
    params: &WorldParams,
) -> Result<WorldData, base::content::ContentError> {
    crate::history::check_content()?;
    Ok(generate_world(seed, params))
}

/// Generate the world (steps 1 to 12) for a seed.
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

    let mut layer_seeds = [0u64; LAYER_STAGE_COUNT];
    for stage in LayerStage::ALL {
        layer_seeds[stage as usize] = mix(seed ^ stage.tag());
    }
    let mut layer_hashes = [0u64; LAYER_STAGE_COUNT];

    let geo_inputs = GeoInputs {
        height: &height,
        sea_level,
        moisture: &moisture,
        river: &rivers.river,
        biome: &biome,
    };
    let geology = geology::make_geology(
        layer_seeds[LayerStage::Geology as usize],
        &geo_inputs,
        params,
    );
    let mut h = Fnv1a::new();
    feed(&mut h, geology.province.as_slice(), u16::to_le_bytes);
    feed(&mut h, &geology.settings, |v| [v as u8]);
    feed(&mut h, &geology.layer_count, |v| [v]);
    for layer in &geology.layers {
        feed(&mut h, layer, |v| [v as u8]);
    }
    feed(&mut h, geology.soil.as_slice(), |v| [v]);
    layer_hashes[LayerStage::Geology as usize] = h.finish();

    let deposits = deposit::make_deposits(
        layer_seeds[LayerStage::Deposits as usize],
        &geology,
        &biome,
        params,
    );
    let mut h = Fnv1a::new();
    feed(&mut h, deposits.mineral.as_slice(), |v| [v]);
    feed(&mut h, deposits.richness.as_slice(), |v| [v]);
    feed(&mut h, deposits.host.as_slice(), |v| [v]);
    layer_hashes[LayerStage::Deposits as usize] = h.finish();

    let mut region_seeds = [0u64; REGION_STAGE_COUNT];
    for stage in RegionStage::ALL {
        region_seeds[stage as usize] = mix(seed ^ stage.tag());
    }
    let region_inputs = RegionInputs {
        height: &height,
        biome: &biome,
        river: &rivers.river,
        geology: &geology,
        deposits: &deposits,
    };
    let regions = region::make_regions(
        region_seeds[RegionStage::Regions as usize],
        &region_inputs,
        params,
    );
    let mut h = Fnv1a::new();
    feed(&mut h, regions.id.as_slice(), u16::to_le_bytes);
    for info in &regions.info {
        feed(&mut h, &[info.size], u32::to_le_bytes);
        feed(&mut h, &[info.cx, info.cy], u16::to_le_bytes);
        h.write(&[info.minerals]);
    }
    for e in &regions.edges {
        feed(&mut h, &[e.a, e.b], u16::to_le_bytes);
        feed(&mut h, &[e.border, e.cost], u32::to_le_bytes);
    }
    feed(&mut h, regions.deposits.mineral.as_slice(), |v| [v]);
    feed(&mut h, regions.deposits.richness.as_slice(), |v| [v]);
    feed(&mut h, regions.deposits.host.as_slice(), |v| [v]);
    let region_hashes = [h.finish()];

    WorldData {
        seed,
        stage_seeds,
        stage_hashes: hashes,
        layer_seeds,
        layer_hashes,
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
        geology,
        deposits,
        region_seeds,
        region_hashes,
        regions,
    }
}

/// Make a world-size grid from a row-major vector. A wrong length gives a grid of the fill value.
pub(crate) fn world_grid<T: Clone>(cells: Vec<T>, fill: T) -> Grid<T> {
    match Grid::from_vec(WORLD_SIZE, WORLD_SIZE, cells) {
        Some(grid) => grid,
        None => Grid::new(WORLD_SIZE, WORLD_SIZE, fill),
    }
}
