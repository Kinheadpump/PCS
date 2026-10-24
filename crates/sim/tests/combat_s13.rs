//! Sprint 13 tests (sim part): creatures, beasts, lairs, threats, melee combat, drafting,
//! hunting, and raiders (R-031, R-032, R-071).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use base::content::MaterialCategory;
use sim::scenario::sim_from_ascii;
use sim::time::TICKS_PER_DAY;
use sim::{
    ColonistId, CombatEvent, CombatReject, Command, Mode, RaidEvent, RaidOutcome, Sim, SimRules,
    Skill, melee_damage, save,
};
use std::sync::Arc;

/// An open map of `w` x `h` tiles with a colonist on each of the given tiles.
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

fn quiet(w: usize, h: usize, colonists: &[(usize, usize)]) -> Sim {
    let mut sim = sim_from_ascii(&open_map(w, h, colonists), 1).unwrap();
    sim.debug_set_needs_enabled(false);
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

fn first(sim: &Sim) -> ColonistId {
    sim.colonists().iter().next().unwrap().0
}

fn count_kind(sim: &Sim, key: &str) -> usize {
    let k = sim.rules().creature.kind_index(key).unwrap();
    sim.creatures().iter().filter(|(_, c)| c.kind == k).count()
}

fn material(sim: &Sim, key: &str) -> u16 {
    sim.rules().material_index(key).unwrap()
}

fn units_of(sim: &Sim, key: &str) -> u64 {
    sim.items().total_units(material(sim, key))
}

fn is_edge(sim: &Sim, x: u16, y: u16) -> bool {
    let (w, h) = (sim.map().width() as u16, sim.map().height() as u16);
    x == 0 || y == 0 || x == w - 1 || y == h - 1
}

// ---- R-032: damage, wounds, drafting ----

#[test]
fn r032_damage_is_base_times_combat_level_plus_a_roll_of_0_to_20_percent() {
    assert_eq!(melee_damage(10, 0, 10, 0), 10);
    assert_eq!(melee_damage(10, 3, 10, 0), 13);
    assert_eq!(melee_damage(10, 3, 10, 20), 15);
    assert_eq!(melee_damage(6, 0, 10, 20), 7);
    assert_eq!(melee_damage(10, 10, 10, 0), 20);
    let mut sim = quiet(8, 4, &[(0, 0)]);
    let id = first(&sim);
    assert_eq!(sim.colonist_damage_range(id), Some((6, 7)));
    sim.set_skill_xp(id, Skill::Combat, 3000);
    assert_eq!(sim.colonist_damage_range(id), Some((7, 8)));
}

#[test]
fn r032_a_hit_of_10_or_more_damage_adds_the_condition_wounded() {
    let mut sim = quiet(8, 4, &[(0, 0)]);
    let id = first(&sim);
    let full = sim.colonist(id).unwrap().health;
    sim.debug_hurt_colonist(id, 9);
    let c = sim.colonist(id).unwrap();
    assert!(!c.wounded, "9 damage is no wound");
    assert_eq!(c.health, full - 9, "damage decreases health");
    sim.debug_hurt_colonist(id, 10);
    let c = sim.colonist(id).unwrap();
    assert!(c.wounded, "10 damage is a wound");
    assert_eq!(c.health, full - 19);
}

#[test]
fn r032_a_colonist_at_0_health_dies_of_wounds() {
    let mut sim = quiet(8, 4, &[(0, 0), (1, 0)]);
    let id = first(&sim);
    sim.debug_hurt_colonist(id, 1000);
    assert!(sim.colonist(id).is_none());
    assert_eq!(sim.graveyard().len(), 1);
    assert_eq!(sim.graveyard()[0].cause, sim::DeathCause::Wounds);
}

#[test]
fn r032_a_drafted_colonist_obeys_a_move_order_in_the_same_tick() {
    let mut sim = quiet(20, 4, &[(0, 0)]);
    let id = first(&sim);
    sim.step(&[
        Command::Draft {
            colonist: id,
            drafted: true,
        },
        Command::DraftMove {
            colonist: id,
            x: 10,
            y: 0,
        },
    ]);
    let c = sim.colonist(id).unwrap();
    assert!(c.drafted);
    assert_eq!(c.target, Some((10, 0)));
    assert!(!c.path.is_empty(), "the path is there in the same tick");
    assert!(c.progress > 0, "the colonist already walks");
    run(&mut sim, 200);
    assert_eq!(sim.colonist(id).unwrap().tile(), (10, 0));
}

#[test]
fn r032_a_move_order_needs_a_drafted_colonist_and_a_reachable_tile() {
    let mut sim = quiet(20, 4, &[(0, 0)]);
    let id = first(&sim);
    sim.step(&[Command::DraftMove {
        colonist: id,
        x: 5,
        y: 0,
    }]);
    assert!(
        sim.drain_combat_events()
            .contains(&CombatEvent::Rejected(CombatReject::NotDrafted))
    );
    sim.step(&[Command::Draft {
        colonist: id,
        drafted: true,
    }]);
    sim.set_wall(3, 0, true);
    sim.set_wall(3, 1, true);
    sim.set_wall(3, 2, true);
    sim.set_wall(3, 3, true);
    sim.step(&[Command::DraftMove {
        colonist: id,
        x: 5,
        y: 0,
    }]);
    assert!(
        sim.drain_combat_events()
            .contains(&CombatEvent::Rejected(CombatReject::NoPath))
    );
}

#[test]
fn r032_drafting_stops_the_job_and_the_colonist_takes_no_new_job() {
    let mut sim = sim_from_ascii("c....T..", 1).unwrap();
    sim.debug_set_needs_enabled(false);
    let id = first(&sim);
    sim.step(&[Command::Designate {
        kind: sim::JobKind::Cut,
        x: 5,
        y: 0,
    }]);
    run(&mut sim, 20);
    assert!(
        sim.colonist(id).unwrap().job.is_some(),
        "the colonist works"
    );
    sim.step(&[Command::Draft {
        colonist: id,
        drafted: true,
    }]);
    assert!(sim.colonist(id).unwrap().job.is_none());
    run(&mut sim, 400);
    assert!(
        sim.colonist(id).unwrap().job.is_none(),
        "a drafted colonist takes no job"
    );
    sim.step(&[Command::Draft {
        colonist: id,
        drafted: false,
    }]);
    run(&mut sim, 20);
    assert!(
        sim.colonist(id).unwrap().job.is_some(),
        "the colonist works again"
    );
}

#[test]
fn r032_a_drafted_colonist_does_not_start_a_need_action() {
    let mut sim = sim_from_ascii("c.......", 1).unwrap();
    let id = first(&sim);
    sim.step(&[Command::Draft {
        colonist: id,
        drafted: true,
    }]);
    sim.debug_set_rest(id, 0);
    run(&mut sim, 50);
    assert!(sim.colonist(id).unwrap().act.is_none());
}

#[test]
fn r032_a_drafted_colonist_attacks_a_wolf_and_kills_it() {
    let mut sim = quiet(30, 6, &[(0, 2)]);
    let id = first(&sim);
    let wolf = sim.debug_spawn_creature("wolf", 6, 2).unwrap();
    sim.step(&[
        Command::Draft {
            colonist: id,
            drafted: true,
        },
        Command::Attack {
            colonist: id,
            creature: wolf,
        },
    ]);
    run(&mut sim, 1500);
    assert!(sim.creature(wolf).is_none(), "the wolf is dead");
    let events = sim.drain_combat_events();
    assert!(events.iter().any(|e| matches!(
        e,
        CombatEvent::CreatureDied { by: Some(b), .. } if *b == id
    )));
    assert!(sim.colonist(id).is_some(), "the colonist lives");
    assert!(
        sim.colonist(id).unwrap().drafted,
        "the colonist stays drafted"
    );
    assert!(sim.colonist(id).unwrap().foe.is_none());
    assert!(sim.colonist(id).unwrap().skill_xp[Skill::Combat.index()] > 0);
}

#[test]
fn s13_a_dead_animal_leaves_meat_and_hide_that_colonists_haul() {
    let mut sim = quiet(10, 4, &[(0, 0)]);
    let deer = sim.debug_spawn_creature("deer", 4, 1).unwrap();
    sim.debug_hurt_creature(deer, 1000, None);
    assert!(sim.creature(deer).is_none());
    assert_eq!(units_of(&sim, "meat"), 5);
    assert_eq!(units_of(&sim, "hide"), 2);
    let all_to_haul = sim.items().iter().all(|(_, i)| i.to_haul);
    assert!(all_to_haul, "the drops wait for a hauling job");
}

#[test]
fn r032_a_colonist_that_is_not_drafted_strikes_back_at_a_creature_next_to_it() {
    let mut sim = quiet(10, 4, &[(0, 0)]);
    let id = first(&sim);
    let wolf = sim.debug_spawn_creature("wolf", 1, 0).unwrap();
    sim.debug_drive_creature(wolf);
    run(&mut sim, 900);
    assert!(sim.creature(wolf).is_none(), "the colonist fought back");
    assert!(sim.colonist(id).is_some());
}

#[test]
fn s13_hunting_gives_meat_and_hide_and_the_hunter_goes_back_to_work() {
    let mut sim = quiet(30, 6, &[(0, 2)]);
    let id = first(&sim);
    let deer = sim.debug_spawn_creature("deer", 8, 2).unwrap();
    sim.step(&[Command::Attack {
        colonist: id,
        creature: deer,
    }]);
    assert!(sim.colonist_under_orders(id));
    assert!(
        !sim.colonist(id).unwrap().drafted,
        "a hunter is not drafted"
    );
    run(&mut sim, 2500);
    assert!(sim.creature(deer).is_none(), "the deer is dead");
    let c = sim.colonist(id).unwrap();
    assert!(c.foe.is_none());
    assert!(!sim.colonist_under_orders(id), "the hunter is free again");
    assert!(
        c.skill_xp[Skill::Hunting.index()] > 0,
        "hunting gives experience"
    );
    assert_eq!(units_of(&sim, "meat"), 5);
    assert_eq!(sim.creature_kills(), (0, 1));
}

#[test]
fn s13_a_deer_flees_when_hurt_and_a_boar_fights_back() {
    let mut sim = quiet(30, 10, &[(0, 5)]);
    let deer = sim.debug_spawn_creature("deer", 10, 5).unwrap();
    let boar = sim.debug_spawn_creature("boar", 20, 5).unwrap();
    sim.debug_hurt_creature(deer, 1, Some(first(&sim)));
    sim.debug_hurt_creature(boar, 1, Some(first(&sim)));
    assert_eq!(sim.creature(deer).unwrap().mode, Mode::Flee);
    assert_eq!(sim.creature(boar).unwrap().mode, Mode::Chase);
}

#[test]
fn s13_game_animals_wander_inside_their_radius_and_do_not_attack() {
    let mut sim = quiet(40, 20, &[(5, 10)]);
    let id = first(&sim);
    let deer = sim.debug_spawn_creature("deer", 20, 10).unwrap();
    run(&mut sim, 3000);
    let c = sim.creature(deer).unwrap();
    let r = sim.rules().creature.wander_radius;
    assert!(u32::from(c.x.abs_diff(20)).max(u32::from(c.y.abs_diff(10))) <= r);
    assert_ne!((c.x, c.y), (20, 10), "the deer moved");
    assert_eq!(sim.colonist(id).unwrap().health, 100);
}

// ---- R-031: beasts, lairs, threats ----

fn loud_notice(sim: &mut Sim, quiet_days: u32) {
    with_rules(sim, |r| {
        r.notice.beast_div = 1;
        r.notice.beast_max_pct = 100;
        r.notice.per_colonist = 200;
        r.creature.beast_quiet_days = quiet_days;
    });
}

#[test]
fn r031_a_beast_arrives_at_the_map_edge_when_the_daily_roll_hits() {
    let mut sim = quiet(40, 20, &[(20, 10)]);
    loud_notice(&mut sim, 0);
    run_to(&mut sim, TICKS_PER_DAY + 1);
    let events = sim.drain_combat_events();
    let arrived = events
        .iter()
        .find_map(|e| match e {
            CombatEvent::BeastArrived { x, y, count, .. } => Some((*x, *y, *count)),
            _ => None,
        })
        .expect("a beast arrived");
    assert!(is_edge(&sim, arrived.0, arrived.1));
    assert_eq!(sim.creatures().len() as u32, arrived.2);
    assert!(sim.creatures().iter().all(|(_, c)| c.driven));
}

#[test]
fn r031_no_beast_arrives_in_the_quiet_days() {
    let mut sim = quiet(40, 20, &[(20, 10)]);
    loud_notice(&mut sim, 2);
    run_to(&mut sim, 2 * TICKS_PER_DAY + 1);
    assert!(sim.creatures().is_empty(), "day 2 is still quiet");
    run_to(&mut sim, 3 * TICKS_PER_DAY + 1);
    assert!(!sim.creatures().is_empty(), "day 3 brings the beast");
}

#[test]
fn r031_beast_level_and_group_size_come_from_notice() {
    let mut low = quiet(40, 20, &[(20, 10)]);
    with_rules(&mut low, |r| r.creature.beast_quiet_days = 0);
    run_to(&mut low, TICKS_PER_DAY + 1);
    assert_eq!(low.beast_level(), 1);
    low.debug_spawn_beasts();
    assert_eq!(low.creatures().len(), 1, "level 1 gives a group of 1");
    assert_eq!(count_kind(&low, "wolf"), 1, "a bear needs level 3");

    let mut high = quiet(40, 20, &[(20, 10)]);
    with_rules(&mut high, |r| {
        r.creature.beast_quiet_days = 0;
        r.notice.per_colonist = 200;
    });
    run_to(&mut high, TICKS_PER_DAY + 1);
    // Notice 200 gives level 1 + 200 / 25 = 9 and a group of min(4, 1 + 9 / 3).
    assert_eq!(high.beast_level(), 9);
    high.debug_spawn_beasts();
    assert_eq!(high.creatures().len(), 4);
}

#[test]
fn r031_a_hunting_beast_chases_and_wounds_a_colonist() {
    let mut sim = quiet(40, 10, &[(5, 5)]);
    let id = first(&sim);
    let bear = sim.debug_spawn_creature("bear", 30, 5).unwrap();
    sim.debug_drive_creature(bear);
    run(&mut sim, 900);
    let events = sim.drain_combat_events();
    assert!(events.iter().any(|e| matches!(
        e,
        CombatEvent::Hit {
            by_colonist: false,
            ..
        }
    )));
    let c = sim.colonist(id);
    assert!(
        c.is_none_or(|c| c.health < 100),
        "the bear hurt the colonist"
    );
}

#[test]
fn r031_a_beast_that_kills_a_colonist_gives_the_cause_wounds() {
    let mut sim = quiet(20, 6, &[(2, 3), (3, 3)]);
    let ids: Vec<ColonistId> = sim.colonists().iter().map(|(i, _)| i).collect();
    for &id in &ids {
        sim.debug_set_health(id, 1);
    }
    let bear = sim.debug_spawn_creature("bear", 6, 3).unwrap();
    sim.debug_drive_creature(bear);
    run(&mut sim, 600);
    assert!(!sim.graveyard().is_empty());
    assert!(
        sim.graveyard()
            .iter()
            .all(|g| g.cause == sim::DeathCause::Wounds)
    );
}

#[test]
fn r031_lair_beasts_sleep_until_notice_is_30_and_the_quiet_days_end() {
    let mut sim = quiet(60, 20, &[(5, 10)]);
    with_rules(&mut sim, |r| r.creature.beast_quiet_days = 2);
    sim.debug_add_lair("wolf", 50, 10, 2);
    assert_eq!(sim.creatures().len(), 2);
    run_to(&mut sim, 3 * TICKS_PER_DAY + 1);
    assert!(!sim.lairs_active(), "Notice is below 30");
    assert!(!sim.lairs_awake());
    assert!(sim.creatures().iter().all(|(_, c)| !c.driven));
    assert!(
        sim.creatures().iter().all(|(_, c)| c.tile() == (50, 10)),
        "sleeping beasts do not move"
    );
    with_rules(&mut sim, |r| r.notice.per_colonist = 60);
    assert!(sim.lairs_active());
    run_to(&mut sim, 4 * TICKS_PER_DAY + 1);
    assert!(sim.lairs_awake());
    assert!(sim.creatures().iter().all(|(_, c)| c.driven));
    assert!(
        sim.drain_combat_events()
            .contains(&CombatEvent::LairsWoke { count: 2 })
    );
}

#[test]
fn r031_lair_beasts_do_not_wake_in_the_quiet_days() {
    let mut sim = quiet(60, 20, &[(5, 10)]);
    with_rules(&mut sim, |r| {
        r.creature.beast_quiet_days = 5;
        r.notice.per_colonist = 60;
    });
    sim.debug_add_lair("wolf", 50, 10, 1);
    run_to(&mut sim, 3 * TICKS_PER_DAY + 1);
    assert!(sim.lairs_active());
    assert!(!sim.lairs_awake(), "the first days are safe");
}

#[test]
fn r031_an_awake_threat_releases_horrors_at_its_site() {
    let mut sim = quiet(40, 20, &[(5, 10)]);
    sim.debug_add_threat(7, 2, 30, 10, &[(30, 9)]);
    assert!(
        sim.creatures().is_empty(),
        "a dormant threat has no creature"
    );
    sim.debug_wake_threat(0);
    // Level 2 plus Notice / 25: opening the seal adds 50 Notice, so the strength is 4.
    let level = sim.threat_strength(0).unwrap();
    assert_eq!(level, 4);
    // A group is 1 + 4 / 3 = 2 horrors.
    assert_eq!(sim.creatures().len(), 2);
    for (_, c) in sim.creatures().iter() {
        assert!(c.driven);
        assert!(c.x.abs_diff(30) <= 2 && c.y.abs_diff(10) <= 2);
    }
    assert_eq!(
        count_kind(&sim, "shade") + count_kind(&sim, "tomb_horror"),
        2
    );
    assert!(sim.drain_combat_events().iter().any(|e| matches!(
        e,
        CombatEvent::ThreatReleased {
            site: 7,
            count: 2,
            level: 4
        }
    )));
}

// ---- R-071: raiders ----

fn start_fight(sim: &mut Sim, strength: u8, hostility: u8) {
    sim.debug_add_rival(strength, hostility, 2);
    assert!(sim.debug_start_raid(0));
    while !sim.raid_choice_pending() {
        sim.step(&[]);
    }
    sim.step(&[Command::RaidFight]);
}

#[test]
fn r071_raiders_come_in_as_creatures_at_the_entry_tile() {
    let mut sim = quiet(60, 20, &[(2, 10)]);
    start_fight(&mut sim, 8, 100);
    let raid = sim.raid().unwrap().clone();
    assert_eq!(raid.fighters, 8);
    assert_eq!(sim.raiders_on_map(raid.id), 8);
    assert!(sim.creatures().iter().all(|(_, c)| c.tile() == raid.entry));
    assert!(is_edge(&sim, raid.entry.0, raid.entry.1));
    let raider = sim.rules().creature.kind_index("raider").unwrap();
    assert!(sim.creatures().iter().all(|(_, c)| c.kind == raider));
}

#[test]
fn r071_dead_raiders_count_as_down_and_half_down_ends_the_raid() {
    let mut sim = quiet(60, 20, &[(2, 10)]);
    start_fight(&mut sim, 8, 100);
    let ids: Vec<_> = sim.creatures().iter().map(|(i, _)| i).collect();
    run(&mut sim, 100);
    for &id in &ids[..3] {
        sim.debug_hurt_creature(id, 1000, None);
    }
    assert_eq!(sim.raid().unwrap().down, 3);
    run(&mut sim, 2);
    assert!(sim.raid().is_some(), "less than half are down");
    sim.debug_hurt_creature(ids[3], 1000, None);
    run(&mut sim, 2);
    assert!(sim.raid().is_none(), "half are down");
    assert_eq!(sim.raid_log()[0].outcome, RaidOutcome::Repelled);
    // The four raiders that are left walk off the map.
    assert_eq!(sim.creatures().len(), 4);
    run(&mut sim, 3000);
    assert!(sim.creatures().is_empty(), "they left the map");
}

#[test]
fn r071_raiders_attack_colonists() {
    let mut sim = quiet(60, 20, &[(40, 10)]);
    let id = first(&sim);
    start_fight(&mut sim, 12, 100);
    run(&mut sim, 1500);
    let events = sim.drain_combat_events();
    assert!(events.iter().any(|e| matches!(
        e,
        CombatEvent::Hit {
            by_colonist: false,
            ..
        }
    )));
    assert!(sim.colonist(id).is_none_or(|c| c.health < 100));
}

#[test]
fn r071_raiders_carry_a_stack_away_and_leave_with_it() {
    let mut sim = quiet(60, 20, &[(0, 0)]);
    let who = sim.colonists().iter().next().unwrap().0;
    sim.debug_clear_tools(who);
    let gold = sim
        .rules()
        .materials
        .iter()
        .enumerate()
        .max_by_key(|(_, m)| m.value)
        .unwrap()
        .0 as u16;
    sim.debug_spawn_item(gold, 10, 40, 10);
    start_fight(&mut sim, 4, 100);
    run(&mut sim, 6000);
    assert!(sim.raid().is_none(), "the raid is over");
    assert_eq!(sim.items().total_units(gold), 0, "the stack is gone");
    let log = &sim.raid_log()[0];
    assert_eq!(log.outcome, RaidOutcome::Plundered);
    assert!(log.value_taken > 0);
    assert!(sim.creatures().is_empty(), "the raiders left the map");
    let events = sim.drain_raid_events();
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(e, RaidEvent::Looted { .. }))
            .count(),
        1
    );
}

