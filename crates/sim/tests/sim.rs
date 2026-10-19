//! Scenario, save, replay, golden hash, and timing tests for the `sim` crate.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use base::Pcg32;
use base::hash::Fnv1a;
use sim::scenario::sim_from_ascii;
use sim::{ColonistId, Command, Event, PathFinder, Sim, save};
use worldgen::{EmbarkSummary, generate_local};

fn forest_sim(seed: u64) -> Sim {
    let data = generate_local(&EmbarkSummary::temperate_forest());
    Sim::new(&data, seed, 0xC0FFEE)
}

fn colonist_ids(sim: &Sim) -> Vec<ColonistId> {
    sim.colonists().iter().map(|(id, _)| id).collect()
}

/// The command log of a seeded random-walk: at some ticks, a colonist gets a random goal.
fn random_commands(sim: &Sim, rng: &mut Pcg32, tick: u64) -> Vec<Command> {
    if !tick.is_multiple_of(25) {
        return Vec::new();
    }
    let ids = colonist_ids(sim);
    let id = ids[rng.below(ids.len() as u32) as usize];
    let w = sim.map().width();
    let h = sim.map().height();
    // Goals near the center, so most of them are reachable.
    let x = (w / 2) as i32 + rng.range(-40, 40);
    let y = (h / 2) as i32 + rng.range(-40, 40);
    vec![Command::MoveColonist {
        id,
        x: x as u16,
        y: y as u16,
    }]
}

fn run_random(sim: &mut Sim, ticks: u64, cmd_seed: u64) -> Vec<(u64, Vec<Command>)> {
    let mut rng = Pcg32::from_seed(cmd_seed);
    let mut log = Vec::new();
    for _ in 0..ticks {
        let cmds = random_commands(sim, &mut rng, sim.tick());
        if !cmds.is_empty() {
            log.push((sim.tick(), cmds.clone()));
        }
        sim.step(&cmds);
    }
    log
}

#[test]
fn colonist_walks_around_a_wall() {
    let mut sim = sim_from_ascii(
        "
        ..........#.........
        ..........#.........
        .c........#.........
        ..........#.........
        ..........#.........
        ..........#.........
        ....................",
        7,
    )
    .unwrap();
    let id = colonist_ids(&sim)[0];
    sim.step(&[Command::MoveColonist { id, x: 18, y: 2 }]);
    let mut visited = Vec::new();
    let mut arrived = false;
    for _ in 0..1_000 {
        sim.step(&[]);
        let c = sim.colonist(id).unwrap();
        assert!(
            sim.map().passable(i32::from(c.x), i32::from(c.y)),
            "colonist inside a wall"
        );
        visited.push(c.tile());
        for e in sim.drain_events() {
            if matches!(e, Event::ColonistArrived { colonist, .. } if colonist == id) {
                arrived = true;
            }
        }
        if arrived {
            break;
        }
    }
    assert!(arrived, "no arrival event");
    assert_eq!(sim.colonist(id).unwrap().tile(), (18, 2));
    assert!(
        visited.contains(&(10, 6)),
        "must use the gap in the bottom row"
    );
    assert!(!visited.iter().any(|&(x, y)| x == 10 && y < 6));
}

#[test]
fn colonist_reaches_a_reachable_target_in_the_expected_time() {
    let mut sim = sim_from_ascii("c.....", 1).unwrap();
    let id = colonist_ids(&sim)[0];
    sim.step(&[Command::MoveColonist { id, x: 5, y: 0 }]);
    // 5 straight steps, 10 ticks each; the first tick of the move was step() above.
    for _ in 0..49 {
        sim.step(&[]);
    }
    assert_eq!(sim.colonist(id).unwrap().tile(), (5, 0));
    assert!(sim.colonist(id).unwrap().path.is_empty());
}

#[test]
fn unreachable_move_fails_in_the_same_tick_with_an_event() {
    let mut sim = sim_from_ascii("c.#..\n..#..\n..#..", 1).unwrap();
    let id = colonist_ids(&sim)[0];
    sim.step(&[Command::MoveColonist { id, x: 4, y: 1 }]);
    assert_eq!(
        sim.drain_events(),
        vec![Event::PathFailed {
            colonist: id,
            x: 4,
            y: 1
        }]
    );
    assert_eq!(sim.path_queue_len(), 0);
    // A goal inside a wall and a goal outside the map fail the same way.
    sim.step(&[Command::MoveColonist { id, x: 2, y: 0 }]);
    sim.step(&[Command::MoveColonist { id, x: 99, y: 0 }]);
    assert_eq!(sim.drain_events().len(), 2);
    assert_eq!(sim.colonist(id).unwrap().tile(), (0, 0));
}

