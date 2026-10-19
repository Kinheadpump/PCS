//! World terrain tools: gen-world and the world layer images of dump-layers.

use super::ppm;
use worldgen::{
    LayerStage, Mineral, Rock, Stage, WORLD_SIZE, WorldBiome, WorldData, WorldParams,
    generate_world,
};

const WORLD_USAGE: &str = "usage: tools gen-world --seed <n> [--out <dir>]";

/// Parse an optional seed pair. The default seed is 1.
fn parse_seed(args: &[String]) -> Result<u64, String> {
    match args {
        [] => Ok(1),
        [flag, value] if flag == "--seed" => value
            .parse::<u64>()
            .map_err(|e| format!("bad seed {value:?}: {e}")),
        _ => Err(WORLD_USAGE.to_owned()),
    }
}

fn biome_color(b: WorldBiome) -> [u8; 3] {
    match b {
        WorldBiome::Ocean => [30, 60, 140],
        WorldBiome::Lake => [70, 130, 220],
        WorldBiome::Glacier => [235, 245, 250],
        WorldBiome::HighMountain => [140, 135, 135],
        WorldBiome::Volcanic => [90, 40, 40],
        WorldBiome::Coast => [226, 210, 150],
        WorldBiome::Floodplain => [120, 170, 90],
        WorldBiome::Tundra => [170, 180, 160],
        WorldBiome::ColdDesert => [190, 185, 170],
        WorldBiome::Taiga => [50, 100, 80],
        WorldBiome::Steppe => [190, 180, 100],
        WorldBiome::Shrubland => [160, 150, 90],
        WorldBiome::Grassland => [120, 180, 80],
        WorldBiome::TemperateForest => [40, 130, 50],
        WorldBiome::Marsh => [80, 120, 100],
        WorldBiome::Desert => [225, 200, 120],
        WorldBiome::Savanna => [200, 170, 70],
        WorldBiome::TropicalForest => [30, 150, 70],
        WorldBiome::Rainforest => [10, 110, 60],
    }
}

fn height_color(w: &WorldData, x: i32, y: i32) -> [u8; 3] {
    let rel = w.height_above_sea(x, y);
    if rel < 0 {
        let depth = (-rel).min(2000) as u32;
        let g = 30 + (60 - depth * 60 / 2000);
        let b = 120 + (100 - depth * 100 / 2000);
        [10, g as u8, b as u8]
    } else {
        let t = rel.min(3000) as u32 * 255 / 3000;
        [
            (60 + t * 190 / 255) as u8,
            (120 + t * 100 / 255) as u8,
            (50 + t * 200 / 255) as u8,
        ]
    }
}

fn temperature_color(t: i8) -> [u8; 3] {
    let v = ((i32::from(t) + 30).clamp(0, 60) * 255 / 60) as u8;
    [v, 60, 255 - v]
}

fn moisture_color(m: u8) -> [u8; 3] {
    [200 - m / 2, 170 - m / 4, 60 + m / 2 + m / 4]
}

fn river_color(w: &WorldData, x: i32, y: i32) -> [u8; 3] {
    if w.height_above_sea(x, y) < 0 {
        return [20, 40, 90];
    }
    if w.lake.get(x, y).copied().unwrap_or(0) != 0 {
        return [90, 170, 250];
    }
    match w.river.get(x, y).copied().unwrap_or(0) {
        1 => [90, 150, 255],
        2 => [50, 110, 255],
        3 => [20, 60, 240],
        _ => [110, 110, 100],
    }
}

fn rock_color(r: Rock) -> [u8; 3] {
    const COLORS: [[u8; 3]; Rock::COUNT] = [
        [214, 180, 120],
        [120, 110, 100],
        [200, 200, 180],
        [170, 130, 130],
        [140, 140, 160],
        [235, 230, 240],
        [50, 50, 60],
        [130, 100, 80],
    ];
    COLORS[r as usize]
}