#[test]
fn r071_a_raider_that_dies_drops_what_it_carries() {
    let mut sim = quiet(60, 20, &[(0, 0)]);
    let gold = sim
        .rules()
        .materials
        .iter()
        .enumerate()
        .max_by_key(|(_, m)| m.value)
        .unwrap()
        .0 as u16;
    sim.debug_spawn_item(gold, 10, 40, 10);
    start_fight(&mut sim, 4, 100);
    let mut carrier = None;
    for _ in 0..4000 {
        sim.step(&[]);
        carrier = sim
            .creatures()
            .iter()
            .find(|(_, c)| c.loot.is_some())
            .map(|(i, _)| i);
        if carrier.is_some() {
            break;
        }
    }
    let carrier = carrier.expect("a raider took the stack");
    assert_eq!(sim.items().total_units(gold), 0);
    sim.debug_hurt_creature(carrier, 1000, None);
    assert_eq!(
        sim.items().total_units(gold),
        10,
        "the stack lies there again"
    );
}

#[test]
fn r071_raiders_with_nothing_to_take_and_no_colonist_in_reach_leave() {
    let mut sim = quiet(60, 20, &[(0, 0)]);
    start_fight(&mut sim, 2, 100);
    run(&mut sim, 12_000);
    assert!(sim.raid().is_none());
    assert!(sim.creatures().is_empty());
}

