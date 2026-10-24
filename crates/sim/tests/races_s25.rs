//! Sprint 25 tests (sim part): races (R-048, R-049). Races are content values: the speed of
//! ageing, the age of an elder, the comfort band, and the weight among migrants. Each pair of
//! races has a rule for attraction, for partners, and for offspring and the race of the child.
//! Races have no spells.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use sim::races::{ChildRace, load_race_rules};
use sim::scenario::sim_from_ascii;
use sim::{
    ColonistId, ComfortCause, Command, LifeStage, OpinionCause, Sex, Sim, SimRules, Standing, save,
};
use std::sync::Arc;

const HOUR: u64 = 100;

fn open_map(w: usize, h: usize, colonists: &[(usize, usize)]) -> String {
    let mut rows = vec![vec!['.'; w]; h];
    for &(x, y) in colonists {
        rows[y][x] = 'c';
    }
    rows.iter()
        .map(|r| r.iter().collect::<String>())
        .collect::<Vec<_>>()
        .join("\n")
}

fn calm(colonists: &[(usize, usize)]) -> Sim {
    let mut sim = sim_from_ascii(&open_map(20, 10, colonists), 1).unwrap();
    sim.set_climate(200, 0, 128);
    sim
}

fn with_rules(sim: &mut Sim, edit: impl FnOnce(&mut SimRules)) {
    let mut rules = sim.rules().clone();
    edit(&mut rules);
    sim.debug_set_rules(Arc::new(rules));
}

fn ids(sim: &Sim) -> Vec<ColonistId> {
    sim.colonists().iter().map(|(id, _)| id).collect()
}

fn run_hours(sim: &mut Sim, hours: u64) {
    for _ in 0..hours {
        for id in ids(sim) {
            sim.debug_set_hunger(id, 100);
            sim.debug_set_rest(id, 100);
        }
        for _ in 0..HOUR {
            sim.step(&[]);
        }
    }
}

fn run_days(sim: &mut Sim, days: u64) {
    run_hours(sim, days * 24);
}

/// Edit the pair of races a and b in the rules.
fn set_pair(sim: &mut Sim, a: u8, b: u8, edit: impl FnOnce(&mut sim::races::RacePairRule)) {
    with_rules(sim, |r| {
        let p = r
            .races
            .pairs
            .iter_mut()
            .find(|p| (p.a, p.b) == (a.min(b), a.max(b)))
            .unwrap();
        edit(p);
    });
}

/// A woman and a man of the races `ra` and `rb` with the highest possible opinion of each other.
fn pair_sim(ra: u8, rb: u8) -> (Sim, ColonistId, ColonistId) {
    let mut sim = calm(&[(1, 1), (2, 1)]);
    with_rules(&mut sim, |r| r.family.min_food_days = 0);
    let all = ids(&sim);
    sim.debug_set_sex(all[0], Sex::Female);
    sim.debug_set_sex(all[1], Sex::Male);
    sim.debug_set_race(all[0], ra);
    sim.debug_set_race(all[1], rb);
    (sim, all[0], all[1])
}

fn bond(sim: &mut Sim, a: ColonistId, b: ColonistId) {
    run_hours(sim, 1);
    sim.debug_set_chemistry(a, b, 10);
    sim.debug_set_relation_hours(a, b, 10_000, 10_000, 0);
    run_hours(sim, 2);
}

#[test]
fn r048_the_content_has_four_races_and_every_pair() {
    let rules = SimRules::builtin();
    let races = &rules.races;
    assert_eq!(races.races.len(), 4);
    assert_eq!(races.races[0].key, "folk");
    assert!(races.races.iter().all(|r| r.age_scale_pct > 0));
    let n = races.races.len();
    assert_eq!(races.pairs.len(), n * (n - 1) / 2);
    for a in 0..n {
        for b in a + 1..n {
            assert!(
                races
                    .pairs
                    .iter()
                    .any(|p| usize::from(p.a) == a && usize::from(p.b) == b)
            );
        }
    }
}

#[test]
fn r048_the_loader_needs_every_pair_and_known_keys() {
    let ok = r#"(races: [
        (key: "a", name: "A", age_scale_pct: 100, elder_age_delta: 0, comfort_low_delta: 0, migrant_weight: 1),
        (key: "b", name: "B", age_scale_pct: 100, elder_age_delta: 0, comfort_low_delta: 0, migrant_weight: 1),
    ], pairs: [(a: "a", b: "b", opinion: 0, partner: true, offspring: true, child: "mother")])"#;
    assert!(load_race_rules("races.ron", ok).is_ok());
    assert!(
        load_race_rules(
            "races.ron",
            &ok.replace("child: \"mother\"", "child: \"cat\"")
        )
        .is_err()
    );
    assert!(
        load_race_rules(
            "races.ron",
            &ok.replace("a: \"a\", b: \"b\"", "a: \"a\", b: \"x\"")
        )
        .is_err()
    );
    let none = ok.split("pairs:").next().unwrap().to_owned() + "pairs: [])";
    assert!(load_race_rules("races.ron", &none).is_err());
}