fn mineral_color(m: Mineral) -> [u8; 3] {
    const COLORS: [[u8; 3]; Mineral::COUNT] = [
        [20, 20, 20],
        [200, 90, 50],
        [60, 200, 140],
        [180, 190, 210],
        [250, 220, 40],
        [220, 60, 220],
    ];
    COLORS[m as usize]
}

/// Rock image: the top layer of the province, dark on water.
fn rock_image_color(w: &WorldData, x: i32, y: i32) -> [u8; 3] {
    if w.height_above_sea(x, y) < 0 {
        return [20, 40, 90];
    }
    match w.geology.rock_at(x, y, 0) {
        Some(r) => rock_color(r),
        None => [0, 0, 0],
    }
}

/// Deposit image: mineral color scaled by richness on grey land.
fn deposit_image_color(w: &WorldData, x: i32, y: i32) -> [u8; 3] {
    if w.height_above_sea(x, y) < 0 {
        return [20, 40, 90];
    }
    match w.deposits.at(x, y) {
        Some((m, richness)) => {
            let c = mineral_color(m);
            let k = 60 + u32::from(richness) * 65;
            [
                (u32::from(c[0]) * k / 255) as u8,
                (u32::from(c[1]) * k / 255) as u8,
                (u32::from(c[2]) * k / 255) as u8,
            ]
        }
        None => [95, 95, 90],
    }
}

fn soil_color(w: &WorldData, x: i32, y: i32) -> [u8; 3] {
    let d = w.geology.soil.get(x, y).copied().unwrap_or(0);
    [200 - d * 2, 170 - d, 90]
}

/// Render one layer: the color function gets the cell coordinates.
fn image(color: impl Fn(i32, i32) -> [u8; 3]) -> Vec<u8> {
    let size = WORLD_SIZE as i32;
    let mut rgb = Vec::with_capacity((size * size * 3) as usize);
    for y in 0..size {
        for x in 0..size {
            rgb.extend_from_slice(&color(x, y));
        }
    }
    ppm(WORLD_SIZE, WORLD_SIZE, &rgb)
}

/// Make the world layer images: (file name, PPM bytes).
pub fn layer_files(w: &WorldData) -> Vec<(&'static str, Vec<u8>)> {
    let cell = |x: i32, y: i32| (y * WORLD_SIZE as i32 + x) as usize;
    vec![
        ("world_height.ppm", image(|x, y| height_color(w, x, y))),
        (
            "world_temperature.ppm",
            image(|x, y| temperature_color(w.temperature.as_slice()[cell(x, y)])),
        ),
        (
            "world_moisture.ppm",
            image(|x, y| moisture_color(w.moisture.as_slice()[cell(x, y)])),
        ),
        ("world_river.ppm", image(|x, y| river_color(w, x, y))),
        (
            "world_biome.ppm",
            image(|x, y| biome_color(w.biome.as_slice()[cell(x, y)])),
        ),
        ("world_rock.ppm", image(|x, y| rock_image_color(w, x, y))),
        (
            "world_deposit.ppm",
            image(|x, y| deposit_image_color(w, x, y)),
        ),
        ("world_soil.ppm", image(|x, y| soil_color(w, x, y))),
    ]
}

fn write_layers(w: &WorldData, dir: &str) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("cannot create {dir}: {e}"))?;
    for (file, bytes) in layer_files(w) {
        let path = std::path::Path::new(dir).join(file);
        std::fs::write(&path, bytes)
            .map_err(|e| format!("cannot write {}: {e}", path.display()))?;
    }
    Ok(())
}

/// dump-layers world: write the world layer images and return the biome stage hash.
pub fn dump_world_layers(dir: &str, rest: &[String]) -> Result<String, String> {
    let seed = parse_seed(rest)?;
    let w = generate_world(seed, &WorldParams::default());
    write_layers(&w, dir)?;
    Ok(format!("{:016x}", w.stage_hashes[Stage::Biomes as usize]))
}

