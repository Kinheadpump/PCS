//! Sprint 17 tests (sim part): disease and medicine (R-060 to R-064).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use sim::scenario::sim_from_ascii;
use sim::{
    ColonistId, Command, DeathCause, DiseaseEvent, DiseaseKind, HealthCause, JobKind, NeedAct, Sim,
    SimRules, SpeedModCause, save,
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

fn run_to(sim: &mut Sim, tick: u64) {
    while sim.tick() < tick {
        sim.step(&[]);
    }
}

fn with_rules(sim: &mut Sim, edit: impl FnOnce(&mut SimRules)) {
    let mut rules = sim.rules().clone();
    edit(&mut rules);
    sim.debug_set_rules(Arc::new(rules));
}

/// A colony on flat ground at 20 degrees. No grace period.
fn flat(map: &str) -> Sim {
    let mut sim = sim_from_ascii(map, 1).unwrap();
    sim.set_climate(200, 0, 128);
    with_rules(&mut sim, |r| r.disease.grace_days = 0);
    sim
}

#[test]
fn r060_the_disease_content_is_valid_and_linked() {
    let rules = SimRules::default();
    assert_eq!(rules.disease.food_poisoning.start, 40);
    assert_eq!(rules.disease.wound_infection.untreated, 15);
    assert_eq!(rules.disease.winter_fever.treated, -30);
    assert!(rules.disease.winter_fever.contagious);
    assert!(rules.material_index("herbs").is_some());
    assert_eq!(
        rules.materials[usize::from(rules.medicine)].key,
        "herbal_medicine"
    );
    assert_eq!(rules.raw_foods.len(), 3);
    assert!(rules.recipes.iter().any(|r| r.key == "herbal_medicine"));
    assert!(rules.farm.wild.iter().any(|w| w.id == "wild_herb"));
    let text = "(grace_days: 1)";
    assert!(base::content::load_disease_rules("disease.ron", text).is_err());
}

#[test]
fn r060_a_colony_starts_with_10_days_without_new_disease() {
    assert_eq!(SimRules::default().disease.grace_days, 10);
}

#[test]
fn r060_severity_changes_each_day_by_the_content_value() {
    let mut sim = flat("c.........");
    let id = ids(&sim)[0];
    assert!(sim.debug_infect(id, DiseaseKind::WoundInfection));
    assert!(
        !sim.debug_infect(id, DiseaseKind::WoundInfection),
        "only once"
    );
    let report = &sim.illnesses(id)[0];
    assert_eq!((report.severity, report.change), (20, 15));
    sim.debug_disease_day();
    assert_eq!(sim.illnesses(id)[0].severity, 35);
    sim.debug_set_treated(id, 100);
    let report = &sim.illnesses(id)[0];
    assert_eq!(report.change, -20);
    assert_eq!(report.parts[0].cause, sim::IllnessCause::Treatment);
    sim.debug_disease_day();
    assert_eq!(sim.illnesses(id)[0].severity, 15);
}

#[test]
fn r060_a_disease_at_severity_0_ends() {
    let mut sim = flat("c.........");
    let id = ids(&sim)[0];
    sim.debug_infect(id, DiseaseKind::FoodPoisoning);
    sim.drain_disease_events();
    sim.debug_disease_day();
    assert_eq!(sim.illnesses(id)[0].severity, 20);
    sim.debug_disease_day();
    assert!(!sim.is_sick(id));
    assert!(matches!(
        sim.drain_disease_events().as_slice(),
        [DiseaseEvent::Recovered {
            kind: DiseaseKind::FoodPoisoning,
            ..
        }]
    ));
}

#[test]
fn r060_above_severity_50_a_disease_costs_2_health_each_hour() {
    let mut sim = flat("c.........");
    let id = ids(&sim)[0];
    sim.debug_set_health(id, 50);
    sim.debug_infect(id, DiseaseKind::WinterFever);
    let below = sim.health_report(id).unwrap();
    assert!(below.parts.iter().all(|p| p.cause != HealthCause::Disease));
    sim.debug_set_severity(id, DiseaseKind::WinterFever, 51);
    let above = sim.health_report(id).unwrap();
    let part = above
        .parts
        .iter()
        .find(|p| p.cause == HealthCause::Disease)
        .unwrap();
    assert_eq!(part.per_hour, -2);
    assert_eq!(above.per_hour, -2, "a severe disease stops the healing");
    run(&mut sim, 3 * HOUR);
    assert_eq!(sim.colonist(id).unwrap().health, 44);
}

#[test]
fn r060_a_colonist_can_die_of_a_disease() {
    let mut sim = flat("c.........");
    let id = ids(&sim)[0];
    sim.debug_set_health(id, 2);
    sim.debug_infect(id, DiseaseKind::WinterFever);
    sim.debug_set_severity(id, DiseaseKind::WinterFever, 80);
    run(&mut sim, HOUR + 1);
    assert_eq!(sim.graveyard().len(), 1);
    assert_eq!(sim.graveyard()[0].cause, DeathCause::Disease);
}

#[test]
fn r060_a_disease_lowers_the_work_speed_with_a_named_part() {
    let mut sim = flat("c.........");
    let id = ids(&sim)[0];
    let before = sim
        .effective_work_speed(id, JobKind::Cut)
        .unwrap()
        .total_pct;
    sim.debug_infect(id, DiseaseKind::FoodPoisoning);
    let after = sim.effective_work_speed(id, JobKind::Cut).unwrap();
    assert!(
        after
            .mods
            .iter()
            .any(|m| matches!((m.cause, m.pct), (SpeedModCause::Disease { kind: 0 }, -50)))
    );
    assert_eq!(after.total_pct, (before - 50).max(0));
}

#[test]
fn r061_food_poisoning_chance_is_20_percent_raw_and_5_percent_cooked() {
    let sim = flat("c.........");
    let id = ids(&sim)[0];
    let berries = mat(&sim, "berries");
    let meal = mat(&sim, "meal");
    assert_eq!(sim.food_poisoning_permille(id, berries, 0), 200);
    assert_eq!(sim.food_poisoning_permille(id, meal, 0), 50);
}

#[test]
fn r061_cooking_skill_lowers_the_cooked_chance_by_half_a_percent_per_level() {
    let mut sim = flat("c.........");
    let id = ids(&sim)[0];
    let meal = mat(&sim, "meal");
    sim.set_skill_xp(id, sim::Skill::Cooking, 1_000_000);
    let level = u32::from(sim.skill_level(id, sim::Skill::Cooking).unwrap());
    assert!(level > 0);
    assert_eq!(
        sim.food_poisoning_permille(id, meal, 0),
        50u32.saturating_sub(5 * level)
    );
}

#[test]
fn r061_rotten_food_adds_30_percent() {
    let sim = flat("c.........");
    let id = ids(&sim)[0];
    let berries = mat(&sim, "berries");
    let meal = mat(&sim, "meal");
    let half = sim.rules().spoil.rotten_at / 2;
    assert_eq!(sim.food_poisoning_permille(id, berries, half - 1), 200);
    assert_eq!(sim.food_poisoning_permille(id, berries, half), 500);
    assert_eq!(sim.food_poisoning_permille(id, meal, half), 350);
}

#[test]
fn r061_eating_raw_food_can_cause_food_poisoning_after_the_grace_days() {
    let mut sim = flat("c.........");
    with_rules(&mut sim, |r| {
        r.disease.grace_days = 1;
        r.disease.food_raw_pct = 100;
    });
    let id = ids(&sim)[0];
    let berries = mat(&sim, "berries");
    sim.debug_set_hunger(id, 10);
    sim.debug_spawn_item(berries, 1, 4, 0);
    run(&mut sim, 300);
    assert!(!sim.is_sick(id), "no new disease in the grace days");
    run_to(&mut sim, u64::from(DAY));
    sim.debug_set_hunger(id, 10);
    sim.debug_spawn_item(berries, 1, 4, 0);
    run(&mut sim, 300);
    let ill = sim.illnesses(id);
    assert_eq!(ill.len(), 1);
    assert_eq!(ill[0].kind, DiseaseKind::FoodPoisoning);
    assert_eq!(ill[0].severity, 40);
}

#[test]
fn r062_a_wounded_colonist_without_a_bed_gets_a_wound_infection() {
    let mut sim = flat("c.........");
    with_rules(&mut sim, |r| r.disease.wound_pct_no_bed = 100);
    let id = ids(&sim)[0];
    sim.debug_wound(id);
    sim.debug_disease_day();
    let ill = sim.illnesses(id);
    assert_eq!(ill.len(), 1);
    assert_eq!(
        (ill[0].kind, ill[0].severity),
        (DiseaseKind::WoundInfection, 20)
    );
}

#[test]
fn r062_a_colonist_that_is_not_wounded_gets_no_wound_infection() {
    let mut sim = flat("c.........");
    with_rules(&mut sim, |r| r.disease.wound_pct_no_bed = 100);
    let id = ids(&sim)[0];
    sim.debug_disease_day();
    assert!(!sim.is_sick(id));
}

#[test]
fn r062_a_treated_wound_uses_the_treated_chance_of_1_percent() {
    let mut sim = flat("c.........");
    with_rules(&mut sim, |r| {
        r.disease.wound_pct_no_bed = 100;
        r.disease.wound_pct_treated = 0;
    });
    let id = ids(&sim)[0];
    sim.debug_wound(id);
    sim.debug_set_treated(id, 1000);
    sim.debug_disease_day();
    assert!(!sim.is_sick(id));
}

#[test]
fn r062_a_wound_infection_without_treatment_grows_15_each_day() {
    let mut sim = flat("c.........");
    let id = ids(&sim)[0];
    sim.debug_infect(id, DiseaseKind::WoundInfection);
    for expected in [35, 50, 65] {
        sim.debug_disease_day();
        assert_eq!(sim.illnesses(id)[0].severity, expected);
    }
}

#[test]
fn r063_a_cold_colonist_can_catch_winter_fever_after_the_grace_days() {
    let mut sim = flat("c.........");
    sim.set_climate(-100, 0, 128);
    with_rules(&mut sim, |r| {
        r.disease.grace_days = 1;
        r.disease.cold_catch_permille = 1000;
    });
    let id = ids(&sim)[0];
    run_to(&mut sim, u64::from(DAY) - 1);
    assert!(!sim.is_sick(id), "no new disease in the grace days");
    run(&mut sim, 2 * HOUR);
    let ill = sim.illnesses(id);
    assert_eq!(ill.len(), 1);
    assert_eq!(ill[0].kind, DiseaseKind::WinterFever);
}

#[test]
fn r063_a_warm_colonist_does_not_catch_winter_fever() {
    let mut sim = flat("c.........");
    with_rules(&mut sim, |r| r.disease.cold_catch_permille = 1000);
    let id = ids(&sim)[0];
    run(&mut sim, 5 * HOUR);
    assert!(!sim.is_sick(id));
}

#[test]
fn r063_a_warm_bed_cures_winter_fever_15_each_day_and_a_cold_night_does_not() {
    let mut sim = flat("c.........");
    let id = ids(&sim)[0];
    let bed = bld(&sim, "bed");
    assert!(sim.debug_build(bed, 3, 0, mat(&sim, "plank")));
    sim.debug_infect(id, DiseaseKind::WinterFever);
    sim.debug_set_severity(id, DiseaseKind::WinterFever, 60);
    sim.debug_set_rest(id, 20);
    run(&mut sim, 3 * HOUR);
    assert!(matches!(
        sim.colonist(id).unwrap().act,
        Some(NeedAct::Sleep {
            bed: Some(_),
            asleep: true
        })
    ));
    sim.debug_disease_day();
    assert_eq!(sim.illnesses(id)[0].severity, 45);
    sim.debug_disease_day();
    assert_eq!(sim.illnesses(id)[0].severity, 55, "no night in a bed: +10");
}

#[test]
fn r063_after_recovery_a_colonist_cannot_catch_winter_fever_for_20_days() {
    let mut sim = flat("c.........");
    let id = ids(&sim)[0];
    sim.debug_infect(id, DiseaseKind::WinterFever);
    sim.debug_set_treated(id, 100_000);
    sim.debug_set_severity(id, DiseaseKind::WinterFever, 25);
    sim.debug_disease_day();
    assert!(!sim.is_sick(id), "treated: 25 - 30 ends it");
    assert!(!sim.debug_infect(id, DiseaseKind::WinterFever));
    sim.debug_set_needs_enabled(false);
    run(&mut sim, 19 * DAY);
    assert!(!sim.debug_infect(id, DiseaseKind::WinterFever), "day 19");
    run(&mut sim, 2 * DAY);
    assert!(sim.debug_infect(id, DiseaseKind::WinterFever), "day 21");
}

/// Two colonists on the ground in one walled room, or in two.
fn sleepers(separate: bool) -> Sim {
    let mut sim = sim_from_ascii(
        "...........\n...........\n...........\n...........\n...c...c...\n...........\n\
         ...........\n...........\n...........",
        1,
    )
    .unwrap();
    sim.set_climate(200, 0, 128);
    with_rules(&mut sim, |r| {
        r.disease.grace_days = 0;
        r.disease.infect_pct = 100;
        r.disease.infect_comfort_pct = 100;
    });
    let (granite, plank, log) = (mat(&sim, "granite"), mat(&sim, "plank"), mat(&sim, "log"));
    let (wall, door, roof) = (bld(&sim, "wall"), bld(&sim, "door"), bld(&sim, "roof"));
    for x in 1..=9u16 {
        for y in [1u16, 7] {
            if (x, y) != (3, 1) {
                assert!(sim.debug_build(wall, x, y, granite));
            }
        }
    }
    for y in 2..=6u16 {
        for x in [1u16, 9] {
            assert!(sim.debug_build(wall, x, y, granite));
        }
    }
    if separate {
        for y in 2..=6u16 {
            assert!(sim.debug_build(wall, 5, y, granite));
        }
    }
    assert!(sim.debug_build(door, 3, 1, plank));
    for y in 2..=6u16 {
        for x in 2..=8u16 {
            if !(separate && x == 5) {
                assert!(sim.debug_build(roof, x, y, log));
            }
        }
    }
    sim.step(&[]);
    sim
}

fn put_to_sleep(sim: &mut Sim) -> (ColonistId, ColonistId) {
    let all = ids(sim);
    let (a, b) = (all[0], all[1]);
    sim.debug_set_rest(a, 20);
    sim.debug_set_rest(b, 20);
    run(sim, 3 * HOUR);
    for id in [a, b] {
        assert!(matches!(
            sim.colonist(id).unwrap().act,
            Some(NeedAct::Sleep { asleep: true, .. })
        ));
    }
    (a, b)
}

#[test]
fn r063_winter_fever_spreads_to_a_colonist_that_sleeps_in_the_same_room() {
    let mut sim = sleepers(false);
    let (a, b) = put_to_sleep(&mut sim);
    assert_eq!(
        sim.room_at(3, 4).map(|r| r.tiles[0]),
        sim.room_at(7, 4).map(|r| r.tiles[0])
    );
    sim.debug_infect(a, DiseaseKind::WinterFever);
    sim.debug_disease_day();
    run(&mut sim, 2 * HOUR);
    sim.debug_disease_day();
    assert!(sim.is_sick(b), "same room");
}

#[test]
fn r064_contagion_happens_only_between_colonists_in_the_same_room() {
    let mut sim = sleepers(true);
    let (a, b) = put_to_sleep(&mut sim);
    assert_ne!(
        sim.room_at(3, 4).map(|r| r.tiles[0]),
        sim.room_at(7, 4).map(|r| r.tiles[0])
    );
    sim.debug_infect(a, DiseaseKind::WinterFever);
    for _ in 0..3 {
        run(&mut sim, HOUR);
        sim.debug_disease_day();
    }
    assert!(!sim.is_sick(b), "other room");
}

/// A patient in a bed, an idle doctor, and one herbal medicine.
fn clinic() -> (Sim, ColonistId, ColonistId) {
    let mut sim = flat("cc........");
    with_rules(&mut sim, |r| r.disease.treat_base_pct = 100);
    let bed = bld(&sim, "bed");
    assert!(sim.debug_build(bed, 5, 0, mat(&sim, "plank")));
    let ids = ids(&sim);
    sim.debug_set_rest(ids[0], 20);
    run(&mut sim, 2 * HOUR);
    assert!(matches!(
        sim.colonist(ids[0]).unwrap().act,
        Some(NeedAct::Sleep {
            bed: Some((5, 0)),
            asleep: true
        })
    ));
    (sim, ids[0], ids[1])
}

#[test]
fn r064_a_doctor_treats_a_sick_colonist_in_a_bed_with_1_medicine() {
    let (mut sim, patient, doctor) = clinic();
    let medicine = mat(&sim, "herbal_medicine");
    sim.debug_spawn_item(medicine, 1, 8, 0);
    sim.debug_infect(patient, DiseaseKind::FoodPoisoning);
    sim.drain_disease_events();
    let xp = sim.colonist(doctor).unwrap().skill_xp[sim::Skill::Medicine.index()];
    run(&mut sim, 3 * HOUR);
    assert!(sim.is_treated(patient));
    assert_eq!(sim.items().total_units(medicine), 0);
    assert!(sim.drain_disease_events().iter().any(|e| matches!(
        e,
        DiseaseEvent::Treated {
            x: 5,
            y: 0,
            success: true
        }
    )));
    let after = sim.colonist(doctor).unwrap().skill_xp[sim::Skill::Medicine.index()];
    assert!(after > xp, "the doctor learns medicine");
}

#[test]
fn r064_the_treatment_changes_the_next_daily_severity_by_the_treated_value() {
    let (mut sim, patient, _) = clinic();
    let medicine = mat(&sim, "herbal_medicine");
    sim.debug_spawn_item(medicine, 1, 8, 0);
    sim.debug_infect(patient, DiseaseKind::WinterFever);
    sim.debug_set_severity(patient, DiseaseKind::WinterFever, 60);
    run(&mut sim, 3 * HOUR);
    assert!(sim.is_treated(patient));
    assert_eq!(sim.illnesses(patient)[0].change, -30);
    sim.debug_disease_day();
    assert_eq!(sim.illnesses(patient)[0].severity, 30);
}

#[test]
fn r064_no_doctor_job_without_medicine() {
    let (mut sim, patient, _) = clinic();
    sim.debug_infect(patient, DiseaseKind::FoodPoisoning);
    run(&mut sim, 3 * HOUR);
    assert!(!sim.is_treated(patient));
    assert!(sim.jobs().iter().all(|(_, j)| !j.is_doctor()));
}

#[test]
fn r064_a_failed_treatment_uses_the_medicine_and_gives_no_treatment() {
    let (mut sim, patient, _) = clinic();
    with_rules(&mut sim, |r| {
        r.disease.treat_base_pct = 0;
        r.disease.treat_per_level_pct = 0;
    });
    let medicine = mat(&sim, "herbal_medicine");
    sim.debug_spawn_item(medicine, 1, 8, 0);
    sim.debug_infect(patient, DiseaseKind::FoodPoisoning);
    sim.drain_disease_events();
    run(&mut sim, 3 * HOUR);
    assert!(!sim.is_treated(patient));
    assert_eq!(sim.items().total_units(medicine), 0);
    assert!(
        sim.drain_disease_events()
            .iter()
            .any(|e| matches!(e, DiseaseEvent::Treated { success: false, .. }))
    );
}

#[test]
fn r064_a_doctor_job_goes_away_when_the_patient_is_gone() {
    let (mut sim, patient, _) = clinic();
    let medicine = mat(&sim, "herbal_medicine");
    sim.debug_spawn_item(medicine, 1, 8, 0);
    sim.debug_infect(patient, DiseaseKind::FoodPoisoning);
    run(&mut sim, HOUR);
    sim.debug_kill(patient);
    run(&mut sim, 2);
    assert!(sim.jobs().iter().all(|(_, j)| !j.is_doctor()));
    sim.check_invariants().unwrap();
}

#[test]
fn r064_sick_colonists_sleep_in_a_sick_room_if_a_bed_is_free_there() {
    let mut sim = sim_from_ascii(
        "c........\n.........\n.........\n.........\n.........\n.........\n.........\n.........\n.........",
        1,
    )
    .unwrap();
    sim.set_climate(200, 0, 128);
    with_rules(&mut sim, |r| r.disease.grace_days = 0);
    let (plank, log) = (mat(&sim, "plank"), mat(&sim, "log"));
    let (roof, bed) = (bld(&sim, "roof"), bld(&sim, "bed"));
    ring(&mut sim);
    for y in 4..=6u16 {
        for x in 4..=6u16 {
            assert!(sim.debug_build(roof, x, y, log));
        }
    }
    assert!(sim.debug_build(bed, 1, 0, plank));
    assert!(sim.debug_build(bed, 5, 5, plank));
    sim.step(&[]);
    assert!(!sim.is_sick_room(5, 5));
    sim.step(&[Command::SetSickRoom {
        x: 5,
        y: 5,
        on: true,
    }]);
    assert!(sim.is_sick_room(5, 5));
    assert!(!sim.is_sick_room(1, 0));
    let id = ids(&sim)[0];
    sim.debug_infect(id, DiseaseKind::FoodPoisoning);
    sim.debug_set_rest(id, 20);
    run(&mut sim, 6 * HOUR);
    let Some(NeedAct::Sleep { bed, .. }) = sim.colonist(id).unwrap().act else {
        panic!("the colonist should sleep");
    };
    assert_eq!(
        bed,
        Some((5, 5)),
        "the sick colonist takes the sick room bed"
    );
    sim.step(&[Command::SetSickRoom {
        x: 5,
        y: 5,
        on: false,
    }]);
    assert!(!sim.is_sick_room(5, 5));
}

#[test]
fn r064_a_healthy_colonist_leaves_the_bed_of_a_sick_room_alone() {
    let mut sim = sim_from_ascii("c........\n.........\n.........\n.........\n.........\n.........\n.........\n.........\n.........", 1).unwrap();
    sim.set_climate(200, 0, 128);
    let (plank, log) = (mat(&sim, "plank"), mat(&sim, "log"));
    let (roof, bed) = (bld(&sim, "roof"), bld(&sim, "bed"));
    ring(&mut sim);
    for y in 4..=6u16 {
        for x in 4..=6u16 {
            assert!(sim.debug_build(roof, x, y, log));
        }
    }
    assert!(sim.debug_build(bed, 4, 0, plank));
    assert!(sim.debug_build(bed, 5, 5, plank));
    sim.step(&[]);
    sim.step(&[Command::SetSickRoom {
        x: 5,
        y: 5,
        on: true,
    }]);
    let id = ids(&sim)[0];
    sim.debug_set_rest(id, 20);
    run(&mut sim, 6 * HOUR);
    let Some(NeedAct::Sleep { bed, .. }) = sim.colonist(id).unwrap().act else {
        panic!("the colonist should sleep");
    };
    assert_eq!(bed, Some((4, 0)));
}

#[test]
fn r064_a_sick_room_command_outside_a_room_is_refused() {
    let mut sim = flat("c.........");
    sim.step(&[Command::SetSickRoom {
        x: 3,
        y: 0,
        on: true,
    }]);
    assert!(!sim.is_sick_room(3, 0));
    assert!(
        sim.drain_job_events()
            .iter()
            .any(|e| matches!(e, sim::JobEvent::Rejected(_)))
    );
}

#[test]
fn r060_disease_state_survives_save_and_load() {
    let mut a = flat("cc........");
    let id = ids(&a)[0];
    a.debug_infect(id, DiseaseKind::WinterFever);
    a.debug_set_treated(id, 500);
    run(&mut a, 250);
    let mut b = save::from_bytes(&save::to_bytes(&a)).unwrap();
    assert_eq!(b.illnesses(id), a.illnesses(id));
    assert_eq!(b.is_treated(id), a.is_treated(id));
    assert_eq!(b.state_hash(), a.state_hash());
    run(&mut a, DAY);
    run(&mut b, DAY);
    assert_eq!(b.state_hash(), a.state_hash());
}

#[test]
fn r060_a_healthy_colony_has_the_same_state_with_two_runs() {
    let make = || {
        let mut s = sim_from_ascii("cc........", 7).unwrap();
        s.set_climate(200, 0, 128);
        run(&mut s, DAY);
        s.state_hash()
    };
    assert_eq!(make(), make());
}

/// Walls round (4..=6, 4..=6) with a door at (5,3).
fn ring(sim: &mut Sim) {
    let (granite, plank) = (mat(sim, "granite"), mat(sim, "plank"));
    let (wall, door) = (bld(sim, "wall"), bld(sim, "door"));
    for x in 3..=7u16 {
        for y in 3..=7u16 {
            if x != 3 && x != 7 && y != 3 && y != 7 {
                continue;
            }
            if (x, y) == (5, 3) {
                assert!(sim.debug_build(door, x, y, plank));
            } else {
                assert!(sim.debug_build(wall, x, y, granite));
            }
        }
    }
}
