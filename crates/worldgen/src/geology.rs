//! Step 10: geology. Provinces with a setting, 2 or 3 rock layers, and soil depth per cell.

use crate::biome::WorldBiome;
use crate::deposit::Mineral;
use crate::world::{WORLD_SIZE, WorldParams, world_grid};
use base::Grid;
use base::content::{ContentError, ContentIssue, load_rocks};
use base::rng::mix;
use std::sync::OnceLock;

/// Rock types. The order is stable: it feeds hashes and the affinity table.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Rock {
    /// Sedimentary, sand grains.
    Sandstone,
    /// Sedimentary, clay.
    Shale,
    /// Sedimentary, calcium.
    Limestone,
    /// Igneous, deep.
    Granite,
    /// Metamorphic, banded.
    Gneiss,
    /// Metamorphic limestone.
    Marble,
    /// Igneous, surface lava.
    Basalt,
    /// Compressed volcanic ash.
    Tuff,
}

impl Rock {
    /// Number of rock types.
    pub const COUNT: usize = 8;
    /// All rock types in index order.
    pub const ALL: [Rock; Rock::COUNT] = [
        Rock::Sandstone,
        Rock::Shale,
        Rock::Limestone,
        Rock::Granite,
        Rock::Gneiss,
        Rock::Marble,
        Rock::Basalt,
        Rock::Tuff,
    ];

    /// Short stable name.
    pub fn name(self) -> &'static str {
        const NAMES: [&str; Rock::COUNT] = [
            "sandstone",
            "shale",
            "limestone",
            "granite",
            "gneiss",
            "marble",
            "basalt",
            "tuff",
        ];
        NAMES[self as usize]
    }
}

/// Geological setting of a province.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Setting {
    /// Low inland land: sedimentary rock.
    Plain,
    /// Mountains: igneous and metamorphic rock.
    Mountain,
    /// Volcanic land: lava and ash rock.
    Volcanic,
    /// Coast: limestone and sandstone.
    Coast,
}

impl Setting {
    /// All settings in index order.
    pub const ALL: [Setting; 4] = [
        Setting::Plain,
        Setting::Mountain,
        Setting::Volcanic,
        Setting::Coast,
    ];
    /// Short stable name.
    pub fn name(self) -> &'static str {
        const SETTING_NAMES: [&str; 4] = ["plain", "mountain", "volcanic", "coast"];
        SETTING_NAMES[self as usize]
    }
}

const ROCKS_RON: &str = include_str!("../../../assets/content/rocks.ron");
const DEPOSITS_RON: &str = include_str!("../../../assets/content/deposits.ron");

/// Game tables from rocks.ron and deposits.ron. Parsed once, then read only.
#[derive(Debug)]
pub struct Tables {
    pools: [[Rock; 4]; 4],
    affinity: [[u8; Mineral::COUNT]; Rock::COUNT],
}

static TABLES: OnceLock<Result<Tables, ContentError>> = OnceLock::new();

impl Tables {
    /// The 4 rock types that can appear in a province of a setting.
    pub fn pool(&self, setting: Setting) -> [Rock; 4] {
        self.pools
            .get(setting as usize)
            .copied()
            .unwrap_or([Rock::Sandstone; 4])
    }

    /// Affinity of a mineral to a host rock, 0 to 100.
    pub fn affinity(&self, rock: Rock, mineral: Mineral) -> u8 {
        let row = self.affinity.get(rock as usize);
        row.and_then(|r| r.get(mineral as usize))
            .copied()
            .unwrap_or(0)
    }
}

fn issue(entry: &str, message: String) -> ContentIssue {
    ContentIssue {
        entry: Some(entry.to_string()),
        message,
    }
}

fn build_tables() -> Result<Tables, ContentError> {
    let table = load_rocks("rocks.ron", ROCKS_RON)?;
    let mut issues = Vec::new();
    let names = Rock::ALL.iter().map(|r| r.name());
    if !table.rocks.iter().map(String::as_str).eq(names) {
        issues.push(issue(
            "rocks",
            "must match the Rock type, in order".to_string(),
        ));
    }
    let names = Setting::ALL.iter().map(|s| s.name());
    if !table.settings.iter().map(|s| s.key.as_str()).eq(names) {
        issues.push(issue(
            "settings",
            "must match the Setting type, in order".to_string(),
        ));
    }
    let mut pools = [[Rock::Sandstone; 4]; 4];
    for (def, out) in table.settings.iter().zip(pools.iter_mut()) {
        let found = def.pool.iter().filter_map(|&i| Rock::ALL.get(i));
        let rocks: Vec<Rock> = found.copied().collect();
        let pool: Result<[Rock; 4], _> = rocks.try_into();
        if let Ok(pool) = pool {
            *out = pool;
        } else {
            issues.push(issue(&def.key, "pool must hold 4 rocks".to_string()));
        }
    }
    if !issues.is_empty() {
        let file = "rocks.ron".to_string();
        return Err(ContentError { file, issues });
    }
    let affinity = crate::deposit::load_affinity(&table, DEPOSITS_RON)?;
    Ok(Tables { pools, affinity })
}

