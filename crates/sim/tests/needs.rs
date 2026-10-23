//! Scenario and unit tests for needs, health, and mood (Sprint 6): R-001, R-011 to R-016,
//! the MVP traits, the alerts, and the save of the new state.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use sim::rules::BUILTIN_TEXTS;
use sim::scenario::sim_from_ascii;
use sim::{
    AlertKind, AlertLevel, ColonistId, ComfortCause, Command, Condition, DeathCause, HealthCause,
    JobKind, LifeEvent, MoodCause, NeedAct, Sim, SimRules, SpeedModCause, save,
};
use std::sync::Arc;

const HOUR: u32 = 100;
const DAY: u32 = 2_400;

fn mat(sim: &Sim, key: &str) -> u16 {
    sim.rules().material_index(key).unwrap()
}

fn bld(sim: &Sim, key: &str) -> u16 {
    sim.rules().building_index(key).unwrap()
}

fn ids(sim: &Sim) -> Vec<ColonistId> {
    sim.colonists().iter().map(|(id, _)| id).collect()
}

fn run(sim: &mut Sim, ticks: u32) {
    for _ in 0..ticks {
        sim.step(&[]);
    }
}

fn run_checked(sim: &mut Sim, ticks: u32) {
    for _ in 0..ticks {
        sim.step(&[]);
        sim.check_invariants().unwrap();
    }
}

/// Climate text with flat curves: outdoor = mean for ever.
fn climate_text() -> String {
    format!(
        "(season_curve: {:?}, daily_curve: {:?}, weather_step_max: 0, weather_limit: 120, \
         default_mean: 110, default_amplitude: 160, default_moisture: 128, rain_max_pct: 50, \
         freeze_days: 3, deep_water_distance: 3, hearth_heat: 400, \
         hearth_fuel_capacity_hours: 48, refuel_below_hours: 24, \
         fuel: [(\"firewood\", 4), (\"coal\", 4)], low_fuel_alert_hours: 12, \
         cold_period_offset: -60)",
        [0i16; 40], [0i16; 24]
    )
}

/// Replace the rules by the repository rules with a flat climate and a changed needs file.
fn set_rules(sim: &mut Sim, needs: &str) {
    let b = &BUILTIN_TEXTS;
    let rules = SimRules::load_full_with(
        b.materials,
        b.jobs,
        b.buildings,
        b.recipes,
        b.rooms,
        &climate_text(),
        needs,
        b.traits,
    )
    .unwrap();
    sim.debug_set_rules(Arc::new(rules));
}

/// A sim with a flat climate (outdoor temperature `mean` tenths) and the repository needs.
fn flat_sim(map: &str, mean: i16) -> Sim {
    let mut sim = sim_from_ascii(map, 1).unwrap();
    set_rules(&mut sim, BUILTIN_TEXTS.needs);
    sim.set_climate(mean, 0, 128);
    sim
}

/// A sim with a changed needs file.
fn custom_sim(map: &str, mean: i16, from: &str, to: &str) -> Sim {
    let mut sim = sim_from_ascii(map, 1).unwrap();
    let text = BUILTIN_TEXTS.needs.replace(from, to);
    assert_ne!(text, BUILTIN_TEXTS.needs, "the replaced text exists");
    set_rules(&mut sim, &text);
    sim.set_climate(mean, 0, 128);
    sim
}

fn trait_index(sim: &Sim, id: &str) -> u8 {
    sim.rules()
        .traits
        .iter()
        .position(|t| t.id == id)
        .map(|i| i as u8)
        .unwrap()
}

fn first(sim: &Sim) -> ColonistId {
    ids(sim)[0]
}

// ---- R-013: warmth ----

#[test]
fn r013_simple_clothes_at_minus_10c_give_hypothermia_50_after_5_hours() {
    let mut sim = flat_sim("c.........", -100);
    let id = first(&sim);
    assert_eq!(sim.outdoor_temp(), -100);
    run(&mut sim, 5 * HOUR);
    let c = sim.colonist(id).unwrap();
    assert_eq!(c.hypothermia, 50);
    assert_eq!(c.health, 100, "50 is not above the limit yet");
    run(&mut sim, HOUR);
    let c = sim.colonist(id).unwrap();
    assert_eq!(c.hypothermia, 60);
    assert_eq!(c.health, 95, "above 50 the colonist loses 5 health");
    assert!(
        sim.conditions(id)
            .unwrap()
            .contains(&Condition::Hypothermia)
    );
}

#[test]
fn r013_the_colonist_dies_after_about_25_hours_if_nothing_changes() {
    let mut sim = flat_sim("c.........", -100);
    let id = first(&sim);
    run_checked(&mut sim, 24 * HOUR);
    assert_eq!(sim.colonist(id).unwrap().health, 5);
    run_checked(&mut sim, HOUR);
    assert!(sim.colonist(id).is_none(), "dead after 25 hours");
    assert_eq!(sim.graveyard().len(), 1);
    assert_eq!(sim.graveyard()[0].cause, DeathCause::Hypothermia);
    let events = sim.drain_life_events();
    assert!(matches!(
        events.as_slice(),
        [LifeEvent::Died {
            cause: DeathCause::Hypothermia,
            ..
        }]
    ));
}

