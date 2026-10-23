//! Sprint 18 tests (sim part): rival colonies, hostility, raids, and the tribute
//! (R-070, R-071, R-072).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use sim::scenario::sim_from_ascii;
use sim::time::TICKS_PER_DAY;
use sim::{
    Command, HostilityCause, RaidEvent, RaidOutcome, RaidPhase, RaidReject, Sim, SimRules, save,
};
use std::sync::Arc;

const MAP: &str = "c.......\n........\n........\n........\n........\n........\n........\n........";

fn quiet() -> Sim {
    let mut sim = sim_from_ascii(MAP, 1).unwrap();
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

/// The index of the most valuable material, and its unit value.
fn rich_material(sim: &Sim) -> (u16, u32) {
    let (i, m) = sim
        .rules()
        .materials
        .iter()
        .enumerate()
        .max_by_key(|(_, m)| m.value)
        .unwrap();
    (i as u16, u32::from(m.value))
}

fn part(sim: &Sim, cause: HostilityCause) -> i32 {
    sim.rival_hostility(0)
        .unwrap()
        .parts
        .iter()
        .find(|p| p.cause == cause)
        .unwrap()
        .value
}

#[test]
fn r070_hostility_is_the_sum_of_history_wealth_and_notice() {
    let mut sim = quiet();
    sim.debug_add_rival(8, 30, 2);
    let (mat, value) = rich_material(&sim);
    let h = sim.rival_hostility(0).unwrap();
    assert_eq!(part(&sim, HostilityCause::History), 30);
    assert_eq!(part(&sim, HostilityCause::Wealth), 0);
    let notice = sim.notice().total;
    assert_eq!(part(&sim, HostilityCause::Notice), (notice / 2) as i32);
    assert_eq!(
        h.total as i32,
        h.parts.iter().map(|p| p.value).sum::<i32>().clamp(0, 100)
    );
    // 5,000 value units of wealth give 5 hostility.
    sim.debug_spawn_item(mat, 5_000 / value + 1, 3, 3);
    assert_eq!(part(&sim, HostilityCause::Wealth), 5);
    assert!(sim.rival_hostility(0).unwrap().total > h.total);
}

#[test]
fn r070_hostility_stops_at_100() {
    let mut sim = quiet();
    sim.debug_add_rival(8, 100, 2);
    sim.debug_add_noise(2, 2, 5_000);
    assert_eq!(sim.rival_hostility(0).unwrap().total, 100);
    assert!(sim.rival_hostility(1).is_none());
}

#[test]
fn r071_chance_is_hostility_over_20_percent_up_to_5_percent() {
    let mut sim = quiet();
    sim.debug_add_rival(8, 40, 2);
    sim.debug_add_rival(8, 100, 2);
    sim.debug_add_rival(8, 0, 2);
    let notice0 = sim.notice().total / 2;
    // Hostility 40 + Notice part gives h / 20 percent = h / 2 per-mille.
    let h0 = sim.rival_hostility(0).unwrap().total;
    assert_eq!(h0, 40 + notice0);
    assert_eq!(sim.raid_chance_permille(0), h0 / 2);
    assert_eq!(sim.raid_chance_permille(1), 50, "5 percent is the maximum");
    sim.debug_add_noise(0, 0, 10_000);
    assert_eq!(sim.raid_chance_permille(1), 50);
    assert_eq!(sim.raid_chance_permille(9), 0);
}

#[test]
fn r071_raid_size_is_strength_times_hostility_and_at_least_2() {
    let mut sim = quiet();
    sim.debug_add_rival(12, 50, 2);
    sim.debug_add_rival(3, 4, 2);
    let h = sim.rival_hostility(0).unwrap().total;
    assert_eq!(sim.raid_size(0), (12 * h / 100).max(2));
    assert_eq!(sim.raid_size(1), 2);
    assert_eq!(sim.raid_size(7), 0);
}

#[test]
fn r071_no_raid_in_the_quiet_days_of_the_run() {
    let mut sim = quiet();
    with_rules(&mut sim, |r| r.raid.max_pct = 100);
    sim.debug_add_rival(12, 100, 2);
    run_to(&mut sim, 20 * TICKS_PER_DAY);
    assert!(sim.raid().is_none() && sim.drain_raid_events().is_empty());
    assert!(sim.rivals()[0].last_raid_day.is_none());
}

#[test]
fn r071_a_certain_rival_raids_the_day_after_the_quiet_days() {
    let mut sim = quiet();
    // 100 percent chance: max_pct 100 and chance_div 1 give a per-mille chance of 1000.
    with_rules(&mut sim, |r| {
        r.raid.max_pct = 100;
        r.raid.chance_div = 1;
    });
    sim.debug_add_rival(8, 100, 2);
    run_to(&mut sim, 21 * TICKS_PER_DAY);
    let raid = sim.raid().expect("raid");
    assert_eq!(raid.phase, RaidPhase::Scouting);
    assert_eq!(raid.fighters, sim.raid_size(0));
    assert_eq!(sim.rivals()[0].last_raid_day, Some(21));
}

#[test]
fn r071_a_scout_comes_one_day_before_the_raiders_and_warns() {
    let mut sim = quiet();
    sim.debug_add_rival(8, 60, 2);
    run(&mut sim, 5);
    assert!(sim.debug_start_raid(0));
    let events = sim.drain_raid_events();
    let RaidEvent::ScoutSeen {
        x,
        y,
        fighters,
        arrives_tick,
        ..
    } = events[0]
    else {
        panic!("{events:?}")
    };
    assert_eq!(x, 7, "east edge");
    assert!(sim.map().passable(i32::from(x), i32::from(y)));
    assert_eq!(arrives_tick, sim.tick() + TICKS_PER_DAY);
    let w = sim.raid_warning().unwrap();
    assert_eq!((w.x, w.y, w.fighters), (x, y, fighters));
    assert_eq!(w.ticks_left, TICKS_PER_DAY);
    assert!(!sim.raid_choice_pending());
    run(&mut sim, TICKS_PER_DAY - 1);
    assert!(sim.raid_warning().is_some());
    run(&mut sim, 1);
    assert!(sim.raid_warning().is_none());
    assert!(sim.raid_choice_pending());
    assert!(matches!(
        sim.drain_raid_events().as_slice(),
        [RaidEvent::Arrived { .. }]
    ));
}

#[test]
fn r071_a_rival_colony_waits_after_a_raid() {
    let mut sim = quiet();
    with_rules(&mut sim, |r| {
        r.raid.max_pct = 100;
        r.raid.chance_div = 1;
        r.raid.quiet_days = 0;
        r.raid.stay_ticks = 10;
    });
    sim.debug_add_rival(8, 100, 2);
    run_to(&mut sim, TICKS_PER_DAY);
    assert!(sim.raid().is_some());
    run(&mut sim, TICKS_PER_DAY + 5);
    sim.step(&[Command::RaidFight]);
    run(&mut sim, 400);
    assert!(sim.raid().is_none());
    assert_eq!(sim.raid_log().len(), 1);
    // The cooldown is 5 days: no new raid before day 6.
    run_to(&mut sim, 5 * TICKS_PER_DAY);
    assert!(sim.raid().is_none());
    run_to(&mut sim, 6 * TICKS_PER_DAY);
    assert!(sim.raid().is_some());
}

fn arrived(sim: &mut Sim) {
    assert!(sim.debug_start_raid(0));
    run(sim, TICKS_PER_DAY);
    assert!(sim.raid_choice_pending());
    sim.drain_raid_events();
}

#[test]
fn r072_the_tribute_is_200_for_each_raider() {
    let mut sim = quiet();
    sim.debug_add_rival(12, 100, 2);
    arrived(&mut sim);
    let fighters = sim.raid().unwrap().fighters;
    assert_eq!(sim.tribute_due(), Some(200 * u64::from(fighters)));
}

#[test]
fn r072_paying_removes_the_goods_and_the_raiders_leave_and_hostility_falls() {
    let mut sim = quiet();
    sim.debug_add_rival(8, 80, 2);
    let (mat, value) = rich_material(&sim);
    sim.debug_spawn_item(mat, 400 / value + 50, 2, 2);
    sim.debug_spawn_item(mat, 3, 4, 4);
    arrived(&mut sim);
    let before = sim.rival_hostility(0).unwrap().total;
    let due = sim.tribute_due().unwrap();
    assert!(sim.can_pay_tribute());
    let goods = sim.tribute_goods();
    let big = goods.iter().find(|g| g.count > 3).unwrap();
    let units = u32::try_from(due.div_ceil(u64::from(big.unit_value))).unwrap();
    assert!(units <= big.count);
    sim.step(&[Command::RaidPay {
        goods: vec![(big.item, units)],
    }]);
    assert!(sim.raid().is_none());
    let events = sim.drain_raid_events();
    let Some(RaidEvent::Ended {
        outcome, hostility, ..
    }) = events.last().copied()
    else {
        panic!("{events:?}")
    };
    assert_eq!(outcome, RaidOutcome::Paid);
    assert_eq!(hostility, sim.rival_hostility(0).unwrap().total);
    assert!(hostility < before);
    assert_eq!(
        sim.items().total_units(big.material),
        u64::from(big.count - units) + 3
    );
    assert_eq!(sim.raid_log()[0].outcome, RaidOutcome::Paid);
    assert_eq!(part(&sim, HostilityCause::Raids), -20);
}

#[test]
fn r072_the_player_cannot_pay_with_too_few_goods_or_wrong_goods() {
    let mut sim = quiet();
    sim.debug_add_rival(8, 80, 2);
    let (mat, _) = rich_material(&sim);
    sim.debug_spawn_item(mat, 2, 2, 2);
    arrived(&mut sim);
    assert!(!sim.can_pay_tribute());
    let item = sim.tribute_goods()[0].item;
    sim.step(&[Command::RaidPay {
        goods: vec![(item, 2)],
    }]);
    assert_eq!(
        sim.drain_raid_events(),
        vec![RaidEvent::Rejected(RaidReject::TooLittle)]
    );
    sim.step(&[Command::RaidPay {
        goods: vec![(item, 99)],
    }]);
    assert_eq!(
        sim.drain_raid_events(),
        vec![RaidEvent::Rejected(RaidReject::BadGoods)]
    );
    sim.step(&[Command::RaidPay {
        goods: vec![(item, 1), (item, 1)],
    }]);
    assert_eq!(
        sim.drain_raid_events(),
        vec![RaidEvent::Rejected(RaidReject::BadGoods)]
    );
    assert!(
        sim.raid_choice_pending(),
        "a refused command changes nothing"
    );
    assert_eq!(sim.items().total_units(mat), 2);
}

#[test]
fn r072_raid_commands_without_a_waiting_raid_are_refused() {
    let mut sim = quiet();
    sim.debug_add_rival(8, 80, 2);
    sim.step(&[Command::RaidFight]);
    sim.step(&[Command::RaidPay { goods: Vec::new() }]);
    assert_eq!(
        sim.drain_raid_events(),
        vec![
            RaidEvent::Rejected(RaidReject::NoChoice),
            RaidEvent::Rejected(RaidReject::NoChoice)
        ]
    );
    assert!(sim.debug_start_raid(0));
    sim.step(&[Command::RaidFight]);
    assert_eq!(
        sim.drain_raid_events().last(),
        Some(&RaidEvent::Rejected(RaidReject::NoChoice)),
        "the raiders have not arrived"
    );
}

#[test]
fn r072_the_raiders_wait_for_the_answer_then_fight_by_themselves() {
    let mut sim = quiet();
    sim.debug_add_rival(8, 80, 2);
    arrived(&mut sim);
    run(&mut sim, 1_000);
    assert!(sim.raid_choice_pending(), "the raid does not move");
    run(&mut sim, 200);
    let (phase, id) = {
        let raid = sim.raid().unwrap();
        (raid.phase, raid.id)
    };
    assert_eq!(phase, RaidPhase::Fighting);
    assert!(sim.drain_raid_events().contains(&RaidEvent::FightBegan {
        raid: id,
        auto: true
    }));
}

#[test]
fn r071_raiders_carry_away_items_and_leave_and_hostility_rises_by_10() {
    let mut sim = quiet();
    sim.debug_add_rival(8, 50, 2);
    let (mat, _) = rich_material(&sim);
    sim.debug_spawn_item(mat, 10, 2, 2);
    sim.debug_spawn_item(mat, 5, 3, 3);
    arrived(&mut sim);
    let before = sim.rival_hostility(0).unwrap().total;
    sim.step(&[Command::RaidFight]);
    run(&mut sim, 5_000);
    assert!(sim.raid().is_none());
    assert_eq!(sim.items().total_units(mat), 0);
    let log = &sim.raid_log()[0];
    assert_eq!(log.outcome, RaidOutcome::Plundered);
    assert!(log.value_taken > 0);
    assert_eq!(part(&sim, HostilityCause::Raids), 10);
    assert_eq!(
        sim.rival_hostility(0).unwrap().total,
        (before + 10).min(100)
    );
    let events = sim.drain_raid_events();
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(e, RaidEvent::Looted { .. }))
            .count(),
        2
    );
}

