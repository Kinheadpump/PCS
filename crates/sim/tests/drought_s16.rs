//! Sprint 16 tests (sim part): drought and irrigation (R-025), wildfire and fire spread
//! (R-024, R-026).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use sim::scenario::sim_from_ascii;
use sim::time::TICKS_PER_DAY;
use sim::{Command, DroughtEvent, GrowthCause, Sim, SimRules, save};
use std::sync::Arc;

/// Tick of the first day of summer (day 11 of the year).
const SUMMER: u64 = 10 * TICKS_PER_DAY;

fn quiet(text: &str, moisture: u8) -> Sim {
    let mut sim = sim_from_ascii(text, 1).unwrap();
    sim.debug_set_needs_enabled(false);
    sim.set_climate(110, 160, moisture);
    sim
}

fn with_rules(sim: &mut Sim, edit: impl FnOnce(&mut SimRules)) {
    let mut rules = sim.rules().clone();
    edit(&mut rules);
    sim.debug_set_rules(Arc::new(rules));
}

fn run(sim: &mut Sim, ticks: u64) {
    for _ in 0..ticks {
        sim.step(&[]);
    }
}

fn run_to(sim: &mut Sim, tick: u64) {
    while sim.tick() < tick {
        sim.step(&[]);
    }
}

#[test]
fn r025_a_drought_starts_after_5_days_without_rain() {
    let mut sim = quiet("cc....", 0);
    run_to(&mut sim, 4 * TICKS_PER_DAY);
    assert_eq!(sim.dry_days(), 4);
    assert!(!sim.drought_active());
    assert!(sim.drain_drought_events().is_empty());
    run_to(&mut sim, 5 * TICKS_PER_DAY);
    assert_eq!(sim.dry_days(), 5);
    assert!(sim.drought_active());
    assert_eq!(
        sim.drain_drought_events(),
        vec![DroughtEvent::DroughtBegan { day: 5 }]
    );
}

#[test]
fn r025_a_rain_day_ends_the_drought_and_resets_the_counter() {
    let mut sim = quiet("cc....", 255);
    let mut dry = 0u32;
    let mut saw_rain = false;
    for _ in 0..80 {
        run(&mut sim, TICKS_PER_DAY);
        if sim.weather_today().rain {
            dry = 0;
            saw_rain = true;
        } else {
            dry += 1;
        }
        assert_eq!(sim.dry_days(), dry, "day {}", sim.tick() / TICKS_PER_DAY);
        assert_eq!(sim.drought_active(), dry >= 5);
    }
    assert!(saw_rain);
}

#[test]
fn r025_a_drought_halves_crop_growth_except_dry_soil_and_irrigated_tiles() {
    let mut sim = quiet("c.......~", 0);
    let normal = sim.growth_pct(0, 0, false);
    assert_eq!(normal.total, 100);
    sim.debug_set_dry_days(5);
    let dry = sim.growth_pct(0, 0, false);
    assert_eq!(dry.total, 50);
    assert_eq!(
        dry.parts.iter().map(|p| p.pct).sum::<i32>(),
        50,
        "the total is the sum of the parts"
    );
    assert!(dry.parts.iter().any(|p| p.cause == GrowthCause::Drought));
    assert_eq!(sim.growth_pct(0, 0, true).total, 100, "dry soil crop");
    assert_eq!(sim.growth_pct(5, 0, false).total, 100, "water 3 tiles away");
}

#[test]
fn r025_wild_plants_do_not_grow_again_in_a_drought() {
    let mut sim = quiet("cc....", 0);
    assert!(sim.wild_plants_regrow());
    sim.debug_set_dry_days(5);
    assert!(!sim.wild_plants_regrow());
}

#[test]
fn r025_a_tile_within_4_tiles_of_water_is_irrigated() {
    let sim = quiet("c.......~", 0);
    assert!(sim.irrigated(4, 0), "4 tiles from water");
    assert!(!sim.irrigated(3, 0), "5 tiles from water");
}

#[test]
fn r025_a_ditch_that_touches_water_irrigates_and_one_that_does_not_does_not() {
    let mut sim = quiet("c..............~", 0);
    // A ditch chain from the water at x = 15 to x = 12, and a separate ditch at x = 1.
    for x in [14, 13, 12] {
        assert!(sim.debug_add_ditch(x, 0));
    }
    assert!(sim.debug_add_ditch(1, 0));
    assert!(
        sim.ditch_connected(12, 0),
        "connected through other ditches"
    );
    assert!(!sim.ditch_connected(1, 0));
    assert!(
        sim.irrigated(8, 0),
        "4 tiles from the connected ditch at 12"
    );
    assert!(!sim.irrigated(7, 0));
    assert!(
        !sim.irrigated(5, 0),
        "near only the ditch that does not touch water"
    );
}

