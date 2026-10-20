//! Step 9: biomes from a temperature and moisture table, with overrides.

use crate::hydro::DIRS;
use crate::world::{WORLD_SIZE, WorldParams, world_grid};
use base::Grid;

/// Biome of a world cell.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum WorldBiome {
    /// Open sea (including inland seas).
    Ocean,
    /// Lake.
    Lake,
    /// Permanent ice.
    Glacier,
    /// Bare high mountains.
    HighMountain,
    /// Volcanic land.
    Volcanic,
    /// Beach and low coast.
    Coast,
    /// Low land along a river.
    Floodplain,
    /// Cold, treeless plain.
    Tundra,
    /// Cold and dry.
    ColdDesert,
    /// Cold conifer forest.
    Taiga,
    /// Dry grass plain.
    Steppe,
    /// Dry bushes.
    Shrubland,
    /// Mild grass plain.
    Grassland,
    /// Mild forest.
    TemperateForest,
    /// Wet low land.
    Marsh,
    /// Hot and dry.
    Desert,
    /// Hot grass plain with trees.
    Savanna,
    /// Hot and wet forest.
    TropicalForest,
    /// Hot and very wet jungle.
    Rainforest,
}

impl WorldBiome {
    /// Number of biomes.
    pub const COUNT: usize = 19;

    /// All biomes in order.
    pub const ALL: [WorldBiome; WorldBiome::COUNT] = [
        B::Ocean,
        B::Lake,
        B::Glacier,
        B::HighMountain,
        B::Volcanic,
        B::Coast,
        B::Floodplain,
        B::Tundra,
        B::ColdDesert,
        B::Taiga,
        B::Steppe,
        B::Shrubland,
        B::Grassland,
        B::TemperateForest,
        B::Marsh,
        B::Desert,
        B::Savanna,
        B::TropicalForest,
        B::Rainforest,
    ];

    /// Name for tools and the world map.
    pub fn name(self) -> &'static str {
        match self {
            WorldBiome::Ocean => "ocean",
            WorldBiome::Lake => "lake",
            WorldBiome::Glacier => "glacier",
            WorldBiome::HighMountain => "high mountain",
            WorldBiome::Volcanic => "volcanic",
            WorldBiome::Coast => "coast",
            WorldBiome::Floodplain => "floodplain",
            WorldBiome::Tundra => "tundra",
            WorldBiome::ColdDesert => "cold desert",
            WorldBiome::Taiga => "taiga",
            WorldBiome::Steppe => "steppe",
            WorldBiome::Shrubland => "shrubland",
            WorldBiome::Grassland => "grassland",
            WorldBiome::TemperateForest => "temperate forest",
            WorldBiome::Marsh => "marsh",
            WorldBiome::Desert => "desert",
            WorldBiome::Savanna => "savanna",
            WorldBiome::TropicalForest => "tropical forest",
            WorldBiome::Rainforest => "rainforest",
        }
    }
}

use WorldBiome as B;

/// Table of 6 temperature bands (cold to hot) by 6 moisture bands (dry to wet).
const TABLE: [[WorldBiome; 6]; 6] = [
    [
        B::ColdDesert,
        B::Tundra,
        B::Tundra,
        B::Tundra,
        B::Tundra,
        B::Tundra,
    ],
    [
        B::ColdDesert,
        B::Tundra,
        B::Taiga,
        B::Taiga,
        B::Taiga,
        B::Taiga,
    ],
    [
        B::ColdDesert,
        B::Steppe,
        B::Steppe,
        B::TemperateForest,
        B::TemperateForest,
        B::Marsh,
    ],
    [
        B::ColdDesert,
        B::Steppe,
        B::TemperateForest,
        B::TemperateForest,
        B::TemperateForest,
        B::Marsh,
    ],
    [
        B::Desert,
        B::Shrubland,
        B::Grassland,
        B::Savanna,
        B::TropicalForest,
        B::Rainforest,
    ],
    [
        B::Desert,
        B::Desert,
        B::Savanna,
        B::Savanna,
        B::TropicalForest,
        B::Rainforest,
    ],
];

/// Upper limits of the temperature bands in degrees Celsius (the last band is open).
const TEMP_LIMITS: [i32; 5] = [-10, 0, 9, 18, 25];

/// Upper limits of the moisture bands (the last band is open).
const MOISTURE_LIMITS: [i32; 5] = [30, 60, 95, 135, 180];

/// Height above sea level below which a cell next to the ocean is coast.
const COAST_MAX_HEIGHT: i32 = 80;

/// Outputs of earlier stages that the biome step reads.
pub struct BiomeInputs<'a> {
    /// Height.
    pub height: &'a Grid<i16>,
    /// Sea level.
    pub sea_level: i16,
    /// Lake IDs.
    pub lake: &'a Grid<u16>,
    /// Mean temperature.
    pub temperature: &'a Grid<i8>,
    /// Moisture.
    pub moisture: &'a Grid<u8>,
    /// River sizes.
    pub river: &'a Grid<u8>,
    /// Volcanic flags.
    pub volcanic: &'a Grid<bool>,
}

fn band(value: i32, limits: &[i32; 5]) -> usize {
    limits.iter().filter(|&&l| value >= l).count()
}

/// Biome of one land cell from the table, before the overrides.
pub fn table_biome(temperature: i32, moisture: i32) -> WorldBiome {
    TABLE[band(temperature, &TEMP_LIMITS)][band(moisture, &MOISTURE_LIMITS)]
}

/// Make the biome of each cell. Overrides in order: ocean, lake, glacier, high mountain,
/// volcanic, coast, floodplain. Then the table.
pub fn make_biomes(inputs: &BiomeInputs, params: &WorldParams) -> Grid<WorldBiome> {
    let size = WORLD_SIZE as i32;
    let sea = i32::from(inputs.sea_level);
    let height = inputs.height.as_slice();
    let mut out = Vec::with_capacity(height.len());
    for y in 0..size {
        for x in 0..size {
            let i = (y * size + x) as usize;
            let rel = i32::from(height[i]) - sea;
            let temperature = i32::from(inputs.temperature.as_slice()[i]);
            let moisture = i32::from(inputs.moisture.as_slice()[i]);
            let touches_ocean = DIRS.iter().any(|&(dx, dy)| {
                inputs
                    .height
                    .get(x + dx, y + dy)
                    .is_some_and(|&h| i32::from(h) < sea)
            });
            let biome = if rel < 0 {
                B::Ocean
            } else if inputs.lake.as_slice()[i] != 0 {
                B::Lake
            } else if temperature <= i32::from(params.glacier_max_temp) {
                B::Glacier
            } else if inputs.volcanic.as_slice()[i] {
                B::Volcanic
            } else if rel >= i32::from(params.mountain_min) {
                B::HighMountain
            } else if touches_ocean && rel < COAST_MAX_HEIGHT {
                B::Coast
            } else if inputs.river.as_slice()[i] >= 1 && rel < i32::from(params.floodplain_max) {
                B::Floodplain
            } else {
                table_biome(temperature, moisture)
            };
            out.push(biome);
        }
    }
    world_grid(out, B::Ocean)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_corners() {
        assert_eq!(table_biome(-30, 0), B::ColdDesert);
        assert_eq!(table_biome(30, 255), B::Rainforest);
        assert_eq!(table_biome(30, 0), B::Desert);
        assert_eq!(table_biome(12, 130), B::TemperateForest);
    }

    #[test]
    fn biome_count_matches_last_variant() {
        assert_eq!(B::Rainforest as usize + 1, WorldBiome::COUNT);
    }
}
