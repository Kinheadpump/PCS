//! Sprint 23 tests (sim part): colony growth (R-017, R-040). Mood and bonds change the migrant
//! chance, each as a named part. A colonist leaves when the colony fails the life goal for a
//! long time and the mood is low.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use sim::scenario::sim_from_ascii;
use sim::{ChanceCause, ChronicleKind, ColonistId, LifeEvent, Sim};

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
    let mut sim = sim_from_ascii(&open_map(30, 10, colonists), 1).unwrap();
    sim.set_climate(200, 0, 128);
    sim
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

fn part(sim: &Sim, pick: fn(&ChanceCause) -> bool) -> Option<i32> {
    sim.migrant_chance()
        .parts
        .iter()
        .find(|p| pick(&p.cause))
        .map(|p| p.value)
}

#[test]
fn r017_the_chance_is_a_sum_of_named_parts_and_the_total_follows_the_limits() {
    let sim = calm(&[(1, 1), (2, 1)]);
    let chance = sim.migrant_chance();
    let sum: i32 = chance.parts.iter().map(|p| p.value).sum();
    let max = i32::try_from(sim.rules().migrant.max_pct).unwrap();
    assert_eq!(i32::try_from(chance.total).unwrap(), sum.clamp(0, max));
    assert_eq!(chance.total, sim.migrant_chance_pct());
    assert!(chance.parts.iter().any(|p| p.cause == ChanceCause::Base));
}

#[test]
fn r040_a_good_mood_raises_the_chance_and_a_bad_mood_lowers_it() {
    let mut sim = calm(&[(1, 1), (2, 1)]);
    assert_eq!(part(&sim, |c| *c == ChanceCause::Mood), Some(0));
    for id in ids(&sim) {
        for _ in 0..3 {
            sim.debug_add_memory(id, "had_company");
            sim.debug_add_memory(id, "ate_meal");
            sim.debug_add_memory(id, "slept_bed");
        }
    }
    let good = part(&sim, |c| *c == ChanceCause::Mood).unwrap();
    assert!(good > 0, "{good}");
    for id in ids(&sim) {
        for key in [
            "colonist_died",
            "saw_danger",
            "ate_raw",
            "slept_ground",
            "lonely",
        ] {
            sim.debug_add_memory(id, key);
        }
    }
    let bad = part(&sim, |c| *c == ChanceCause::Mood).unwrap();
    assert!(bad < 0, "{bad}");
    let r = sim.rules().migrant.clone();
    assert!(bad >= -i32::try_from(r.mood_pct_max).unwrap());
}

#[test]
fn r040_friends_and_partners_raise_the_chance_to_a_limit() {
    let mut sim = calm(&[(1, 1), (2, 1), (3, 1), (4, 1)]);
    let all = ids(&sim);
    run_hours(&mut sim, 1);
    assert_eq!(part(&sim, |c| *c == ChanceCause::Bonds), Some(0));
    sim.debug_set_relation_hours(all[0], all[1], 480, 0, 0);
    run_hours(&mut sim, 1);
    let one = part(&sim, |c| *c == ChanceCause::Bonds).unwrap();
    assert!(one >= 1, "{one}");
    for i in 0..4 {
        for j in (i + 1)..4 {
            sim.debug_set_relation_hours(all[i], all[j], 10_000, 0, 0);
        }
    }
    run_hours(&mut sim, 1);
    let many = part(&sim, |c| *c == ChanceCause::Bonds).unwrap();
    assert_eq!(
        many,
        i32::try_from(sim.rules().migrant.bond_pct_max).unwrap()
    );
}

