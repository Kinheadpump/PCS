//! Sprint 9 world tests: steps 10 (geology) and 11 (deposits).
//! Stage hashes, rule 2 (earlier stages unchanged), and deposit invariants.

use worldgen::deposit::affinity;
use worldgen::{
    LAYER_STAGE_COUNT, LayerStage, Mineral, Rock, STAGE_COUNT, Stage, WORLD_SIZE, WorldBiome,
    WorldData, WorldParams, generate_world,
};

const GOLDEN_SEEDS: [u64; 3] = [1, 2, 3];

fn world(seed: u64) -> WorldData {
    generate_world(seed, &WorldParams::default())
}

const GOLDEN_FILE: &str = "tests/golden_world_s9.txt";

fn hex(hash: u64) -> String {
    let mut s = String::new();
    for shift in (0..16).rev() {
        let nibble = ((hash >> (shift * 4)) & 15) as u32;
        s.push(char::from_digit(nibble, 16).unwrap_or(char::from(48)));
    }
    s
}

const PFX: &str = "seed";
const DOT: &str = ".";
const EQ: &str = "=";

fn golden_line(seed: u64, name: &str, hash: u64) -> String {
    let mut out = PFX.to_string();
    out.push_str(&seed.to_string());
    out.push_str(DOT);
    out.push_str(name);
    out.push_str(EQ);
    out.push_str(&hex(hash));
    out
}

fn golden_text() -> String {
    let mut out = String::new();
    for seed in GOLDEN_SEEDS {
        let w = world(seed);
        for stage in LayerStage::ALL {
            let hash = w.layer_hashes[stage as usize];
            out.push_str(&golden_line(seed, stage.name(), hash));
            out.push(char::from(10));
        }
    }
    out
}

/// Read the golden file. lines() drops a carriage return, so a Windows checkout compares equal.
#[allow(clippy::expect_used)]
fn golden_file() -> String {
    let text = std::fs::read_to_string(GOLDEN_FILE).expect(MISSING);
    let mut out = String::new();
    for line in text.lines() {
        out.push_str(line);
        out.push(char::from(10));
    }
    out
}

const MISSING: &str = "golden file is missing";

fn update_golden() -> bool {
    std::env::var_os(UPDATE_VAR).is_some()
}

const UPDATE_VAR: &str = "UPDATE_GOLDEN";

#[test]
fn stage_hash_geology() {
    check_stage(LayerStage::Geology);
}

#[test]
fn stage_hash_deposits() {
    check_stage(LayerStage::Deposits);
}

fn check_stage(stage: LayerStage) {
    let a = world(1);
    let b = world(1);
    let c = world(2);
    let k = stage as usize;
    assert_eq!(a.layer_hashes[k], b.layer_hashes[k]);
    assert_ne!(a.layer_hashes[k], c.layer_hashes[k]);
    assert_ne!(a.layer_seeds[k], c.layer_seeds[k]);
    if update_golden() {
        return;
    }
    let key = golden_line(1, stage.name(), a.layer_hashes[k]);
    assert!(golden_file().lines().any(|l| l == key), "{key}");
}

const S8_GOLDEN: &str = "tests/golden_world.txt";

fn s8_hash_lines() -> String {
    let mut out = String::new();
    for seed in GOLDEN_SEEDS {
        let w = world(seed);
        for stage in Stage::ALL {
            let hash = w.stage_hashes[stage as usize];
            out.push_str(&golden_line(seed, stage.name(), hash));
            out.push(char::from(10));
        }
    }
    out
}

#[allow(clippy::expect_used)]
fn s8_file() -> String {
    let text = std::fs::read_to_string(S8_GOLDEN).expect(MISSING);
    let mut out = String::new();
    for line in text.lines() {
        out.push_str(line);
        out.push(char::from(10));
    }
    out
}

#[test]
fn r_s9_2_earlier_stage_hashes_match_the_s8_file() {
    assert_eq!(s8_file(), s8_hash_lines());
    assert_eq!(STAGE_COUNT, 9);
    assert_eq!(LAYER_STAGE_COUNT, 2);
}

#[test]
fn r_s9_1_golden_stage_hashes_match_on_every_platform() {
    if update_golden() {
        write_golden(&golden_text());
        return;
    }
    assert_eq!(golden_file(), golden_text());
}

#[allow(clippy::expect_used)]
fn write_golden(text: &str) {
    std::fs::write(GOLDEN_FILE, text).expect(MISSING);
}

fn changed(seed: u64, change: fn(&mut WorldParams)) -> WorldData {
    let mut params = WorldParams::default();
    change(&mut params);
    generate_world(seed, &params)
}

#[test]
fn r_s9_2_a_geology_change_keeps_steps_1_to_9_and_changes_deposits() {
    let base = world(7);
    let other = changed(7, |p| p.layer3_per_mille = 0);
    assert_eq!(base.stage_hashes, other.stage_hashes);
    assert_ne!(base.layer_hashes[0], other.layer_hashes[0]);
    assert_ne!(base.layer_hashes[1], other.layer_hashes[1]);
}

#[test]
fn r_s9_2_a_deposit_change_keeps_steps_1_to_10() {
    let base = world(7);
    let other = changed(7, |p| p.deposit_base_per_10k += 20);
    assert_eq!(base.stage_hashes, other.stage_hashes);
    assert_eq!(base.layer_hashes[0], other.layer_hashes[0]);
    assert_ne!(base.layer_hashes[1], other.layer_hashes[1]);
}

#[test]
fn r_s9_2_a_biome_change_keeps_steps_1_to_8() {
    let base = world(7);
    let other = changed(7, |p| p.glacier_max_temp += 12);
    assert_eq!(base.stage_hashes[..8], other.stage_hashes[..8]);
    assert_ne!(base.layer_hashes[0], other.layer_hashes[0]);
}