#[test]
fn r071_raiders_with_nothing_to_take_leave() {
    let mut sim = quiet();
    sim.debug_add_rival(8, 50, 2);
    arrived(&mut sim);
    sim.step(&[Command::RaidFight]);
    run(&mut sim, 3_000);
    assert_eq!(sim.raid_log()[0].outcome, RaidOutcome::Left);
    assert_eq!(part(&sim, HostilityCause::Raids), 0);
}

#[test]
fn r071_raiders_leave_when_half_of_them_are_down_and_hostility_falls_by_10() {
    let mut sim = quiet();
    sim.debug_add_rival(12, 100, 2);
    let (mat, _) = rich_material(&sim);
    sim.debug_spawn_item(mat, 10, 2, 2);
    arrived(&mut sim);
    let fighters = sim.raid().unwrap().fighters;
    sim.step(&[Command::RaidFight]);
    sim.raid_fighters_down(fighters / 2 - 1);
    run(&mut sim, 2);
    assert!(sim.raid().is_some(), "less than half are down");
    sim.raid_fighters_down(1);
    run(&mut sim, 1);
    assert!(sim.raid().is_none());
    assert_eq!(sim.raid_log()[0].outcome, RaidOutcome::Repelled);
    assert_eq!(part(&sim, HostilityCause::Raids), -10);
    assert_eq!(sim.items().total_units(mat), 10, "nothing was taken");
}