fn building(sim: &Sim, key: &str) -> u16 {
    sim.rules().building_index(key).unwrap()
}

#[test]
fn r071_raiders_break_a_wall_in_a_time_from_the_hardness() {
    let mut sim = quiet(40, 14, &[(1, 1), (25, 7)]);
    let wall = building(&sim, "wall");
    let granite = material(&sim, "granite");
    for (x, y) in [
        (24, 6),
        (25, 6),
        (26, 6),
        (24, 7),
        (26, 7),
        (24, 8),
        (25, 8),
        (26, 8),
    ] {
        assert!(sim.debug_build(wall, x, y, granite));
    }
    let ticks = sim.raid_break_ticks(24, 6).unwrap();
    start_fight(&mut sim, 3, 100);
    let start = sim.tick();
    let mut breached_at = None;
    for _ in 0..8000 {
        sim.step(&[]);
        if sim
            .drain_combat_events()
            .iter()
            .any(|e| matches!(e, CombatEvent::Breached { .. }))
        {
            breached_at = Some(sim.tick() - start);
            break;
        }
    }
    let at = breached_at.expect("a raider broke a wall");
    assert!(
        at >= u64::from(ticks),
        "the break takes at least {ticks} ticks, it took {at}"
    );
    assert!(at <= u64::from(ticks) + 1200, "and not much more: {at}");
    let walls = (24..=26)
        .flat_map(|x| (6..=8).map(move |y| (x, y)))
        .filter(|&(x, y)| (x, y) != (25, 7) && sim.raid_break_ticks(x, y).is_some())
        .count();
    assert_eq!(walls, 7, "one wall is gone");
}

