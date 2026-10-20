//! D-046: biome balance. Temperate forest is a common biome and no biome dominates the land.

use worldgen::{WorldBiome, WorldParams, generate_world};

/// Share of each biome in per-mille of land cells.
fn land_shares(seed: u64) -> [u32; WorldBiome::COUNT] {
    let w = generate_world(seed, &WorldParams::default());
    let mut counts = [0u32; WorldBiome::COUNT];
    let mut land = 0u32;
    for &b in w.biome.as_slice() {
        if !matches!(b, WorldBiome::Ocean | WorldBiome::Lake) {
            counts[b as usize] += 1;
            land += 1;
        }
    }
    counts.map(|c| c * 1000 / land.max(1))
}

/// D-046: temperate forest is at least 8% of the land (it was 1 to 3%), and no biome is more
/// than 30% of the land, on each of 12 seeds.
#[test]
fn d046_temperate_forest_is_common_and_no_biome_dominates() {
    for seed in 1..=12u64 {
        let s = land_shares(seed);
        let forest = s[WorldBiome::TemperateForest as usize];
        assert!(
            forest >= 80,
            "seed {seed}: temperate forest {forest} per mille"
        );
        for b in WorldBiome::ALL {
            assert!(
                s[b as usize] <= 300,
                "seed {seed}: {} is {} per mille of land",
                b.name(),
                s[b as usize]
            );
        }
    }
}