#[test]
fn r025_a_colonist_digs_a_ditch_and_nobody_walks_on_it() {
    let mut sim = quiet("c..~", 0);
    assert!(sim.map().passable(1, 0));
    sim.step(&[Command::DigDitch { x: 1, y: 0 }]);
    assert!(!sim.has_ditch(1, 0), "it takes work time");
    run(&mut sim, 1_000);
    assert!(sim.has_ditch(1, 0));
    assert!(!sim.map().passable(1, 0));
    assert!(sim.designation(1, 0).is_none());
    sim.check_invariants().unwrap();
}

#[test]
fn r025_a_ditch_order_is_refused_on_water_rock_trees_and_ditches() {
    let mut sim = quiet("c.~TR", 0);
    for x in [2, 3, 4] {
        sim.step(&[Command::DigDitch { x, y: 0 }]);
        assert!(
            sim.drain_job_events()
                .iter()
                .any(|e| matches!(e, sim::JobEvent::Rejected(_)))
        );
        assert!(sim.designation(x, 0).is_none());
    }
    assert!(sim.debug_add_ditch(1, 0));
    sim.step(&[Command::DigDitch { x: 1, y: 0 }]);
    assert!(
        sim.drain_job_events()
            .iter()
            .any(|e| matches!(e, sim::JobEvent::Rejected(_)))
    );
}

fn lightning_rules(sim: &mut Sim, pct: u32) {
    with_rules(sim, |r| r.drought.lightning_pct = pct);
}

fn lightning(events: &[DroughtEvent]) -> Vec<(u16, u16, bool)> {
    events
        .iter()
        .filter_map(|e| match *e {
            DroughtEvent::Lightning { x, y, started } => Some((x, y, started)),
            _ => None,
        })
        .collect()
}

#[test]
fn r026_lightning_in_a_summer_drought_starts_a_fire_of_intensity_5_on_a_tree() {
    let mut sim = quiet("cc..T.T..T", 0);
    lightning_rules(&mut sim, 100);
    run_to(&mut sim, SUMMER);
    let strikes = lightning(&sim.drain_drought_events());
    assert_eq!(strikes.len(), 1);
    let (x, y, started) = strikes[0];
    assert!(started);
    assert!(sim.map().tree(i32::from(x), i32::from(y)) || sim.fire_at(x, y).is_some());
    let fire = sim.fire_at(x, y).unwrap();
    assert_eq!(fire.intensity, 5);
}

#[test]
fn r026_no_lightning_outside_summer_or_without_a_drought() {
    let mut sim = quiet("cc..T.T..T", 0);
    lightning_rules(&mut sim, 100);
    run_to(&mut sim, SUMMER - 1);
    assert!(lightning(&sim.drain_drought_events()).is_empty(), "spring");
    let mut wet = quiet("cc..T.T..T", 255);
    lightning_rules(&mut wet, 100);
    wet.debug_set_dry_days(0);
    // Rain days reset the counter, so some summer days have no drought.
    let mut checked = 0;
    for _ in 0..30 {
        run(&mut wet, TICKS_PER_DAY);
        let strikes = lightning(&wet.drain_drought_events());
        if !wet.drought_active() {
            assert!(strikes.is_empty());
            checked += 1;
        }
    }
    assert!(checked > 0);
}

#[test]
fn r026_lightning_chance_0_never_strikes() {
    let mut sim = quiet("cc..T.T..T", 0);
    lightning_rules(&mut sim, 0);
    run_to(&mut sim, SUMMER + 5 * TICKS_PER_DAY);
    assert!(lightning(&sim.drain_drought_events()).is_empty());
}

fn fast_fire(sim: &mut Sim) {
    with_rules(sim, |r| {
        r.drought.tree_flammability = 100;
        r.drought.tree_fuel = 5;
    });
}

#[test]
fn r024_fire_spreads_to_adjacent_trees_and_a_tree_burns_down() {
    let mut sim = quiet("cc.TTTTT", 0);
    fast_fire(&mut sim);
    sim.debug_ignite(3, 0, 10);
    run(&mut sim, 2_000);
    for x in 3..8 {
        assert!(!sim.map().tree(x, 0), "tree {x} burned");
    }
    assert_eq!(sim.fire_count(), 0, "the fire is out");
    assert!(
        sim.drain_drought_events()
            .iter()
            .any(|e| matches!(e, DroughtEvent::TreeBurned { .. }))
    );
}

#[test]
fn r024_fire_does_not_cross_a_firebreak_of_2_soil_tiles() {
    let mut sim = quiet("cc.TT..TT", 0);
    fast_fire(&mut sim);
    sim.debug_ignite(3, 0, 10);
    run(&mut sim, 3_000);
    assert!(
        !sim.map().tree(3, 0) && !sim.map().tree(4, 0),
        "left trees burned"
    );
    assert!(
        sim.map().tree(7, 0) && sim.map().tree(8, 0),
        "right trees are safe"
    );
}