#[test]
fn r071_raiders_break_a_door_before_they_walk_through_it() {
    let mut sim = quiet(40, 14, &[(1, 1), (25, 7)]);
    let wall = building(&sim, "wall");
    let door = building(&sim, "door");
    let granite = material(&sim, "granite");
    let plank = sim
        .rules()
        .materials
        .iter()
        .position(|m| m.key == "plank" && m.category == MaterialCategory::Wood)
        .unwrap() as u16;
    for (x, y) in [
        (24, 6),
        (25, 6),
        (26, 6),
        (24, 7),
        (24, 8),
        (25, 8),
        (26, 8),
    ] {
        assert!(sim.debug_build(wall, x, y, granite));
    }
    assert!(sim.debug_build(door, 26, 7, plank));
    let ticks = sim.raid_break_ticks(26, 7).unwrap();
    assert!(
        ticks < sim.raid_break_ticks(24, 6).unwrap(),
        "a door breaks faster than a granite wall"
    );
    start_fight(&mut sim, 3, 100);
    let start = sim.tick();
    let mut found = None;
    for _ in 0..4000 {
        sim.step(&[]);
        for e in sim.drain_combat_events() {
            if let CombatEvent::Breached { x, y } = e {
                found = Some((x, y, sim.tick() - start));
            }
        }
        if found.is_some() {
            break;
        }
    }
    let (x, y, at) = found.expect("a raider broke the door");
    assert_eq!((x, y), (26, 7));
    assert!(at >= u64::from(ticks));
    assert!(sim.raid_break_ticks(26, 7).is_none(), "the door is gone");
}

