//! Sprint 10 tests: step 13 (world history). Rule 8 over 100 seeds, R-040 map results,
//! the prefix property, determinism, and the golden history hashes.

#![allow(clippy::expect_used)]

use worldgen::history::{EventKind, History, HistoryStage, NEVER, NO_EVENT, SiteKind};
use worldgen::{
    WORLD_SIZE, WorldData, WorldParams, check_history_content, generate_history, generate_world,
    try_generate_world,
};

const GOLDEN_SEEDS: [u64; 3] = [1, 2, 3];
const GOLDEN_FILE: &str = "tests/golden_history_s10.txt";
const MISSING: &str = "golden file is missing";
const UPDATE_VAR: &str = "UPDATE_GOLDEN";
const CHUNK: u64 = 25;

fn world(seed: u64) -> WorldData {
    generate_world(seed, &WorldParams::default())
}

fn history(w: &WorldData, years: Option<u32>) -> History {
    generate_history(w, years).expect("embedded history tables are valid")
}

fn hex(hash: u64) -> String {
    let mut s = String::new();
    for shift in (0..16).rev() {
        let nibble = ((hash >> (shift * 4)) & 15) as u32;
        s.push(char::from_digit(nibble, 16).unwrap_or(char::from(48)));
    }
    s
}

fn golden_text() -> String {
    let mut out = String::new();
    for seed in GOLDEN_SEEDS {
        let h = history(&world(seed), None);
        for stage in HistoryStage::ALL {
            out.push_str(&format!(
                "seed{seed}.{}={}",
                stage.name(),
                hex(h.hashes[stage as usize])
            ));
            out.push(char::from(10));
        }
    }
    out
}

/// Read a file. lines() drops a carriage return, so a Windows checkout compares equal.
fn read_lines(path: &str) -> String {
    let text = std::fs::read_to_string(path).expect(MISSING);
    let mut out = String::new();
    for line in text.lines() {
        out.push_str(line);
        out.push(char::from(10));
    }
    out
}

fn cell(x: u16, y: u16) -> usize {
    usize::from(y) * WORLD_SIZE as usize + usize::from(x)
}

#[test]
fn r_s10_1_embedded_history_content_is_valid() {
    assert!(check_history_content().is_ok());
    assert!(try_generate_world(1, &WorldParams::default()).is_ok());
}

#[test]
#[allow(clippy::expect_used)]
fn r_s10_2_golden_history_hashes_match_on_every_platform() {
    if std::env::var_os(UPDATE_VAR).is_some() {
        std::fs::write(GOLDEN_FILE, golden_text()).expect(MISSING);
        return;
    }
    assert_eq!(read_lines(GOLDEN_FILE), golden_text());
}

#[test]
fn r_s10_3_history_is_deterministic_and_leaves_the_world_alone() {
    let w = world(7);
    let before = (w.stage_hashes, w.layer_hashes, w.region_hashes);
    let a = history(&w, None);
    let b = history(&w, None);
    assert_eq!(a.hashes, b.hashes);
    assert_eq!(a.events.len(), b.events.len());
    assert_eq!(before, (w.stage_hashes, w.layer_hashes, w.region_hashes));
    let other = history(&world(8), None);
    assert_ne!(a.hashes, other.hashes);
    assert_ne!(a.seed, other.seed);
    for stage in HistoryStage::ALL {
        assert!(!w.stage_hashes.contains(&a.hashes[stage as usize]));
    }
}

#[test]
fn r_s10_4_short_history_is_a_prefix_of_a_long_history() {
    for seed in [1u64, 2, 3] {
        let w = world(seed);
        let short = history(&w, Some(1000));
        let long = history(&w, Some(3000));
        assert_eq!(short.years, 1000);
        assert_eq!(long.years, 3000);
        let n = short.events.len();
        assert!(n > 0 && n < long.events.len());
        assert_eq!(short.events[..], long.events[..n]);
        assert_eq!(short.alive_by_year[..], long.alive_by_year[..1000]);
    }
}

#[test]
fn r_s10_5_years_are_clamped_to_the_table_range() {
    let w = world(1);
    assert_eq!(history(&w, Some(1)).years, 1000);
    assert_eq!(history(&w, Some(999_999)).years, 5000);
    assert_eq!(history(&w, None).years, 3000);
}

