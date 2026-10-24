//! Sprint 21 tests (sim part): relationships and bonds (R-035, R-036). Two colonists have an
//! opinion of each other. The opinion is a sum of named parts. The opinion gives a standing
//! (friend, rival, enemy, partner), and the standing changes the mood.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use sim::scenario::sim_from_ascii;
use sim::{ColonistId, MoodCause, OpinionCause, Sim, Social, Standing, save};

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

fn calm(w: usize, h: usize, colonists: &[(usize, usize)]) -> Sim {
    let mut sim = sim_from_ascii(&open_map(w, h, colonists), 1).unwrap();
    sim.set_climate(200, 0, 128);
    sim
}

fn ids(sim: &Sim) -> Vec<ColonistId> {
    sim.colonists().iter().map(|(id, _)| id).collect()
}

/// Run `hours` game hours. Hunger and rest are full every hour, so that no need moves a colonist.
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

fn memory_value(sim: &Sim, id: ColonistId, key: &str) -> Option<i32> {
    let index = sim.rules().needs.memory_index(key)?;
    sim.mood(id)?
        .parts
        .iter()
        .find(|p| p.cause == MoodCause::Memory { key: index })
        .map(|p| p.value)
}

fn part(sim: &Sim, a: ColonistId, b: ColonistId, pick: fn(&OpinionCause) -> bool) -> Option<i32> {
    sim.relation(a, b)?
        .parts
        .iter()
        .find(|p| pick(&p.cause))
        .map(|p| p.value)
}

#[test]
fn r035_the_content_has_ordered_limits_and_the_memory_keys() {
    let rules = sim::SimRules::builtin();
    let r = &rules.relations;
    assert!(r.enemy_at < r.rival_at && r.rival_at < 0 && 0 < r.friend_at);
    assert!(r.friend_at < r.partner_at);
    for key in [
        "near_friend",
        "near_rival",
        "near_enemy",
        "friend_died",
        "partner_died",
        "new_partner",
    ] {
        assert!(rules.needs.memory_index(key).is_some(), "{key}");
    }
}

#[test]
fn r035_colonists_that_are_near_get_an_opinion_and_far_ones_do_not() {
    let mut sim = calm(40, 10, &[(1, 1), (3, 1), (35, 8)]);
    let all = ids(&sim);
    run_hours(&mut sim, 25);
    let r = sim.relation(all[0], all[1]).unwrap();
    assert_eq!(r.standing, Standing::Acquaintance);
    assert!(sim.relation(all[0], all[2]).is_none());
    assert!(sim.relations_of(all[2]).is_empty());
    let per = i32::try_from(sim.rules().relations.together_hours_per_point).unwrap();
    let together = part(&sim, all[0], all[1], |c| {
        matches!(c, OpinionCause::Together { .. })
    })
    .unwrap();
    assert_eq!(together, 25 / per);
}

#[test]
fn r035_the_opinion_is_symmetric_and_the_sum_of_its_named_parts() {
    let mut sim = calm(10, 10, &[(1, 1), (2, 1)]);
    let all = ids(&sim);
    sim.debug_set_social(all[0], Social::Extrovert);
    sim.debug_set_social(all[1], Social::Extrovert);
    run_hours(&mut sim, 30);
    let ab = sim.relation(all[0], all[1]).unwrap();
    let ba = sim.relation(all[1], all[0]).unwrap();
    assert_eq!(ab, ba);
    assert_eq!(ab.parts.iter().map(|p| p.value).sum::<i32>(), ab.opinion);
    assert!(
        ab.parts
            .iter()
            .any(|p| p.cause == OpinionCause::Temperament)
    );
    assert!(ab.parts.iter().any(|p| p.cause == OpinionCause::Chemistry));
}

#[test]
fn r035_temperament_is_plus_for_the_same_trait_and_minus_for_opposite_traits() {
    let mut sim = calm(10, 10, &[(1, 1), (2, 1), (3, 1)]);
    let all = ids(&sim);
    sim.debug_set_social(all[0], Social::Extrovert);
    sim.debug_set_social(all[1], Social::Extrovert);
    sim.debug_set_social(all[2], Social::Introvert);
    run_hours(&mut sim, 2);
    let temper = |a, b| part(&sim, a, b, |c| *c == OpinionCause::Temperament);
    let rules = sim.rules().relations.clone();
    assert_eq!(temper(all[0], all[1]), Some(rules.same_trait));
    assert_eq!(temper(all[0], all[2]), Some(rules.opposite_trait));
}

