//! Local map generation (TECHNICAL_DESIGN §8.2): layers of enums, no content strings.
//! The game maps the enums to `materials.ron` entries.

use crate::embark::{EmbarkSites, EmbarkSummary, embark_rules};
use crate::noise::{fbm, threshold_for_share};
use crate::stamp::{PlacedSite, SiteTile, stamp};
use base::Grid;
use base::hash::Fnv1a;
use base::rng::mix;

const TAG_MOUNTAIN: u64 = 0x4c4f_4341_4c5f_4d54;
const TAG_WATER: u64 = 0x4c4f_4341_4c5f_5754;
const TAG_SAND: u64 = 0x4c4f_4341_4c5f_5344;
const TAG_CLAY: u64 = 0x4c4f_4341_4c5f_434c;
const TAG_ORE: u64 = 0x4c4f_4341_4c5f_4f52;
const TAG_TREE: u64 = 0x4c4f_4341_4c5f_5452;
const TAG_TREE_ROLL: u64 = 0x4c4f_4341_4c52_4f4c;

/// Ground layer (the surface of a tile).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Ground {
    /// Fertile soil.
    Soil = 0,
    /// Sand.
    Sand = 1,
    /// Clay.
    Clay = 2,
    /// Water.
    Water = 3,
    /// Bare stone under a rock wall.
    Stone = 4,
}

/// Rock layer: the solid rock wall material of a tile.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum RockKind {
    /// No rock wall here.
    None = 0,
    /// Sandstone (outer mountain).
    Sandstone = 1,
    /// Limestone (middle mountain).
    Limestone = 2,
    /// Granite (deep mountain).
    Granite = 3,
}

/// Ore layer: veins in rock.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Ore {
    /// No ore.
    None = 0,
    /// Coal.
    Coal = 1,
    /// Copper ore.
    Copper = 2,
    /// Iron ore.
    Iron = 3,
}

/// Output of local generation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LocalMapData {
    /// Seed that made the map.
    pub seed: u64,
    /// Ground layer.
    pub ground: Grid<Ground>,
    /// Rock wall layer. A tile with a rock wall has `Ground::Stone`.
    pub rock: Grid<RockKind>,
    /// Ore layer. Ore is only inside rock walls.
    pub ore: Grid<Ore>,
    /// True where a tree stands.
    pub tree: Grid<bool>,
    /// Structure layer: buildings, rubble, tunnels, seals, and debris (S11).
    pub site: Grid<SiteTile>,
    /// True where an old road runs (S11).
    pub road: Grid<bool>,
    /// Sites that were placed, in placement order (S11).
    pub placed: Vec<PlacedSite>,
    /// Tiles with salvage in ruins (S11).
    pub salvage: Vec<(u16, u16)>,
}

impl LocalMapData {
    /// Stable 64-bit hash (FNV-1a) of the size and all layers.
    pub fn hash(&self) -> u64 {
        let mut h = Fnv1a::new();
        h.write_u32(self.ground.width());
        h.write_u32(self.ground.height());
        let bytes: Vec<u8> = self.ground.as_slice().iter().map(|&g| g as u8).collect();
        h.write(&bytes);
        let bytes: Vec<u8> = self.rock.as_slice().iter().map(|&r| r as u8).collect();
        h.write(&bytes);
        let bytes: Vec<u8> = self.ore.as_slice().iter().map(|&o| o as u8).collect();
        h.write(&bytes);
        let bytes: Vec<u8> = self.tree.as_slice().iter().map(|&t| u8::from(t)).collect();
        h.write(&bytes);
        h.finish()
    }
}

/// Sample an fBm field over the whole map in row-major order.
fn field(seed: u64, w: u32, h: u32, cell: u32, octaves: u32) -> Vec<u16> {
    let mut v = Vec::with_capacity(w as usize * h as usize);
    for y in 0..h as i32 {
        for x in 0..w as i32 {
            v.push(fbm(seed, x, y, cell, octaves));
        }
    }
    v
}