#[test]
fn r071_a_raid_with_the_player_paying_spawns_no_raiders() {
    let mut sim = quiet(60, 20, &[(2, 10)]);
    sim.debug_add_rival(8, 100, 2);
    assert!(sim.debug_start_raid(0));
    while !sim.raid_choice_pending() {
        sim.step(&[]);
    }
    assert!(
        sim.creatures().is_empty(),
        "nobody fights before the answer"
    );
}

// ---- leather and meat ----

#[test]
fn s13_the_leather_chain_has_recipes_and_clothes() {
    let sim = quiet(8, 4, &[(0, 0)]);
    let rules = sim.rules();
    let recipe = |key: &str| rules.recipes.iter().find(|r| r.key == key).unwrap();
    let tan = recipe("tan_hide");
    assert_eq!(tan.inputs, vec![(material(&sim, "hide"), 2)]);
    assert_eq!(tan.outputs, vec![(material(&sim, "leather"), 1)]);
    let sew = recipe("leather_clothes");
    assert_eq!(sew.outputs, vec![(material(&sim, "leather_clothes"), 1)]);
    let cook = recipe("meal_meat");
    assert_eq!(cook.outputs, vec![(material(&sim, "meal"), 1)]);
    assert!(
        rules
            .needs
            .clothes_def(material(&sim, "leather_clothes"))
            .is_some()
    );
    assert!(rules.needs.food(material(&sim, "meat")).is_some());
    let cat = |k: &str| rules.materials[usize::from(material(&sim, k))].category;
    assert_eq!(cat("hide"), MaterialCategory::Leather);
    assert_eq!(cat("leather"), MaterialCategory::Leather);
    assert_eq!(cat("meat"), MaterialCategory::Food);
}

