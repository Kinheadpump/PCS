//! Sprint 20 tests (sim part): wants and personality (R-033, R-034). Each colonist has a
//! social trait and one life goal. Met wants and unmet wants give named mood memories.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use base::content::BuildingKind;
use sim::scenario::sim_from_ascii;
use sim::{ColonistId, MoodCause, Sim, Skill, Social, save};

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

/// A sim with a mild, flat climate, so that the colonists stay alive.
fn calm(map: &str) -> Sim {
    let mut sim = sim_from_ascii(map, 1).unwrap();
    sim.set_climate(200, 0, 128);
    sim
}

fn ids(sim: &Sim) -> Vec<ColonistId> {
    sim.colonists().iter().map(|(id, _)| id).collect()
}

/// Run `hours` game hours. Every hour the colonists get full hunger and rest, so that no
/// need action moves them.
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

#[test]
fn r033_the_content_has_goals_and_the_memory_keys_that_the_sim_needs() {
    let rules = sim::SimRules::builtin();
    assert!(rules.wants.goals.len() >= 3);
    for key in [
        "had_company",
        "lonely",
        "time_alone",
        "crowded",
        "goal_met",
        "goal_unmet",
    ] {
        assert!(rules.needs.memory_index(key).is_some(), "{key}");
    }
    let mut seen: Vec<&str> = Vec::new();
    for g in &rules.wants.goals {
        assert!(!seen.contains(&g.id.as_str()), "duplicate {}", g.id);
        seen.push(&g.id);
    }
}

#[test]
fn r033_scenario_colonists_are_neutral_and_have_no_goal() {
    let sim = calm(&open_map(6, 6, &[(1, 1), (3, 3)]));
    for id in ids(&sim) {
        let w = sim.wants(id).unwrap();
        assert_eq!(w.social, Social::Neutral);
        assert!(w.goal.is_none());
    }
}

#[test]
fn r033_a_new_colony_gives_every_colonist_a_social_trait_and_a_goal() {
    let data = worldgen::generate_local(&worldgen::EmbarkSummary::temperate_forest());
    let sim = Sim::new(&data, 1, 0);
    let mut social = [0u32; 3];
    for id in ids(&sim) {
        let w = sim.wants(id).unwrap();
        social[w.social as usize] += 1;
        assert!(w.goal.is_some());
    }
    assert!(social[1] + social[2] >= 3, "most colonists have a trait");
}

#[test]
fn r033_the_same_seed_gives_the_same_personalities() {
    let data = worldgen::generate_local(&worldgen::EmbarkSummary::temperate_forest());
    let a = Sim::new(&data, 3, 0);
    let b = Sim::new(&data, 3, 0);
    let c = Sim::new(&data, 4, 0);
    let list = |s: &Sim| -> Vec<(Social, Option<String>)> {
        ids(s)
            .into_iter()
            .map(|id| {
                let w = s.wants(id).unwrap();
                (w.social, w.goal.map(|g| g.id))
            })
            .collect()
    };
    assert_eq!(list(&a), list(&b));
    assert_ne!(list(&a), list(&c), "another seed, other personalities");
}

#[test]
fn r034_an_extrovert_with_company_gets_a_good_memory_and_alone_a_lonely_one() {
    let mut sim = calm(&open_map(30, 10, &[(1, 1), (3, 1), (25, 8)]));
    let all = ids(&sim);
    for &id in &all {
        sim.debug_set_social(id, Social::Extrovert);
    }
    run_hours(&mut sim, 2);
    assert_eq!(memory_value(&sim, all[0], "had_company"), Some(3));
    assert_eq!(memory_value(&sim, all[1], "had_company"), Some(3));
    assert_eq!(memory_value(&sim, all[2], "had_company"), None);
    assert_eq!(memory_value(&sim, all[2], "lonely"), None, "not yet");
    let after = u64::from(sim.rules().wants.lonely_after_hours);
    run_hours(&mut sim, after);
    assert_eq!(memory_value(&sim, all[2], "lonely"), Some(-4));
    assert_eq!(memory_value(&sim, all[0], "lonely"), None);
}