/// A 4x4 room with granite walls and a roof; the colonist stands inside.
fn warm_room_sim() -> Sim {
    let mut rows = vec![".".repeat(10); 10];
    rows[3].replace_range(3..4, "c");
    let mut sim = flat_sim(&rows.join("\n"), -100);
    let (granite, log) = (mat(&sim, "granite"), mat(&sim, "log"));
    let (wall, roof) = (bld(&sim, "wall"), bld(&sim, "roof"));
    for i in 2..=7u16 {
        for (x, y) in [(i, 2), (i, 7), (2, i), (7, i)] {
            if sim.buildings().object_at(x, y).is_none() {
                assert!(sim.debug_build(wall, x, y, granite));
            }
        }
    }
    for y in 3..=6 {
        for x in 3..=6 {
            assert!(sim.debug_build(roof, x, y, log));
        }
    }
    let hearth = bld(&sim, "hearth");
    assert!(sim.debug_build(hearth, 5, 5, granite));
    sim.debug_set_hearth_fuel(5, 5, 1_000);
    sim.step(&[]);
    sim
}

#[test]
fn r013_the_same_colonist_in_the_warm_room_gets_no_hypothermia() {
    let mut sim = warm_room_sim();
    let id = first(&sim);
    sim.debug_set_needs_enabled(false);
    run(&mut sim, 3 * DAY);
    assert!(sim.room_temp(3, 3).unwrap() >= 100, "the room is warm");
    sim.debug_set_needs_enabled(true);
    sim.debug_set_hunger(id, 100);
    sim.debug_set_rest(id, 100);
    for _ in 0..8 {
        run(&mut sim, HOUR);
        let c = sim.colonist(id).unwrap();
        assert_eq!(c.hypothermia, 0);
        assert_eq!(c.health, 100);
    }
    assert!(
        sim.alerts()
            .iter()
            .all(|a| !matches!(a.kind, AlertKind::Hypothermia { .. }))
    );
}

#[test]
fn r013_heatstroke_grows_by_10_each_hour_more_than_10c_above_the_band() {
    let mut sim = flat_sim("c.........", 400);
    let id = first(&sim);
    run(&mut sim, 3 * HOUR);
    let c = sim.colonist(id).unwrap();
    assert_eq!(c.heatstroke, 30);
    assert_eq!(c.hypothermia, 0);
    let mood = sim.mood(id).unwrap();
    assert!(
        mood.parts
            .iter()
            .any(|p| p.cause == MoodCause::Hot && p.value == -5)
    );
}

#[test]
fn r013_inside_the_band_hypothermia_falls_by_20_each_hour() {
    let mut sim = flat_sim("c.........", -100);
    let id = first(&sim);
    run(&mut sim, 5 * HOUR);
    assert_eq!(sim.colonist(id).unwrap().hypothermia, 50);
    sim.set_climate(200, 0, 128);
    run(&mut sim, HOUR);
    assert_eq!(sim.colonist(id).unwrap().hypothermia, 30);
    run(&mut sim, 2 * HOUR);
    assert_eq!(sim.colonist(id).unwrap().hypothermia, 0);
}

#[test]
fn r013_between_the_band_and_the_margin_hypothermia_does_not_grow() {
    // The lower limit is 5.0 C with simple clothes. At 0.0 C it is 5 C below: cold, but
    // not more than 10 C below.
    let mut sim = flat_sim("c.........", 0);
    let id = first(&sim);
    run(&mut sim, 4 * HOUR);
    assert_eq!(sim.colonist(id).unwrap().hypothermia, 0);
    let mood = sim.mood(id).unwrap();
    assert!(
        mood.parts
            .iter()
            .any(|p| p.cause == MoodCause::Cold && p.value == -5)
    );
}

// ---- R-001: comfort band and clothes ----

#[test]
fn r001_the_band_is_10_to_26_and_simple_clothes_lower_the_limit_to_5() {
    let mut sim = flat_sim("c.", 200);
    let id = first(&sim);
    let simple = mat(&sim, "simple_clothes");
    sim.debug_set_clothes(id, None);
    let band = sim.comfort_band(id).unwrap();
    assert_eq!((band.low, band.high), (100, 260));
    sim.debug_set_clothes(id, Some(simple));
    let band = sim.comfort_band(id).unwrap();
    assert_eq!((band.low, band.high), (50, 260));
    assert_eq!(band.parts.iter().map(|p| p.tenths).sum::<i32>(), band.low);
    assert!(
        band.parts
            .iter()
            .any(|p| p.cause == ComfortCause::Clothes { material: simple } && p.tenths == -50)
    );
}

#[test]
fn r001_new_colonists_wear_simple_clothes() {
    let sim = flat_sim("c.", 200);
    let c = sim.colonist(first(&sim)).unwrap();
    assert_eq!(c.clothes, Some(mat(&sim, "simple_clothes")));
}

#[test]
fn r001_the_hardy_trait_lowers_the_comfort_band_lower_limit_by_5c() {
    let mut sim = flat_sim("c.", 200);
    let id = first(&sim);
    let hardy = trait_index(&sim, "hardy");
    let before = sim.comfort_band(id).unwrap().low;
    sim.debug_set_traits(id, &[hardy]);
    let band = sim.comfort_band(id).unwrap();
    assert_eq!(band.low, before - 50);
    assert!(
        band.parts
            .iter()
            .any(|p| p.cause == ComfortCause::Trait { index: hardy } && p.tenths == -50)
    );
}