// ---- budget, save, determinism, new colony ----

#[test]
fn p04_creatures_use_the_path_budget() {
    let mut sim = quiet(120, 40, &[(5, 20), (6, 20)]);
    let ids: Vec<ColonistId> = sim.colonists().iter().map(|(i, _)| i).collect();
    for k in 0..120u16 {
        let wolf = sim
            .debug_spawn_creature("wolf", 100 + (k % 10), 2 + (k / 10) * 3)
            .unwrap();
        sim.debug_drive_creature(wolf);
    }
    let mut most = 0;
    for _ in 0..200 {
        sim.step(&[Command::MoveColonist {
            id: ids[0],
            x: 40,
            y: 20,
        }]);
        most = most.max(sim.path_stats().searches);
    }
    assert!(
        most <= sim::PATH_BUDGET_SEARCHES,
        "the searches stay in the budget"
    );
    assert!(
        sim.colonist(ids[0]).is_some_and(|c| c.x > 5),
        "the colonist walks"
    );
}

fn fight_sim() -> Sim {
    let mut sim = quiet(60, 20, &[(40, 10), (41, 10)]);
    start_fight(&mut sim, 6, 100);
    sim
}

#[test]
fn s13_a_save_in_the_middle_of_a_fight_loads_and_goes_on_the_same_way() {
    let mut a = fight_sim();
    run(&mut a, 250);
    assert!(!a.creatures().is_empty());
    let bytes = save::to_bytes(&a);
    let mut b = save::from_bytes(&bytes).unwrap();
    b.debug_set_needs_enabled(false);
    assert_eq!(a.state_hash(), b.state_hash());
    run(&mut a, 1500);
    run(&mut b, 1500);
    assert_eq!(a.state_hash(), b.state_hash());
}

