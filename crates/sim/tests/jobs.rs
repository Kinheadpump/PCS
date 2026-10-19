//! Scenario and invariant tests for items, jobs, designations, stockpiles, priorities,
//! and skills (Sprint 3).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use base::Pcg32;
use sim::scenario::sim_from_ascii;
use sim::{ColonistId, Command, ItemFilter, ItemLoc, JobEvent, JobKind, Sim, Skill, save};
use worldgen::RockKind;

fn ids(sim: &Sim) -> Vec<ColonistId> {
    sim.colonists().iter().map(|(id, _)| id).collect()
}

fn log_material(sim: &Sim) -> u16 {
    sim.rules().log
}

fn stone_material(sim: &Sim) -> u16 {
    sim.rules().rock_material(RockKind::Granite)
}

fn run(sim: &mut Sim, ticks: u32) {
    for _ in 0..ticks {
        sim.step(&[]);
        sim.check_invariants().unwrap();
    }
}

fn designate(kind: JobKind, x: u16, y: u16) -> Command {
    Command::Designate { kind, x, y }
}

fn zone(x0: u16, y0: u16, x1: u16, y1: u16, filter: ItemFilter) -> Command {
    Command::CreateStockpile {
        x0,
        y0,
        x1,
        y1,
        filter,
    }
}

fn set_priority(colonist: ColonistId, job: JobKind, priority: u8) -> Command {
    Command::SetPriority {
        colonist,
        job,
        priority,
    }
}

#[test]
fn s3_designated_tree_becomes_a_log_item() {
    let mut sim = sim_from_ascii("c.T..", 1).unwrap();
    sim.step(&[designate(JobKind::Cut, 2, 0)]);
    assert!(sim.map().tree(2, 0));
    run(&mut sim, 600);
    assert!(!sim.map().tree(2, 0), "the tree is gone");
    let log = log_material(&sim);
    assert_eq!(
        sim.items().total_units(log),
        u64::from(sim.rules().jobs.log_yield)
    );
    assert!(sim.designation(2, 0).is_none());
    assert_eq!(sim.jobs().len(), 0, "no stockpile: no hauling job");
    let (_, item) = sim.items().iter().next().unwrap();
    assert!(item.to_haul && item.reserved_by.is_none());
}

#[test]
fn s3_designated_rock_tile_becomes_floor_and_stone_item() {
    let mut sim = sim_from_ascii("cR..", 1).unwrap();
    assert!(!sim.map().passable(1, 0));
    sim.step(&[designate(JobKind::Mine, 1, 0)]);
    run(&mut sim, 1_500);
    assert_eq!(sim.map().rock(1, 0), Some(RockKind::None));
    assert!(sim.map().passable(1, 0), "a floor tile");
    assert_eq!(sim.map().ground(1, 0), Some(worldgen::Ground::Stone));
    let stone = stone_material(&sim);
    assert_eq!(
        sim.items().total_units(stone),
        u64::from(sim.rules().jobs.stone_yield)
    );
    assert_eq!(sim.regions().id_at(1, 0), sim.regions().id_at(0, 0));
}

#[test]
fn s3_cut_logs_are_hauled_to_the_stockpile_and_the_unit_count_stays() {
    let mut sim = sim_from_ascii("c.T.....\n........\n........", 2).unwrap();
    sim.step(&[
        zone(5, 2, 7, 2, ItemFilter::All),
        designate(JobKind::Cut, 2, 0),
    ]);
    run(&mut sim, 1_500);
    let log = log_material(&sim);
    assert_eq!(
        sim.items().total_units(log),
        u64::from(sim.rules().jobs.log_yield)
    );
    assert_eq!(sim.items().len(), 1);
    let (_, item) = sim.items().iter().next().unwrap();
    assert_eq!(item.loc, ItemLoc::Tile(5, 2), "first free stockpile tile");
    assert!(!item.to_haul && item.reserved_by.is_none());
    assert!(sim.jobs().is_empty());
}

