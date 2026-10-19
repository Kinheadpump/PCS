//! Step 11: deposits. A stateless hash roll for each cell and mineral, weighted by host rock.

use crate::biome::WorldBiome;
use crate::geology::{Geology, Rock, roll};
use crate::world::{WORLD_SIZE, WorldParams, world_grid};
use base::Grid;

/// Minerals. The order is stable: it feeds hashes and the affinity table.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Mineral {
    /// Fuel.
    Coal,
    /// Iron ore.
    Iron,
    /// Copper ore.
    Copper,
    /// Tin ore.
    Tin,
    /// Gold ore.
    Gold,
    /// Gem stones.
    Gems,
}

impl Mineral {
    /// Number of minerals.
    pub const COUNT: usize = 6;
    /// All minerals in index order.
    pub const ALL: [Mineral; Mineral::COUNT] = [
        Mineral::Coal,
        Mineral::Iron,
        Mineral::Copper,
        Mineral::Tin,
        Mineral::Gold,
        Mineral::Gems,
    ];

    /// Short stable name.
    pub fn name(self) -> &'static str {
        const NAMES: [&str; Mineral::COUNT] = ["coal", "iron", "copper", "tin", "gold", "gems"];
        NAMES[self as usize]
    }
}

/// Affinity of each mineral (columns, Mineral order) to each rock (rows, Rock order), 0 to 100.
#[rustfmt::skip]
const AFFINITY: [[u8; Mineral::COUNT]; Rock::COUNT] = [
    [30, 20, 10,  0,  0,  0], // sandstone
    [60, 10,  0,  0,  0,  0], // shale
    [20, 40, 20,  0,  0,  5], // limestone
    [ 0, 20, 30, 60, 30, 10], // granite
    [ 0, 30, 30, 30, 20, 20], // gneiss
    [ 0,  0, 10,  0, 30, 50], // marble
    [ 0, 20, 50,  0, 10, 20], // basalt
    [ 0, 10, 30,  0, 20, 40], // tuff
];

/// Affinity of a mineral to a host rock, 0 to 100.
pub fn affinity(rock: Rock, mineral: Mineral) -> u8 {
    AFFINITY[rock as usize][mineral as usize]
}

/// Output of step 11. Each cell holds at most 1 deposit.
#[derive(Clone, Debug)]
pub struct Deposits {
    /// Mineral index plus 1 (0 for no deposit).
    pub mineral: Grid<u8>,
    /// Richness 1 to 3 (0 for no deposit).
    pub richness: Grid<u8>,
    /// Index of the host rock layer (0 is the top layer).
    pub host: Grid<u8>,
}

impl Deposits {
    /// Mineral and richness of the deposit at a cell, if any.
    pub fn at(&self, x: i32, y: i32) -> Option<(Mineral, u8)> {
        let m = usize::from(*self.mineral.get(x, y)?);
        let richness = *self.richness.get(x, y)?;
        if m == 0 {
            return None;
        }
        Mineral::ALL.get(m - 1).map(|&mineral| (mineral, richness))
    }

    /// Number of deposit cells.
    pub fn count(&self) -> usize {
        self.mineral.as_slice().iter().filter(|&&m| m != 0).count()
    }
}

/// Pick the best mineral hit at a cell: the largest margin of probability over the roll.
/// Ties go to the lower mineral index.
fn best_hit(seed: u64, cell: usize, rocks: &[Rock], params: &WorldParams) -> Option<usize> {
    let mut best: Option<(i64, usize)> = None;
    for m in Mineral::ALL {
        let total: u32 = rocks.iter().map(|&r| u32::from(affinity(r, m))).sum();
        if total == 0 {
            continue;
        }
        let p = i64::from(params.deposit_base_per_10k) * i64::from(total) / 100;
        let r = (roll(seed, cell as u64, m as u64) % 10_000) as i64;
        if r < p && best.is_none_or(|(b, _)| p - r > b) {
            best = Some((p - r, m as usize));
        }
    }
    best.map(|(_, m)| m)
}

/// Make the deposits on land cells.
pub fn make_deposits(
    seed: u64,
    geology: &Geology,
    biome: &Grid<WorldBiome>,
    params: &WorldParams,
) -> Deposits {
    let size = WORLD_SIZE as usize;
    let mut mineral = vec![0u8; size * size];
    let mut richness = vec![0u8; size * size];
    let mut host = vec![0u8; size * size];
    for i in 0..size * size {
        let b = biome.as_slice()[i];
        if b == WorldBiome::Ocean || b == WorldBiome::Lake {
            continue;
        }
        let id = usize::from(geology.province.as_slice()[i]);
        let count = usize::from(geology.layer_count[id]);
        let rocks = &geology.layers[id][..count];
        let Some(m) = best_hit(seed, i, rocks, params) else {
            continue;
        };
        let mn = Mineral::ALL[m];
        let mut host_layer = 0usize;
        for (k, &r) in rocks.iter().enumerate() {
            if affinity(r, mn) > affinity(rocks[host_layer], mn) {
                host_layer = k;
            }
        }
        let v = roll(seed, i as u64, 0x100 + m as u64) % 1000;
        let rich3 = u64::from(params.rich3_per_mille);
        let rich2 = u64::from(params.rich2_per_mille);
        mineral[i] = m as u8 + 1;
        richness[i] = if v < rich3 {
            3
        } else if v < rich3 + rich2 {
            2
        } else {
            1
        };
        host[i] = host_layer as u8;
    }
    Deposits {
        mineral: world_grid(mineral, 0),
        richness: world_grid(richness, 0),
        host: world_grid(host, 0),
    }
}