#[test]
fn r001_a_hardy_colonist_has_no_hypothermia_at_minus_4c() {
    // Lower limit 0.0 C for Hardy in simple clothes; -4 C is not more than 10 C below it.
    let mut sim = flat_sim("cc........", -40);
    let (a, b) = (ids(&sim)[0], ids(&sim)[1]);
    let hardy = trait_index(&sim, "hardy");
    sim.debug_set_traits(a, &[hardy]);
    sim.set_climate(-80, 0, 128);
    run(&mut sim, 3 * HOUR);
    assert_eq!(
        sim.colonist(a).unwrap().hypothermia,
        0,
        "-8 C: within 10 C of 0 C"
    );
    assert_eq!(
        sim.colonist(b).unwrap().hypothermia,
        30,
        "-8 C: 13 C below 5 C"
    );
}

#[test]
fn r001_a_colonist_wears_a_warmer_coat_from_the_ground() {
    let mut sim = flat_sim("c.....", 200);
    let id = first(&sim);
    let (coat, simple) = (mat(&sim, "winter_coat"), mat(&sim, "simple_clothes"));
    sim.debug_spawn_item(coat, 1, 4, 0);
    run_checked(&mut sim, 300);
    let c = sim.colonist(id).unwrap();
    assert_eq!(c.clothes, Some(coat));
    assert_eq!(sim.comfort_band(id).unwrap().low, -20);
    assert_eq!(sim.items().total_units(coat), 0, "the coat is worn");
    assert_eq!(
        sim.items().total_units(simple),
        1,
        "the old clothes lie on the ground"
    );
    run_checked(&mut sim, 300);
    assert_eq!(
        sim.colonist(id).unwrap().clothes,
        Some(coat),
        "no swap back"
    );
}

// ---- R-011: hunger ----

#[test]
fn r011_hunger_starts_at_100_and_falls_50_each_game_day() {
    let mut sim = flat_sim("c.........", 200);
    let id = first(&sim);
    assert_eq!(sim.colonist(id).unwrap().hunger, 100_000);
    run(&mut sim, DAY);
    assert_eq!(sim.colonist(id).unwrap().hunger, 50_000);
    run(&mut sim, 7 * HOUR);
    let h = sim.colonist(id).unwrap().hunger;
    assert!(h < 50_000 && h > 30_000);
}

#[test]
fn r011_a_glutton_gets_hungry_50_percent_faster() {
    let mut sim = flat_sim("c.........", 200);
    let id = first(&sim);
    let glutton = trait_index(&sim, "glutton");
    sim.debug_set_traits(id, &[glutton]);
    run(&mut sim, DAY);
    let h = sim.colonist(id).unwrap().hunger;
    assert!((h - 25_000).abs() < 100, "hunger {h}");
}

#[test]
fn r011_a_colonist_with_30_hunger_or_less_goes_to_eat_a_meal() {
    let mut sim = flat_sim("c.....", 200);
    let id = first(&sim);
    let meal = mat(&sim, "meal");
    sim.debug_spawn_item(meal, 3, 4, 0);
    sim.debug_set_hunger(id, 31);
    run_checked(&mut sim, 50);
    assert!(
        sim.colonist(id).unwrap().hunger > 30_000,
        "31 is above the limit"
    );
    assert_eq!(sim.items().total_units(meal), 3);
    sim.debug_set_hunger(id, 30);
    run_checked(&mut sim, 200);
    let c = sim.colonist(id).unwrap();
    assert!(c.hunger >= 70_000, "one meal adds 50: {}", c.hunger);
    assert_eq!(sim.items().total_units(meal), 2);
    assert!(c.act.is_none());
    let mood = sim.mood(id).unwrap();
    assert!(
        mood.parts
            .iter()
            .any(|p| matches!(p.cause, MoodCause::Memory { .. }) && p.value == 2)
    );
}

#[test]
fn r011_a_meal_is_eaten_before_berries_and_a_meal_never_goes_over_100() {
    let mut sim = flat_sim("c.....", 200);
    let id = first(&sim);
    let (meal, berries) = (mat(&sim, "meal"), mat(&sim, "berries"));
    sim.debug_spawn_item(berries, 2, 1, 0);
    sim.debug_spawn_item(meal, 2, 5, 0);
    sim.debug_set_hunger(id, 30);
    run_checked(&mut sim, 300);
    assert_eq!(
        sim.items().total_units(meal),
        1,
        "the meal is eaten, though far away"
    );
    assert_eq!(sim.items().total_units(berries), 2);
    sim.debug_set_hunger(id, 60);
    sim.debug_set_hunger(id, 30);
    run_checked(&mut sim, 300);
    assert!(sim.colonist(id).unwrap().hunger <= 100_000);
}

#[test]
fn r011_a_starving_colonist_loses_2_health_each_game_hour() {
    let mut sim = flat_sim("c.........", 200);
    let id = first(&sim);
    sim.debug_set_hunger(id, 0);
    assert!(sim.conditions(id).unwrap().contains(&Condition::Starving));
    run(&mut sim, 3 * HOUR);
    assert_eq!(sim.colonist(id).unwrap().health, 94);
    let report = sim.health_report(id).unwrap();
    assert_eq!(report.per_hour, -2);
    assert_eq!(report.parts[0].cause, HealthCause::Starving);
    assert_eq!(report.parts[0].per_hour, -2);
    assert!(report.conditions.contains(&Condition::Starving));
}