#[test]
fn path_requests_obey_the_search_budget() {
    // 30 colonists in a row, all asked to move in the same tick.
    let row = format!("{}\n{}", "c".repeat(30), ".".repeat(30));
    let mut sim = sim_from_ascii(&row, 1).unwrap();
    let cmds: Vec<Command> = colonist_ids(&sim)
        .into_iter()
        .map(|id| Command::MoveColonist { id, x: 0, y: 1 })
        .collect();
    sim.step(&cmds);
    assert_eq!(sim.path_stats().searches, 20);
    assert_eq!(sim.path_queue_len(), 10);
    sim.step(&[]);
    assert_eq!(sim.path_stats().searches, 10);
    assert_eq!(sim.path_queue_len(), 0);
}

// Save tests (TECHNICAL_DESIGN 11.1).

#[test]
fn save_load_gives_an_equal_hash() {
    let mut sim = forest_sim(5);
    run_random(&mut sim, 300, 11);
    let bytes = save::to_bytes(&sim);
    let loaded = save::from_bytes(&bytes).unwrap();
    assert_eq!(sim.state_hash(), loaded.state_hash());
    assert_eq!(loaded.content_hash(), 0xC0FFEE);
    assert_eq!(loaded.regions(), sim.regions(), "caches are rebuilt");
}

#[test]
fn save_load_then_1000_ticks_equals_1000_ticks_without_save() {
    let mut a = forest_sim(5);
    run_random(&mut a, 300, 11);
    let mut b = save::from_bytes(&save::to_bytes(&a)).unwrap();
    // Same command stream for both: continue the same generator state.
    let mut rng_a = Pcg32::from_seed(77);
    let mut rng_b = Pcg32::from_seed(77);
    for _ in 0..1_000 {
        let ca = random_commands(&a, &mut rng_a, a.tick());
        let cb = random_commands(&b, &mut rng_b, b.tick());
        assert_eq!(ca, cb);
        a.step(&ca);
        b.step(&cb);
    }
    assert_eq!(a.state_hash(), b.state_hash());
}

#[test]
fn seed_and_command_log_reproduce_the_final_hash() {
    let mut first = forest_sim(9);
    let log = run_random(&mut first, 1_500, 123);
    assert!(!log.is_empty());
    let mut replay = forest_sim(9);
    let mut next = 0;
    for tick in 0..1_500u64 {
        let cmds: &[Command] = match log.get(next) {
            Some((t, c)) if *t == tick => {
                next += 1;
                c
            }
            _ => &[],
        };
        replay.step(cmds);
    }
    assert_eq!(first.state_hash(), replay.state_hash());
}

#[test]
fn damaged_bytes_return_err_and_never_panic() {
    let mut sim = forest_sim(3);
    run_random(&mut sim, 100, 5);
    let bytes = save::to_bytes(&sim);
    // Every truncation.
    for len in (0..bytes.len()).step_by(997).chain([0, 1, 27, 28, 29]) {
        assert!(save::from_bytes(&bytes[..len]).is_err(), "len {len}");
    }
    // Flipped bytes in the header and in the payload.
    let mut rng = Pcg32::from_seed(99);
    for i in 0..200 {
        let mut bad = bytes.clone();
        let pos = if i < 28 {
            i
        } else {
            rng.below(bad.len() as u32) as usize
        };
        bad[pos] ^= 1 << rng.below(8);
        assert!(save::from_bytes(&bad).is_err(), "flip at {pos}");
    }
    // Random garbage and a valid header with garbage payload.
    let garbage: Vec<u8> = (0..500).map(|_| rng.below(256) as u8).collect();
    assert!(save::from_bytes(&garbage).is_err());
    let mut fake = bytes[..28].to_vec();
    fake.extend_from_slice(&garbage);
    assert!(save::from_bytes(&fake).is_err());
    // Extra bytes at the end.
    let mut longer = bytes.clone();
    longer.push(0);
    assert!(save::from_bytes(&longer).is_err());
}

#[test]
fn a_save_of_a_256_map_fits_the_size_target() {
    let sim = forest_sim(1);
    assert!(save::to_bytes(&sim).len() < 10 * 1024 * 1024, "P-10");
}

// Golden hashes.

