//! Realms (S26, R-051, R-052, D-073, `INFINITE_WORLD.md` option B).
//!
//! The infinite world is a plane of realms. A realm is one finite world of 256 x 256 cells
//! with its own sub-seed. The edge of each realm is ocean, so a strait lies between two
//! realms. Realm (0, 0) uses the seed itself, so the world of a seed does not change.

use crate::biome::WorldBiome;
use crate::world::{WORLD_SIZE, WorldData, WorldParams, generate_world};
use base::rng::mix;
use std::sync::Arc;

/// Version of the world generator. Raise it when a world step changes its output. A save keeps
/// the version, and the game refuses a save with another version.
pub const WORLD_GEN_VERSION: u32 = 1;

/// Tag of the realm sub-seeds.
const TAG_REALM: u64 = 0x5245_414c_4d5f_5332;

/// The position of a realm on the plane.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RealmPos {
    /// Realm column. East is plus.
    pub rx: i32,
    /// Realm row. South is plus.
    pub ry: i32,
}

/// The sub-seed of a realm. Realm (0, 0) has the seed itself.
pub fn realm_seed(seed: u64, pos: RealmPos) -> u64 {
    if pos.rx == 0 && pos.ry == 0 {
        return seed;
    }
    let x = u64::from(pos.rx as u32);
    let y = u64::from(pos.ry as u32);
    mix(seed ^ TAG_REALM ^ mix(x ^ (y << 32)))
}

/// Make the world of a realm.
pub fn generate_realm(seed: u64, pos: RealmPos, params: &WorldParams) -> WorldData {
    generate_world(realm_seed(seed, pos), params)
}

/// The 8 neighbors of a realm: north, north-east, east, south-east, south, south-west, west,
/// north-west.
pub fn realm_neighbors(pos: RealmPos) -> [RealmPos; 8] {
    const STEPS: [(i32, i32); 8] = [
        (0, -1),
        (1, -1),
        (1, 0),
        (1, 1),
        (0, 1),
        (-1, 1),
        (-1, 0),
        (-1, -1),
    ];
    STEPS.map(|(dx, dy)| RealmPos {
        rx: pos.rx.saturating_add(dx),
        ry: pos.ry.saturating_add(dy),
    })
}

/// Distance of two realms in realm steps (a step may go diagonal). This is the travel cost
/// of an expedition.
pub fn realm_distance(a: RealmPos, b: RealmPos) -> u32 {
    let dx = i64::from(a.rx) - i64::from(b.rx);
    let dy = i64::from(a.ry) - i64::from(b.ry);
    u32::try_from(dx.abs().max(dy.abs())).unwrap_or(u32::MAX)
}

/// What the plane map shows for a realm.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RealmSummary {
    /// The realm.
    pub pos: RealmPos,
    /// The sub-seed of the realm.
    pub seed: u64,
    /// Land share in per-mille.
    pub land_per_mille: u32,
    /// Cells of each biome, indexed by `WorldBiome as usize`.
    pub biome_cells: [u32; WorldBiome::COUNT],
    /// Cells with a river.
    pub river_cells: u32,
    /// Hash of the last world step. Two equal worlds have equal hashes.
    pub hash: u64,
}

/// Count the cells of a realm for the plane map.
pub fn summarize(pos: RealmPos, world: &WorldData) -> RealmSummary {
    let mut biome_cells = [0u32; WorldBiome::COUNT];
    for &b in world.biome.as_slice() {
        biome_cells[b as usize] += 1;
    }
    let river_cells = world.river.as_slice().iter().filter(|&&r| r > 0).count();
    RealmSummary {
        pos,
        seed: world.seed,
        land_per_mille: world.land_per_mille(),
        biome_cells,
        river_cells: u32::try_from(river_cells).unwrap_or(u32::MAX),
        hash: world.region_hashes[0],
    }
}

/// A cache that holds a few realms. The oldest unused realm goes out first.
pub struct RealmCache {
    seed: u64,
    params: WorldParams,
    capacity: usize,
    entries: Vec<(RealmPos, Arc<WorldData>, u64)>,
    clock: u64,
    hits: u32,
    misses: u32,
}

impl RealmCache {
    /// A cache for the realms of a seed. It holds `capacity` realms (at least 1).
    pub fn new(seed: u64, params: WorldParams, capacity: usize) -> Self {
        Self {
            seed,
            params,
            capacity: capacity.max(1),
            entries: Vec::new(),
            clock: 0,
            hits: 0,
            misses: 0,
        }
    }

    /// The world of a realm. A missing realm is made now.
    pub fn get(&mut self, pos: RealmPos) -> Arc<WorldData> {
        self.clock += 1;
        if let Some(e) = self.entries.iter_mut().find(|e| e.0 == pos) {
            e.2 = self.clock;
            self.hits += 1;
            return Arc::clone(&e.1);
        }
        self.misses += 1;
        let world = Arc::new(generate_realm(self.seed, pos, &self.params));
        if self.entries.len() >= self.capacity
            && let Some(oldest) = (0..self.entries.len()).min_by_key(|&i| self.entries[i].2)
        {
            self.entries.remove(oldest);
        }
        self.entries.push((pos, Arc::clone(&world), self.clock));
        world
    }

    /// True if the realm is in the cache.
    pub fn contains(&self, pos: RealmPos) -> bool {
        self.entries.iter().any(|e| e.0 == pos)
    }

    /// Make the first realm of `wanted` that is not in the cache and return it. `None` if all
    /// are there. The game calls this once between frames, so that no frame makes two realms.
    pub fn generate_next(&mut self, wanted: &[RealmPos]) -> Option<RealmPos> {
        let missing = wanted.iter().copied().find(|&p| !self.contains(p))?;
        self.get(missing);
        Some(missing)
    }

    /// Realms in the cache.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// True if the cache holds no realm.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Requests that the cache answered from memory.
    pub fn hits(&self) -> u32 {
        self.hits
    }

    /// Requests that made a realm.
    pub fn misses(&self) -> u32 {
        self.misses
    }
}

/// Cells of one side of a realm.
pub const REALM_SIZE: u32 = WORLD_SIZE;