#[test]
fn s3_hauling_does_not_change_the_item_count() {
    let mut sim = sim_from_ascii("c.......\n........\n........", 3).unwrap();
    let log = log_material(&sim);
    sim.debug_spawn_item(log, 7, 3, 0);
    sim.debug_spawn_item(log, 5, 4, 1);
    sim.step(&[
        zone(0, 2, 1, 2, ItemFilter::All),
        designate(JobKind::Haul, 3, 0),
        designate(JobKind::Haul, 4, 1),
    ]);
    let before = (sim.items().all_units(), sim.items().len());
    for _ in 0..2_000 {
        sim.step(&[]);
        sim.check_invariants().unwrap();
        assert_eq!(
            sim.items().all_units(),
            before.0,
            "units never change while hauling"
        );
    }
    assert!(sim.items().len() <= before.1, "stacks may merge");
    for (_, item) in sim.items().iter() {
        let ItemLoc::Tile(x, y) = item.loc else {
            panic!("an item is still carried");
        };
        assert_eq!(y, 2);
        assert!(x <= 1);
    }
}

#[test]
fn s3_stacks_of_one_material_merge_in_the_stockpile_up_to_the_stack_limit() {
    let mut sim = sim_from_ascii("c.......\n........", 4).unwrap();
    let log = log_material(&sim);
    sim.debug_spawn_item(log, 8, 2, 0);
    sim.debug_spawn_item(log, 8, 3, 0);
    sim.step(&[
        zone(6, 1, 7, 1, ItemFilter::All),
        designate(JobKind::Haul, 2, 0),
        designate(JobKind::Haul, 3, 0),
    ]);
    run(&mut sim, 1_500);
    assert_eq!(sim.items().total_units(log), 16);
    assert_eq!(sim.items().len(), 1, "8 + 8 fits in one stack");
}

#[test]
fn s3_stockpile_filter_keeps_other_materials_out() {
    let mut sim = sim_from_ascii("c.T.....\n........", 5).unwrap();
    let stone = stone_material(&sim);
    sim.step(&[
        zone(5, 1, 7, 1, ItemFilter::only(vec![stone])),
        designate(JobKind::Cut, 2, 0),
    ]);
    run(&mut sim, 1_500);
    let log = log_material(&sim);
    assert_eq!(sim.items().total_units(log), 4);
    let (_, item) = sim.items().iter().next().unwrap();
    assert_eq!(
        item.loc,
        ItemLoc::Tile(2, 0),
        "the log stays where the tree stood"
    );
    assert!(item.to_haul, "it waits for a stockpile that accepts it");
    // A filter that accepts logs now gets the log.
    let sp = sim.stockpiles().iter().next().unwrap().0;
    sim.step(&[Command::SetStockpileFilter {
        id: sp,
        filter: ItemFilter::All,
    }]);
    run(&mut sim, 1_000);
    let (_, item) = sim.items().iter().next().unwrap();
    assert!(matches!(item.loc, ItemLoc::Tile(x, 1) if x >= 5));
}

#[test]
fn s3_overlapping_or_outside_stockpiles_and_bad_designations_are_rejected() {
    let mut sim = sim_from_ascii("c...\n....", 1).unwrap();
    sim.step(&[
        zone(0, 0, 1, 1, ItemFilter::All),
        zone(1, 1, 2, 1, ItemFilter::All),
        zone(2, 0, 9, 9, ItemFilter::All),
    ]);
    assert_eq!(sim.stockpiles().len(), 1);
    let rejected = sim
        .drain_job_events()
        .into_iter()
        .filter(|e| matches!(e, JobEvent::Rejected(_)))
        .count();
    assert_eq!(rejected, 2);
    sim.step(&[
        designate(JobKind::Cut, 1, 1),
        designate(JobKind::Mine, 1, 1),
    ]);
    assert_eq!(sim.drain_job_events().len(), 2);
    assert!(sim.jobs().is_empty());
}