#[test]
fn r048_a_race_ages_at_its_own_speed() {
    let mut sim = calm(&[(1, 1), (2, 1)]);
    let all = ids(&sim);
    let base = sim.rules().family.days_per_age_year;
    sim.debug_set_race(all[1], 1);
    let slow = sim.race_age_days(1);
    assert_eq!(slow, base * sim.rules().races.races[1].age_scale_pct / 100);
    assert!(slow > base);
    for id in &all {
        sim.debug_set_age(*id, 30);
        sim.debug_set_age_days(*id, base - 1);
    }
    run_days(&mut sim, 1);
    assert_eq!(sim.colonist(all[0]).unwrap().age, 31);
    assert_eq!(sim.colonist(all[1]).unwrap().age, 30);
    sim.debug_set_age_days(all[1], slow - 1);
    run_days(&mut sim, 1);
    assert_eq!(sim.colonist(all[1]).unwrap().age, 31);
}

#[test]
fn r048_a_race_has_its_own_elder_age() {
    let mut sim = calm(&[(1, 1)]);
    let id = ids(&sim)[0];
    sim.debug_set_race(id, 1);
    let elder = sim.race_elder_age(1);
    assert_ne!(elder, sim.rules().family.elder_age);
    sim.debug_set_age(id, sim.rules().family.elder_age);
    assert_eq!(sim.family(id).unwrap().stage, LifeStage::Adult);
    sim.debug_set_age(id, elder);
    assert_eq!(sim.family(id).unwrap().stage, LifeStage::Elder);
}

#[test]
fn r048_the_comfort_band_has_a_named_race_part() {
    let mut sim = calm(&[(1, 1), (2, 1)]);
    let all = ids(&sim);
    let delta = i32::from(sim.rules().races.races[1].comfort_low_delta);
    assert_ne!(delta, 0);
    let folk = sim.comfort_band(all[0]).unwrap();
    sim.debug_set_race(all[1], 1);
    let other = sim.comfort_band(all[1]).unwrap();
    assert!(
        !folk
            .parts
            .iter()
            .any(|p| matches!(p.cause, ComfortCause::Race { .. }))
    );
    let part = other
        .parts
        .iter()
        .find(|p| p.cause == ComfortCause::Race { index: 1 })
        .unwrap();
    assert_eq!(part.tenths, delta);
    assert_eq!(other.low, folk.low + delta);
}

#[test]
fn r048_migrants_pick_a_race_by_weight() {
    let mut sim = calm(&[(1, 1)]);
    with_rules(&mut sim, |r| {
        for (i, race) in r.races.races.iter_mut().enumerate() {
            race.migrant_weight = if i == 3 { 5 } else { 0 };
        }
    });
    assert!(sim.debug_offer_migrants(6));
    let offer = sim.migrant_offer().unwrap().clone();
    assert!(offer.profiles.iter().all(|p| p.race == 3));
    sim.step(&[Command::AcceptMigrants]);
    let joined = sim.colonists().iter().filter(|(_, c)| c.race == 3).count();
    assert_eq!(joined, 6);
}

#[test]
fn r048_the_default_weights_give_several_races() {
    let mut sim = calm(&[(1, 1)]);
    let mut seen = [false; 4];
    for _ in 0..12 {
        if sim.debug_offer_migrants(5) {
            for p in &sim.migrant_offer().unwrap().profiles {
                seen[usize::from(p.race)] = true;
            }
            sim.step(&[Command::RefuseMigrants]);
        }
    }
    assert!(seen[0], "folk are the most common");
    assert!(seen.iter().filter(|&&s| s).count() >= 3, "{seen:?}");
}

#[test]
fn r049_a_pair_of_the_same_race_has_the_default_rule() {
    let sim = calm(&[(1, 1)]);
    for race in 0..4 {
        let p = sim.race_pair(race, race);
        assert_eq!((p.opinion, p.partner, p.offspring), (0, true, true));
        assert_eq!(p.child, ChildRace::Mother);
    }
    assert_eq!(sim.race_pair(1, 2), sim.race_pair(2, 1));
}

#[test]
fn r049_the_pair_opinion_is_a_named_part_of_the_relation() {
    let (mut sim, a, b) = pair_sim(0, 2);
    set_pair(&mut sim, 0, 2, |p| {
        p.opinion = 7;
        p.partner = false;
    });
    run_hours(&mut sim, 2);
    let report = sim.relation(a, b).unwrap();
    let part = report
        .parts
        .iter()
        .find(|p| p.cause == OpinionCause::Race)
        .unwrap();
    assert_eq!(part.value, 7);
    // Two people of the same race have no race part.
    let (mut same, c, d) = pair_sim(0, 0);
    run_hours(&mut same, 2);
    assert!(
        same.relation(c, d)
            .unwrap()
            .parts
            .iter()
            .all(|p| p.cause != OpinionCause::Race)
    );
}