#[test]
fn r040_a_colonist_leaves_when_the_goal_fails_for_long_and_the_mood_is_low() {
    let mut sim = calm(&[(1, 1), (2, 1), (25, 8)]);
    let all = ids(&sim);
    assert!(sim.debug_set_goal(all[0], Some("green_thumb")));
    let w = sim.rules().wants.clone();
    let hours = u64::from(w.goal_patience_hours + w.goal_leave_extra_hours);
    // Not before the time, even with a bad mood.
    sim.debug_set_goal_unmet_hours(all[0], w.goal_patience_hours + w.goal_leave_extra_hours - 3);
    for _ in 0..2 {
        for key in ["colonist_died", "saw_danger", "ate_raw", "slept_ground"] {
            sim.debug_add_memory(all[0], key);
        }
        run_hours(&mut sim, 1);
    }
    assert!(sim.colonists().get(all[0]).is_some());
    for _ in 0..6 {
        for key in ["colonist_died", "saw_danger", "ate_raw", "slept_ground"] {
            sim.debug_add_memory(all[0], key);
        }
        run_hours(&mut sim, 1);
    }
    assert!(
        sim.colonists().get(all[0]).is_none(),
        "{hours} hours: the colonist left"
    );
    assert!(
        sim.drain_life_events()
            .iter()
            .any(|e| matches!(e, LifeEvent::Left { .. }))
    );
    assert!(
        sim.chronicle()
            .iter()
            .any(|e| e.kind == ChronicleKind::LeftGoals)
    );
}

#[test]
fn r040_a_colonist_with_a_good_mood_stays_although_the_goal_fails() {
    let mut sim = calm(&[(1, 1), (2, 1)]);
    let all = ids(&sim);
    assert!(sim.debug_set_goal(all[0], Some("green_thumb")));
    let w = sim.rules().wants.clone();
    sim.debug_set_goal_unmet_hours(
        all[0],
        w.goal_patience_hours + w.goal_leave_extra_hours + 100,
    );
    run_hours(&mut sim, 12);
    assert!(sim.colonists().get(all[0]).is_some());
}

#[test]
fn r040_the_last_colonists_do_not_leave() {
    let mut sim = calm(&[(1, 1)]);
    let id = ids(&sim)[0];
    assert!(sim.debug_set_goal(id, Some("green_thumb")));
    let w = sim.rules().wants.clone();
    sim.debug_set_goal_unmet_hours(id, w.goal_patience_hours + w.goal_leave_extra_hours + 100);
    for _ in 0..8 {
        for key in ["colonist_died", "saw_danger", "ate_raw", "slept_ground"] {
            sim.debug_add_memory(id, key);
        }
        run_hours(&mut sim, 1);
    }
    assert!(sim.colonists().get(id).is_some(), "min_colonists_to_leave");
}

#[test]
fn r040_a_child_does_not_count_unmet_goal_hours_and_never_leaves() {
    let mut sim = calm(&[(1, 1), (2, 1)]);
    let all = ids(&sim);
    sim.debug_set_age(all[0], 5);
    assert!(sim.debug_set_goal(all[0], Some("green_thumb")));
    run_hours(&mut sim, 30);
    assert_eq!(sim.wants(all[0]).unwrap().goal.unwrap().unmet_hours, 0);
    let w = sim.rules().wants.clone();
    sim.debug_set_goal_unmet_hours(
        all[0],
        w.goal_patience_hours + w.goal_leave_extra_hours + 100,
    );
    for _ in 0..8 {
        for key in ["colonist_died", "saw_danger", "ate_raw", "slept_ground"] {
            sim.debug_add_memory(all[0], key);
        }
        run_hours(&mut sim, 1);
    }
    assert!(sim.colonists().get(all[0]).is_some());
}

#[test]
fn r040_the_goal_report_tells_when_the_colonist_may_leave() {
    let mut sim = calm(&[(1, 1), (2, 1)]);
    let id = ids(&sim)[0];
    assert!(sim.debug_set_goal(id, Some("green_thumb")));
    let g = sim.wants(id).unwrap().goal.unwrap();
    let w = sim.rules().wants.clone();
    assert_eq!(
        g.leave_after_hours,
        w.goal_patience_hours + w.goal_leave_extra_hours
    );
}
