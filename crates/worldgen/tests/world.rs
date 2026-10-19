//! World terrain tests: stage hashes, acceptance rules 1 to 7 (TECHNICAL_DESIGN §11.3), timing.

use worldgen::climate::wind_dir;
use worldgen::hydro::{DIRS, NO_DIR};
use worldgen::{
    STAGE_COUNT, Stage, WORLD_SIZE, WorldBiome, WorldData, WorldParams, generate_world,
};

const GOLDEN_SEEDS: [u64; 3] = [1, 2, 3];

type Change = fn(&mut WorldParams);

fn world(seed: u64) -> WorldData {
    generate_world(seed, &WorldParams::default())
}

fn golden_path() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("golden_world.txt")
}

fn without_cr(text: &str) -> String {
    text.replace(char::from(13), "")
}

fn golden_text() -> String {
    let mut out = String::new();
    for seed in GOLDEN_SEEDS {
        let w = world(seed);
        for stage in Stage::ALL {
            let line = format!(
                "seed{}.{}={:016x}",
                seed,
                stage.name(),
                w.stage_hashes[stage as usize]
            );
            out.push_str(&line);
            out.push(char::from(10));
        }
    }
    out
}

/// Check one stage: repeatable, seed dependent, and equal to the golden value.
#[allow(clippy::expect_used)]
fn check_stage(stage: Stage) {
    let a = world(1);
    let b = world(1);
    let c = world(2);
    let k = stage as usize;
    assert_eq!(
        a.stage_hashes[k],
        b.stage_hashes[k],
        "{} repeat",
        stage.name()
    );
    assert_ne!(
        a.stage_hashes[k],
        c.stage_hashes[k],
        "{} seed",
        stage.name()
    );
    if std::env::var_os("UPDATE_GOLDEN").is_some() {
        return;
    }
    let expected = std::fs::read_to_string(golden_path()).expect("golden_world.txt is missing");
    let key = format!("seed1.{}={:016x}", stage.name(), a.stage_hashes[k]);
    assert!(
        without_cr(&expected).lines().any(|l| l == key),
        "stage hash {} not in golden file",
        key
    );
}

#[test]
fn stage_hash_seeds() {
    check_stage(Stage::Seeds);
}
#[test]
fn stage_hash_uplift() {
    check_stage(Stage::Uplift);
}
#[test]
fn stage_hash_height() {
    check_stage(Stage::Height);
}
#[test]
fn stage_hash_sea_level() {
    check_stage(Stage::SeaLevel);
}
#[test]
fn stage_hash_lakes() {
    check_stage(Stage::Lakes);
}
#[test]
fn stage_hash_temperature() {
    check_stage(Stage::Temperature);
}
#[test]
fn stage_hash_moisture() {
    check_stage(Stage::Moisture);
}
#[test]
fn stage_hash_rivers() {
    check_stage(Stage::Rivers);
}
#[test]
fn stage_hash_biomes() {
    check_stage(Stage::Biomes);
}

#[test]
fn r_s8_1_golden_stage_hashes_match_on_every_platform() {
    let actual = golden_text();
    if std::env::var_os("UPDATE_GOLDEN").is_some() {
        std::fs::write(golden_path(), &actual).expect("write golden_world.txt");
        return;
    }
    let expected = std::fs::read_to_string(golden_path()).expect("golden_world.txt is missing");
    assert_eq!(
        without_cr(&expected),
        actual,
        "golden hash changed; run with UPDATE_GOLDEN=1 only if the task permits"
    );
}

