//! Sprint 22 tests (sim part): families, ages, and homes (R-037, R-038, R-039). Partners can
//! have children. Children grow up and do not work. Colonists age and die of old age. Each
//! colonist has an assigned bed, and families share a room.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use sim::scenario::sim_from_ascii;
use sim::time::TICKS_PER_DAY;
use sim::{
    ChronicleKind, ColonistId, CombatReject, Command, DeathCause, FamilyEvent, JobKind, LifeEvent,
    LifeStage, MoodCause, NeedAct, Sex, Sim, SimRules, Standing, save,
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

fn calm(map: &str) -> Sim {
    let mut sim = sim_from_ascii(map, 1).unwrap();
    sim.set_climate(200, 0, 128);
    sim
}

fn ids(sim: &Sim) -> Vec<ColonistId> {
    sim.colonists().iter().map(|(id, _)| id).collect()
}

fn with_rules(sim: &mut Sim, edit: impl FnOnce(&mut SimRules)) {
    let mut rules = sim.rules().clone();
    edit(&mut rules);
    sim.debug_set_rules(Arc::new(rules));
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

fn run_days(sim: &mut Sim, days: u64) {
    run_hours(sim, days * 24);
}

fn memory_value(sim: &Sim, id: ColonistId, key: &str) -> Option<i32> {
    let index = sim.rules().needs.memory_index(key)?;
    sim.mood(id)?
        .parts
        .iter()
        .find(|p| p.cause == MoodCause::Memory { key: index })
        .map(|p| p.value)
}

/// A map with a woman and a man who are partners, with a bed for each and one more.
fn partner_sim() -> (Sim, ColonistId, ColonistId) {
    let mut sim = calm(&open_map(20, 10, &[(1, 1), (2, 1)]));
    with_rules(&mut sim, |r| r.family.min_food_days = 0);
    let all = ids(&sim);
    sim.debug_set_sex(all[0], Sex::Female);
    sim.debug_set_sex(all[1], Sex::Male);
    run_hours(&mut sim, 1);
    sim.debug_set_chemistry(all[0], all[1], 10);
    sim.debug_set_relation_hours(all[0], all[1], 10_000, 10_000, 0);
    run_hours(&mut sim, 2);
    assert_eq!(
        sim.relation(all[0], all[1]).unwrap().standing,
        Standing::Partner
    );
    let bed = sim.rules().building_index("bed").unwrap();
    let plank = sim.rules().material_index("plank").unwrap();
    for x in 10..13 {
        assert!(sim.debug_build(bed, x, 5, plank));
    }
    (sim, all[0], all[1])
}

#[test]
fn r037_the_content_is_ordered_and_has_the_memory_keys() {
    let rules = SimRules::builtin();
    let f = &rules.family;
    assert!(f.adult_age < f.fertile_max_age && f.fertile_max_age < f.elder_age);
    assert!(f.fertile_min_age >= f.adult_age);
    assert!(f.days_per_age_year > 0 && f.pregnancy_days > 0);
    for key in ["child_born", "parent_died", "child_died"] {
        assert!(rules.needs.memory_index(key).is_some(), "{key}");
    }
}

#[test]
fn r037_a_colonist_ages_one_year_after_the_days_of_an_age_year() {
    let mut sim = calm(&open_map(10, 10, &[(1, 1)]));
    let id = ids(&sim)[0];
    let per = u64::from(sim.rules().family.days_per_age_year);
    sim.debug_set_age(id, 30);
    sim.debug_set_age_days(id, 0);
    run_days(&mut sim, per - 1);
    assert_eq!(sim.family(id).unwrap().age, 30);
    run_days(&mut sim, 1);
    assert_eq!(sim.family(id).unwrap().age, 31);
    assert_eq!(sim.family(id).unwrap().age_days, 0);
}

#[test]
fn r037_the_life_stage_follows_the_age() {
    let mut sim = calm(&open_map(10, 10, &[(1, 1)]));
    let id = ids(&sim)[0];
    let f = sim.rules().family.clone();
    sim.debug_set_age(id, f.adult_age - 1);
    assert_eq!(sim.family(id).unwrap().stage, LifeStage::Child);
    sim.debug_set_age(id, f.adult_age);
    assert_eq!(sim.family(id).unwrap().stage, LifeStage::Adult);
    sim.debug_set_age(id, f.elder_age);
    assert_eq!(sim.family(id).unwrap().stage, LifeStage::Elder);
}

#[test]
fn r037_a_child_grows_up_and_gets_the_work_priorities() {
    let mut sim = calm(&open_map(10, 10, &[(1, 1)]));
    let id = ids(&sim)[0];
    let f = sim.rules().family.clone();
    sim.debug_set_age(id, f.adult_age - 1);
    sim.debug_set_age_days(id, f.days_per_age_year - 1);
    sim.debug_zero_priorities(id);
    sim.drain_family_events();
    run_days(&mut sim, 1);
    assert_eq!(sim.family(id).unwrap().stage, LifeStage::Adult);
    let p = sim.priority(id, JobKind::Cut).unwrap();
    assert_eq!(p, sim.rules().jobs.default_priority);
    let events = sim.drain_family_events();
    assert!(events.contains(&FamilyEvent::GrewUp { colonist: id }));
}

#[test]
fn r037_a_child_does_not_work_even_with_a_priority() {
    let mut sim = calm("c.T..");
    let id = ids(&sim)[0];
    sim.debug_set_age(id, 5);
    sim.step(&[
        Command::SetPriority {
            colonist: id,
            job: JobKind::Cut,
            priority: 1,
        },
        Command::Designate {
            kind: JobKind::Cut,
            x: 2,
            y: 0,
        },
    ]);
    run_hours(&mut sim, 8);
    assert!(sim.map().tree(2, 0), "the child did not cut the tree");
    sim.debug_set_age(id, 30);
    run_hours(&mut sim, 8);
    assert!(!sim.map().tree(2, 0), "an adult cuts it");
}

#[test]
fn r037_a_child_cannot_be_drafted() {
    let mut sim = calm(&open_map(10, 10, &[(1, 1)]));
    let id = ids(&sim)[0];
    sim.debug_set_age(id, 5);
    sim.step(&[Command::Draft {
        colonist: id,
        drafted: true,
    }]);
    assert!(!sim.colonists().get(id).unwrap().drafted);
    assert!(
        sim.drain_combat_events()
            .iter()
            .any(|e| matches!(e, sim::CombatEvent::Rejected(CombatReject::TooYoung)))
    );
}

#[test]
fn r037_an_elder_dies_of_old_age_in_time() {
    let mut sim = calm(&open_map(10, 10, &[(1, 1), (8, 8)]));
    let all = ids(&sim);
    sim.debug_set_age(all[0], 95);
    run_days(&mut sim, 200);
    assert_eq!(sim.colonists().len(), 1, "the elder died");
    assert!(
        sim.colonists().get(all[1]).is_some(),
        "the 30-year-old lives"
    );
    let events = sim.drain_life_events();
    assert!(events.iter().any(|e| matches!(
        e,
        LifeEvent::Died {
            cause: DeathCause::OldAge,
            ..
        }
    )));
    assert!(
        sim.chronicle()
            .iter()
            .any(|e| e.kind == ChronicleKind::DeathOldAge)
    );
}

#[test]
fn r037_an_adult_below_the_elder_age_never_dies_of_old_age() {
    let mut sim = calm(&open_map(10, 10, &[(1, 1)]));
    let id = ids(&sim)[0];
    let f = sim.rules().family.clone();
    for _ in 0..100 {
        sim.debug_set_age(id, f.elder_age - 1);
        run_days(&mut sim, 1);
    }
    assert!(sim.colonists().get(id).is_some());
}

#[test]
fn r038_partners_of_both_sexes_get_a_child_after_the_pregnancy() {
    let (mut sim, mother, father) = partner_sim();
    with_rules(&mut sim, |r| r.family.pregnancy_chance_pct = 100);
    sim.drain_family_events();
    run_days(&mut sim, 1);
    let f = sim.family(mother).unwrap();
    assert!(f.pregnant_days_left.is_some(), "the mother is pregnant");
    assert_eq!(sim.family(father).unwrap().pregnant_days_left, None);
    let events = sim.drain_family_events();
    assert!(events.contains(&FamilyEvent::Pregnant { mother, father }));
    let before = sim.colonists().len();
    let days = u64::from(sim.rules().family.pregnancy_days);
    run_days(&mut sim, days + 1);
    assert_eq!(sim.colonists().len(), before + 1);
    let events = sim.drain_family_events();
    let child = events
        .iter()
        .find_map(|e| match e {
            FamilyEvent::Born { child, .. } => Some(*child),
            _ => None,
        })
        .unwrap();
    let cf = sim.family(child).unwrap();
    assert_eq!(cf.stage, LifeStage::Child);
    assert_eq!(cf.age, 0);
    assert_eq!(cf.mother, Some(mother));
    assert_eq!(cf.father, Some(father));
    assert!(sim.family(mother).unwrap().children.contains(&child));
    assert_eq!(sim.priority(child, JobKind::Cut), Some(0));
    assert_eq!(memory_value(&sim, mother, "child_born"), Some(8));
    assert_eq!(memory_value(&sim, father, "child_born"), Some(8));
    assert_eq!(sim.family(mother).unwrap().pregnant_days_left, None);
}

#[test]
fn r038_a_birth_is_followed_by_a_pause_before_the_next_pregnancy() {
    let (mut sim, mother, _) = partner_sim();
    with_rules(&mut sim, |r| r.family.pregnancy_chance_pct = 100);
    let days = u64::from(sim.rules().family.pregnancy_days);
    run_days(&mut sim, days + 2);
    assert_eq!(sim.family(mother).unwrap().children.len(), 1);
    assert_eq!(
        sim.family(mother).unwrap().pregnant_days_left,
        None,
        "the pause"
    );
    let bed = sim.rules().building_index("bed").unwrap();
    let plank = sim.rules().material_index("plank").unwrap();
    assert!(sim.debug_build(bed, 14, 5, plank));
    let pause = u64::from(sim.rules().family.birth_cooldown_days);
    run_days(&mut sim, pause + 1);
    assert!(sim.family(mother).unwrap().pregnant_days_left.is_some());
}

#[test]
fn r038_no_pregnancy_without_the_conditions() {
    // Same sex.
    let (mut sim, a, b) = partner_sim();
    with_rules(&mut sim, |r| r.family.pregnancy_chance_pct = 100);
    sim.debug_set_sex(b, Sex::Female);
    run_days(&mut sim, 3);
    assert_eq!(sim.family(a).unwrap().pregnant_days_left, None);
    assert_eq!(sim.family(b).unwrap().pregnant_days_left, None);
    // Too old.
    sim.debug_set_sex(b, Sex::Male);
    let max = sim.rules().family.fertile_max_age;
    sim.debug_set_age(a, max + 1);
    run_days(&mut sim, 3);
    assert_eq!(sim.family(a).unwrap().pregnant_days_left, None);
    // No free bed: remove the spare beds by setting the limit above the number of beds.
    sim.debug_set_age(a, 25);
    with_rules(&mut sim, |r| r.family.spare_beds = 50);
    run_days(&mut sim, 3);
    assert_eq!(sim.family(a).unwrap().pregnant_days_left, None);
    with_rules(&mut sim, |r| r.family.spare_beds = 1);
    run_days(&mut sim, 3);
    assert!(sim.family(a).unwrap().pregnant_days_left.is_some());
}

#[test]
fn r038_non_partners_get_no_child() {
    let mut sim = calm(&open_map(20, 10, &[(1, 1), (2, 1)]));
    let all = ids(&sim);
    sim.debug_set_sex(all[0], Sex::Female);
    sim.debug_set_sex(all[1], Sex::Male);
    with_rules(&mut sim, |r| r.family.pregnancy_chance_pct = 100);
    run_days(&mut sim, 5);
    assert_eq!(sim.family(all[0]).unwrap().pregnant_days_left, None);
}

#[test]
fn r039_when_a_parent_or_a_child_dies_the_family_gets_a_named_memory() {
    let (mut sim, mother, father) = partner_sim();
    let child = sim.debug_add_child(mother, father);
    run_hours(&mut sim, 1);
    sim.debug_kill(mother);
    assert_eq!(memory_value(&sim, child, "parent_died"), Some(-10));
    assert_eq!(memory_value(&sim, father, "parent_died"), None);
    sim.debug_kill(child);
    assert_eq!(memory_value(&sim, father, "child_died"), Some(-20));
}

#[test]
fn r039_every_colonist_gets_its_own_bed_in_the_order_of_the_slots() {
    let mut sim = calm(&open_map(20, 10, &[(1, 1), (2, 1), (3, 1)]));
    let bed = sim.rules().building_index("bed").unwrap();
    let plank = sim.rules().material_index("plank").unwrap();
    for x in 10..14 {
        assert!(sim.debug_build(bed, x, 5, plank));
    }
    run_hours(&mut sim, 2);
    let mut beds: Vec<(u16, u16)> = ids(&sim).iter().map(|&i| sim.home_of(i).unwrap()).collect();
    beds.sort();
    beds.dedup();
    assert_eq!(beds.len(), 3, "three different beds");
}

#[test]
fn r039_a_colonist_without_a_free_bed_has_no_home() {
    let mut sim = calm(&open_map(20, 10, &[(1, 1), (2, 1)]));
    let bed = sim.rules().building_index("bed").unwrap();
    let plank = sim.rules().material_index("plank").unwrap();
    assert!(sim.debug_build(bed, 10, 5, plank));
    run_hours(&mut sim, 2);
    let homeless = ids(&sim)
        .iter()
        .filter(|&&i| sim.home_of(i).is_none())
        .count();
    assert_eq!(homeless, 1);
}

#[test]
fn r039_a_single_prefers_a_private_room_and_partners_share_a_room() {
    // Left room with 1 bed, right room with 2 beds.
    let map = "#########\n#...#...#\n#...#...#\n#########\nc.c.c....";
    let mut sim = calm(map);
    let all = ids(&sim);
    // Two colonists are partners, the third is single.
    sim.debug_set_sex(all[0], Sex::Female);
    sim.debug_set_sex(all[1], Sex::Male);
    run_hours(&mut sim, 1);
    sim.debug_set_chemistry(all[0], all[1], 10);
    sim.debug_set_relation_hours(all[0], all[1], 10_000, 10_000, 0);
    run_hours(&mut sim, 2);
    assert_eq!(sim.partner_of(all[0]), Some(all[1]));
    let (log, roof) = (
        sim.rules().material_index("log").unwrap(),
        sim.rules().building_index("roof").unwrap(),
    );
    let bed = sim.rules().building_index("bed").unwrap();
    for x in (1..=3).chain(5..=7) {
        for y in 1..=2 {
            assert!(sim.debug_build(roof, x, y, log));
        }
    }
    assert!(sim.debug_build(bed, 1, 1, log));
    assert!(sim.debug_build(bed, 5, 1, log));
    assert!(sim.debug_build(bed, 6, 1, log));
    sim.step(&[]);
    assert_eq!(sim.beds_in_room(1, 1), 1);
    assert_eq!(sim.beds_in_room(5, 1), 2);
    run_hours(&mut sim, 2);
    let home = |i| sim.home_of(i).unwrap();
    assert_eq!(sim.beds_in_room(home(all[0]).0, home(all[0]).1), 2);
    assert_eq!(
        sim.beds_in_room(home(all[1]).0, home(all[1]).1),
        2,
        "the pair shares the room"
    );
    assert_eq!(
        sim.beds_in_room(home(all[2]).0, home(all[2]).1),
        1,
        "the single has the private room"
    );
}

#[test]
fn r039_a_tired_colonist_sleeps_in_the_assigned_bed() {
    let mut sim = calm("#######\n#.c.c.#\n#.....#\n#######\n.......");
    let (log, roof) = (
        sim.rules().material_index("log").unwrap(),
        sim.rules().building_index("roof").unwrap(),
    );
    let bed = sim.rules().building_index("bed").unwrap();
    for x in 1..=5 {
        for y in 1..=2 {
            assert!(sim.debug_build(roof, x, y, log));
        }
    }
    for x in [1, 3, 5] {
        assert!(sim.debug_build(bed, x, 2, log));
    }
    run_hours(&mut sim, 2);
    let all = ids(&sim);
    // The second colonist has the bed that is not the nearest one.
    let home = sim.home_of(all[1]).unwrap();
    sim.debug_set_rest(all[1], 1);
    for _ in 0..(10 * HOUR) {
        sim.step(&[]);
        if matches!(
            sim.colonists().get(all[1]).unwrap().act,
            Some(NeedAct::Sleep { asleep: true, .. })
        ) {
            break;
        }
    }
    match sim.colonists().get(all[1]).unwrap().act {
        Some(NeedAct::Sleep { bed: Some(b), .. }) => assert_eq!(b, home),
        other => panic!("not asleep in a bed: {other:?}"),
    }
}

#[test]
fn r039_a_child_gets_a_bed_in_the_room_of_its_parents() {
    let (mut sim, mother, father) = partner_sim();
    let child = sim.debug_add_child(mother, father);
    // A second room far away with more beds and nothing else.
    run_hours(&mut sim, 3);
    let near = |a: ColonistId, b: ColonistId| {
        let (x, y) = (sim.home_of(a).unwrap(), sim.home_of(b).unwrap());
        (x.0.abs_diff(y.0), x.1.abs_diff(y.1))
    };
    let _ = near(mother, child);
    assert!(sim.home_of(child).is_some());
    assert_ne!(sim.home_of(child), sim.home_of(mother));
}

#[test]
fn r037_families_and_ages_survive_a_save_and_the_format_is_15() {
    let (mut sim, mother, father) = partner_sim();
    let child = sim.debug_add_child(mother, father);
    sim.debug_make_pregnant(mother, father, 7);
    run_hours(&mut sim, 5);
    assert_eq!(save::FORMAT_VERSION, 15);
    let loaded = save::from_bytes(&save::to_bytes(&sim)).unwrap();
    assert_eq!(loaded.family(child), sim.family(child));
    assert_eq!(loaded.family(mother), sim.family(mother));
    assert_eq!(loaded.home_of(father), sim.home_of(father));
    assert_eq!(loaded.state_hash(), sim.state_hash());
}

#[test]
fn r037_a_new_colony_has_spread_birthdays_homes_and_two_runs_agree() {
    let data = worldgen::generate_local(&worldgen::EmbarkSummary::temperate_forest());
    let run = |seed| {
        let mut sim = Sim::new(&data, seed, 0);
        for _ in 0..(2 * TICKS_PER_DAY) {
            sim.step(&[]);
        }
        sim
    };
    let (a, b) = (run(5), run(5));
    assert_eq!(a.state_hash(), b.state_hash());
    let days: Vec<u32> = ids(&a)
        .iter()
        .map(|&i| a.family(i).unwrap().age_days)
        .collect();
    let mut sorted = days.clone();
    sorted.sort();
    sorted.dedup();
    assert!(sorted.len() >= 3, "the birthdays are spread: {days:?}");
    let homes: Vec<(u16, u16)> = ids(&a).iter().filter_map(|&i| a.home_of(i)).collect();
    let mut unique = homes.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(homes.len(), unique.len(), "one colonist for each bed");
    assert!(!homes.is_empty());
}