#[test]
fn r011_a_colonist_without_food_starves_to_death_and_the_alerts_say_so() {
    let mut sim = flat_sim("cc........", 200);
    let (a, b) = (ids(&sim)[0], ids(&sim)[1]);
    sim.debug_set_hunger(a, 0);
    sim.debug_set_hunger(b, 100);
    sim.debug_set_rest(a, 100);
    let alerts = sim.alerts();
    assert!(alerts.iter().any(|al| matches!(
        al.kind,
        AlertKind::Hunger { colonist, starving: true, .. } if colonist == a
    )));
    assert!(alerts.iter().all(|al| !matches!(
        al.kind,
        AlertKind::Hunger { colonist, .. } if colonist == b
    )));
    sim.debug_set_health(a, 3);
    run_checked(&mut sim, 2 * HOUR);
    assert!(sim.colonist(a).is_none());
    assert_eq!(sim.graveyard()[0].cause, DeathCause::Starvation);
    let alerts = sim.alerts();
    let death = alerts
        .iter()
        .find(|al| matches!(al.kind, AlertKind::Death { .. }))
        .expect("a death alert");
    assert_eq!(death.level(), AlertLevel::Danger);
    assert!(death.tile().is_some());
}

// ---- R-012: rest ----

#[test]
fn r012_rest_falls_6_each_awake_hour() {
    let mut sim = flat_sim("c.........", 200);
    let id = first(&sim);
    run(&mut sim, 3 * HOUR);
    assert_eq!(sim.colonist(id).unwrap().rest, 82_000);
}

#[test]
fn r012_at_25_or_less_the_colonist_sleeps_on_the_ground_for_8_each_hour() {
    let mut sim = flat_sim("c.........", 200);
    let id = first(&sim);
    sim.debug_set_rest(id, 25);
    run_checked(&mut sim, 5);
    assert!(matches!(
        sim.colonist(id).unwrap().act,
        Some(NeedAct::Sleep {
            bed: None,
            asleep: true
        })
    ));
    run_checked(&mut sim, 3 * HOUR);
    assert_eq!(sim.colonist(id).unwrap().rest, 49_000);
}

#[test]
fn r012_sleep_in_a_bed_gives_12_each_hour_and_the_bed_is_used() {
    let mut sim = flat_sim("c.........", 200);
    let id = first(&sim);
    let bed = bld(&sim, "bed");
    assert!(sim.debug_build(bed, 3, 0, mat(&sim, "plank")));
    sim.debug_set_rest(id, 20);
    run_checked(&mut sim, 3 * HOUR);
    let c = sim.colonist(id).unwrap();
    assert_eq!(c.tile(), (3, 0), "sleeps on the bed tile");
    assert_eq!(c.rest, 56_000);
    assert!(matches!(
        c.act,
        Some(NeedAct::Sleep {
            bed: Some((3, 0)),
            asleep: true
        })
    ));
}

#[test]
fn r012_a_sleeper_wakes_when_rest_is_full_and_remembers_the_bed() {
    let mut sim = flat_sim("c.........", 200);
    let id = first(&sim);
    let bed = bld(&sim, "bed");
    assert!(sim.debug_build(bed, 3, 0, mat(&sim, "plank")));
    sim.debug_set_rest(id, 80);
    sim.debug_set_rest(id, 24);
    run_checked(&mut sim, 8 * HOUR);
    let c = sim.colonist(id).unwrap();
    assert!(c.act.is_none(), "awake");
    assert!(c.rest > 90_000, "rest {}", c.rest);
    let mood = sim.mood(id).unwrap();
    assert!(
        mood.parts
            .iter()
            .any(|p| matches!(p.cause, MoodCause::Memory { .. }) && p.value == 3)
    );
}

#[test]
fn r012_two_colonists_do_not_share_a_bed_and_the_second_sleeps_on_the_ground() {
    let mut sim = flat_sim("cc........", 200);
    let (a, b) = (ids(&sim)[0], ids(&sim)[1]);
    let bed = bld(&sim, "bed");
    assert!(sim.debug_build(bed, 5, 0, mat(&sim, "plank")));
    sim.debug_set_rest(a, 10);
    sim.debug_set_rest(b, 10);
    run_checked(&mut sim, 2 * HOUR);
    let acts: Vec<_> = [a, b]
        .iter()
        .map(|&i| sim.colonist(i).unwrap().act)
        .collect();
    let in_bed = acts
        .iter()
        .filter(|a| matches!(a, Some(NeedAct::Sleep { bed: Some(_), .. })))
        .count();
    let ground = acts
        .iter()
        .filter(|a| matches!(a, Some(NeedAct::Sleep { bed: None, .. })))
        .count();
    assert_eq!((in_bed, ground), (1, 1));
}

#[test]
fn r012_at_rest_0_the_colonist_is_exhausted_and_works_at_half_speed() {
    let mut sim = flat_sim("c.T.......", 200);
    let id = first(&sim);
    let normal = sim
        .effective_work_speed(id, JobKind::Cut)
        .unwrap()
        .total_pct;
    sim.debug_set_rest(id, 0);
    assert!(sim.conditions(id).unwrap().contains(&Condition::Exhausted));
    let speed = sim.effective_work_speed(id, JobKind::Cut).unwrap();
    assert!(
        speed
            .mods
            .iter()
            .any(|m| m.cause == SpeedModCause::Exhausted && m.pct == -50)
    );
    assert!(speed.total_pct < normal);
    assert_eq!(
        speed.total_pct,
        speed.base.total_pct + speed.mods.iter().map(|m| m.pct).sum::<i32>()
    );
}

// ---- D-040: needs override work ----