#[test]
fn r034_an_introvert_alone_is_content_and_in_a_crowd_gets_a_bad_memory() {
    let crowd: Vec<(usize, usize)> = (0..5).map(|i| (2 + i, 1)).collect();
    let mut all_tiles = crowd.clone();
    all_tiles.push((25, 8));
    let mut sim = calm(&open_map(30, 10, &all_tiles));
    let all = ids(&sim);
    for &id in &all {
        sim.debug_set_social(id, Social::Introvert);
    }
    run_hours(&mut sim, 2);
    let lone = *all.last().unwrap();
    assert_eq!(memory_value(&sim, lone, "time_alone"), Some(3));
    assert_eq!(memory_value(&sim, all[0], "time_alone"), None);
    assert_eq!(memory_value(&sim, all[0], "crowded"), None, "not yet");
    let after = u64::from(sim.rules().wants.crowd_after_hours);
    run_hours(&mut sim, after);
    assert_eq!(memory_value(&sim, all[2], "crowded"), Some(-3));
    assert_eq!(memory_value(&sim, lone, "crowded"), None);
}

#[test]
fn r034_a_neutral_colonist_has_no_social_memory() {
    let mut sim = calm(&open_map(10, 10, &[(1, 1), (2, 1)]));
    run_hours(&mut sim, 40);
    for id in ids(&sim) {
        for key in ["had_company", "lonely", "time_alone", "crowded"] {
            assert_eq!(memory_value(&sim, id, key), None);
        }
    }
}

#[test]
fn r034_a_sleeping_colonist_does_not_change_the_social_counters() {
    let mut sim = calm(&open_map(30, 10, &[(25, 8)]));
    let id = ids(&sim)[0];
    sim.debug_set_social(id, Social::Extrovert);
    // Tired: the colonist walks to sleep on the ground. Do not refill the rest.
    sim.debug_set_rest(id, 1);
    for _ in 0..(40 * HOUR) {
        sim.step(&[]);
        sim.debug_set_hunger(id, 100);
    }
    let w = sim.wants(id).unwrap();
    assert!(w.alone_hours < 40, "the sleeping hours do not count");
}

#[test]
fn r034_a_met_goal_gives_a_named_memory_at_once() {
    let mut sim = calm(&open_map(10, 10, &[(1, 1)]));
    let id = ids(&sim)[0];
    sim.set_skill_xp(id, Skill::Farming, 100_000);
    assert!(sim.debug_set_goal(id, Some("green_thumb")));
    let g = sim.wants(id).unwrap().goal.unwrap();
    assert!(g.met);
    assert_eq!(g.progress, g.target);
    run_hours(&mut sim, 2);
    assert_eq!(memory_value(&sim, id, "goal_met"), Some(4));
    assert_eq!(memory_value(&sim, id, "goal_unmet"), None);
}

#[test]
fn r034_an_unmet_goal_costs_nothing_until_the_patience_ends() {
    let mut sim = calm(&open_map(10, 10, &[(1, 1)]));
    let id = ids(&sim)[0];
    assert!(sim.debug_set_goal(id, Some("green_thumb")));
    let patience = u64::from(sim.rules().wants.goal_patience_hours);
    assert!(patience >= 24 * 10, "patience is days long");
    run_hours(&mut sim, patience - 1);
    assert_eq!(memory_value(&sim, id, "goal_unmet"), None);
    assert_eq!(
        u64::from(sim.wants(id).unwrap().goal.unwrap().unmet_hours),
        patience - 1
    );
}