/// The game tables. Parsed from the embedded files on the first call.
/// A bad embedded file gives an error. World generation start must check this.
pub fn tables() -> Result<&'static Tables, &'static ContentError> {
    TABLES.get_or_init(build_tables).as_ref()
}

/// Check the embedded rock and mineral tables. Call it at world generation start.
pub fn check_tables() -> Result<(), ContentError> {
    tables().map(|_| ()).map_err(Clone::clone)
}

/// Output of step 10.
#[derive(Clone, Debug)]
pub struct Geology {
    /// Province ID of each cell.
    pub province: Grid<u16>,
    /// Setting of each province, indexed by province ID.
    pub settings: Vec<Setting>,
    /// Number of rock layers (2 or 3) of each province.
    pub layer_count: Vec<u8>,
    /// Rock layers of each province from the top. Entries at or above layer_count repeat the last layer.
    pub layers: Vec<[Rock; 3]>,
    /// Soil depth of each cell in decimeters (0 on water and ice).
    pub soil: Grid<u8>,
}

impl Geology {
    /// Rock of a layer (0 is the top layer) at a cell. None outside the map or above the layer count.
    pub fn rock_at(&self, x: i32, y: i32, layer: usize) -> Option<Rock> {
        let id = usize::from(*self.province.get(x, y)?);
        if layer >= usize::from(*self.layer_count.get(id)?) {
            return None;
        }
        self.layers.get(id).map(|l| l[layer])
    }

    /// Setting of the province at a cell.
    pub fn setting_at(&self, x: i32, y: i32) -> Option<Setting> {
        let id = usize::from(*self.province.get(x, y)?);
        self.settings.get(id).copied()
    }
}

/// Stateless hash of a seed and two integers.
pub(crate) fn roll(seed: u64, a: u64, b: u64) -> u64 {
    mix(mix(seed ^ a.wrapping_mul(0x9E37_79B9_7F4A_7C15)) ^ b.wrapping_mul(0xC2B2_AE3D_27D4_EB4F))
}

/// Inputs of the geology step. Only outputs of earlier stages.
pub struct GeoInputs<'a> {
    /// Height.
    pub height: &'a Grid<i16>,
    /// Sea level.
    pub sea_level: i16,
    /// Moisture.
    pub moisture: &'a Grid<u8>,
    /// River size.
    pub river: &'a Grid<u8>,
    /// Biome.
    pub biome: &'a Grid<WorldBiome>,
}

/// Lattice cells per side for a province cell size.
fn lattice(params: &WorldParams) -> i32 {
    let c = params.province_cell.max(4) as i32;
    (WORLD_SIZE as i32 + c - 1) / c
}

/// Build the province grid: jittered lattice points, nearest point wins, ties go to the lower ID.
fn make_provinces(seed: u64, params: &WorldParams) -> Grid<u16> {
    let c = params.province_cell.max(4) as i32;
    let n = lattice(params);
    let mut pts = Vec::with_capacity((n * n) as usize);
    for gy in 0..n {
        for gx in 0..n {
            let r = roll(seed, gx as u64, gy as u64);
            let jx = (r % c as u64) as i32;
            let jy = ((r >> 32) % c as u64) as i32;
            pts.push((gx * c + jx, gy * c + jy));
        }
    }
    let size = WORLD_SIZE as i32;
    let mut cells = Vec::with_capacity((size * size) as usize);
    for y in 0..size {
        for x in 0..size {
            let (cx, cy) = (x / c, y / c);
            let mut best = (i32::MAX, 0i32);
            for gy in (cy - 1).max(0)..=(cy + 1).min(n - 1) {
                for gx in (cx - 1).max(0)..=(cx + 1).min(n - 1) {
                    let id = gy * n + gx;
                    let (px, py) = pts[id as usize];
                    let d = (px - x) * (px - x) + (py - y) * (py - y);
                    if (d, id) < best {
                        best = (d, id);
                    }
                }
            }
            cells.push(best.1 as u16);
        }
    }
    world_grid(cells, 0u16)
}