/// R-040: each event type writes a map result.
#[test]
fn r040_each_event_type_writes_a_map_result() {
    let mut seen = [false; EventKind::COUNT];
    for seed in 1..=20u64 {
        let h = history(&world(seed), None);
        let owned = h.owner.as_slice().iter().filter(|&&o| o != 0).count();
        assert!(owned > 0, "seed {seed}: no owned cells");
        for (index, e) in h.events.iter().enumerate() {
            seen[e.kind as usize] = true;
            let index = index as u32;
            match e.kind {
                EventKind::FoundCiv => {
                    let c = h.civ(e.actors[0]).expect("civ");
                    assert_eq!(c.found_event, index);
                    assert_eq!(u32::from(c.capital), e.place);
                }
                EventKind::FoundSettlement => {
                    let s = h.settlement(e.actors[1]).expect("settlement");
                    let site = h.site(e.place).expect("site");
                    assert_eq!(s.site, site.id);
                    assert_eq!(site.event, index);
                    assert_eq!((s.x, s.y), (site.x, site.y));
                }
                EventKind::OpenMine => {
                    let site = h.site(e.place).expect("site");
                    assert_eq!(site.event, index);
                    assert!(site.mineral > 0);
                }
                EventKind::MineDepleted => {
                    let site = h.site(e.place).expect("site");
                    assert_eq!(site.cause, index);
                    assert!(h.depletion.as_slice()[cell(site.x, site.y)] > 0);
                }
                EventKind::BuildRoad => {
                    assert!(e.extra >= 2);
                    assert!(h.road.as_slice().iter().any(|&r| r > 0));
                }
                EventKind::Battle => {
                    let site = h.site(e.place).expect("site");
                    assert_eq!(site.event, index);
                    assert!(matches!(site.kind, SiteKind::Battlefield));
                }
                EventKind::Raze | EventKind::CivCollapse => {
                    let ruins = h.sites.iter().filter(|s| s.cause == index);
                    assert!(ruins.clone().count() > 0 || e.kind == EventKind::CivCollapse);
                    for r in ruins {
                        assert!(matches!(r.kind, SiteKind::Ruin));
                    }
                }
                EventKind::ArtifactLost => {
                    let a = h
                        .artifacts
                        .iter()
                        .find(|a| a.event == index)
                        .expect("artifact");
                    assert_eq!(a.site, e.place);
                    assert!(h.site(a.site).is_some());
                }
                EventKind::Seal => {
                    let site = h.site(e.place).expect("site");
                    assert_eq!(site.cause, index);
                    assert!(matches!(site.kind, SiteKind::Sealed));
                    assert!(h.sanctity.as_slice().iter().any(|&s| s > 0));
                }
            }
        }
    }
    for kind in EventKind::ALL {
        assert!(seen[kind as usize], "{} never happened", kind.name());
    }
}

/// Rule 8 on one seed.
fn check_rule_8(seed: u64) {
    let w = world(seed);
    let h = history(&w, None);
    let t = worldgen::history::history_tables().expect("tables");
    let r = &t.rules;
    assert!(h.civs.len() <= r.max_civs as usize, "seed {seed}");
    assert!(
        h.settlements.len() <= r.max_settlements as usize,
        "seed {seed}"
    );
    assert!(h.sites.len() <= r.max_sites as usize, "seed {seed}");
    assert!(h.artifacts.len() <= r.max_artifacts as usize, "seed {seed}");
    assert!(h.events.len() <= r.max_events as usize, "seed {seed}");
    // Alive civilizations stay in 2..=8 for at least 90% of the years.
    assert_eq!(h.alive_by_year.len(), h.years as usize);
    let ok = h
        .alive_by_year
        .iter()
        .filter(|&&a| (2..=8).contains(&a))
        .count();
    assert!(
        ok * 10 >= h.alive_by_year.len() * 9,
        "seed {seed}: {ok} years in range"
    );
    // Event years do not decrease and stay inside the history.
    let mut last = 0;
    for e in &h.events {
        assert!(e.year >= last, "seed {seed}");
        assert!(u32::from(e.year) < h.years, "seed {seed}");
        last = e.year;
    }
    // Each ruin refers to a destruction or abandon event.
    for s in h.sites.iter().filter(|s| matches!(s.kind, SiteKind::Ruin)) {
        let cause = h.events.get(s.cause as usize).expect("ruin cause");
        assert!(
            matches!(cause.kind, EventKind::Raze | EventKind::CivCollapse),
            "seed {seed}"
        );
        assert_ne!(s.ended, NEVER, "seed {seed}");
        assert!(cause.year == s.ended, "seed {seed}");
    }
    // Each lost artifact has a valid site.
    for a in &h.artifacts {
        let site = h.site(a.site).expect("artifact site");
        assert!(usize::from(site.region) <= w.regions.count(), "seed {seed}");
        assert!(site.region > 0, "seed {seed}");
        let e = h.events.get(a.event as usize).expect("artifact event");
        assert!(matches!(e.kind, EventKind::ArtifactLost), "seed {seed}");
    }
    // Small regions are ignored (D-053) and every site lies inside its region.
    for s in &h.sites {
        let region = w
            .regions
            .info
            .get(usize::from(s.region) - 1)
            .expect("region");
        if matches!(
            s.kind,
            SiteKind::Settlement | SiteKind::Mine | SiteKind::Ruin
        ) {
            assert!(region.size >= r.min_region_cells, "seed {seed}");
        }
        assert_eq!(
            w.regions.id.as_slice()[cell(s.x, s.y)],
            s.region,
            "seed {seed}"
        );
    }
    // Event references stay valid.
    for c in &h.civs {
        assert!(h.events.get(c.found_event as usize).is_some());
        assert!(c.end_event == NO_EVENT || h.events.get(c.end_event as usize).is_some());
    }
}

fn rule_8_chunk(n: u64) {
    for seed in (n * CHUNK + 1)..=(n * CHUNK + CHUNK) {
        check_rule_8(seed);
    }
}

#[test]
fn rule_8_history_limits_hold_seeds_001_to_025() {
    rule_8_chunk(0);
}

#[test]
fn rule_8_history_limits_hold_seeds_026_to_050() {
    rule_8_chunk(1);
}

#[test]
fn rule_8_history_limits_hold_seeds_051_to_075() {
    rule_8_chunk(2);
}

#[test]
fn rule_8_history_limits_hold_seeds_076_to_100() {
    rule_8_chunk(3);
}

/// P-06: history for 3,000 years inside 1 s in release (run with the ignored flag).
#[test]
#[ignore = "timing test, run in release"]
#[allow(clippy::disallowed_methods)]
fn p06_history_time_for_3000_years() {
    let mut worst = std::time::Duration::ZERO;
    for seed in 1..=10u64 {
        let w = world(seed);
        let start = std::time::Instant::now();
        let _h = history(&w, Some(3000));
        worst = worst.max(start.elapsed());
    }
    println!("worst {worst:?}");
    assert!(worst.as_millis() < 1000);
}