#[test]
fn r035_chemistry_is_the_same_for_the_same_seed_and_inside_its_limit() {
    let make = |seed| {
        let mut sim = sim_from_ascii(&open_map(10, 10, &[(1, 1), (2, 1), (3, 1)]), seed).unwrap();
        sim.set_climate(200, 0, 128);
        run_hours(&mut sim, 2);
        let all = ids(&sim);
        let c = |a, b| part(&sim, a, b, |c| *c == OpinionCause::Chemistry).unwrap_or(0);
        vec![c(all[0], all[1]), c(all[0], all[2]), c(all[1], all[2])]
    };
    let (a, b, other) = (make(1), make(1), make(2));
    assert_eq!(a, b);
    let limit = SimLimit::get();
    assert!(a.iter().all(|v| v.abs() <= limit));
    assert_ne!(a, other, "another seed, other chemistry");
}

struct SimLimit;
impl SimLimit {
    fn get() -> i32 {
        sim::SimRules::builtin().relations.chemistry_max
    }
}

#[test]
fn r035_a_colonist_in_a_bad_mood_makes_quarrels_that_lower_the_opinion() {
    let mut sim = calm(10, 10, &[(1, 1), (2, 1)]);
    let all = ids(&sim);
    for _ in 0..48 {
        for key in ["colonist_died", "saw_danger", "ate_raw", "slept_ground"] {
            sim.debug_add_memory(all[0], key);
        }
        run_hours(&mut sim, 1);
    }
    assert!(sim.mood(all[0]).unwrap().total < 35);
    let q = part(&sim, all[0], all[1], |c| {
        matches!(c, OpinionCause::Quarrels { .. })
    })
    .unwrap();
    let per = i32::try_from(sim.rules().relations.quarrel_hours_per_point).unwrap();
    assert!(q < 0 && q >= -(48 / per));
}

#[test]
fn r035_a_sleeping_pair_near_each_other_gets_the_shared_sleep_part() {
    let mut sim = calm(10, 10, &[(1, 1), (2, 1)]);
    let all = ids(&sim);
    for &id in &all {
        sim.debug_set_rest(id, 1);
    }
    for _ in 0..(30 * HOUR) {
        sim.step(&[]);
        for &id in &all {
            sim.debug_set_hunger(id, 100);
        }
    }
    let s = part(&sim, all[0], all[1], |c| {
        matches!(c, OpinionCause::SharedSleep { .. })
    })
    .unwrap();
    assert!(s >= 1);
}

#[test]
fn r035_standing_follows_the_opinion_and_each_change_makes_an_event() {
    let mut sim = calm(10, 10, &[(1, 1), (2, 1)]);
    let all = ids(&sim);
    let rules = sim.rules().relations.clone();
    run_hours(&mut sim, 1);
    sim.drain_relation_events();
    // Together hours that give exactly `friend_at` points are enough to pass the limit.
    let hours = rules.together_hours_per_point * u32::try_from(rules.together_max).unwrap();
    sim.debug_set_relation_hours(all[0], all[1], hours, 0, 0);
    run_hours(&mut sim, 1);
    let r = sim.relation(all[0], all[1]).unwrap();
    assert_eq!(r.standing, Standing::Friend);
    let events = sim.drain_relation_events();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].from, Standing::Acquaintance);
    assert_eq!(events[0].to, Standing::Friend);
    // Quarrels drag it down to a rival and an enemy.
    sim.debug_set_relation_hours(all[0], all[1], 0, 0, 100_000);
    run_hours(&mut sim, 1);
    let r = sim.relation(all[0], all[1]).unwrap();
    assert!(matches!(r.standing, Standing::Rival | Standing::Enemy));
}

#[test]
fn r036_a_friend_near_gives_a_good_memory_and_an_enemy_near_a_bad_one() {
    let mut sim = calm(10, 10, &[(1, 1), (2, 1), (3, 1)]);
    let all = ids(&sim);
    run_hours(&mut sim, 1);
    sim.debug_set_relation_hours(all[0], all[1], 480, 0, 0);
    sim.debug_set_relation_hours(all[0], all[2], 0, 0, 100_000);
    run_hours(&mut sim, 2);
    assert_eq!(memory_value(&sim, all[0], "near_friend"), Some(2));
    assert_eq!(memory_value(&sim, all[1], "near_friend"), Some(2));
    assert_eq!(memory_value(&sim, all[0], "near_enemy"), Some(-4));
    assert_eq!(memory_value(&sim, all[2], "near_enemy"), Some(-4));
    assert_eq!(memory_value(&sim, all[2], "near_friend"), None);
}