fn priority_scene(mine: u8, haul: u8) -> Sim {
    // Hauling is near and mining is far, so only the priority can choose mining.
    let mut sim = sim_from_ascii("c.......R\n.........", 6).unwrap();
    let id = ids(&sim)[0];
    let log = log_material(&sim);
    sim.debug_spawn_item(log, 3, 1, 0);
    sim.step(&[
        set_priority(id, JobKind::Mine, mine),
        set_priority(id, JobKind::Haul, haul),
    ]);
    // Commands come first in a tick, and the colonist takes a job in the same tick.
    sim.step(&[
        zone(0, 1, 1, 1, ItemFilter::All),
        designate(JobKind::Haul, 1, 0),
        designate(JobKind::Mine, 8, 0),
    ]);
    sim
}

#[test]
fn s3_priority_1_mining_is_chosen_before_priority_2_hauling() {
    let sim = priority_scene(1, 2);
    let id = ids(&sim)[0];
    let job = sim.colonist(id).unwrap().job.unwrap();
    assert_eq!(job.kind, JobKind::Mine);
    // The swapped priorities choose the near hauling job.
    let sim = priority_scene(2, 1);
    let job = sim.colonist(ids(&sim)[0]).unwrap().job.unwrap();
    assert_eq!(job.kind, JobKind::Haul);
}

#[test]
fn s3_priority_off_never_takes_the_job() {
    let sim = priority_scene(0, 0);
    let id = ids(&sim)[0];
    assert!(sim.colonist(id).unwrap().job.is_none());
    assert_eq!(sim.pick_job(id), None);
    assert_eq!(sim.jobs().len(), 2);
}

#[test]
fn s3_equal_priority_chooses_the_nearest_job_then_the_lowest_id() {
    let mut sim = sim_from_ascii("T.c.T.T\n.......", 1).unwrap();
    let id = ids(&sim)[0];
    // Trees are at distance 2 (x=0), 2 (x=4), and 4 (x=6). Designate the far one first.
    sim.step(&[
        designate(JobKind::Cut, 6, 0),
        designate(JobKind::Cut, 4, 0),
        designate(JobKind::Cut, 0, 0),
    ]);
    let active = sim.colonist(id).unwrap().job.unwrap();
    assert_eq!(
        active.stand,
        (4, 0),
        "equal distance: the lower job ID (designated first) wins"
    );
}

#[test]
fn s3_job_board_gives_no_job_in_a_different_region() {
    let mut sim = sim_from_ascii("c.#.T\n..#..\n..#..", 1).unwrap();
    let id = ids(&sim)[0];
    sim.step(&[designate(JobKind::Cut, 4, 0)]);
    assert_eq!(sim.jobs().len(), 1);
    assert_eq!(sim.pick_job(id), None);
    run(&mut sim, 300);
    assert!(sim.map().tree(4, 0), "nobody can reach the tree");
    assert_eq!(sim.jobs().len(), 1, "the job waits on the board");
    // The same job in the colonist region is given.
    let mut open = sim_from_ascii("c.#.T\n.....\n..#..", 1).unwrap();
    open.step(&[designate(JobKind::Cut, 4, 0)]);
    let id = ids(&open)[0];
    assert!(open.colonist(id).unwrap().job.is_some());
}

#[test]
fn s3_mining_needs_a_floor_neighbor_in_the_colonist_region() {
    let mut sim = sim_from_ascii("c.#R.\n..###", 1).unwrap();
    let id = ids(&sim)[0];
    sim.step(&[designate(JobKind::Mine, 3, 0)]);
    assert_eq!(sim.pick_job(id), None, "(4,0) is in another region");
    assert!(sim.colonist(id).unwrap().job.is_none());
}

#[test]
fn s3_cancel_removes_the_designation_and_frees_the_colonist() {
    let mut sim = sim_from_ascii("c..........T", 1).unwrap();
    let id = ids(&sim)[0];
    sim.step(&[designate(JobKind::Cut, 11, 0)]);
    assert!(sim.colonist(id).unwrap().job.is_some());
    run(&mut sim, 20);
    sim.step(&[Command::CancelDesignation { x: 11, y: 0 }]);
    run(&mut sim, 5);
    assert!(sim.colonist(id).unwrap().job.is_none());
    assert!(sim.jobs().is_empty());
    assert!(sim.designation(11, 0).is_none());
    assert!(sim.map().tree(11, 0));
    assert!(
        sim.drain_job_events()
            .iter()
            .any(|e| matches!(e, JobEvent::Aborted { .. }))
    );
}