/// Make a local map from an embark summary. Same summary gives the same map.
pub fn generate_local(embark: &EmbarkSummary) -> LocalMapData {
    let (w, h) = (embark.width, embark.height);
    let len = w as usize * h as usize;
    let stage = |tag: u64| mix(embark.seed ^ tag);

    // Mountain: the highest part of a noise field is solid rock.
    let mountain = field(stage(TAG_MOUNTAIN), w, h, 64, 5);
    let m_share = u32::from(embark.mountain_amount.min(1000));
    let m_thr = threshold_for_share(&mountain, 1000 - m_share);
    let m_span = (65536 - m_thr).max(1);

    // Water: the lowest part of another field, outside rock.
    let wet = field(stage(TAG_WATER), w, h, 40, 4);
    let w_thr = threshold_for_share(&wet, u32::from(embark.water_amount));

    let sand = field(stage(TAG_SAND), w, h, 48, 3);
    let s_thr = threshold_for_share(&sand, 1000 - u32::from(embark.sand_amount.min(1000)));
    let clay = field(stage(TAG_CLAY), w, h, 24, 3);
    let c_thr = threshold_for_share(&clay, 1000 - u32::from(embark.clay_amount.min(1000)));
    let grove = field(stage(TAG_TREE), w, h, 24, 3);

    let mut ground = Grid::new(w, h, Ground::Soil);
    let mut rock = Grid::new(w, h, RockKind::None);
    let mut ore = Grid::new(w, h, Ore::None);
    let mut tree = Grid::new(w, h, false);

    // Ore: ridged noise gives thin veins. Richer ore gives wider veins.
    let vein_width = 400 + 500 * u32::from(embark.ore_richness.min(3));
    let ore_stage = stage(TAG_ORE);
    let tree_roll_seed = stage(TAG_TREE_ROLL);

    for i in 0..len {
        let (x, y) = ((i % w as usize) as i32, (i / w as usize) as i32);
        let m = u32::from(mountain[i]);
        let g = if m >= m_thr {
            let depth = (m - m_thr) * 3 / m_span;
            let kind = match depth {
                0 => RockKind::Sandstone,
                1 => RockKind::Limestone,
                _ => RockKind::Granite,
            };
            rock.set(x, y, kind);
            if embark.ore_richness > 0 {
                let candidates = match kind {
                    RockKind::Sandstone => [Ore::Coal, Ore::Copper],
                    RockKind::Limestone => [Ore::Coal, Ore::Iron],
                    _ => [Ore::Copper, Ore::Iron],
                };
                for (n, candidate) in candidates.into_iter().enumerate() {
                    let v = fbm(mix(ore_stage ^ u64::from(candidate as u8)), x, y, 14, 2);
                    if u32::from(v.abs_diff(32768)) < vein_width + n as u32 * 100 {
                        ore.set(x, y, candidate);
                        break;
                    }
                }
            }
            Ground::Stone
        } else if u32::from(wet[i]) < w_thr {
            Ground::Water
        } else if u32::from(sand[i]) >= s_thr {
            Ground::Sand
        } else if u32::from(clay[i]) >= c_thr {
            Ground::Clay
        } else {
            Ground::Soil
        };
        ground.set(x, y, g);

        // Trees: groves from noise (mean 1.0 at 32768), single tiles from a hash roll.
        if matches!(g, Ground::Soil | Ground::Clay) {
            let chance = u64::from(embark.tree_density) * u64::from(grove[i]) / 32768;
            let key = (u64::from(x as u32) << 32) | u64::from(y as u32);
            if mix(tree_roll_seed ^ key) % 1000 < chance {
                tree.set(x, y, true);
            }
        }
    }

    LocalMapData {
        seed: embark.seed,
        ground,
        rock,
        ore,
        tree,
        site: Grid::new(w, h, SiteTile::None),
        road: Grid::new(w, h, false),
        placed: Vec::new(),
        salvage: Vec::new(),
    }
}

impl LocalMapData {
    /// Stable hash of the history layers: structure, roads, placed sites, and salvage.
    /// `hash()` does not include them, so maps without history keep their hash.
    pub fn site_hash(&self) -> u64 {
        let mut h = Fnv1a::new();
        let bytes: Vec<u8> = self.site.as_slice().iter().map(|&s| s as u8).collect();
        h.write(&bytes);
        let bytes: Vec<u8> = self.road.as_slice().iter().map(|&r| u8::from(r)).collect();
        h.write(&bytes);
        for p in &self.placed {
            h.write_u32(p.id);
            h.write(&[p.kind as u8, p.width, p.height, u8::from(p.fallback)]);
            h.write(&p.x.to_le_bytes());
            h.write(&p.y.to_le_bytes());
            h.write_u32(p.age);
        }
        for &(x, y) in &self.salvage {
            h.write(&x.to_le_bytes());
            h.write(&y.to_le_bytes());
        }
        h.finish()
    }
}

/// Make a local map and stamp the history of the embark area on it (R-041). The ground,
/// rock, ore, and tree layers come from `generate_local`, then ore loses depletion, roads and
/// sites are stamped. A map always has at least 1 site: if `sites` has none, a small ruin is
/// added. Same input gives the same map.
pub fn generate_local_with_sites(embark: &EmbarkSummary, sites: &EmbarkSites) -> LocalMapData {
    let mut map = generate_local(embark);
    if let Ok(rules) = embark_rules() {
        stamp(&mut map, sites, rules);
    }
    map
}
