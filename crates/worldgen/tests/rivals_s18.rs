//! Sprint 18 tests (worldgen part): rival colonies near the embark cell (R-070).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use worldgen::history::{EventKind, NEVER, SiteKind};
use worldgen::{
    EmbarkRival, History, WORLD_SIZE, WorldData, WorldParams, check_embark_content,
    embark_from_world, generate_history, generate_world,
};

fn world(seed: u64) -> (WorldData, History) {
    let w = generate_world(seed, &WorldParams::default());
    let h = generate_history(&w, None).expect("tables");
    (w, h)
}

/// Every embark cell on a grid with its rivals.
fn rival_cells_step(
    w: &WorldData,
    h: &History,
    step: usize,
) -> Vec<((i32, i32), Vec<EmbarkRival>)> {
    let mut cells = Vec::new();
    for y in (0..WORLD_SIZE as i32).step_by(step) {
        for x in (0..WORLD_SIZE as i32).step_by(step) {
            if let Some((_, sites)) = embark_from_world(w, h, x, y) {
                cells.push(((x, y), sites.rivals));
            }
        }
    }
    cells
}

fn rival_cells(w: &WorldData, h: &History) -> Vec<((i32, i32), Vec<EmbarkRival>)> {
    rival_cells_step(w, h, 3)
}

#[test]
fn r070_the_embark_table_is_valid() {
    check_embark_content().unwrap();
}

#[test]
fn r070_a_rival_is_a_living_settlement_of_a_living_civilization_near_the_cell() {
    let (w, h) = world(7);
    let cells = rival_cells(&w, &h);
    let with: Vec<_> = cells.iter().filter(|(_, r)| !r.is_empty()).collect();
    assert!(!with.is_empty(), "some cell has a rival colony");
    for ((x, y), rivals) in &with {
        assert!(rivals.len() <= 3);
        let mut last = 0;
        for r in rivals {
            let site = h.site(r.site).unwrap();
            assert_eq!(site.kind, SiteKind::Settlement);
            assert_eq!(site.ended, NEVER);
            assert_eq!(h.civ(r.civ).unwrap().collapsed, NEVER);
            let d = (i32::from(site.x) - x)
                .abs()
                .max((i32::from(site.y) - y).abs());
            assert!(d <= 3 && d == i32::from(r.distance));
            assert!(r.distance >= last, "nearest first");
            last = r.distance;
        }
    }
}

#[test]
fn r070_strength_is_3_to_12_and_hostility_is_0_to_100() {
    let (w, h) = world(7);
    let mut seen = 0;
    for (_, rivals) in rival_cells(&w, &h) {
        for r in rivals {
            seen += 1;
            assert!((3..=12).contains(&r.strength), "{r:?}");
            assert!(r.base_hostility <= 100 && r.edge < 4);
        }
    }
    assert!(seen > 0);
}

#[test]
fn r070_a_war_in_the_embark_area_raises_the_base_hostility() {
    let mut at_war = 0;
    let mut at_peace = 0;
    for seed in 1..=4 {
        let (w, h) = world(seed);
        for ((x, y), rivals) in rival_cells_step(&w, &h, 1) {
            for r in rivals {
                let war: u32 = h
                    .events
                    .iter()
                    .filter(|e| e.actors[0] == r.civ || e.actors[1] == r.civ)
                    .filter(|e| {
                        h.site(e.place).is_some_and(|s| {
                            (i32::from(s.x) - x).abs() <= 1 && (i32::from(s.y) - y).abs() <= 1
                        })
                    })
                    .map(|e| match e.kind {
                        EventKind::Battle => 15,
                        EventKind::Raze => 25,
                        _ => 0,
                    })
                    .sum();
                let aggression = u32::from(h.civ(r.civ).unwrap().aggression) / 4;
                assert_eq!(
                    u32::from(r.base_hostility),
                    (aggression + war).min(100),
                    "{r:?}"
                );
                if war > 0 {
                    at_war += 1;
                } else {
                    at_peace += 1;
                }
            }
        }
    }
    assert!(at_peace > 0);
    assert!(at_war > 0, "some rival fought in the embark area");
}

#[test]
fn r070_rivals_are_deterministic_and_reach_the_local_map() {
    let (w, h) = world(7);
    let (x, y, sites) = rival_cells(&w, &h)
        .into_iter()
        .find(|(_, r)| !r.is_empty())
        .map(|((x, y), _)| (x, y, embark_from_world(&w, &h, x, y).unwrap()))
        .map(|(x, y, (_, s))| (x, y, s))
        .unwrap();
    let again = embark_from_world(&w, &h, x, y).unwrap().1;
    assert_eq!(sites.rivals, again.rivals);
    let (summary, _) = embark_from_world(&w, &h, x, y).unwrap();
    let map = worldgen::generate_local_with_sites(&summary, &sites);
    assert_eq!(map.rivals, sites.rivals);
}