#[test]
fn r071_raiders_break_doors_and_walls_in_a_time_from_the_hardness() {
    let mut sim = quiet();
    let rules = sim.rules().clone();
    let wall = rules
        .buildings
        .iter()
        .position(|b| b.key == "wall")
        .unwrap() as u16;
    let door = rules
        .buildings
        .iter()
        .position(|b| b.key == "door")
        .unwrap() as u16;
    let floor = rules
        .buildings
        .iter()
        .position(|b| b.key == "floor")
        .unwrap() as u16;
    let granite = rules
        .materials
        .iter()
        .position(|m| m.key == "granite")
        .unwrap() as u16;
    let wood = rules
        .materials
        .iter()
        .position(|m| m.category == base::content::MaterialCategory::Wood)
        .unwrap() as u16;
    assert!(sim.debug_build(wall, 3, 3, granite));
    assert!(sim.debug_build(door, 4, 3, wood));
    assert!(sim.debug_build(floor, 5, 3, granite));
    let per = rules.raid.break_ticks_per_hardness;
    let hard = |m: u16| u32::from(rules.materials[usize::from(m)].hardness);
    assert_eq!(sim.raid_break_ticks(3, 3), Some(hard(granite) * per));
    assert_eq!(sim.raid_break_ticks(4, 3), Some(hard(wood) * per));
    assert!(hard(granite) > hard(wood));
    assert_eq!(
        sim.raid_break_ticks(5, 3),
        None,
        "a floor does not stop raiders"
    );
    assert_eq!(sim.raid_break_ticks(0, 5), None);
}

