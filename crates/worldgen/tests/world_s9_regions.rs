//! Sprint 9 world tests: step 12 (regions, region graph, deposit repair).
//! Stage hash, rule 2 (earlier stages unchanged), and region invariants.
//! The world has a fixed size of 256 by 256 cells, so the invariants run over 30 seeds.

use worldgen::deposit::affinity;
use worldgen::{
    LayerStage, REGION_STAGE_COUNT, RegionStage, Stage, WORLD_SIZE, WorldBiome, WorldData,
    WorldParams, generate_world,
};

const GOLDEN_SEEDS: [u64; 3] = [1, 2, 3];
const GOLDEN_FILE: &str = "tests/golden_world_s9_regions.txt";
const S8_GOLDEN: &str = "tests/golden_world.txt";
const S9_GOLDEN: &str = "tests/golden_world_s9.txt";
const MISSING: &str = "golden file is missing";
const UPDATE_VAR: &str = "UPDATE_GOLDEN";
const SEEDS: u64 = 30;

fn world(seed: u64) -> WorldData {
    generate_world(seed, &WorldParams::default())
}

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
        for stage in RegionStage::ALL {
            let hash = w.region_hashes[stage as usize];
            out.push_str(&golden_line(seed, stage.name(), hash));
            out.push(char::from(10));
        }
    }
    out
}

/// Read a file. lines() drops a carriage return, so a Windows checkout compares equal.
#[allow(clippy::expect_used)]
fn read_lines(path: &str) -> String {
    let text = std::fs::read_to_string(path).expect(MISSING);
    let mut out = String::new();
    for line in text.lines() {
        out.push_str(line);
        out.push(char::from(10));
    }
    out
}

fn update_golden() -> bool {
    std::env::var_os(UPDATE_VAR).is_some()
}

#[test]
fn stage_hash_regions() {
    let a = world(1);
    let b = world(1);
    let c = world(2);
    let k = RegionStage::Regions as usize;
    assert_eq!(REGION_STAGE_COUNT, 1);
    assert_eq!(a.region_hashes[k], b.region_hashes[k]);
    assert_ne!(a.region_hashes[k], c.region_hashes[k]);
    assert_ne!(a.region_seeds[k], c.region_seeds[k]);
    assert_ne!(a.region_seeds[k], a.layer_seeds[1]);
    if update_golden() {
        return;
    }
    let key = golden_line(1, RegionStage::Regions.name(), a.region_hashes[k]);
    assert!(read_lines(GOLDEN_FILE).lines().any(|l| l == key), "{key}");
}

#[test]
#[allow(clippy::expect_used)]
fn r_s9_1_golden_region_hashes_match_on_every_platform() {
    if update_golden() {
        std::fs::write(GOLDEN_FILE, golden_text()).expect(MISSING);
        return;
    }
    assert_eq!(read_lines(GOLDEN_FILE), golden_text());
}