#[test]
fn r034_an_unmet_goal_gives_a_bad_memory_after_the_patience() {
    let mut sim = calm(&open_map(10, 10, &[(1, 1)]));
    let id = ids(&sim)[0];
    assert!(sim.debug_set_goal(id, Some("green_thumb")));
    let patience = u64::from(sim.rules().wants.goal_patience_hours);
    run_hours(&mut sim, patience + 2);
    assert_eq!(memory_value(&sim, id, "goal_unmet"), Some(-4));
    // The goal is met later: the counter resets and the good memory comes.
    sim.set_skill_xp(id, Skill::Farming, 100_000);
    run_hours(&mut sim, 2);
    assert_eq!(sim.wants(id).unwrap().goal.unwrap().unmet_hours, 0);
    assert_eq!(memory_value(&sim, id, "goal_met"), Some(4));
}

#[test]
fn r034_the_private_room_goal_needs_a_room_with_one_bed() {
    let mut sim = calm("#####\n#...#\n#####\nc....");
    let id = ids(&sim)[0];
    let (log, roof) = (
        sim.rules().material_index("log").unwrap(),
        sim.rules().building_index("roof").unwrap(),
    );
    let bed = sim.rules().building_index("bed").unwrap();
    assert!(sim.rules().is_kind(bed, BuildingKind::Bed));
    for x in 1..=3 {
        assert!(sim.debug_build(roof, x, 1, log));
    }
    assert!(sim.debug_build(bed, 1, 1, log));
    sim.step(&[]);
    assert_eq!(sim.beds_in_room(1, 1), 1);
    assert!(sim.debug_build(bed, 2, 1, log));
    sim.step(&[]);
    assert_eq!(sim.beds_in_room(1, 1), 2);
    assert_eq!(sim.beds_in_room(0, 3), 0, "outside any room");
    assert!(sim.debug_set_goal(id, Some("own_room")));
    sim.debug_set_sleep_room(id, 2);
    assert!(!sim.wants(id).unwrap().goal.unwrap().met);
    sim.debug_set_sleep_room(id, 1);
    assert!(sim.wants(id).unwrap().goal.unwrap().met);
    sim.debug_set_sleep_room(id, 0);
    assert!(!sim.wants(id).unwrap().goal.unwrap().met);
}

#[test]
fn r034_the_mood_total_is_still_the_sum_of_the_named_parts() {
    let mut sim = calm(&open_map(10, 10, &[(1, 1), (2, 1)]));
    let all = ids(&sim);
    sim.debug_set_social(all[0], Social::Extrovert);
    sim.set_skill_xp(all[0], Skill::Farming, 100_000);
    assert!(sim.debug_set_goal(all[0], Some("green_thumb")));
    run_hours(&mut sim, 3);
    let mood = sim.mood(all[0]).unwrap();
    assert_eq!(mood.parts.iter().map(|p| p.value).sum::<i32>(), mood.sum);
    assert!(memory_value(&sim, all[0], "had_company").is_some());
    assert!(memory_value(&sim, all[0], "goal_met").is_some());
}

#[test]
fn r033_the_personality_survives_a_save_and_the_format_is_current() {
    let mut sim = calm(&open_map(10, 10, &[(1, 1)]));
    let id = ids(&sim)[0];
    sim.debug_set_social(id, Social::Introvert);
    assert!(sim.debug_set_goal(id, Some("own_room")));
    run_hours(&mut sim, 5);
    let bytes = save::to_bytes(&sim);
    assert_eq!(save::FORMAT_VERSION, 17);
    let loaded = save::from_bytes(&bytes).unwrap();
    let w = loaded.wants(ids(&loaded)[0]).unwrap();
    assert_eq!(w.social, Social::Introvert);
    assert_eq!(w.goal.unwrap().id, "own_room");
    assert_eq!(loaded.state_hash(), sim.state_hash());
}

#[test]
fn r033_arriving_migrants_get_a_social_trait_and_a_goal() {
    let mut sim = calm(&open_map(12, 12, &[(1, 1)]));
    sim.debug_offer_migrants(2);
    sim.step(&[sim::Command::AcceptMigrants]);
    assert!(ids(&sim).len() >= 3);
    for id in ids(&sim).into_iter().skip(1) {
        let w = sim.wants(id).unwrap();
        assert!(w.goal.is_some());
    }
}