#[test]
fn s3_removing_the_stockpile_while_carrying_keeps_the_item() {
    let mut sim = sim_from_ascii("c.........", 1).unwrap();
    let log = log_material(&sim);
    sim.debug_spawn_item(log, 4, 1, 0);
    sim.step(&[
        zone(9, 0, 9, 0, ItemFilter::All),
        designate(JobKind::Haul, 1, 0),
    ]);
    let id = ids(&sim)[0];
    for _ in 0..40 {
        sim.step(&[]);
    }
    assert!(sim.colonist(id).unwrap().carrying.is_some(), "on the way");
    let sp = sim.stockpiles().iter().next().unwrap().0;
    sim.step(&[Command::RemoveStockpile { id: sp }]);
    // The colonist still walks to the old destination and drops the item there.
    run(&mut sim, 400);
    assert_eq!(sim.items().total_units(log), 4);
    assert!(sim.colonist(id).unwrap().carrying.is_none());
}

#[test]
fn r010_skill_level_4_gives_work_speed_120_percent_with_a_why_chain() {
    let mut sim = sim_from_ascii("c.T", 1).unwrap();
    let id = ids(&sim)[0];
    sim.set_skill_xp(id, Skill::Woodcutting, 4_000);
    assert_eq!(sim.skill_level(id, Skill::Woodcutting), Some(4));
    let speed = sim.work_speed(id, JobKind::Cut).unwrap();
    assert_eq!(speed.total_pct, 120);
    assert_eq!(
        speed.total_pct,
        speed.parts.iter().map(|p| p.pct).sum::<i32>()
    );
    assert_eq!(
        sim.work_speed(id, JobKind::Mine).unwrap().total_pct,
        100,
        "another skill"
    );
    assert_eq!(sim.work_speed(id, JobKind::Haul).unwrap().total_pct, 100);
}

#[test]
fn r010_a_faster_colonist_finishes_the_work_sooner_and_gains_xp() {
    let work = |xp: u32| -> (u64, u32) {
        let mut sim = sim_from_ascii("cT", 1).unwrap();
        let id = ids(&sim)[0];
        sim.set_skill_xp(id, Skill::Woodcutting, xp);
        sim.step(&[designate(JobKind::Cut, 1, 0)]);
        let mut guard = 0;
        while sim.map().tree(1, 0) && guard < 2_000 {
            sim.step(&[]);
            guard += 1;
        }
        (
            sim.tick(),
            sim.colonist(id).unwrap().skill_xp[Skill::Woodcutting.index()],
        )
    };
    let (slow, xp0) = work(0);
    let (fast, xp4) = work(4_000);
    // 300 ticks at 100% and 250 ticks at 120%, after about 10 ticks of walking.
    assert!(slow > fast, "{slow} vs {fast}");
    assert!((slow as i64 - 320).abs() <= 15, "{slow}");
    assert!((fast as i64 - 270).abs() <= 15, "{fast}");
    assert_eq!(xp0, 100, "xp per completed job");
    assert_eq!(xp4, 4_100);
}

// ---- invariants over seeded random scenarios ----

fn random_scenario(seed: u64) -> (Sim, Pcg32) {
    let mut rng = Pcg32::from_seed(seed.wrapping_mul(7919).wrapping_add(13));
    let (w, h) = (14usize, 9usize);
    let mut rows = Vec::new();
    for y in 0..h {
        let mut row = String::new();
        for x in 0..w {
            let ch = if (x, y) == (0, 0) {
                'c'
            } else {
                match rng.below(100) {
                    0..=59 => '.',
                    60..=74 => 'T',
                    75..=86 => 'R',
                    87..=94 => '#',
                    _ => '~',
                }
            };
            row.push(ch);
        }
        rows.push(row);
    }
    // More colonists on open tiles.
    let mut placed = 0;
    for _ in 0..40 {
        if placed == 3 {
            break;
        }
        let (x, y) = (rng.below(w as u32) as usize, rng.below(h as u32) as usize);
        if rows[y].as_bytes()[x] == b'.' {
            rows[y].replace_range(x..=x, "c");
            placed += 1;
        }
    }
    (sim_from_ascii(&rows.join("\n"), seed).unwrap(), rng)
}