fn lines_of(seed_hashes: impl Fn(&WorldData) -> Vec<(&'static str, u64)>) -> String {
    let mut out = String::new();
    for seed in GOLDEN_SEEDS {
        let w = world(seed);
        for (name, hash) in seed_hashes(&w) {
            out.push_str(&golden_line(seed, name, hash));
            out.push(char::from(10));
        }
    }
    out
}

#[test]
fn r_s9_2_steps_1_to_11_match_the_unmodified_golden_files() {
    let s8 = lines_of(|w| {
        Stage::ALL
            .iter()
            .map(|&s| (s.name(), w.stage_hashes[s as usize]))
            .collect()
    });
    assert_eq!(read_lines(S8_GOLDEN), s8);
    let s9 = lines_of(|w| {
        LayerStage::ALL
            .iter()
            .map(|&s| (s.name(), w.layer_hashes[s as usize]))
            .collect()
    });
    assert_eq!(read_lines(S9_GOLDEN), s9);
}

#[test]
fn r_s9_2_a_region_change_keeps_steps_1_to_11_and_changes_step_12() {
    let base = world(7);
    let mut params = WorldParams::default();
    params.region_min_cells += 20;
    let other = generate_world(7, &params);
    assert_eq!(base.stage_hashes, other.stage_hashes);
    assert_eq!(base.layer_hashes, other.layer_hashes);
    assert_ne!(base.region_hashes, other.region_hashes);
    assert_eq!(
        base.deposits.mineral.as_slice(),
        other.deposits.mineral.as_slice()
    );
}

#[test]
fn r_s9_2_a_deposit_change_changes_the_region_hash() {
    let base = world(7);
    let mut params = WorldParams::default();
    params.deposit_base_per_10k += 20;
    let other = generate_world(7, &params);
    assert_eq!(base.stage_hashes, other.stage_hashes);
    assert_ne!(base.region_hashes, other.region_hashes);
}

fn is_land(w: &WorldData, i: usize) -> bool {
    let b = w.biome.as_slice()[i];
    b != WorldBiome::Ocean && b != WorldBiome::Lake
}

#[test]
fn r_s9_3_each_land_cell_is_in_one_region_and_water_is_in_none() {
    for seed in 1..=SEEDS {
        let w = world(seed);
        let ids = w.regions.id.as_slice();
        let mut sizes = vec![0u32; w.regions.count() + 1];
        for (i, &id) in ids.iter().enumerate() {
            assert_eq!(id != 0, is_land(&w, i), "seed {seed} cell {i}");
            assert!(usize::from(id) <= w.regions.count());
            sizes[usize::from(id)] += 1;
        }
        for (k, info) in w.regions.info.iter().enumerate() {
            assert!(info.size > 0);
            assert_eq!(info.size, sizes[k + 1], "seed {seed} region {}", k + 1);
        }
    }
}

#[test]
fn r_s9_5_the_land_region_count_is_300_to_500_on_90_percent_of_seeds() {
    let mut good = 0;
    for seed in 1..=SEEDS {
        let n = world(seed).regions.count();
        if (300..=500).contains(&n) {
            good += 1;
        }
    }
    assert!(good * 10 >= SEEDS * 9, "only {good} of {SEEDS} seeds");
}

fn neighbors_of(c: usize) -> Vec<usize> {
    let size = WORLD_SIZE as usize;
    let (x, y) = (c % size, c / size);
    let mut next = Vec::new();
    if x > 0 {
        next.push(c - 1);
    }
    if x + 1 < size {
        next.push(c + 1);
    }
    if y > 0 {
        next.push(c - size);
    }
    if y + 1 < size {
        next.push(c + size);
    }
    next
}

#[test]
fn r_s9_4_each_region_is_one_4_connected_area() {
    for seed in 1..=SEEDS {
        let w = world(seed);
        let ids = w.regions.id.as_slice();
        let mut seen = vec![false; ids.len()];
        let mut areas = vec![0u32; w.regions.count() + 1];
        for start in 0..ids.len() {
            if ids[start] == 0 || seen[start] {
                continue;
            }
            areas[usize::from(ids[start])] += 1;
            seen[start] = true;
            let mut stack = vec![start];
            while let Some(c) = stack.pop() {
                for n in neighbors_of(c) {
                    if !seen[n] && ids[n] == ids[start] {
                        seen[n] = true;
                        stack.push(n);
                    }
                }
            }
        }
        assert!(areas.iter().skip(1).all(|&n| n == 1), "seed {seed}");
    }
}

fn region_minerals(w: &WorldData) -> Vec<u8> {
    let mut mask = vec![0u8; w.regions.count() + 1];
    for (i, &id) in w.regions.id.as_slice().iter().enumerate() {
        let m = w.regions.deposits.mineral.as_slice()[i];
        if id == 0 || m == 0 {
            continue;
        }
        let slot = usize::from(id);
        mask[slot] |= 1u8 << (m - 1);
    }
    mask
}

#[test]
fn r_s9_6_each_land_region_has_at_least_one_deposit_type() {
    for seed in 1..=SEEDS {
        let w = world(seed);
        let mask = region_minerals(&w);
        for (k, info) in w.regions.info.iter().enumerate() {
            assert_ne!(mask[k + 1], 0);
            assert_eq!(mask[k + 1], info.minerals);
        }
    }
}

fn changed_cells(w: &WorldData) -> Vec<u32> {
    let (a, b) = (&w.deposits, &w.regions.deposits);
    let mut changed = Vec::new();
    for i in 0..a.mineral.as_slice().len() {
        if a.mineral.as_slice()[i] != b.mineral.as_slice()[i] {
            changed.push(i as u32);
            assert_eq!(a.mineral.as_slice()[i], 0);
        } else {
            assert_eq!(a.richness.as_slice()[i], b.richness.as_slice()[i]);
            assert_eq!(a.host.as_slice()[i], b.host.as_slice()[i]);
        }
    }
    changed
}

#[test]
fn r_s9_6_the_repair_is_an_overlay_and_uses_host_rock_with_affinity() {
    let mut repaired_total = 0;
    let size = WORLD_SIZE as usize;
    for seed in 1..=SEEDS {
        let w = world(seed);
        let b = &w.regions.deposits;
        let changed = changed_cells(&w);
        assert_eq!(changed, w.regions.repaired);
        repaired_total += changed.len();
        for &cell in &w.regions.repaired {
            let x = (cell as usize % size) as i32;
            let y = (cell as usize / size) as i32;
            let (mineral, richness) = b.at(x, y).unwrap_or((worldgen::Mineral::Coal, 0));
            assert_eq!(richness, 1);
            let layer = usize::from(b.host.get(x, y).copied().unwrap_or(9));
            let rock = w.geology.rock_at(x, y, layer);
            assert!(rock.is_some_and(|r| affinity(r, mineral) > 0));
        }
    }
    assert!(repaired_total > 0);
}

#[test]
fn r_s9_7_graph_edges_are_symmetric_sorted_and_costed() {
    for seed in 1..=SEEDS {
        let w = world(seed);
        let r = &w.regions;
        let sorted = r
            .edges
            .windows(2)
            .all(|p| (p[0].a, p[0].b) < (p[1].a, p[1].b));
        assert!(sorted);
        for e in &r.edges {
            assert!(e.a < e.b && usize::from(e.b) <= r.count());
            assert!(e.border > 0 && e.cost > 0);
            let ab = r.neighbors(e.a).iter().find(|l| l.to == e.b);
            let ba = r.neighbors(e.b).iter().find(|l| l.to == e.a);
            assert!(ab.is_some() && ba.is_some());
            assert_eq!(ab.map(|l| l.cost), ba.map(|l| l.cost));
        }
        let mut links = 0;
        for id in 1..=r.count() as u16 {
            let n = r.neighbors(id);
            assert!(n.windows(2).all(|p| p[0].to < p[1].to));
            assert!(n.iter().all(|l| l.to != id));
            links += n.len();
        }
        assert_eq!(links, r.edges.len() * 2);
    }
}

/// P-05: world generation steps 1 to 12 inside 300 ms in release (run with the ignored flag).
#[test]
#[ignore = "timing test, run in release"]
#[allow(clippy::disallowed_methods)]
fn p05_world_generation_time_steps_1_to_12() {
    let mut worst = std::time::Duration::ZERO;
    for seed in 1..=10u64 {
        let start = std::time::Instant::now();
        let _w = world(seed);
        worst = worst.max(start.elapsed());
    }
    println!("worst {worst:?}");
    assert!(worst.as_millis() < 300);
}