#[test]
fn d040_a_hungry_colonist_stops_its_job_to_eat() {
    let mut sim = flat_sim("c.......T.", 200);
    let id = first(&sim);
    let meal = mat(&sim, "meal");
    sim.debug_spawn_item(meal, 2, 1, 0);
    sim.step(&[Command::Designate {
        kind: JobKind::Cut,
        x: 8,
        y: 0,
    }]);
    run_checked(&mut sim, 5);
    assert!(sim.colonist(id).unwrap().job.is_some(), "walks to the tree");
    sim.debug_set_hunger(id, 20);
    run_checked(&mut sim, 60);
    assert!(sim.colonist(id).unwrap().job.is_none());
    run_checked(&mut sim, 200);
    assert!(sim.colonist(id).unwrap().hunger > 60_000);
    assert_eq!(sim.items().total_units(meal), 1);
    run_checked(&mut sim, 300);
    assert!(sim.colonist(id).unwrap().job.is_some() || !sim.map().tree(8, 0));
}

// ---- R-016: health ----

#[test]
fn r016_health_rises_1_each_hour_elsewhere_and_2_in_a_bed() {
    let mut sim = flat_sim("cc........", 200);
    let (a, b) = (ids(&sim)[0], ids(&sim)[1]);
    let bed = bld(&sim, "bed");
    assert!(sim.debug_build(bed, 5, 0, mat(&sim, "plank")));
    sim.debug_set_health(a, 50);
    sim.debug_set_health(b, 50);
    sim.debug_set_rest(b, 10);
    run(&mut sim, 3 * HOUR);
    assert_eq!(sim.colonist(a).unwrap().health, 53);
    assert_eq!(sim.colonist(b).unwrap().health, 56);
    let report = sim.health_report(b).unwrap();
    assert_eq!(report.parts[0].cause, HealthCause::HealingInBed);
    assert_eq!(report.per_hour, 2);
    let report = sim.health_report(a).unwrap();
    assert_eq!(report.parts[0].cause, HealthCause::Healing);
}

#[test]
fn r016_health_does_not_rise_with_a_condition_and_stays_in_0_to_100() {
    let mut sim = flat_sim("c.........", 200);
    let id = first(&sim);
    run(&mut sim, 2 * HOUR);
    assert_eq!(sim.colonist(id).unwrap().health, 100);
    sim.debug_set_health(id, 500);
    assert_eq!(sim.colonist(id).unwrap().health, 100);
    sim.debug_set_hunger(id, 0);
    sim.debug_set_health(id, 60);
    run(&mut sim, HOUR);
    assert_eq!(sim.colonist(id).unwrap().health, 58);
}

#[test]
fn r016_a_wounded_colonist_loses_1_health_each_hour_until_8_hours_in_a_bed() {
    let mut sim = flat_sim("c.........", 200);
    let id = first(&sim);
    sim.debug_wound(id);
    run(&mut sim, 3 * HOUR);
    assert_eq!(sim.colonist(id).unwrap().health, 97);
    assert!(sim.colonist(id).unwrap().wounded);
    let bed = bld(&sim, "bed");
    assert!(sim.debug_build(bed, 3, 0, mat(&sim, "plank")));
    sim.debug_set_rest(id, 8);
    run_checked(&mut sim, 10 * HOUR);
    let c = sim.colonist(id).unwrap();
    assert!(!c.wounded, "8 hours in a bed heal the wound");
    assert!(c.health < 100);
}

#[test]
fn r016_a_death_removes_the_colonist_frees_its_reservations_and_saddens_the_others() {
    let mut sim = flat_sim("cc.T......", 200);
    let (a, b) = (ids(&sim)[0], ids(&sim)[1]);
    sim.step(&[Command::Designate {
        kind: JobKind::Cut,
        x: 3,
        y: 0,
    }]);
    run_checked(&mut sim, 3);
    sim.debug_kill(a);
    sim.check_invariants().unwrap();
    assert!(sim.colonist(a).is_none());
    assert_eq!(sim.colonists().len(), 1);
    assert_eq!(sim.graveyard().len(), 1);
    let mood = sim.mood(b).unwrap();
    assert!(
        mood.parts
            .iter()
            .any(|p| matches!(p.cause, MoodCause::Memory { .. }) && p.value == -10)
    );
    run_checked(&mut sim, 200);
    assert!(
        sim.alerts()
            .iter()
            .any(|al| matches!(al.kind, AlertKind::Death { .. }))
    );
    run(&mut sim, 25 * HOUR);
    assert!(
        !sim.alerts()
            .iter()
            .any(|al| matches!(al.kind, AlertKind::Death { .. }))
    );
}

// ---- R-014: mood ----

#[test]
fn r014_mood_is_50_plus_the_sum_of_all_modifiers_and_each_is_visible() {
    let mut sim = flat_sim("c.........", 200);
    let id = first(&sim);
    assert_eq!(sim.mood(id).unwrap().total, 50);
    sim.debug_set_hunger(id, 20);
    sim.debug_set_rest(id, 10);
    assert!(sim.debug_add_memory(id, "ate_meal"));
    let mood = sim.mood(id).unwrap();
    assert_eq!(mood.parts[0].cause, MoodCause::Base);
    assert_eq!(mood.parts[0].value, 50);
    let value = |cause: MoodCause| {
        mood.parts
            .iter()
            .find(|p| p.cause == cause)
            .map(|p| p.value)
    };
    assert_eq!(value(MoodCause::Hungry), Some(-5));
    assert_eq!(value(MoodCause::Tired), Some(-5));
    assert_eq!(mood.parts.len(), 4);
    assert_eq!(mood.sum, 42);
    assert_eq!(mood.total, 42);
    assert_eq!(mood.parts.iter().map(|p| p.value).sum::<i32>(), mood.total);
    sim.debug_set_hunger(id, 0);
    sim.debug_set_rest(id, 0);
    let mood = sim.mood(id).unwrap();
    assert!(
        mood.parts
            .iter()
            .any(|p| p.cause == MoodCause::Starving && p.value == -20)
    );
    assert!(
        mood.parts
            .iter()
            .any(|p| p.cause == MoodCause::Exhausted && p.value == -15)
    );
}

