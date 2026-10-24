//! Sprint 15 tests (sim part): fire from hearths, burning things, extinguish jobs (R-023,
//! R-024), migrants (R-017), the legacy score and the ironman hook (R-050).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use sim::scenario::sim_from_ascii;
use sim::time::{TICKS_PER_DAY, TICKS_PER_HOUR, TICKS_PER_YEAR};
use sim::{
    BuildLayer, Command, DroughtEvent, JobKind, MigrantBlocker, MigrantEvent, Sim, SimRules, save,
};
use std::sync::Arc;

const OPEN: &str = "c.........\n..........\n..........\n..........\n..........\n..........";

fn quiet(text: &str) -> Sim {
    let mut sim = sim_from_ascii(text, 1).unwrap();
    sim.debug_set_needs_enabled(false);
    sim.set_climate(110, 160, 0);
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

/// The only colonist does not take mine jobs, so it does not put out fires.
fn no_firefighter(sim: &mut Sim) {
    let id = sim.colonists().iter().next().unwrap().0;
    sim.step(&[Command::SetPriority {
        colonist: id,
        job: JobKind::Mine,
        priority: 0,
    }]);
}

fn material(sim: &Sim, key: &str) -> u16 {
    sim.rules()
        .materials
        .iter()
        .position(|m| m.key == key)
        .unwrap() as u16
}

fn building(sim: &Sim, key: &str) -> u16 {
    sim.rules().building_index(key).unwrap()
}

/// A hearth at (5, 3) with fuel, and nothing flammable.
fn hearth_sim() -> Sim {
    let mut sim = quiet(OPEN);
    let (hearth, granite) = (building(&sim, "hearth"), material(&sim, "granite"));
    assert!(sim.debug_build(hearth, 5, 3, granite));
    assert!(sim.debug_set_hearth_fuel(5, 3, 500));
    sim
}

fn certain_hearth_fire(sim: &mut Sim) {
    with_rules(sim, |r| {
        r.drought.hearth_fire_pct = 100;
        r.drought.hearth_fire_grace_days = 2;
    });
}

// ---- R-023 ----

#[test]
fn r023_a_lit_hearth_sets_a_flammable_item_next_to_it_on_fire() {
    let mut sim = hearth_sim();
    certain_hearth_fire(&mut sim);
    let fw = material(&sim, "firewood");
    sim.debug_spawn_item(fw, 3, 6, 3);
    sim.debug_spawn_item(fw, 3, 8, 3);
    run_to(&mut sim, 3 * TICKS_PER_DAY);
    assert!(
        sim.fire_at(6, 3).is_some()
            || sim
                .drain_drought_events()
                .iter()
                .any(|e| { matches!(e, DroughtEvent::HearthFire { x: 6, y: 3 }) })
    );
    assert!(sim.fire_at(8, 3).is_none(), "2 tiles away is out of reach");
}

#[test]
fn r023_no_hearth_fire_in_the_grace_days() {
    let mut sim = hearth_sim();
    with_rules(&mut sim, |r| {
        r.drought.hearth_fire_pct = 100;
        r.drought.hearth_fire_grace_days = 10;
    });
    sim.debug_spawn_item(material(&sim, "firewood"), 3, 6, 3);
    run_to(&mut sim, 10 * TICKS_PER_DAY - 1);
    assert_eq!(sim.fire_count(), 0);
    assert!(
        sim.drain_drought_events()
            .iter()
            .all(|e| !matches!(e, DroughtEvent::HearthFire { .. }))
    );
    run_to(&mut sim, 11 * TICKS_PER_DAY);
    let events = sim.drain_drought_events();
    assert!(
        events
            .iter()
            .any(|e| matches!(e, DroughtEvent::HearthFire { .. })),
        "{events:?}"
    );
}

#[test]
fn r023_a_hearth_without_fuel_starts_no_fire() {
    let mut sim = hearth_sim();
    certain_hearth_fire(&mut sim);
    sim.debug_set_hearth_fuel(5, 3, 0);
    sim.debug_spawn_item(material(&sim, "firewood"), 3, 6, 3);
    run_to(&mut sim, 4 * TICKS_PER_DAY);
    assert_eq!(sim.fire_count(), 0);
}

#[test]
fn r023_stone_and_soil_do_not_burn() {
    let mut sim = hearth_sim();
    certain_hearth_fire(&mut sim);
    sim.debug_spawn_item(material(&sim, "granite"), 5, 6, 3);
    sim.debug_spawn_item(material(&sim, "soil"), 5, 4, 3);
    let wall = building(&sim, "wall");
    assert!(sim.debug_build(wall, 5, 4, material(&sim, "granite")));
    run_to(&mut sim, 5 * TICKS_PER_DAY);
    assert_eq!(sim.fire_count(), 0);
}

#[test]
fn r023_a_wooden_bed_next_to_a_hearth_can_burn_but_a_floor_does_not_start_a_fire() {
    let mut sim = hearth_sim();
    certain_hearth_fire(&mut sim);
    let (bed, floor, log) = (
        building(&sim, "bed"),
        building(&sim, "floor"),
        material(&sim, "log"),
    );
    assert!(sim.debug_build(floor, 6, 3, log));
    run_to(&mut sim, 4 * TICKS_PER_DAY);
    assert_eq!(sim.fire_count(), 0, "a floor is not an object");
    assert!(sim.debug_build(bed, 4, 3, log));
    run_to(&mut sim, 5 * TICKS_PER_DAY);
    assert!(
        sim.drain_drought_events()
            .iter()
            .any(|e| matches!(e, DroughtEvent::HearthFire { x: 4, y: 3 }))
    );
}

// ---- R-024 ----

#[test]
fn r024_a_burning_tile_spreads_to_flammable_neighbors_and_not_to_stone() {
    let mut sim = quiet(OPEN);
    let (fw, granite) = (material(&sim, "firewood"), material(&sim, "granite"));
    sim.debug_spawn_item(fw, 3, 4, 3);
    sim.debug_spawn_item(fw, 3, 5, 3);
    sim.debug_spawn_item(granite, 3, 4, 4);
    assert!(sim.debug_ignite(4, 3, 10));
    run(&mut sim, 200);
    assert!(sim.fire_at(5, 3).is_some() || sim.items().at_tile(5, 3).is_empty());
    assert!(sim.fire_at(4, 4).is_none());
    assert_eq!(sim.items().at_tile(4, 4).len(), 1, "stone stays");
}

#[test]
fn r024_when_the_fuel_is_gone_flammable_items_and_buildings_are_gone() {
    let mut sim = quiet(OPEN);
    no_firefighter(&mut sim);
    let (fw, granite, log) = (
        material(&sim, "firewood"),
        material(&sim, "granite"),
        material(&sim, "log"),
    );
    let bed = building(&sim, "bed");
    sim.debug_spawn_item(fw, 3, 4, 3);
    sim.debug_spawn_item(granite, 3, 4, 3);
    assert!(sim.debug_build(bed, 4, 3, log));
    assert!(sim.debug_ignite(4, 3, 1));
    let steps = u64::from(sim.rules().drought.thing_fuel);
    let step_ticks = u64::from(sim.rules().drought.fire_step_ticks);
    run(&mut sim, steps * step_ticks + 20);
    assert_eq!(sim.fire_count(), 0);
    let left: Vec<_> = sim
        .items()
        .at_tile(4, 3)
        .iter()
        .map(|&i| sim.items().get(i).unwrap().material)
        .collect();
    assert_eq!(left, vec![granite]);
    assert!(sim.buildings().get(4, 3, BuildLayer::Object).is_none());
    assert!(sim.drain_drought_events().iter().any(|e| matches!(
        e,
        DroughtEvent::ThingsBurned {
            x: 4,
            y: 3,
            items: 1,
            buildings: 1
        }
    )));
}

#[test]
fn r024_rain_puts_out_fires_with_no_roof_but_not_under_a_roof() {
    let mut sim = quiet(OPEN);
    sim.set_climate(110, 160, 255);
    while !sim.weather_today().rain {
        run(&mut sim, TICKS_PER_DAY);
        assert!(sim.tick() < 200 * TICKS_PER_DAY, "no rain in 200 days");
    }
    let (fw, log) = (material(&sim, "firewood"), material(&sim, "log"));
    let roof = building(&sim, "roof");
    sim.debug_spawn_item(fw, 3, 2, 2);
    sim.debug_spawn_item(fw, 3, 7, 2);
    assert!(sim.debug_build(roof, 7, 2, log));
    assert!(sim.debug_ignite(2, 2, 3));
    assert!(sim.debug_ignite(7, 2, 3));
    run(&mut sim, 10);
    assert!(sim.fire_at(2, 2).is_none(), "open fire is out");
    assert!(
        sim.fire_at(7, 2).is_some(),
        "the fire under a roof burns on"
    );
}

#[test]
fn r024_a_colonist_with_an_extinguish_job_decreases_intensity_by_2_each_hour() {
    let mut sim = quiet("c.........\n..........\n..........");
    with_rules(&mut sim, |r| r.drought.thing_fuel = 1_000);
    sim.debug_spawn_item(material(&sim, "firewood"), 3, 4, 1);
    assert!(sim.debug_ignite(4, 1, 7));
    run(&mut sim, 30);
    assert!(
        sim.jobs()
            .iter()
            .any(|(_, j)| j.is_extinguish() && (j.x, j.y) == (4, 1))
    );
    let intensity = |s: &Sim| s.fire_at(4, 1).map(|f| f.intensity);
    run(&mut sim, TICKS_PER_HOUR * 6);
    let after_one = intensity(&sim);
    assert!(after_one.is_none_or(|i| i <= 5), "{after_one:?}");
    run(&mut sim, TICKS_PER_HOUR * 12);
    assert!(sim.fire_at(4, 1).is_none(), "7 -> 5 -> 3 -> 1 -> out");
    assert!(
        sim.drain_drought_events()
            .iter()
            .any(|e| matches!(e, DroughtEvent::FireOut { x: 4, y: 1 }))
    );
    assert!(sim.jobs().iter().all(|(_, j)| !j.is_extinguish()));
}

#[test]
fn r024_a_forest_fire_far_from_the_colony_makes_no_extinguish_job() {
    let mut sim = quiet("c.........\n.....T....\n..........");
    assert!(sim.debug_ignite(5, 1, 5));
    run(&mut sim, 40);
    assert_eq!(sim.fire_count(), 1);
    assert!(sim.jobs().iter().all(|(_, j)| !j.is_extinguish()));
}

#[test]
fn s15_extinguishing_makes_no_noise() {
    let mut sim = quiet("c.........\n..........\n..........");
    sim.debug_spawn_item(material(&sim, "firewood"), 3, 4, 1);
    assert!(sim.debug_ignite(4, 1, 2));
    let before = sim.notice().total;
    run(&mut sim, TICKS_PER_HOUR * 4);
    assert!(sim.fire_at(4, 1).is_none());
    assert_eq!(sim.notice().total, before);
}

#[test]
fn s15_a_colonist_on_a_burning_tile_loses_health_each_hour() {
    let mut sim = quiet("c.........");
    let id = sim.colonists().iter().next().unwrap().0;
    sim.debug_set_health(id, 80);
    no_firefighter(&mut sim);
    sim.debug_spawn_item(material(&sim, "firewood"), 3, 0, 0);
    assert!(sim.debug_ignite(0, 0, 3));
    run(&mut sim, TICKS_PER_HOUR + 1);
    let c = sim.colonist(id).unwrap();
    assert_eq!(c.health, 70);
    assert!(c.wounded);
}

#[test]
fn s15_the_start_camp_does_not_burn_in_the_first_10_days() {
    let data = worldgen::generate_local(&worldgen::EmbarkSummary::temperate_forest());
    let mut sim = Sim::new(&data, 1, 0);
    let built = sim.buildings().len();
    assert!(
        sim.hearth_tiles()
            .iter()
            .any(|&(x, y)| sim.hearth_burning(x, y))
    );
    run_to(&mut sim, 10 * TICKS_PER_DAY);
    assert!(
        sim.drain_drought_events()
            .iter()
            .all(|e| !matches!(e, DroughtEvent::HearthFire { .. })),
        "no hearth fire"
    );
    assert_eq!(sim.buildings().len(), built, "the camp is whole");
}

#[test]
fn s15_a_save_during_a_fire_goes_on_the_same_way() {
    let mut sim = quiet(OPEN);
    sim.debug_spawn_item(material(&sim, "firewood"), 3, 4, 3);
    sim.debug_ignite(4, 3, 5);
    run(&mut sim, 50);
    let mut loaded = save::from_bytes(&save::to_bytes(&sim)).unwrap();
    loaded.debug_set_needs_enabled(false);
    run(&mut sim, 1_000);
    run(&mut loaded, 1_000);
    assert_eq!(loaded.state_hash(), sim.state_hash());
}

// ---- R-017 ----

/// A colony with one colonist, plenty of food, and 2 beds.
fn colony() -> Sim {
    let mut sim = quiet(OPEN);
    let (bed, log) = (building(&sim, "bed"), material(&sim, "log"));
    assert!(sim.debug_build(bed, 3, 3, log));
    assert!(sim.debug_build(bed, 4, 3, log));
    sim.debug_spawn_item(material(&sim, "meal"), 100, 2, 2);
    with_rules(&mut sim, |r| {
        r.migrant.base_pct = 100;
        r.migrant.max_pct = 100;
        r.migrant.quiet_days = 1;
    });
    sim
}

#[test]
fn r017_migrants_need_a_free_bed_and_food_for_10_days() {
    let mut sim = colony();
    assert_eq!(sim.migrant_blocker(), None);
    let id = sim.colonists().iter().next().unwrap().0;
    assert!(sim.colonist(id).is_some());
    let (bed, log) = (building(&sim, "bed"), material(&sim, "log"));
    sim.debug_remove_building(3, 3, BuildLayer::Object);
    sim.debug_remove_building(4, 3, BuildLayer::Object);
    assert_eq!(sim.migrant_blocker(), Some(MigrantBlocker::NoFreeBed));
    let _ = sim.colonists().len();
    assert!(sim.debug_build(bed, 3, 3, log));
    assert!(sim.debug_build(bed, 4, 3, log));
    assert_eq!(sim.migrant_blocker(), None);
    let mut hungry = quiet(OPEN);
    assert!(hungry.debug_build(bed, 3, 3, log));
    assert_eq!(hungry.migrant_blocker(), Some(MigrantBlocker::LittleFood));
    run_to(&mut hungry, 3 * TICKS_PER_DAY);
    assert!(hungry.migrant_offer().is_none());
}

#[test]
fn r017_the_chance_is_5_percent_plus_1_for_each_1000_wealth_to_20() {
    let mut sim = quiet(OPEN);
    let wealth = sim
        .notice()
        .parts
        .iter()
        .find(|p| p.cause == sim::NoticeCause::Wealth)
        .unwrap()
        .value;
    assert_eq!(sim.migrant_chance_pct(), 5 + wealth);
    let iron = material(&sim, "iron");
    sim.debug_spawn_item(iron, 1_000, 1, 1);
    assert_eq!(sim.migrant_chance_pct(), 20, "the maximum");
}

#[test]
fn r017_no_migrants_in_the_quiet_days() {
    let mut sim = colony();
    with_rules(&mut sim, |r| r.migrant.quiet_days = 5);
    run_to(&mut sim, 5 * TICKS_PER_DAY);
    assert!(sim.migrant_offer().is_none());
    run_to(&mut sim, 6 * TICKS_PER_DAY);
    assert!(sim.migrant_offer().is_some());
}

#[test]
fn r017_an_accepted_group_joins_with_background_lines() {
    let mut sim = colony();
    run_to(&mut sim, 2 * TICKS_PER_DAY);
    let offer = sim.migrant_offer().expect("offer").clone();
    assert!((1..=2).contains(&offer.profiles.len()));
    assert!(matches!(
        sim.drain_migrant_events().as_slice(),
        [MigrantEvent::Offered { .. }]
    ));
    let before = sim.colonists().len();
    sim.step(&[Command::AcceptMigrants]);
    assert_eq!(sim.colonists().len(), before + offer.profiles.len());
    assert!(sim.migrant_offer().is_none());
    assert_eq!(sim.migrant_counts().0 as usize, offer.profiles.len());
    let newcomers: Vec<_> = sim
        .colonists()
        .iter()
        .filter(|(_, c)| c.background > 0)
        .collect();
    assert_eq!(newcomers.len(), offer.profiles.len());
    for (id, c) in newcomers {
        assert_eq!(c.tile(), offer.entry);
        assert!(!sim.background_line(id).unwrap().is_empty());
        assert!(c.health > 0 && c.hunger > 0, "needs started");
    }
    assert!(matches!(
        sim.drain_migrant_events().as_slice(),
        [MigrantEvent::Joined { .. }]
    ));
    assert!(sim.run_info().peak_population as usize >= sim.colonists().len());
}

#[test]
fn r017_refused_migrants_leave_the_map() {
    let mut sim = colony();
    run_to(&mut sim, 2 * TICKS_PER_DAY);
    let before = sim.colonists().len();
    sim.drain_migrant_events();
    sim.step(&[Command::RefuseMigrants]);
    assert_eq!(sim.colonists().len(), before);
    assert!(sim.migrant_offer().is_none());
    assert!(matches!(
        sim.drain_migrant_events().as_slice(),
        [MigrantEvent::Left {
            timed_out: false,
            ..
        }]
    ));
    assert!(sim.migrant_counts().1 > 0);
    sim.step(&[Command::AcceptMigrants]);
    assert_eq!(sim.drain_migrant_events(), vec![MigrantEvent::NoOffer]);
}

#[test]
fn r017_an_unanswered_offer_leaves_after_the_time_limit() {
    let mut sim = colony();
    run_to(&mut sim, 2 * TICKS_PER_DAY);
    let limit = u64::from(sim.rules().migrant.decision_ticks);
    sim.drain_migrant_events();
    with_rules(&mut sim, |r| {
        r.migrant.base_pct = 0;
        r.migrant.max_pct = 0;
    });
    run(&mut sim, limit - 2);
    assert!(sim.migrant_offer().is_some(), "the offer waits");
    run(&mut sim, 3);
    assert!(sim.migrant_offer().is_none());
    assert!(sim.drain_migrant_events().iter().any(|e| matches!(
        e,
        MigrantEvent::Left {
            timed_out: true,
            ..
        }
    )));
}

#[test]
fn r017_one_offer_at_a_time() {
    let mut sim = colony();
    run_to(&mut sim, 2 * TICKS_PER_DAY);
    let offer = sim.migrant_offer().cloned();
    assert!(!sim.debug_offer_migrants(1));
    run_to(&mut sim, 3 * TICKS_PER_DAY - 1);
    assert_eq!(sim.migrant_offer().cloned(), offer);
}

#[test]
fn r017_a_save_in_the_middle_of_an_offer_loads_and_goes_on_the_same_way() {
    let mut sim = colony();
    run_to(&mut sim, 2 * TICKS_PER_DAY + 10);
    assert!(sim.migrant_offer().is_some());
    let mut loaded = save::from_bytes(&save::to_bytes(&sim)).unwrap();
    loaded.debug_set_needs_enabled(false);
    assert_eq!(loaded.migrant_offer(), sim.migrant_offer());
    sim.step(&[Command::AcceptMigrants]);
    loaded.step(&[Command::AcceptMigrants]);
    run(&mut sim, 500);
    run(&mut loaded, 500);
    assert_eq!(loaded.state_hash(), sim.state_hash());
}

#[test]
fn r017_no_migrants_after_the_run_ended() {
    let mut sim = colony();
    sim.step(&[Command::Abandon]);
    run_to(&mut sim, 4 * TICKS_PER_DAY);
    assert!(sim.migrant_offer().is_none());
}

// ---- R-050 ----

#[test]
fn r050_the_legacy_score_counts_years_peak_crises_and_knowledge() {
    let mut sim = quiet(OPEN);
    let l = sim.legacy();
    assert_eq!(
        (l.years_survived, l.crises_survived, l.knowledge_unlocked),
        (0, 0, 0)
    );
    assert_eq!(l.peak_population, 1);
    sim.debug_set_dry_days(5);
    sim.set_climate(110, 160, 255);
    run_to(&mut sim, 60 * TICKS_PER_DAY);
    assert!(sim.legacy().crises_survived >= 1, "a drought ended");
    sim.step(&[Command::Abandon]);
    let end = sim.tick();
    run(&mut sim, TICKS_PER_YEAR / 100);
    assert_eq!(sim.legacy().years_survived, (end / TICKS_PER_YEAR) as u32);
}

#[test]
fn r050_ironman_asks_for_a_save_at_00_00_of_each_game_day() {
    let mut sim = quiet(OPEN);
    let mut days = 0;
    let mut due_at = Vec::new();
    for _ in 0..3 * TICKS_PER_DAY {
        sim.step(&[]);
        if sim.ironman_save_due() {
            days += 1;
            due_at.push(sim.tick());
        }
    }
    assert_eq!(days, 3);
    assert_eq!(
        due_at,
        vec![TICKS_PER_DAY, 2 * TICKS_PER_DAY, 3 * TICKS_PER_DAY]
    );
    sim.step(&[Command::Abandon]);
    run_to(&mut sim, 4 * TICKS_PER_DAY);
    assert!(!sim.ironman_save_due(), "not after the end");
    let _ = JobKind::Mine;
}