fn random_commands(sim: &Sim, rng: &mut Pcg32, tick: u32) -> Vec<Command> {
    let (w, h) = (sim.map().width(), sim.map().height());
    let mut cmds = Vec::new();
    let tile = |rng: &mut Pcg32| (rng.below(w) as u16, rng.below(h) as u16);
    if tick == 0 {
        for _ in 0..rng.below(3) + 1 {
            let (a, b) = (tile(rng), tile(rng));
            cmds.push(zone(a.0, a.1, b.0, b.1, ItemFilter::All));
        }
        for &id in &ids(sim) {
            for kind in JobKind::ALL {
                if rng.chance(1, 4) {
                    cmds.push(set_priority(id, kind, rng.below(5) as u8));
                }
            }
        }
        for _ in 0..12 {
            let (x, y) = tile(rng);
            let kind = if rng.chance(1, 2) {
                JobKind::Cut
            } else {
                JobKind::Mine
            };
            cmds.push(designate(kind, x, y));
        }
    }
    if tick == 60 || tick == 200 {
        for _ in 0..4 {
            let (x, y) = tile(rng);
            match rng.below(3) {
                0 => cmds.push(Command::CancelDesignation { x, y }),
                1 => cmds.push(designate(JobKind::Haul, x, y)),
                _ => cmds.push(designate(JobKind::Cut, x, y)),
            }
        }
        if let Some((id, _)) = sim.stockpiles().iter().next()
            && rng.chance(1, 3)
        {
            cmds.push(Command::RemoveStockpile { id });
        }
    }
    cmds
}

const CASE_TICKS: u32 = 900;

fn run_random_case(seed: u64, ticks: u32) -> Sim {
    let (mut sim, mut rng) = random_scenario(seed);
    let mut units = sim.items().all_units();
    for tick in 0..ticks {
        let cmds = random_commands(&sim, &mut rng, tick);
        sim.step(&cmds);
        if let Err(e) = sim.check_invariants() {
            panic!("seed {seed} tick {tick}: {e}");
        }
        // A reservation check that does not use the library check.
        let mut reservers: Vec<_> = sim
            .items()
            .iter()
            .filter_map(|(_, item)| item.reserved_by)
            .collect();
        let n = reservers.len();
        reservers.sort();
        reservers.dedup();
        assert_eq!(
            n,
            reservers.len(),
            "seed {seed}: a colonist reserves two items"
        );
        let mut dests: Vec<_> = sim.jobs().dest_reservations().map(|(t, _)| *t).collect();
        let m = dests.len();
        dests.sort();
        dests.dedup();
        assert_eq!(m, dests.len());
        // Units change only when a cut or mine job completes.
        let produced = sim
            .drain_job_events()
            .iter()
            .any(|e| matches!(e, JobEvent::Completed { kind, .. } if *kind != JobKind::Haul));
        let now = sim.items().all_units();
        if produced {
            assert!(now > units, "seed {seed} tick {tick}");
        } else {
            assert_eq!(
                now, units,
                "seed {seed} tick {tick}: units changed without a spawn"
            );
        }
        units = now;
    }
    sim
}

#[test]
fn s3_no_item_is_reserved_by_two_colonists_in_1000_seeded_cases() {
    let mut worked = 0;
    for seed in 0..1_000 {
        let sim = run_random_case(seed, CASE_TICKS);
        if !sim.items().is_empty() {
            worked += 1;
        }
    }
    assert!(worked > 300, "the random cases must do real work: {worked}");
}