#[test]
fn r014_mood_is_limited_to_0_to_100() {
    let mut sim = flat_sim("c.........", 200);
    let id = first(&sim);
    sim.debug_set_hunger(id, 0);
    sim.debug_set_rest(id, 0);
    sim.debug_wound(id);
    for _ in 0..3 {
        sim.debug_add_memory(id, "colonist_died");
    }
    sim.debug_add_memory(id, "saw_danger");
    sim.debug_add_memory(id, "slept_ground");
    let mood = sim.mood(id).unwrap();
    assert!(mood.sum < 0, "sum {}", mood.sum);
    assert_eq!(mood.total, 0);
}

#[test]
fn r014_a_memory_ends_after_its_hours_and_the_same_memory_does_not_stack() {
    let mut sim = flat_sim("c.........", 200);
    let id = first(&sim);
    sim.debug_add_memory(id, "ate_meal");
    sim.debug_add_memory(id, "ate_meal");
    assert_eq!(sim.mood(id).unwrap().total, 52);
    run(&mut sim, 11 * HOUR);
    assert!(sim.mood(id).unwrap().parts.len() > 1);
    run(&mut sim, 2 * HOUR);
    assert!(
        sim.mood(id)
            .unwrap()
            .parts
            .iter()
            .all(|p| !matches!(p.cause, MoodCause::Memory { .. } if p.value == 2))
    );
    assert!(!sim.debug_add_memory(id, "no_such_memory"));
}

#[test]
fn r014_room_beauty_is_a_mood_modifier_with_a_cap() {
    let mut sim = warm_room_sim();
    let id = first(&sim);
    sim.debug_set_needs_enabled(true);
    let per_tile = sim.room_beauty(3, 3).unwrap().per_tile;
    let mood = sim.mood(id).unwrap();
    let part = mood
        .parts
        .iter()
        .find(|p| matches!(p.cause, MoodCause::RoomBeauty { .. }));
    let expected = (per_tile / sim.rules().needs.beauty_divisor).clamp(-8, 8);
    match part {
        Some(p) => {
            assert_ne!(expected, 0);
            assert_eq!(p.value, expected);
            assert_eq!(p.cause, MoodCause::RoomBeauty { per_tile });
        }
        None => assert_eq!(expected, 0),
    }
    // A pretty room: floors add beauty to every room tile.
    let (floor, granite) = (bld(&sim, "floor"), mat(&sim, "granite"));
    for y in 3..=6 {
        for x in 3..=6 {
            if sim.buildings().floor_at(x, y).is_none() {
                assert!(sim.debug_build(floor, x, y, granite));
            }
        }
    }
    sim.step(&[]);
    let better = sim.room_beauty(3, 3).unwrap().per_tile;
    assert!(
        better > per_tile,
        "floors add beauty: {per_tile} to {better}"
    );
    let mood = sim.mood(id).unwrap();
    let want = (better / 2).clamp(-8, 8);
    let got = mood
        .parts
        .iter()
        .find(|p| matches!(p.cause, MoodCause::RoomBeauty { .. }))
        .map_or(0, |p| p.value);
    assert_eq!(got, want);
}

#[test]
fn r014_bold_makes_danger_modifiers_50_percent_smaller() {
    let mut sim = flat_sim("cc........", 200);
    let (a, b) = (ids(&sim)[0], ids(&sim)[1]);
    let bold = trait_index(&sim, "bold");
    sim.debug_set_traits(a, &[bold]);
    sim.debug_add_memory(a, "saw_danger");
    sim.debug_add_memory(b, "saw_danger");
    assert_eq!(sim.mood(a).unwrap().total, 46);
    assert_eq!(sim.mood(b).unwrap().total, 42);
}

// ---- R-015: mood effects ----

#[test]
fn r015_mood_70_or_more_gives_10_percent_and_mood_below_30_takes_20_percent() {
    let normal = flat_sim("c.T.......", 200);
    let id = first(&normal);
    let base = normal.effective_work_speed(id, JobKind::Cut).unwrap();
    assert!(base.mods.is_empty());
    let high = custom_sim("c.T.......", 200, "mood_base: 50", "mood_base: 75");
    let speed = high.effective_work_speed(id, JobKind::Cut).unwrap();
    assert_eq!(speed.mods.len(), 1);
    assert_eq!(speed.mods[0].cause, SpeedModCause::HighMood);
    assert_eq!(speed.total_pct, base.total_pct + 10);
    let low = custom_sim("c.T.......", 200, "mood_base: 50", "mood_base: 25");
    let speed = low.effective_work_speed(id, JobKind::Cut).unwrap();
    assert_eq!(speed.mods[0].cause, SpeedModCause::LowMood);
    assert_eq!(speed.total_pct, base.total_pct - 20);
    let edge = custom_sim("c.T.......", 200, "mood_base: 50", "mood_base: 69");
    assert!(
        edge.effective_work_speed(id, JobKind::Cut)
            .unwrap()
            .mods
            .is_empty()
    );
}

