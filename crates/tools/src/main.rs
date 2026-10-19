//! Headless command-line tools.

mod assetgen;
mod bench;
mod dumpsave;

use base::Grid;
use base::content::load_materials;
use base::hash::Fnv1a;
use base::rng::Pcg32;
use std::process::ExitCode;
use worldgen::{EmbarkSummary, Ground, LocalMapData, Ore, RockKind, generate_local};

const HASH_GRID_SIZE: u32 = 64;

/// Make a grid of random values from a seed and return its stable hash.
fn grid_hash(seed: u64) -> u64 {
    let mut rng = Pcg32::from_seed(seed);
    let cells = (0..HASH_GRID_SIZE * HASH_GRID_SIZE)
        .map(|_| rng.next_u32())
        .collect();
    let Some(grid) = Grid::from_vec(HASH_GRID_SIZE, HASH_GRID_SIZE, cells) else {
        return 0;
    };
    let mut h = Fnv1a::new();
    h.write_u32(grid.width());
    h.write_u32(grid.height());
    for &v in grid.as_slice() {
        h.write_u32(v);
    }
    h.finish()
}

fn parse_seed(args: &[String]) -> Result<u64, String> {
    match args {
        [flag, value] if flag == "--seed" => value
            .parse::<u64>()
            .map_err(|e| format!("bad seed {value:?}: {e}")),
        _ => Err("usage: tools hash --seed <number>".to_owned()),
    }
}

const USAGE: &str = "usage: tools hash --seed <n> | dump-layers <preset> <out_dir> [--seed <n>] | validate-content <file> | assetgen [--seed <n>] [--out <dir>] | dump-save <file> | bench [--ticks <n>]";

/// Pick a test preset by name.
fn preset(name: &str) -> Result<EmbarkSummary, String> {
    match name {
        "temperate_forest" => Ok(EmbarkSummary::temperate_forest()),
        "cold_hills" => Ok(EmbarkSummary::cold_hills()),
        "dry_plain" => Ok(EmbarkSummary::dry_plain()),
        _ => Err(format!("unknown preset {name:?}")),
    }
}

/// Encode an RGB image as a binary PPM (P6).
fn ppm(width: u32, height: u32, rgb: &[u8]) -> Vec<u8> {
    let mut out = format!(
        "P6
{width} {height}
255
"
    )
    .into_bytes();
    out.extend_from_slice(rgb);
    out
}

fn layer_image<T>(grid: &Grid<T>, color: impl Fn(&T) -> [u8; 3]) -> Vec<u8> {
    let rgb: Vec<u8> = grid.as_slice().iter().flat_map(color).collect();
    ppm(grid.width(), grid.height(), &rgb)
}

/// Make the layer images of a local map: (file name, PPM bytes).
fn layer_files(map: &LocalMapData) -> Vec<(&'static str, Vec<u8>)> {
    vec![
        (
            "ground.ppm",
            layer_image(&map.ground, |g| match g {
                Ground::Soil => [96, 72, 48],
                Ground::Sand => [214, 196, 138],
                Ground::Clay => [170, 110, 84],
                Ground::Water => [40, 90, 200],
                Ground::Stone => [110, 110, 116],
            }),
        ),
        (
            "rock.ppm",
            layer_image(&map.rock, |r| match r {
                RockKind::None => [0, 0, 0],
                RockKind::Sandstone => [200, 170, 110],
                RockKind::Limestone => [200, 200, 190],
                RockKind::Granite => [120, 100, 110],
            }),
        ),
        (
            "ore.ppm",
            layer_image(&map.ore, |o| match o {
                Ore::None => [0, 0, 0],
                Ore::Coal => [60, 60, 60],
                Ore::Copper => [200, 110, 50],
                Ore::Iron => [190, 190, 210],
            }),
        ),
        (
            "tree.ppm",
            layer_image(&map.tree, |t| if *t { [20, 130, 40] } else { [0, 0, 0] }),
        ),
    ]
}

fn dump_layers(args: &[String]) -> Result<String, String> {
    let [name, dir, rest @ ..] = args else {
        return Err(USAGE.to_owned());
    };
    let mut embark = preset(name)?;
    match rest {
        [] => {}
        [flag, value] if flag == "--seed" => {
            embark = embark.with_seed(
                value
                    .parse()
                    .map_err(|e| format!("bad seed {value:?}: {e}"))?,
            );
        }
        _ => return Err(USAGE.to_owned()),
    }
    let map = generate_local(&embark);
    std::fs::create_dir_all(dir).map_err(|e| format!("cannot create {dir}: {e}"))?;
    for (file, bytes) in layer_files(&map) {
        let path = std::path::Path::new(dir).join(file);
        std::fs::write(&path, bytes)
            .map_err(|e| format!("cannot write {}: {e}", path.display()))?;
    }
    Ok(format!("{:016x}", map.hash()))
}

fn validate_content(args: &[String]) -> Result<String, String> {
    let [path] = args else {
        return Err(USAGE.to_owned());
    };
    let text = std::fs::read_to_string(path).map_err(|e| format!("cannot read {path}: {e}"))?;
    let materials = load_materials(path, &text).map_err(|e| e.to_string())?;
    Ok(format!("ok: {} materials", materials.len()))
}

fn run(args: &[String]) -> Result<String, String> {
    match args.split_first() {
        Some((cmd, rest)) if cmd == "hash" => Ok(format!("{:016x}", grid_hash(parse_seed(rest)?))),
        Some((cmd, rest)) if cmd == "dump-layers" => dump_layers(rest),
        Some((cmd, rest)) if cmd == "dump-save" => dumpsave::run(rest),
        Some((cmd, rest)) if cmd == "bench" => bench::run(rest),
        Some((cmd, rest)) if cmd == "assetgen" => assetgen::run(rest),
        Some((cmd, rest)) if cmd == "validate-content" => validate_content(rest),
        _ => Err(USAGE.to_owned()),
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(out) => {
            println!("{out}");
            ExitCode::SUCCESS
        }
        Err(msg) => {
            eprintln!("{msg}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grid_hash_is_repeatable() {
        assert_eq!(grid_hash(1), grid_hash(1));
        assert_ne!(grid_hash(1), grid_hash(2));
    }

    #[test]
    fn grid_hash_seed_1_known_answer() {
        assert_eq!(grid_hash(1), 0xf751_a61d_a3d8_c141);
    }

    #[test]
    fn layer_files_are_valid_ppm_sizes() {
        let map = generate_local(&EmbarkSummary::cold_hills());
        for (name, bytes) in layer_files(&map) {
            let header = b"P6
256 256
255
";
            assert!(bytes.starts_with(header), "{name}");
            assert_eq!(bytes.len(), header.len() + 256 * 256 * 3, "{name}");
        }
    }

    #[test]
    fn unknown_preset_is_an_error() {
        assert!(preset("moon").is_err());
    }

    #[test]
    fn bad_arguments_return_errors() {
        assert!(run(&[]).is_err());
        assert!(run(&["hash".into()]).is_err());
        assert!(run(&["hash".into(), "--seed".into(), "x".into()]).is_err());
    }
}