/// Pick the setting of a province from the shares (per-mille) of its land cells.
fn pick_setting(land: u32, volc: u32, mount: u32, coast: u32, params: &WorldParams) -> Setting {
    let share = |count: u32| (count * 1000 / land.max(1)) as i32;
    if land == 0 {
        Setting::Coast
    } else if share(volc) >= params.prov_volcanic_pm {
        Setting::Volcanic
    } else if share(mount) >= params.prov_mountain_pm {
        Setting::Mountain
    } else if share(coast) >= params.prov_coast_pm {
        Setting::Coast
    } else {
        Setting::Plain
    }
}

/// Make the geology: provinces, settings, rock layers, and soil.
pub fn make_geology(seed: u64, inp: &GeoInputs, params: &WorldParams) -> Geology {
    let Ok(tables) = tables() else {
        return fallback_geology(seed, inp, params);
    };
    let province = make_provinces(seed, params);
    let n = (lattice(params) * lattice(params)) as usize;
    let mut land = vec![0u32; n];
    let mut volc = vec![0u32; n];
    let mut mount = vec![0u32; n];
    let mut coast = vec![0u32; n];
    for (i, &p) in province.as_slice().iter().enumerate() {
        let id = usize::from(p);
        let b = inp.biome.as_slice()[i];
        if b == WorldBiome::Ocean || b == WorldBiome::Lake {
            continue;
        }
        land[id] += 1;
        let rel = i32::from(inp.height.as_slice()[i]) - i32::from(inp.sea_level);
        if b == WorldBiome::Volcanic {
            volc[id] += 1;
        }
        if b == WorldBiome::Coast {
            coast[id] += 1;
        }
        if b == WorldBiome::HighMountain || rel >= i32::from(params.rock_mountain_min) {
            mount[id] += 1;
        }
    }
    let mut settings = Vec::with_capacity(n);
    let mut layer_count = Vec::with_capacity(n);
    let mut layers = Vec::with_capacity(n);
    for id in 0..n {
        let setting = pick_setting(land[id], volc[id], mount[id], coast[id], params);
        let r = roll(seed, 0x10_0000 + id as u64, 0);
        let count = if (r % 1000) < u64::from(params.layer3_per_mille) {
            3u8
        } else {
            2u8
        };
        // Draw distinct rocks from the pool of the setting.
        let mut pool = tables.pool(setting).to_vec();
        let mut chosen = [Rock::Sandstone; 3];
        for (k, slot) in chosen.iter_mut().enumerate().take(usize::from(count)) {
            let pick = roll(seed, 0x20_0000 + id as u64, k as u64) as usize % pool.len();
            *slot = pool.remove(pick);
        }
        let last = chosen[usize::from(count) - 1];
        for slot in chosen.iter_mut().skip(usize::from(count)) {
            *slot = last;
        }
        settings.push(setting);
        layer_count.push(count);
        layers.push(chosen);
    }
    let soil = make_soil(seed, inp, params);
    Geology {
        province,
        settings,
        layer_count,
        layers,
        soil,
    }
}

/// Geology for the case that the embedded tables are bad: 1 plain province. Unreachable when
/// the check at world generation start passes. It keeps the stages free of panics.
fn fallback_geology(seed: u64, inp: &GeoInputs, params: &WorldParams) -> Geology {
    let size = WORLD_SIZE as usize;
    Geology {
        province: world_grid(vec![0u16; size * size], 0u16),
        settings: vec![Setting::Plain],
        layer_count: vec![2],
        layers: vec![[Rock::Sandstone, Rock::Shale, Rock::Shale]],
        soil: make_soil(seed, inp, params),
    }
}

/// Soil depth: deep in wet flat land and along rivers, thin on slopes, none on water and ice.
fn make_soil(seed: u64, inp: &GeoInputs, params: &WorldParams) -> Grid<u8> {
    let size = WORLD_SIZE as usize;
    let mut cells = Vec::with_capacity(size * size);
    for i in 0..size * size {
        let b = inp.biome.as_slice()[i];
        if b == WorldBiome::Ocean || b == WorldBiome::Lake || b == WorldBiome::Glacier {
            cells.push(0u8);
            continue;
        }
        let rel = (i32::from(inp.height.as_slice()[i]) - i32::from(inp.sea_level)).max(0);
        let mut s = params.soil_flat
            + i32::from(inp.moisture.as_slice()[i]) * params.soil_wet / 255
            - rel * params.soil_slope / 1000;
        if inp.river.as_slice()[i] > 0 || b == WorldBiome::Floodplain {
            s += params.soil_river;
        }
        if b == WorldBiome::Volcanic {
            s += params.soil_ash;
        }
        if params.soil_jitter > 0 {
            s += (roll(seed, 0x30_0000, i as u64) % params.soil_jitter as u64) as i32;
        }
        cells.push(s.clamp(0, params.soil_max.min(255)) as u8);
    }
    world_grid(cells, 0u8)
}