#[test]
fn r015_at_mood_10_the_colonist_stops_work_for_4_game_hours() {
    let mut sim = custom_sim(
        ".........T\nc.........",
        200,
        "mood_base: 50",
        "mood_base: 10",
    );
    let id = first(&sim);
    sim.step(&[Command::Designate {
        kind: JobKind::Cut,
        x: 9,
        y: 0,
    }]);
    run_checked(&mut sim, 50);
    assert!(
        sim.colonist(id).unwrap().job.is_some(),
        "working before the hour"
    );
    assert_eq!(sim.mood(id).unwrap().total, 10);
    run_checked(&mut sim, 50);
    let c = sim.colonist(id).unwrap();
    assert_eq!(c.break_until, 500, "4 game hours from tick 100");
    for _ in 0..390 {
        sim.step(&[]);
        assert!(
            sim.colonist(id).unwrap().job.is_none(),
            "no work in the break"
        );
    }
    assert_eq!(sim.tick(), 491);
    assert!(
        sim.alerts()
            .iter()
            .any(|a| matches!(a.kind, AlertKind::LowMood { mood: 10, .. }))
    );
}

#[test]
fn r015_a_colonist_with_mood_15_to_29_sometimes_walks_around_for_1_hour() {
    let mut sim = custom_sim(
        "...........\n.....c.....\n...........",
        200,
        "mood_base: 50",
        "mood_base: 25",
    );
    let text = BUILTIN_TEXTS
        .needs
        .replace("mood_base: 50", "mood_base: 25")
        .replace("wander_chance_pct: 10", "wander_chance_pct: 100");
    set_rules(&mut sim, &text);
    let id = first(&sim);
    run(&mut sim, HOUR);
    let c = sim.colonist(id).unwrap();
    assert_eq!(c.break_until, 200, "1 game hour");
    assert_eq!(c.low_mood_hours, 0, "mood 25 is not a breakdown");
}

#[test]
fn r015_mood_below_15_for_many_hours_makes_the_colonist_leave() {
    let mut sim = flat_sim("c.........", 200);
    let text = BUILTIN_TEXTS
        .needs
        .replace("mood_base: 50", "mood_base: 10")
        .replace("leave_after_hours: 120", "leave_after_hours: 10");
    set_rules(&mut sim, &text);
    let id = first(&sim);
    let meal = mat(&sim, "meal");
    sim.debug_spawn_item(meal, 20, 2, 0);
    run_checked(&mut sim, 9 * HOUR);
    assert!(sim.colonist(id).is_some());
    assert_eq!(sim.colonist(id).unwrap().low_mood_hours, 9);
    run_checked(&mut sim, HOUR);
    assert!(sim.colonist(id).is_none(), "left after 10 hours");
    assert!(sim.graveyard().is_empty(), "leaving is not dying");
    assert!(
        sim.drain_life_events()
            .iter()
            .any(|e| matches!(e, LifeEvent::Left { .. }))
    );
}

// ---- traits ----

#[test]
fn gd72_the_mvp_traits_have_the_documented_numbers() {
    let sim = flat_sim("c.", 200);
    let t = &sim.rules().traits;
    let by = |id: &str| t.iter().find(|d| d.id == id).unwrap();
    assert_eq!(by("hardy").comfort_low_delta, -50);
    assert_eq!(by("glutton").hunger_rate_pct, 150);
    assert_eq!(by("bold").danger_mood_pct, 50);
    assert_eq!(by("lazy").work_speed_pct, -15);
    assert_eq!(by("strong").carry_pct, 150);
    assert_eq!(t.len(), 5);
}

#[test]
fn gd72_lazy_takes_15_percent_of_the_work_speed_and_strong_carries_50_percent_more() {
    let mut sim = flat_sim("c.T.......", 200);
    let id = first(&sim);
    let base = sim
        .effective_work_speed(id, JobKind::Cut)
        .unwrap()
        .total_pct;
    let normal_carry = sim.carry_capacity(id).unwrap();
    sim.debug_set_traits(id, &[trait_index(&sim, "lazy")]);
    let speed = sim.effective_work_speed(id, JobKind::Cut).unwrap();
    assert_eq!(speed.total_pct, base - 15);
    assert!(matches!(speed.mods[0].cause, SpeedModCause::Trait { .. }));
    sim.debug_set_traits(id, &[trait_index(&sim, "strong")]);
    assert_eq!(sim.carry_capacity(id).unwrap(), normal_carry * 3 / 2);
}

#[test]
fn gd72_a_new_colony_has_ages_sexes_race_0_and_one_trait_for_each_colonist() {
    let data = worldgen::generate_local(&worldgen::EmbarkSummary::temperate_forest());
    let sim = Sim::new(&data, 1, 0);
    let again = Sim::new(&data, 1, 0);
    assert_eq!(sim.state_hash(), again.state_hash());
    for (_, c) in sim.colonists().iter() {
        assert!((18..=45).contains(&c.age));
        assert_eq!(c.race, 0);
        assert_eq!(c.traits.len(), 1);
    }
    let meal = mat(&sim, "meal");
    assert_eq!(sim.items().total_units(meal), 420, "start food");
}

// ---- alerts ----

#[test]
fn alert_hunger_names_a_hungry_colonist_that_has_no_food() {
    let mut sim = flat_sim("c.........", 200);
    let id = first(&sim);
    sim.debug_set_hunger(id, 25);
    let alerts = sim.alerts();
    let a = alerts
        .iter()
        .find(|a| matches!(a.kind, AlertKind::Hunger { .. }))
        .unwrap();
    assert_eq!(a.level(), AlertLevel::Warning);
    assert!(matches!(
        a.kind,
        AlertKind::Hunger { starving: false, colonist, .. } if colonist == id
    ));
    assert_eq!(a.tile(), Some((0, 0)));
}