#[test]
fn r024_the_spread_chance_is_flammability_times_intensity_over_10_and_doubles_in_a_drought() {
    let mut sim = quiet("cc....", 0);
    // tree_flammability 10, intensity 5: 10 * 5 / 10 = 5 percent.
    assert_eq!(sim.fire_spread_pct(5), 5);
    assert_eq!(sim.fire_spread_pct(10), 10);
    sim.debug_set_dry_days(5);
    assert_eq!(sim.fire_spread_pct(5), 10);
    with_rules(&mut sim, |r| r.drought.tree_flammability = 100);
    assert_eq!(sim.fire_spread_pct(10), 100, "at most 100 percent");
}

#[test]
fn r024_rain_stops_all_outdoor_fire() {
    let mut sim = quiet("cc.TTTTTTTT", 255);
    with_rules(&mut sim, |r| r.drought.tree_fuel = 100_000);
    sim.debug_ignite(3, 0, 5);
    let mut rained = false;
    for _ in 0..40 {
        run(&mut sim, TICKS_PER_DAY / 4);
        if sim.weather_today().rain {
            rained = true;
            run(&mut sim, 20);
            assert_eq!(sim.fire_count(), 0, "rain put the fire out");
            break;
        }
    }
    assert!(rained);
}

#[test]
fn drought_state_survives_save_and_load() {
    let mut a = quiet("cc..T.T..T~", 0);
    assert!(a.debug_add_ditch(8, 0));
    a.debug_set_dry_days(7);
    a.debug_ignite(4, 0, 5);
    run(&mut a, 500);
    let mut b = save::from_bytes(&save::to_bytes(&a)).unwrap();
    b.debug_set_needs_enabled(false);
    assert_eq!(b.dry_days(), a.dry_days());
    assert_eq!(b.fire_count(), a.fire_count());
    assert!(b.has_ditch(8, 0));
    assert_eq!(b.ditch_connected(8, 0), a.ditch_connected(8, 0));
    assert!(
        !b.map().passable(8, 0),
        "the ditch blocks walking after a load"
    );
    run(&mut a, TICKS_PER_DAY);
    run(&mut b, TICKS_PER_DAY);
    assert_eq!(a.state_hash(), b.state_hash());
}

#[test]
fn builtin_drought_content_is_valid_and_rejects_bad_numbers() {
    let text = include_str!("../../../assets/content/drought.ron");
    let ok = base::content::load_drought_rules("drought.ron", text).unwrap();
    assert_eq!(ok, SimRules::default().drought);
    for (from, to) in [
        ("drought_days: 5", "drought_days: 0"),
        ("lightning_pct: 10", "lightning_pct: 101"),
        ("lightning_intensity: 5", "lightning_intensity: 11"),
        ("fire_drought_pct: 200", "fire_drought_pct: 50"),
        ("fire_step_ticks: 10", "fire_step_ticks: 0"),
    ] {
        let bad = text.replace(from, to);
        assert!(base::content::load_drought_rules("drought.ron", &bad).is_err());
    }
}

fn crop_hours(sim: &mut Sim, tile: u16, crop: u16) -> u32 {
    run_to(sim, SUMMER);
    assert!(sim.debug_plant(tile, 0, crop, false), "tile is farmable");
    run(sim, 5 * TICKS_PER_DAY);
    sim.plant_at(tile, 0).map_or(0, |p| p.hours)
}

#[test]
fn r025_a_drought_halves_the_growth_hours_of_a_crop_but_not_of_a_dry_soil_crop() {
    let text = "............~";
    let mut wet = quiet(text, 0);
    with_rules(&mut wet, |r| r.drought.drought_days = 1000);
    let mut dry = quiet(text, 0);
    let mut dry_soil = quiet(text, 0);
    let mut wet_soil = quiet(text, 0);
    with_rules(&mut wet_soil, |r| r.drought.drought_days = 1000);
    let normal = crop_hours(&mut wet, 0, 0);
    let slow = crop_hours(&mut dry, 0, 0);
    assert!(normal > 20, "the crop grows in summer ({normal} hours)");
    assert!(slow.abs_diff(normal / 2) <= 1, "{slow} is half of {normal}");
    let sorghum = |sim: &mut Sim| crop_hours(sim, 0, 3);
    assert_eq!(sorghum(&mut dry_soil), sorghum(&mut wet_soil));
    let mut near = quiet("..~", 0);
    assert_eq!(
        crop_hours(&mut near, 0, 0),
        normal,
        "an irrigated tile grows at normal speed"
    );
}

#[test]
fn r025_a_drought_stops_the_regrowth_of_gathered_wild_plants() {
    // The drought counter blocks the daily countdown in `farm_daily`.
    let mut sim = quiet("............~", 0);
    run_to(&mut sim, SUMMER);
    assert!(!sim.wild_plants_regrow(), "a long dry spell is a drought");
}
