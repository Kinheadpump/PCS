//! Sprint 24 tests (sim part): people of other colonies (R-045, R-046, R-047). A colonist meets
//! the people of calm rival colonies and forms ties with them. A tie can end in a wedding, and
//! the new partner joins the colony. A raid death leaves grudges. Ties and grudges change the
//! hostility of the rival colony.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use sim::scenario::sim_from_ascii;
use sim::{
    ChronicleKind, ColonistId, HostilityCause, MoodCause, NeighborCause, NeighborEvent, Sim,
    SimRules, Standing, save,
};
use std::sync::Arc;

const HOUR: u64 = 100;
const DAY: u64 = 2400;

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

fn with_rules(sim: &mut Sim, edit: impl FnOnce(&mut SimRules)) {
    let mut rules = sim.rules().clone();
    edit(&mut rules);
    sim.debug_set_rules(Arc::new(rules));
}

fn ids(sim: &Sim) -> Vec<ColonistId> {
    sim.colonists().iter().map(|(id, _)| id).collect()
}

/// A calm map with the colonists and one rival colony with the hostility `hostility`. A contact
/// happens each day, and no raid starts by itself.
fn calm(colonists: &[(usize, usize)], hostility: u8) -> Sim {
    let mut sim = sim_from_ascii(&open_map(20, 10, colonists), 1).unwrap();
    sim.set_climate(200, 0, 128);
    with_rules(&mut sim, |r| {
        r.neighbors.contact_permille = 1000;
        r.raid.quiet_days = 10_000;
    });
    sim.debug_add_rival(8, hostility, 2);
    sim
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

fn memory_value(sim: &Sim, id: ColonistId, key: &str) -> Option<i32> {
    let index = sim.rules().needs.memory_index(key)?;
    sim.mood(id)?
        .parts
        .iter()
        .find(|p| p.cause == MoodCause::Memory { key: index })
        .map(|p| p.value)
}

fn ties_part(sim: &Sim) -> i32 {
    sim.rival_hostility(0)
        .unwrap()
        .parts
        .iter()
        .find(|p| p.cause == HostilityCause::Ties)
        .unwrap()
        .value
}

#[test]
fn r045_the_content_is_ordered_and_has_the_memory_keys() {
    let rules = SimRules::builtin();
    let n = &rules.neighbors;
    assert!(n.people_per_rival > 0 && n.contact_days_per_point > 0);
    assert!(
        n.enemy_at < n.rival_at && n.rival_at < 0 && 0 < n.friend_at && n.friend_at < n.partner_at
    );
    assert!(n.person_age_min >= n.partner_min_age && n.person_age_min <= n.person_age_max);
    for key in ["met_neighbor", "grudge_raid", "neighbor_wed"] {
        assert!(rules.needs.memory_index(key).is_some(), "{key}");
    }
}

#[test]
fn r045_the_people_of_a_rival_colony_are_adults_from_the_seed() {
    let a = calm(&[(1, 1)], 10);
    let b = calm(&[(1, 1)], 10);
    let n = a.rules().neighbors.clone();
    for slot in 0..n.people_per_rival {
        let p = a.neighbor_person(0, slot).unwrap();
        assert!((n.person_age_min..=n.person_age_max).contains(&p.age));
        assert_eq!(Some(p), b.neighbor_person(0, slot));
        assert!(!p.wed);
    }
    assert!(a.neighbor_person(0, n.people_per_rival).is_none());
    assert!(a.neighbor_person(1, 0).is_none());
}

#[test]
fn r045_a_contact_with_a_calm_rival_makes_a_tie_and_a_memory() {
    let mut sim = calm(&[(1, 1)], 10);
    let id = ids(&sim)[0];
    run_days(&mut sim, 1);
    assert_eq!(sim.neighbor_contacts(0), 1);
    let ties = sim.neighbor_ties(id);
    assert_eq!(ties.len(), 1);
    assert_eq!(ties[0].rival, 0);
    assert!(matches!(
        ties[0].parts[0].cause,
        NeighborCause::Contact { days: 1 }
    ));
    let events = sim.drain_neighbor_events();
    assert!(
        events
            .iter()
            .any(|e| matches!(e, NeighborEvent::Met { colonist, rival: 0, .. } if *colonist == id))
    );
    assert!(memory_value(&sim, id, "met_neighbor").is_some());
    run_days(&mut sim, 2);
    assert_eq!(sim.neighbor_contacts(0), 3);
}

#[test]
fn r045_a_hostile_rival_makes_no_contact() {
    let mut sim = calm(&[(1, 1)], 100);
    run_days(&mut sim, 3);
    assert_eq!(sim.neighbor_contacts(0), 0);
    assert!(sim.neighbor_ties(ids(&sim)[0]).is_empty());
}

#[test]
fn r045_the_opinion_is_the_sum_of_named_parts() {
    let mut sim = calm(&[(1, 1)], 10);
    let id = ids(&sim)[0];
    let n = sim.rules().neighbors.clone();
    sim.debug_set_neighbor_tie(id, 0, 0, 40, 5);
    let tie = &sim.neighbor_ties(id)[0];
    let contact = i32::try_from(40 / n.contact_days_per_point)
        .unwrap()
        .min(n.contact_max);
    assert_eq!(tie.parts.len(), 3);
    assert_eq!(tie.parts[0].value, contact);
    assert_eq!(tie.parts[1].cause, NeighborCause::Chemistry);
    assert_eq!(tie.parts[1].value, 5);
    assert_eq!(tie.parts[2].cause, NeighborCause::Grudge);
    assert_eq!(tie.parts[2].value, 0);
    assert_eq!(tie.opinion, contact + 5);
    assert_eq!(tie.standing, Standing::Friend);
}

#[test]
fn r045_friends_and_grudges_move_the_hostility_of_the_rival() {
    let mut sim = calm(&[(1, 1)], 10);
    let id = ids(&sim)[0];
    let n = sim.rules().neighbors.clone();
    assert_eq!(ties_part(&sim), 0);
    sim.debug_set_neighbor_tie(id, 0, 0, 40, 5);
    assert_eq!(ties_part(&sim), -i32::try_from(n.friend_hostility).unwrap());
    sim.debug_set_neighbor_grudge(id, 0, 40);
    let grudge_part = i32::try_from(40 / n.grudge_hostility_div).unwrap();
    assert!(grudge_part > 0);
    // The grudge also lowers the opinion, so the tie may stop being a friend.
    let friends = i32::from(sim.neighbor_ties(id)[0].standing == Standing::Friend);
    assert_eq!(
        ties_part(&sim),
        grudge_part - friends * i32::try_from(n.friend_hostility).unwrap()
    );
}

#[test]
fn r046_a_tie_with_a_high_opinion_ends_in_a_wedding() {
    let mut sim = calm(&[(1, 1)], 10);
    let id = ids(&sim)[0];
    sim.debug_set_neighbor_tie(id, 0, 1, 500, 10);
    sim.drain_neighbor_events();
    run_days(&mut sim, 1);
    assert_eq!(sim.colonists().len(), 2);
    let partner = sim.partner_of(id).unwrap();
    let person = sim.neighbor_person(0, 1).unwrap();
    assert!(person.wed);
    let c = sim.colonist(partner).unwrap();
    assert_eq!((c.age, c.sex), (person.age, person.sex));
    assert_eq!(
        sim.relation(id, partner).unwrap().standing,
        Standing::Partner
    );
    assert_eq!(sim.neighbor_weds(0), 1);
    assert!(
        sim.drain_neighbor_events()
            .iter()
            .any(|e| matches!(e, NeighborEvent::Wed { rival: 0, .. }))
    );
    assert!(
        sim.chronicle()
            .iter()
            .any(|e| e.kind == ChronicleKind::Wedding)
    );
    assert!(memory_value(&sim, id, "neighbor_wed").is_some());
    assert!(memory_value(&sim, partner, "neighbor_wed").is_some());
    assert!(ties_part(&sim) < 0);
    // The tie with the wed person ends. Only one person weds.
    assert!(sim.neighbor_ties(id).iter().all(|t| t.slot != 1));
}

#[test]
fn r046_a_colonist_with_a_partner_does_not_wed() {
    let mut sim = calm(&[(1, 1), (2, 1)], 10);
    let all = ids(&sim);
    run_hours(&mut sim, 1);
    sim.debug_set_chemistry(all[0], all[1], 10);
    sim.debug_set_relation_hours(all[0], all[1], 10_000, 10_000, 0);
    run_hours(&mut sim, 2);
    assert!(sim.partner_of(all[0]).is_some());
    sim.debug_set_neighbor_tie(all[0], 0, 1, 500, 10);
    run_days(&mut sim, 1);
    assert_eq!(sim.colonists().len(), 2);
    assert_eq!(sim.neighbor_weds(0), 0);
}

#[test]
fn r046_a_hostile_rival_does_not_wed() {
    let mut sim = calm(&[(1, 1)], 100);
    let id = ids(&sim)[0];
    sim.debug_set_neighbor_tie(id, 0, 1, 500, 10);
    run_days(&mut sim, 2);
    assert_eq!(sim.colonists().len(), 1);
}

#[test]
fn r047_a_raid_death_leaves_grudges_with_the_partner_and_friends() {
    let mut sim = calm(&[(1, 1), (2, 1)], 10);
    let all = ids(&sim);
    run_hours(&mut sim, 1);
    sim.debug_set_chemistry(all[0], all[1], 10);
    sim.debug_set_relation_hours(all[0], all[1], 10_000, 10_000, 0);
    run_hours(&mut sim, 2);
    assert!(sim.partner_of(all[0]).is_some());
    sim.debug_set_neighbor_tie(all[0], 0, 0, 20, 0);
    assert!(sim.debug_start_raid(0));
    sim.drain_neighbor_events();
    sim.debug_kill_wounded(all[1]);
    let amount = sim.rules().neighbors.grudge_partner;
    assert_eq!(sim.neighbor_grudge(all[0], 0), amount);
    assert!(
        sim.drain_neighbor_events().iter().any(|e| matches!(
            e,
            NeighborEvent::Grudge { colonist, rival: 0, amount: a } if *colonist == all[0] && *a == amount
        ))
    );
    assert!(memory_value(&sim, all[0], "grudge_raid").is_some());
    let tie = &sim.neighbor_ties(all[0])[0];
    assert_eq!(tie.parts[2].value, -i32::try_from(amount).unwrap());
}

#[test]
fn r047_a_death_outside_a_raid_leaves_no_grudge() {
    let mut sim = calm(&[(1, 1), (2, 1)], 10);
    let all = ids(&sim);
    sim.debug_kill_wounded(all[1]);
    assert_eq!(sim.neighbor_grudge(all[0], 0), 0);
}

#[test]
fn r047_a_grudge_fades_over_the_days() {
    let mut sim = calm(&[(1, 1)], 100);
    let id = ids(&sim)[0];
    let decay = u64::from(sim.rules().neighbors.grudge_decay_days);
    sim.debug_set_neighbor_grudge(id, 0, 10);
    run_days(&mut sim, decay);
    assert_eq!(sim.neighbor_grudge(id, 0), 9);
    run_days(&mut sim, decay);
    assert_eq!(sim.neighbor_grudge(id, 0), 8);
}

#[test]
fn r047_a_gone_colonist_leaves_no_ties_or_grudges() {
    let mut sim = calm(&[(1, 1), (2, 1)], 10);
    let all = ids(&sim);
    sim.debug_set_neighbor_tie(all[1], 0, 0, 20, 0);
    sim.debug_set_neighbor_grudge(all[1], 0, 10);
    sim.debug_kill(all[1]);
    assert!(sim.neighbor_ties(all[1]).is_empty());
    assert_eq!(sim.neighbor_grudge(all[1], 0), 0);
}

#[test]
fn r045_ties_survive_a_save_and_the_run_is_repeatable() {
    let make = || {
        let mut sim = calm(&[(1, 1)], 10);
        run_days(&mut sim, 4);
        sim
    };
    let a = make();
    let b = make();
    assert_eq!(a.state_hash(), b.state_hash());
    let loaded = save::from_bytes(&save::to_bytes(&a)).unwrap();
    assert_eq!(loaded.state_hash(), a.state_hash());
    assert_eq!(loaded.neighbor_contacts(0), a.neighbor_contacts(0));
    assert_eq!(save::FORMAT_VERSION, 16);
    let _ = DAY;
}