#[test]
fn r_s8_2_a_change_in_stage_k_keeps_the_hashes_before_k() {
    let base = world(7);
    let cases: [(Stage, Change); 8] = [
        (Stage::Uplift, |p| p.ridge_height += 400),
        (Stage::Height, |p| p.height_amp += 500),
        (Stage::SeaLevel, |p| p.land_per_mille = 300),
        (Stage::Lakes, |p| p.lake_depth_min = 200),
        (Stage::Temperature, |p| p.lapse_per_10k += 20),
        (Stage::Moisture, |p| p.rain_base += 10),
        (Stage::Rivers, |p| p.river_small += 100),
        (Stage::Biomes, |p| p.mountain_min -= 300),
    ];
    for (stage, change) in cases {
        let mut params = WorldParams::default();
        change(&mut params);
        let other = generate_world(7, &params);
        let k = stage as usize;
        assert_eq!(
            base.stage_hashes[..k],
            other.stage_hashes[..k],
            "{}",
            stage.name()
        );
        assert_ne!(
            base.stage_hashes[k],
            other.stage_hashes[k],
            "{}",
            stage.name()
        );
    }
    assert_eq!(STAGE_COUNT, Stage::ALL.len());
}

#[test]
fn r_s8_3_land_fraction_is_target_within_half_a_percent() {
    for target in [300u32, 400, 500] {
        for seed in 1..=12u64 {
            let params = WorldParams {
                land_per_mille: target,
                ..WorldParams::default()
            };
            let w = generate_world(seed, &params);
            let got = w.land_per_mille();
            assert!(
                got.abs_diff(target) <= 5,
                "seed {} target {} got {}",
                seed,
                target,
                got
            );
        }
    }
}

#[test]
fn r_s8_4_no_biome_dominates_and_biomes_are_varied() {
    let mut varied = 0;
    let seeds = 50u64;
    for seed in 1..=seeds {
        let w = world(seed);
        let mut counts = [0u32; WorldBiome::COUNT];
        let mut land = 0u32;
        for &b in w.biome.as_slice() {
            if b != WorldBiome::Ocean {
                counts[b as usize] += 1;
                land += 1;
            }
        }
        assert!(land > 0);
        let max = counts.iter().copied().max().unwrap_or(0);
        assert!(
            max * 100 <= land * 35,
            "seed {}: biome share {} of {}",
            seed,
            max,
            land
        );
        if counts.iter().filter(|&&c| c > 0).count() >= 8 {
            varied += 1;
        }
    }
    assert!(
        varied * 10 >= seeds * 9,
        "only {} of {} seeds have 8 biomes",
        varied,
        seeds
    );
}

#[test]
fn r_s8_5_rivers_end_in_ocean_or_lake_without_cycles() {
    let n = (WORLD_SIZE * WORLD_SIZE) as usize;
    let size = WORLD_SIZE as i32;
    for seed in 1..=10u64 {
        let w = world(seed);
        let mut river_cells = 0;
        for (x, y, &r) in w.river.iter() {
            if r == 0 {
                continue;
            }
            river_cells += 1;
            let (mut cx, mut cy) = (x, y);
            let mut steps = 0;
            loop {
                let ocean = w.height.get(cx, cy).copied().unwrap_or(0) < w.sea_level;
                let lake = w.lake.get(cx, cy).copied().unwrap_or(0) != 0;
                if ocean || lake {
                    break;
                }
                steps += 1;
                assert!(steps < n, "seed {}: cycle from ({},{})", seed, x, y);
                let d = w.drain.get(cx, cy).copied().unwrap_or(NO_DIR);
                assert_ne!(
                    d, NO_DIR,
                    "seed {}: river stops on land at ({},{})",
                    seed, cx, cy
                );
                let (dx, dy) = DIRS[d as usize];
                cx += dx;
                cy += dy;
                assert!(cx >= 0 && cy >= 0 && cx < size && cy < size);
            }
        }
        assert!(
            river_cells > 200,
            "seed {}: only {} river cells",
            seed,
            river_cells
        );
    }
}

#[test]
fn r_s8_5_filled_height_never_falls_below_height_and_rivers_have_three_sizes() {
    let w = world(3);
    for (x, y, &f) in w.filled.iter() {
        assert!(f >= *w.height.get(x, y).unwrap_or(&0));
    }
    for size in 1..=3u8 {
        assert!(
            w.river.as_slice().contains(&size),
            "no river of size {}",
            size
        );
    }
}