#[test]
fn r036_when_a_friend_dies_the_friend_gets_a_named_memory_and_the_relation_ends() {
    let mut sim = calm(10, 10, &[(1, 1), (2, 1), (3, 1)]);
    let all = ids(&sim);
    run_hours(&mut sim, 1);
    sim.debug_set_relation_hours(all[0], all[1], 480, 0, 0);
    run_hours(&mut sim, 1);
    sim.debug_kill(all[0]);
    assert_eq!(memory_value(&sim, all[1], "friend_died"), Some(-6));
    assert_eq!(
        memory_value(&sim, all[2], "friend_died"),
        None,
        "not a friend"
    );
    assert!(sim.relation(all[0], all[1]).is_none());
    assert!(
        sim.relations_of(all[1])
            .iter()
            .all(|(o, _, _)| *o != all[0])
    );
}

#[test]
fn r036_two_colonists_with_a_high_opinion_become_partners() {
    let mut sim = calm(10, 10, &[(1, 1), (2, 1), (3, 1)]);
    let all = ids(&sim);
    run_hours(&mut sim, 1);
    sim.drain_relation_events();
    sim.debug_set_chemistry(all[0], all[1], 10);
    sim.debug_set_chemistry(all[0], all[2], 10);
    sim.debug_set_relation_hours(all[0], all[1], 10_000, 10_000, 0);
    sim.debug_set_relation_hours(all[0], all[2], 10_000, 10_000, 0);
    run_hours(&mut sim, 2);
    assert_eq!(sim.partner_of(all[0]), Some(all[1]), "the first in order");
    assert_eq!(sim.partner_of(all[1]), Some(all[0]));
    assert_eq!(sim.partner_of(all[2]), None, "one partner at most");
    assert_eq!(
        sim.relation(all[0], all[1]).unwrap().standing,
        Standing::Partner
    );
    assert_eq!(
        sim.relation(all[0], all[2]).unwrap().standing,
        Standing::Friend
    );
    assert_eq!(memory_value(&sim, all[0], "new_partner"), Some(8));
    let events = sim.drain_relation_events();
    assert!(events.iter().any(|e| e.to == Standing::Partner));
    // The partner dies.
    sim.debug_kill(all[0]);
    assert_eq!(memory_value(&sim, all[1], "partner_died"), Some(-15));
    assert_eq!(sim.partner_of(all[1]), None);
}

#[test]
fn r036_a_colonist_below_the_partner_age_does_not_become_a_partner() {
    let mut sim = calm(10, 10, &[(1, 1), (2, 1)]);
    let all = ids(&sim);
    run_hours(&mut sim, 1);
    sim.debug_set_age(all[0], 15);
    sim.debug_set_chemistry(all[0], all[1], 10);
    sim.debug_set_relation_hours(all[0], all[1], 10_000, 10_000, 0);
    run_hours(&mut sim, 2);
    assert_eq!(sim.partner_of(all[0]), None);
}

#[test]
fn r035_a_scenario_colony_with_one_colonist_has_no_relations() {
    let mut sim = calm(10, 10, &[(1, 1)]);
    run_hours(&mut sim, 30);
    assert!(sim.relations_of(ids(&sim)[0]).is_empty());
}

#[test]
fn r035_relations_survive_a_save_and_the_format_is_current() {
    let mut sim = calm(10, 10, &[(1, 1), (2, 1)]);
    let all = ids(&sim);
    run_hours(&mut sim, 30);
    let bytes = save::to_bytes(&sim);
    assert_eq!(save::FORMAT_VERSION, 20);
    let loaded = save::from_bytes(&bytes).unwrap();
    assert_eq!(
        loaded.relation(all[0], all[1]),
        sim.relation(all[0], all[1])
    );
    assert_eq!(loaded.state_hash(), sim.state_hash());
}

#[test]
fn r035_a_new_colony_grows_relations_and_two_runs_agree() {
    let data = worldgen::generate_local(&worldgen::EmbarkSummary::temperate_forest());
    let run = |seed| {
        let mut sim = Sim::new(&data, seed, 0);
        for _ in 0..(3 * 2400) {
            sim.step(&[]);
        }
        sim
    };
    let (a, b) = (run(5), run(5));
    assert_eq!(a.state_hash(), b.state_hash());
    let first = ids(&a)[0];
    assert!(
        !a.relations_of(first).is_empty(),
        "the start camp is crowded"
    );
}