#[test]
fn s3_random_cases_are_deterministic_and_survive_a_save_in_the_middle() {
    for seed in [3u64, 17, 99] {
        let a = run_random_case(seed, CASE_TICKS);
        let b = run_random_case(seed, CASE_TICKS);
        assert_eq!(a.state_hash(), b.state_hash());
        let (mut x, mut rng_x) = random_scenario(seed);
        let (mut y, mut rng_y) = random_scenario(seed);
        for tick in 0..450 {
            let cx = random_commands(&x, &mut rng_x, tick);
            let cy = random_commands(&y, &mut rng_y, tick);
            x.step(&cx);
            y.step(&cy);
        }
        let mut loaded = save::from_bytes(&save::to_bytes(&y)).unwrap();
        assert_eq!(loaded.state_hash(), x.state_hash());
        loaded.check_invariants().unwrap();
        for tick in 450..CASE_TICKS {
            let cx = random_commands(&x, &mut rng_x, tick);
            let cy = random_commands(&loaded, &mut rng_y, tick);
            assert_eq!(cx, cy);
            x.step(&cx);
            loaded.step(&cy);
        }
        assert_eq!(x.state_hash(), loaded.state_hash(), "seed {seed}");
    }
}

#[test]
fn s3_seven_colonists_clear_many_trees_and_stock_all_logs() {
    let mut rows = Vec::new();
    for y in 0..20 {
        let mut row = String::new();
        for x in 0..30 {
            row.push(if y == 0 && x < 7 {
                'c'
            } else if (x + y) % 3 == 0 && x > 8 && y > 3 {
                'T'
            } else {
                '.'
            });
        }
        rows.push(row);
    }
    let mut sim = sim_from_ascii(&rows.join("\n"), 8).unwrap();
    let mut cmds = vec![zone(0, 2, 7, 3, ItemFilter::All)];
    let mut trees = 0u64;
    for y in 0..20u16 {
        for x in 0..30u16 {
            if sim.map().tree(i32::from(x), i32::from(y)) && trees < 30 {
                cmds.push(designate(JobKind::Cut, x, y));
                trees += 1;
            }
        }
    }
    sim.step(&cmds);
    let log = log_material(&sim);
    let mut done = false;
    for _ in 0..30_000 {
        sim.step(&[]);
        if sim.items().total_units(log) == trees * 4
            && sim.items().iter().all(
                |(_, i)| matches!(i.loc, ItemLoc::Tile(x, y) if x <= 7 && (2..=3).contains(&y)),
            )
            && sim.jobs().is_empty()
        {
            done = true;
            break;
        }
    }
    sim.check_invariants().unwrap();
    assert!(done, "all logs are in the stockpile");
    let workers = sim
        .colonists()
        .iter()
        .filter(|(_, c)| c.skill_xp[Skill::Woodcutting.index()] > 0)
        .count();
    assert!(workers > 1, "work is shared");
}

#[test]
fn s3_item_outside_the_filter_is_not_hauled_and_no_stockpile_means_no_haul_job() {
    let mut sim = sim_from_ascii("c.......", 1).unwrap();
    let log = log_material(&sim);
    let stone = stone_material(&sim);
    sim.debug_spawn_item(log, 4, 3, 0);
    sim.step(&[designate(JobKind::Haul, 3, 0)]);
    assert!(sim.jobs().is_empty(), "no stockpile: no haul job");
    sim.step(&[zone(6, 0, 7, 0, ItemFilter::only(vec![stone]))]);
    run(&mut sim, 300);
    assert!(sim.jobs().is_empty(), "the zone does not accept logs");
    let (_, item) = sim.items().iter().next().unwrap();
    assert_eq!(item.loc, ItemLoc::Tile(3, 0));
    sim.step(&[zone(4, 0, 5, 0, ItemFilter::only(vec![log]))]);
    assert_eq!(sim.jobs().len(), 1, "an accepting zone makes the job");
    run(&mut sim, 400);
    let (_, item) = sim.items().iter().next().unwrap();
    assert_eq!(item.loc, ItemLoc::Tile(4, 0));
}