#[test]
fn s13_the_same_seed_gives_the_same_fight() {
    let mut a = fight_sim();
    let mut b = fight_sim();
    run(&mut a, 1500);
    run(&mut b, 1500);
    assert_eq!(a.state_hash(), b.state_hash());
    let hits = |s: &mut Sim| s.drain_combat_events().len();
    assert_eq!(hits(&mut a), hits(&mut b));
}

fn generated_sim() -> Sim {
    let data = worldgen::generate_local(&worldgen::EmbarkSummary::temperate_forest());
    Sim::new(&data, 1, 0)
}

#[test]
fn s13_a_new_colony_has_game_animals_far_from_the_camp_and_far_lairs() {
    let sim = generated_sim();
    let r = &sim.rules().creature;
    let game: Vec<_> = sim
        .creatures()
        .iter()
        .filter(|(_, c)| !c.lair)
        .map(|(_, c)| c.tile())
        .collect();
    assert!(game.len() as u32 >= r.game_min && game.len() as u32 <= r.game_max);
    for t in &game {
        for (_, col) in sim.colonists().iter() {
            let d = u32::from(t.0.abs_diff(col.x)).max(u32::from(t.1.abs_diff(col.y)));
            assert!(d >= r.game_safe_distance, "an animal starts {d} tiles away");
        }
    }
    assert!(!sim.lairs().is_empty(), "the map has lairs");
    for &(lx, ly) in sim.lairs() {
        for (_, col) in sim.colonists().iter() {
            let d = u32::from(lx.abs_diff(col.x)).max(u32::from(ly.abs_diff(col.y)));
            assert!(d >= r.lair_min_distance / 2);
        }
    }
    assert!(
        sim.creatures()
            .iter()
            .filter(|(_, c)| c.lair)
            .all(|(_, c)| !c.driven),
        "lair beasts sleep at the start"
    );
}