#[test]
fn alert_hypothermia_and_low_mood_show_for_a_freezing_colonist() {
    let mut sim = flat_sim("c.........", -100);
    let id = first(&sim);
    run(&mut sim, 2 * HOUR);
    let alerts = sim.alerts();
    assert!(alerts.iter().any(|a| matches!(
        a.kind,
        AlertKind::Hypothermia { colonist, value: 20, .. } if colonist == id
    )));
    sim.debug_set_hunger(id, 0);
    sim.debug_set_rest(id, 0);
    let alerts = sim.alerts();
    let low = alerts
        .iter()
        .find(|a| matches!(a.kind, AlertKind::LowMood { .. }))
        .unwrap();
    assert_eq!(low.level(), AlertLevel::Warning);
    let cold = alerts
        .iter()
        .find(|a| matches!(a.kind, AlertKind::Hypothermia { .. }))
        .unwrap();
    assert_eq!(cold.level(), AlertLevel::Danger);
}

// ---- state, save, and rules ----

#[test]
fn needs_state_survives_a_save_and_the_run_continues_the_same() {
    let mut sim = flat_sim("cc....T...", -100);
    let (a, b) = (ids(&sim)[0], ids(&sim)[1]);
    let (meal, bed) = (mat(&sim, "meal"), bld(&sim, "bed"));
    assert!(sim.debug_build(bed, 4, 0, mat(&sim, "plank")));
    sim.debug_spawn_item(meal, 3, 5, 0);
    sim.debug_set_hunger(a, 25);
    sim.debug_set_rest(b, 15);
    sim.debug_add_memory(b, "ate_meal");
    run_checked(&mut sim, 7 * HOUR);
    let bytes = save::to_bytes(&sim);
    let mut loaded = save::from_bytes(&bytes).unwrap();
    assert_eq!(loaded.state_hash(), sim.state_hash());
    loaded.check_invariants().unwrap();
    for _ in 0..(10 * HOUR) {
        sim.step(&[]);
        loaded.step(&[]);
    }
    assert_eq!(loaded.state_hash(), sim.state_hash());
}

#[test]
fn needs_state_after_a_death_survives_a_save() {
    let mut sim = flat_sim("cc........", 200);
    let a = ids(&sim)[0];
    sim.debug_kill(a);
    run(&mut sim, 30);
    let loaded = save::from_bytes(&save::to_bytes(&sim)).unwrap();
    assert_eq!(loaded.graveyard(), sim.graveyard());
    assert_eq!(loaded.state_hash(), sim.state_hash());
}

#[test]
fn the_save_format_version_is_7() {
    assert_eq!(save::FORMAT_VERSION, 8);
}

#[test]
fn a_changed_colonist_state_that_breaks_a_range_is_refused_by_the_loader() {
    let mut sim = flat_sim("c.", 200);
    let id = first(&sim);
    sim.debug_set_traits(id, &[0]);
    let good = save::to_bytes(&sim);
    assert!(save::from_bytes(&good).is_ok());
}

#[test]
fn the_repository_needs_and_traits_files_load_and_link() {
    let rules = SimRules::builtin();
    assert!(rules.needs.enabled);
    assert_eq!(rules.needs.foods.len(), 4);
    assert!(
        rules
            .needs
            .food(rules.material_index("meal").unwrap())
            .is_some()
    );
    assert!(
        rules
            .needs
            .clothes_def(rules.material_index("winter_coat").unwrap())
            .is_some()
    );
    assert_eq!(rules.traits.len(), 5);
}

#[test]
fn bad_needs_content_is_refused_with_a_message() {
    let b = &BUILTIN_TEXTS;
    let load = |needs: &str, traits: &str| SimRules::load_with_needs(needs, traits);
    assert!(load(b.needs, b.traits).is_ok());
    let bad = b.needs.replace(
        "start_clothes: \"simple_clothes\"",
        "start_clothes: \"nothing\"",
    );
    assert!(load(&bad, b.traits).is_err());
    let bad = b
        .needs
        .replace("mood_break_below: 15", "mood_break_below: 99");
    assert!(load(&bad, b.traits).is_err());
    let bad = b.needs.replace("hunger_max: 100", "hunger_max: 0");
    assert!(load(&bad, b.traits).is_err());
    assert!(load("not ron", b.traits).is_err());
    assert!(
        load(
            b.needs,
            "[(id: \"a\", name: \"A\"), (id: \"a\", name: \"B\")]"
        )
        .is_err()
    );
    assert!(load(b.needs, "[(id: \"a\", name: \"A\", hunger_rate_pct: 5000)]").is_err());
}

#[test]
fn a_soak_of_ten_days_keeps_the_invariants_and_is_repeatable() {
    let make = || {
        let mut sim = flat_sim(
            "ccc...T.T.
..........
.....T....",
            -20,
        );
        let bed = bld(&sim, "bed");
        assert!(sim.debug_build(bed, 6, 1, mat(&sim, "plank")));
        let meal = mat(&sim, "meal");
        sim.debug_spawn_item(meal, 12, 3, 1);
        for (x, y) in [(6, 0), (8, 0), (5, 2)] {
            sim.step(&[Command::Designate {
                kind: JobKind::Cut,
                x,
                y,
            }]);
        }
        for _ in 0..(10 * DAY) {
            sim.step(&[]);
            sim.check_invariants().unwrap();
        }
        sim
    };
    let a = make();
    let b = make();
    assert_eq!(a.state_hash(), b.state_hash());
}