#[test]
fn r_s9_3_same_seed_gives_same_layer_hashes_and_seeds_differ() {
    let a = world(11);
    let b = world(11);
    let c = world(12);
    assert_eq!(a.layer_hashes, b.layer_hashes);
    assert_eq!(a.layer_seeds, b.layer_seeds);
    assert_ne!(a.layer_hashes, c.layer_hashes);
    assert_ne!(a.layer_seeds[0], a.layer_seeds[1]);
}

#[test]
fn r_s9_4_every_deposit_lies_on_a_host_rock_with_affinity() {
    let size = WORLD_SIZE as i32;
    for seed in 1..=20u64 {
        let w = world(seed);
        for y in 0..size {
            for x in 0..size {
                let Some((mineral, richness)) = w.deposits.at(x, y) else {
                    continue;
                };
                assert!((1..=3).contains(&richness));
                let layer = usize::from(w.deposits.host.get(x, y).copied().unwrap_or(9));
                let rock = w.geology.rock_at(x, y, layer);
                let rock = rock.unwrap_or(Rock::Tuff);
                assert!(w.geology.rock_at(x, y, layer).is_some());
                assert!(affinity(rock, mineral) > 0);
                let biome = w.biome.get(x, y).copied();
                assert!(biome != Some(WorldBiome::Ocean) && biome != Some(WorldBiome::Lake));
            }
        }
    }
}

#[test]
fn r_s9_5_every_province_has_2_or_3_distinct_layers_and_soil_is_bounded() {
    let max = WorldParams::default().soil_max;
    for seed in 1..=20u64 {
        let w = world(seed);
        let g = &w.geology;
        assert_eq!(g.settings.len(), g.layers.len());
        assert_eq!(g.settings.len(), g.layer_count.len());
        for (i, &count) in g.layer_count.iter().enumerate() {
            assert!(count == 2 || count == 3);
            let n = usize::from(count);
            for a in 0..n {
                for b in (a + 1)..n {
                    assert!(g.layers[i][a] != g.layers[i][b]);
                }
            }
        }
        for &p in g.province.as_slice() {
            assert!(usize::from(p) < g.settings.len());
        }
        for (i, &s) in g.soil.as_slice().iter().enumerate() {
            assert!(i32::from(s) <= max);
            if w.biome.as_slice()[i] == WorldBiome::Ocean {
                assert_eq!(s, 0);
            }
        }
    }
}

#[test]
fn r_s9_6_each_world_has_all_minerals_and_a_sane_deposit_count() {
    for seed in 1..=20u64 {
        let w = world(seed);
        let mut per = [0usize; Mineral::COUNT];
        let mut rich = [0usize; 4];
        for (i, &m) in w.deposits.mineral.as_slice().iter().enumerate() {
            if m != 0 {
                per[usize::from(m) - 1] += 1;
                rich[usize::from(w.deposits.richness.as_slice()[i])] += 1;
            }
        }
        for count in per {
            assert!(count >= 10);
        }
        let total = w.deposits.count();
        assert!((300..6000).contains(&total));
        assert_eq!(total, per.iter().sum::<usize>());
        assert_eq!(rich[0], 0);
        assert!(rich[1] > rich[2] && rich[2] > rich[3] && rich[3] > 0);
    }
}

#[test]
fn r_s9_7_rock_decides_the_mineral_mix_and_the_settings_differ() {
    let mut seen = [false; 4];
    let mut tin_on_granite = 0usize;
    let mut tin_total = 0usize;
    for seed in 1..=10u64 {
        let w = world(seed);
        for &s in &w.geology.settings {
            seen[s as usize] = true;
        }
        for y in 0..WORLD_SIZE as i32 {
            for x in 0..WORLD_SIZE as i32 {
                if let Some((Mineral::Tin, _)) = w.deposits.at(x, y) {
                    tin_total += 1;
                    let layer = usize::from(w.deposits.host.get(x, y).copied().unwrap_or(0));
                    if w.geology.rock_at(x, y, layer) == Some(Rock::Granite) {
                        tin_on_granite += 1;
                    }
                }
            }
        }
    }
    assert!(seen.iter().filter(|&&b| b).count() >= 3);
    assert!(tin_total > 0);
    assert!(tin_on_granite * 100 / tin_total >= 60);
}

#[test]
fn r_s9_8_soil_is_deeper_on_river_land_than_on_mountains() {
    let w = world(3);
    let (mut river, mut rn, mut high, mut hn) = (0u32, 0u32, 0u32, 0u32);
    for (i, &s) in w.geology.soil.as_slice().iter().enumerate() {
        let b = w.biome.as_slice()[i];
        if b == WorldBiome::Floodplain {
            river += u32::from(s);
            rn += 1;
        }
        if b == WorldBiome::HighMountain {
            high += u32::from(s);
            hn += 1;
        }
    }
    assert!(rn > 0 && hn > 0);
    assert!(river / rn > high / hn + 10);
}

/// P-05: world generation steps 1 to 11 inside 300 ms in release.
/// Run: cargo test -p worldgen --release --test world_s9 -- --ignored --nocapture
#[test]
#[ignore = "timing test, run in release"]
#[allow(clippy::disallowed_methods)]
fn p05_world_generation_time_steps_1_to_11() {
    let mut worst = std::time::Duration::ZERO;
    for seed in 1..=10u64 {
        let start = std::time::Instant::now();
        let _w = world(seed);
        worst = worst.max(start.elapsed());
    }
    println!("worst {worst:?}");
    assert!(worst.as_millis() < 300);
}