#[test]
fn r049_two_people_whose_races_cannot_be_partners_stay_single() {
    let (mut sim, a, b) = pair_sim(0, 1);
    set_pair(&mut sim, 0, 1, |p| {
        p.opinion = 0;
        p.partner = false;
    });
    bond(&mut sim, a, b);
    assert!(sim.relation(a, b).unwrap().opinion >= sim.rules().relations.partner_at);
    assert_ne!(sim.relation(a, b).unwrap().standing, Standing::Partner);
    assert!(sim.partner_of(a).is_none());
}

#[test]
fn r049_two_people_whose_races_can_be_partners_become_partners() {
    let (mut sim, a, b) = pair_sim(0, 1);
    set_pair(&mut sim, 0, 1, |p| {
        p.opinion = 0;
        p.partner = true;
    });
    bond(&mut sim, a, b);
    assert_eq!(sim.partner_of(a), Some(b));
}

#[test]
fn r049_a_pair_without_offspring_has_no_pregnancy() {
    let (mut sim, a, b) = pair_sim(0, 1);
    set_pair(&mut sim, 0, 1, |p| {
        p.opinion = 0;
        p.partner = true;
        p.offspring = false;
    });
    with_rules(&mut sim, |r| {
        r.family.pregnancy_chance_pct = 100;
        r.family.spare_beds = 0;
    });
    bond(&mut sim, a, b);
    let bed = sim.rules().building_index("bed").unwrap();
    let plank = sim.rules().material_index("plank").unwrap();
    for x in 10..13 {
        assert!(sim.debug_build(bed, x, 5, plank));
    }
    run_days(&mut sim, 6);
    assert_eq!(sim.family(a).unwrap().pregnant_days_left, None);
    // The same pair with offspring does get a pregnancy.
    set_pair(&mut sim, 0, 1, |p| p.offspring = true);
    run_days(&mut sim, 6);
    assert!(sim.family(a).unwrap().pregnant_days_left.is_some());
}

/// The race of a child of a mother of race `rm` and a father of race `rf` for a rule.
fn child_race(rm: u8, rf: u8, child: ChildRace) -> u8 {
    let (mut sim, mother, father) = pair_sim(rm, rf);
    set_pair(&mut sim, rm, rf, |p| {
        p.opinion = 0;
        p.partner = true;
        p.offspring = true;
        p.child = child;
    });
    sim.debug_make_pregnant(mother, father, 1);
    run_days(&mut sim, 2);
    let (_, child) = sim
        .colonists()
        .iter()
        .find(|(_, c)| c.mother == Some(mother))
        .unwrap();
    child.race
}

#[test]
fn r049_the_child_gets_the_race_that_the_rule_names() {
    assert_eq!(child_race(0, 1, ChildRace::Mother), 0);
    assert_eq!(child_race(1, 0, ChildRace::Mother), 1);
    assert_eq!(child_race(0, 1, ChildRace::Father), 1);
    assert_eq!(child_race(1, 0, ChildRace::Father), 0);
    assert_eq!(child_race(0, 1, ChildRace::Race(3)), 3);
    let either = child_race(0, 1, ChildRace::Either);
    assert!(either == 0 || either == 1);
}

#[test]
fn r049_a_wedding_needs_races_that_can_be_partners() {
    let mut sim = calm(&[(1, 1)]);
    with_rules(&mut sim, |r| {
        r.neighbors.contact_permille = 1000;
        r.raid.quiet_days = 10_000;
    });
    // No folk among the neighbors, so that the pair is a cross pair.
    with_rules(&mut sim, |r| r.races.races[0].migrant_weight = 0);
    sim.debug_add_rival(8, 10, 2);
    let id = ids(&sim)[0];
    let person = sim.neighbor_person(0, 1).unwrap();
    assert!(person.race > 0 && usize::from(person.race) < sim.rules().races.races.len());
    set_pair(&mut sim, 0, person.race, |p| p.partner = false);
    sim.debug_set_neighbor_tie(id, 0, 1, 500, 10);
    run_days(&mut sim, 2);
    assert_eq!(sim.colonists().len(), 1);
    set_pair(&mut sim, 0, person.race, |p| p.partner = true);
    run_days(&mut sim, 1);
    assert_eq!(sim.colonists().len(), 2);
    let partner = sim.partner_of(id).unwrap();
    assert_eq!(sim.colonist(partner).unwrap().race, person.race);
}

#[test]
fn r048_races_survive_a_save_and_the_run_is_repeatable() {
    let make = || {
        let (mut sim, a, b) = pair_sim(0, 3);
        sim.debug_set_age_days(a, 3);
        run_days(&mut sim, 3);
        let _ = b;
        sim
    };
    let a = make();
    let b = make();
    assert_eq!(a.state_hash(), b.state_hash());
    let loaded = save::from_bytes(&save::to_bytes(&a)).unwrap();
    assert_eq!(loaded.state_hash(), a.state_hash());
    assert_eq!(
        loaded
            .colonists()
            .iter()
            .filter(|(_, c)| c.race == 3)
            .count(),
        1
    );
    assert_eq!(save::FORMAT_VERSION, 20);
}