#[test]
fn r_s8_6_equator_is_warmer_than_poles_and_height_cools() {
    let n = WORLD_SIZE as i32;
    let mut equator = (0i64, 0i64);
    let mut polar = (0i64, 0i64);
    let mut by_height = [(0i64, 0i64); 3];
    for seed in 1..=10u64 {
        let w = world(seed);
        for (x, y, &t) in w.temperature.iter() {
            let from_equator = (2 * y - (n - 1)).abs();
            if from_equator < n / 4 {
                equator = (equator.0 + i64::from(t), equator.1 + 1);
            } else if from_equator > n * 3 / 4 {
                polar = (polar.0 + i64::from(t), polar.1 + 1);
            }
            let rel = i32::from(w.height.get(x, y).copied().unwrap_or(0)) - i32::from(w.sea_level);
            if from_equator < n && rel >= 0 {
                let band = if rel < 300 {
                    0
                } else if rel < 1000 {
                    1
                } else {
                    2
                };
                by_height[band] = (by_height[band].0 + i64::from(t), by_height[band].1 + 1);
            }
        }
    }
    assert!(
        equator.0 * polar.1 > polar.0 * equator.1,
        "equator not warmer"
    );
    let mean = |b: (i64, i64)| (b.0 * 100) / b.1.max(1);
    assert!(by_height.iter().all(|b| b.1 > 100), "{:?}", by_height);
    assert!(mean(by_height[0]) > mean(by_height[1]), "{:?}", by_height);
    assert!(mean(by_height[1]) > mean(by_height[2]), "{:?}", by_height);
}

#[test]
fn r_s8_7_moisture_behind_mountains_is_lower_than_in_front() {
    let size = WORLD_SIZE as i32;
    let (mut up_sum, mut down_sum, mut samples) = (0i64, 0i64, 0i64);
    for seed in 1..=12u64 {
        let w = world(seed);
        let rel = |x: i32, y: i32| {
            w.height
                .get(x, y)
                .map(|&h| i32::from(h) - i32::from(w.sea_level))
        };
        for (x, y, _) in w.height.iter() {
            if rel(x, y).unwrap_or(0) < 1200 {
                continue;
            }
            let dir = wind_dir(y, size);
            for k in 6..=12 {
                let (ux, dx) = (x - dir * k, x + dir * k);
                let (Some(ur), Some(dr)) = (rel(ux, y), rel(dx, y)) else {
                    continue;
                };
                if (0..600).contains(&ur) && (0..600).contains(&dr) {
                    up_sum += i64::from(w.moisture.get(ux, y).copied().unwrap_or(0));
                    down_sum += i64::from(w.moisture.get(dx, y).copied().unwrap_or(0));
                    samples += 1;
                }
            }
        }
    }
    assert!(samples > 200, "only {} samples", samples);
    assert!(
        down_sum < up_sum,
        "down {} up {} over {}",
        down_sum,
        up_sum,
        samples
    );
}

#[test]
fn layers_have_the_world_size() {
    let w = world(1);
    let expected = (WORLD_SIZE * WORLD_SIZE) as usize;
    assert_eq!(w.height.as_slice().len(), expected);
    assert_eq!(w.temperature.as_slice().len(), expected);
    assert_eq!(w.moisture.as_slice().len(), expected);
    assert_eq!(w.biome.as_slice().len(), expected);
}

/// P-05: world generation steps 1 to 9 well inside 300 ms in release.
/// Run: cargo test -p worldgen --release --test world -- --ignored --nocapture
#[test]
#[ignore = "timing test, run in release"]
#[allow(clippy::disallowed_methods)]
fn p05_world_generation_time() {
    let mut worst = std::time::Duration::ZERO;
    for seed in 1..=10u64 {
        let start = std::time::Instant::now();
        let w = world(seed);
        let elapsed = start.elapsed();
        worst = worst.max(elapsed);
        println!(
            "seed {}: {:?} (biome hash {:016x})",
            seed, elapsed, w.stage_hashes[8]
        );
    }
    assert!(worst.as_millis() < 300, "worst {:?}", worst);
}