/// Parse the arguments of gen-world: a seed and an optional output directory.
fn parse_gen_args(args: &[String]) -> Result<(u64, Option<String>), String> {
    match args {
        [flag, _] if flag == "--seed" => Ok((parse_seed(args)?, None)),
        [flag, value, flag2, dir] if flag == "--seed" && flag2 == "--out" => {
            let seed = parse_seed(&[flag.clone(), value.clone()])?;
            Ok((seed, Some(dir.clone())))
        }
        _ => Err(WORLD_USAGE.to_owned()),
    }
}

const ALL_BIOMES: [WorldBiome; WorldBiome::COUNT] = [
    WorldBiome::Ocean,
    WorldBiome::Lake,
    WorldBiome::Glacier,
    WorldBiome::HighMountain,
    WorldBiome::Volcanic,
    WorldBiome::Coast,
    WorldBiome::Floodplain,
    WorldBiome::Tundra,
    WorldBiome::ColdDesert,
    WorldBiome::Taiga,
    WorldBiome::Steppe,
    WorldBiome::Shrubland,
    WorldBiome::Grassland,
    WorldBiome::TemperateForest,
    WorldBiome::Marsh,
    WorldBiome::Desert,
    WorldBiome::Savanna,
    WorldBiome::TropicalForest,
    WorldBiome::Rainforest,
];

/// gen-world: generate a world, print the stage hashes and statistics.
#[allow(clippy::disallowed_methods)] // tools may read the clock
pub fn gen_world(args: &[String]) -> Result<String, String> {
    let (seed, out) = parse_gen_args(args)?;
    let start = std::time::Instant::now();
    let w = generate_world(seed, &WorldParams::default());
    let micros = start.elapsed().as_micros();
    let mut lines = vec![format!("seed {seed}")];
    for stage in Stage::ALL {
        let hash = w.stage_hashes[stage as usize];
        lines.push(format!("stage {} {:016x}", stage.name(), hash));
    }
    for stage in LayerStage::ALL {
        let hash = w.layer_hashes[stage as usize];
        lines.push(format!("stage {} {:016x}", stage.name(), hash));
    }
    lines.push(format!("sea level {}", w.sea_level));
    lines.push(format!("land per mille {}", w.land_per_mille()));
    let mut counts = [0u32; WorldBiome::COUNT];
    for &b in w.biome.as_slice() {
        counts[b as usize] += 1;
    }
    for b in ALL_BIOMES {
        lines.push(format!("biome {} {}", b.name(), counts[b as usize]));
    }
    for size in 1..=3u8 {
        let n = w.river.as_slice().iter().filter(|&&r| r == size).count();
        lines.push(format!("river size {size} cells {n}"));
    }
    let lakes = w.lake.as_slice().iter().max().copied().unwrap_or(0);
    lines.push(format!("lakes {lakes}"));
    let mut per = [0u32; Mineral::COUNT];
    for &m in w.deposits.mineral.as_slice() {
        if m != 0 {
            per[usize::from(m) - 1] += 1;
        }
    }
    for m in Mineral::ALL {
        lines.push(format!("deposit {} {}", m.name(), per[m as usize]));
    }
    lines.push(format!("generation microseconds {micros}"));
    if let Some(dir) = out {
        write_layers(&w, &dir)?;
        lines.push(format!("layers written to {dir}"));
    }
    Ok(lines.join(&char::from(10).to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn world_layer_files_are_valid_ppm_sizes() {
        let w = generate_world(1, &WorldParams::default());
        let nl = char::from(10);
        let header = format!("P6{nl}256 256{nl}255{nl}").into_bytes();
        for (name, bytes) in layer_files(&w) {
            assert!(bytes.starts_with(&header), "{name}");
            assert_eq!(bytes.len(), header.len() + 256 * 256 * 3, "{name}");
        }
    }

    #[test]
    fn gen_world_prints_eleven_stage_hashes() {
        let args: Vec<String> = vec!["--seed".into(), "2".into()];
        let text = gen_world(&args).unwrap_or_default();
        assert_eq!(text.lines().filter(|l| l.starts_with("stage ")).count(), 11);
        assert!(gen_world(&[]).is_err());
    }
}