#[test]
fn s18_a_save_in_the_middle_of_a_raid_loads_and_goes_on_the_same_way() {
    let mut sim = quiet();
    sim.debug_add_rival(8, 70, 3);
    let (mat, _) = rich_material(&sim);
    sim.debug_spawn_item(mat, 10, 2, 2);
    arrived(&mut sim);
    sim.step(&[Command::RaidFight]);
    run(&mut sim, 350);
    let bytes = save::to_bytes(&sim);
    let mut loaded = save::from_bytes(&bytes).unwrap();
    loaded.debug_set_needs_enabled(false);
    assert_eq!(loaded.state_hash(), sim.state_hash());
    run(&mut sim, 3_000);
    run(&mut loaded, 3_000);
    assert_eq!(loaded.state_hash(), sim.state_hash());
    assert_eq!(loaded.raid_log(), sim.raid_log());
}

#[test]
fn s18_the_same_seed_gives_the_same_raid() {
    let make = || {
        let mut sim = quiet();
        sim.debug_add_rival(10, 90, 0);
        sim.debug_start_raid(0);
        run(&mut sim, 2 * TICKS_PER_DAY);
        sim
    };
    let (a, b) = (make(), make());
    assert_eq!(a.raid().map(|r| r.entry), b.raid().map(|r| r.entry));
    assert_eq!(a.state_hash(), b.state_hash());
}

#[test]
fn s18_no_raid_when_the_run_is_over() {
    let mut sim = quiet();
    with_rules(&mut sim, |r| {
        r.raid.max_pct = 100;
        r.raid.chance_div = 1;
        r.raid.quiet_days = 0;
    });
    sim.debug_add_rival(8, 100, 2);
    sim.step(&[Command::Abandon]);
    run_to(&mut sim, 3 * TICKS_PER_DAY);
    assert!(sim.raid().is_none());
}

#[test]
fn s18_local_generation_gives_rivals_to_the_sim() {
    let data = worldgen::generate_local(&worldgen::EmbarkSummary::temperate_forest());
    let mut data = data;
    data.rivals = vec![worldgen::EmbarkRival {
        site: 9,
        civ: 2,
        culture: 1,
        strength: 7,
        base_hostility: 33,
        edge: 1,
        distance: 2,
    }];
    let sim = Sim::new(&data, 1, 0);
    let r = &sim.rivals()[0];
    assert_eq!(
        (r.site, r.strength, r.base_hostility, r.edge),
        (9, 7, 33, 1)
    );
    let none = Sim::new(
        &worldgen::generate_local(&worldgen::EmbarkSummary::temperate_forest()),
        1,
        0,
    );
    assert!(none.rivals().is_empty());
}