/// 100 runs of the same scenario give the same hash.
fn ascii_scenario_hash() -> u64 {
    let mut sim = sim_from_ascii(
        "
        ccc.........#...........
        ............#...........
        ..~~~.......#...........
        ..~~~.......#.....T.....
        ..~~~.......#...........
        ............#...........
        ....................####
        ........................",
        42,
    )
    .unwrap();
    let ids = colonist_ids(&sim);
    let goals = [(23u16, 0u16), (20, 5), (0, 7)];
    let cmds: Vec<Command> = ids
        .iter()
        .zip(goals)
        .map(|(&id, (x, y))| Command::MoveColonist { id, x, y })
        .collect();
    sim.step(&cmds);
    for _ in 0..600 {
        sim.step(&[]);
    }
    sim.state_hash()
}

#[test]
fn ascii_path_hash_is_identical_over_100_runs() {
    let first = ascii_scenario_hash();
    for _ in 0..99 {
        assert_eq!(ascii_scenario_hash(), first);
    }
}

fn forest_sim_hash() -> u64 {
    let mut sim = forest_sim(1);
    run_random(&mut sim, 1_000, 2024);
    sim.state_hash()
}

/// Hash of 40 A* paths (cost and tiles) on the forest map.
fn forest_paths_hash() -> u64 {
    let data = generate_local(&EmbarkSummary::temperate_forest());
    let sim = Sim::new(&data, 1, 0);
    let map = sim.map();
    let mut rng = Pcg32::from_seed(31);
    let mut finder = PathFinder::default();
    let mut out = Vec::new();
    let mut h = Fnv1a::new();
    let mut done = 0;
    while done < 40 {
        let a = (
            rng.below(map.width()) as u16,
            rng.below(map.height()) as u16,
        );
        let b = (
            rng.below(map.width()) as u16,
            rng.below(map.height()) as u16,
        );
        let r = finder.find(map, a, b, &mut out);
        h.write_u32(u32::from(r.found));
        h.write_u32(r.cost);
        for &(x, y) in &out {
            h.write_u32(u32::from(x) << 16 | u32::from(y));
        }
        done += 1;
    }
    h.finish()
}

#[test]
fn path_golden_hashes() {
    let actual = format!(
        "sim_ascii_600_ticks={:016x}\nsim_forest_seed1_1000_ticks={:016x}\nastar_forest_40_paths={:016x}\n",
        ascii_scenario_hash(),
        forest_sim_hash(),
        forest_paths_hash()
    );
    assert_eq!(forest_sim_hash(), forest_sim_hash(), "repeatable");
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/golden.txt");
    if std::env::var_os("UPDATE_GOLDEN").is_some() {
        std::fs::write(path, &actual).expect("write golden.txt");
        return;
    }
    let expected = std::fs::read_to_string(path).expect("tests/golden.txt is missing");
    assert_eq!(
        expected.replace("\r\n", "\n"),
        actual,
        "golden hash changed; run with UPDATE_GOLDEN=1 only if the task permits"
    );
}

/// P-03 and P-04. Run: `cargo test -p sim --release -- --ignored --nocapture`
#[test]
#[ignore = "timing test, run in release"]
#[allow(clippy::disallowed_methods)]
fn p03_p04_path_timing() {
    let sim = forest_sim(1);
    let map = sim.map();
    let regions = sim.regions();
    let anchor = {
        let (_, c) = sim.colonists().iter().next().unwrap();
        regions.id_at(i32::from(c.x), i32::from(c.y))
    };
    let (w, h) = (map.width() as i32, map.height() as i32);
    let in_region = |x: i32, y: i32| regions.id_at(x, y) == anchor;
    let first = (0..w * h)
        .map(|i| (i % w, i / w))
        .find(|&(x, y)| in_region(x, y))
        .unwrap();
    let last = (0..w * h)
        .rev()
        .map(|i| (i % w, i / w))
        .find(|&(x, y)| in_region(x, y))
        .unwrap();
    let mut finder = PathFinder::default();
    let mut out = Vec::new();
    let a = (first.0 as u16, first.1 as u16);
    let b = (last.0 as u16, last.1 as u16);
    finder.find(map, a, b, &mut out);
    let runs = 200;
    let start = std::time::Instant::now();
    let mut expanded = 0;
    for _ in 0..runs {
        expanded = finder.find(map, a, b, &mut out).expanded;
    }
    let one = start.elapsed() / runs;
    println!(
        "P-03: one path {a:?} -> {b:?}: {one:?} ({} tiles, {expanded} nodes expanded)",
        out.len()
    );
    println!(
        "P-03 target 0.5 ms: {}",
        if one.as_micros() < 500 {
            "met"
        } else {
            "MISSED"
        }
    );

    // P-04: a full tick budget (20 searches).
    let start = std::time::Instant::now();
    for _ in 0..runs {
        for _ in 0..20 {
            finder.find(map, a, b, &mut out);
        }
    }
    println!("20 long searches: {:?}", start.elapsed() / runs);
}