#[test]
fn s13_the_first_days_are_safe_nothing_hunts_the_colony() {
    let mut sim = generated_sim();
    run(&mut sim, 2 * TICKS_PER_DAY);
    assert!(
        sim.creatures().iter().all(|(_, c)| !c.driven),
        "no beast hunts in the first days"
    );
    assert!(
        sim.drain_combat_events().iter().all(|e| !matches!(
            e,
            CombatEvent::Hit {
                by_colonist: false,
                ..
            }
        )),
        "nobody hurt a colonist"
    );
    assert!(sim.raid().is_none());
}

#[test]
fn s13_game_animals_come_back_one_by_one_up_to_the_start_count() {
    let mut sim = generated_sim();
    let game_count = |s: &Sim| {
        s.creatures()
            .iter()
            .filter(|(_, c)| {
                s.creature_kind(c.kind)
                    .is_some_and(|k| k.role == sim::Role::Game)
            })
            .count()
    };
    let start = game_count(&sim);
    let victims: Vec<_> = sim
        .creatures()
        .iter()
        .filter(|(_, c)| !c.lair)
        .take(2)
        .map(|(i, _)| i)
        .collect();
    for v in victims {
        sim.debug_hurt_creature(v, 1000, None);
    }
    assert_eq!(game_count(&sim), start - 2);
    run_to(&mut sim, 6 * TICKS_PER_DAY + 1);
    assert_eq!(game_count(&sim), start, "two animals grew back");
}
