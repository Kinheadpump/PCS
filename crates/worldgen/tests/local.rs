//! Local map tests: determinism, terrain invariants, golden hashes, timing.

use worldgen::{EmbarkSummary, Ground, LocalMapData, Ore, RockKind, generate_local};

fn presets() -> [(&'static str, EmbarkSummary); 3] {
    [
        ("temperate_forest", EmbarkSummary::temperate_forest()),
        ("cold_hills", EmbarkSummary::cold_hills()),
        ("dry_plain", EmbarkSummary::dry_plain()),
    ]
}

/// Size of the largest 4-connected area where `pred(x, y)` is true.
fn largest_area(map: &LocalMapData, pred: impl Fn(i32, i32) -> bool) -> usize {
    let (w, h) = (map.ground.width() as i32, map.ground.height() as i32);
    let mut seen = vec![false; (w * h) as usize];
    let mut best = 0;
    for sy in 0..h {
        for sx in 0..w {
            if seen[(sy * w + sx) as usize] || !pred(sx, sy) {
                continue;
            }
            let mut size = 0;
            let mut stack = vec![(sx, sy)];
            seen[(sy * w + sx) as usize] = true;
            while let Some((x, y)) = stack.pop() {
                size += 1;
                for (nx, ny) in [(x + 1, y), (x - 1, y), (x, y + 1), (x, y - 1)] {
                    if nx >= 0 && ny >= 0 && nx < w && ny < h {
                        let i = (ny * w + nx) as usize;
                        if !seen[i] && pred(nx, ny) {
                            seen[i] = true;
                            stack.push((nx, ny));
                        }
                    }
                }
            }
            best = best.max(size);
        }
    }
    best
}

#[test]
fn local_generation_is_repeatable() {
    for (name, embark) in presets() {
        assert_eq!(
            generate_local(&embark).hash(),
            generate_local(&embark).hash(),
            "{name}"
        );
    }
}

#[test]
fn local_seed_changes_the_map() {
    let a = EmbarkSummary::temperate_forest();
    assert_ne!(
        generate_local(&a).hash(),
        generate_local(&a.with_seed(99)).hash()
    );
}

#[test]
fn local_presets_have_a_rock_area_and_a_water_area() {
    for (name, embark) in presets() {
        let map = generate_local(&embark);
        let rock = largest_area(&map, |x, y| map.rock.get(x, y) != Some(&RockKind::None));
        let water = largest_area(&map, |x, y| map.ground.get(x, y) == Some(&Ground::Water));
        assert!(rock >= 200, "{name}: rock area {rock}");
        assert!(water >= 1, "{name}: water area {water}");
    }
}

#[test]
fn local_forest_and_hills_have_trees() {
    for embark in [
        EmbarkSummary::temperate_forest(),
        EmbarkSummary::cold_hills(),
    ] {
        let map = generate_local(&embark);
        assert!(map.tree.as_slice().iter().any(|&t| t));
    }
}

#[test]
fn local_forest_has_more_trees_than_dry_plain() {
    let count = |e: &EmbarkSummary| {
        generate_local(e)
            .tree
            .as_slice()
            .iter()
            .filter(|&&t| t)
            .count()
    };
    assert!(count(&EmbarkSummary::temperate_forest()) > 4 * count(&EmbarkSummary::dry_plain()));
}

#[test]
fn local_layers_are_consistent() {
    for (name, embark) in presets() {
        let map = generate_local(&embark);
        for (x, y, &rock) in map.rock.iter() {
            let ground = map.ground.get(x, y).copied();
            let ore = map.ore.get(x, y).copied();
            let tree = map.tree.get(x, y).copied();
            if rock == RockKind::None {
                assert_ne!(ground, Some(Ground::Stone), "{name} {x},{y}");
                assert_eq!(ore, Some(Ore::None), "{name}: ore outside rock {x},{y}");
            } else {
                assert_eq!(ground, Some(Ground::Stone), "{name} {x},{y}");
                assert_eq!(tree, Some(false), "{name}: tree in rock {x},{y}");
            }
            if ground == Some(Ground::Water) {
                assert_eq!(tree, Some(false), "{name}: tree in water {x},{y}");
            }
        }
        let ore_tiles = map
            .ore
            .as_slice()
            .iter()
            .filter(|&&o| o != Ore::None)
            .count();
        assert!(ore_tiles > 0, "{name}: no ore");
    }
}

#[test]
fn local_golden_hashes() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/golden.txt");
    let actual: String = presets()
        .iter()
        .map(|(name, e)| format!("local_{name}={:016x}\n", generate_local(e).hash()))
        .collect();
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

/// P-07: local generation <= 200 ms in release.
/// Run: `cargo test -p worldgen --release -- --ignored --nocapture`
#[test]
#[ignore = "timing test, run in release"]
#[allow(clippy::disallowed_methods)]
fn p07_local_generation_time() {
    let embark = EmbarkSummary::cold_hills();
    let start = std::time::Instant::now();
    let map = generate_local(&embark);
    let elapsed = start.elapsed();
    println!("local generation: {elapsed:?} (hash {:016x})", map.hash());
    assert!(elapsed.as_millis() < 200);
}
