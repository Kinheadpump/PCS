//! Scenario and invariant tests for construction, deconstruction, workshops, and rooms
//! (Sprint 4).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use base::Pcg32;
use sim::room::BorderKind;
use sim::rules::BUILTIN_TEXTS;
use sim::scenario::sim_from_ascii;
use sim::{
    BeautyCause, BuildLayer, ColonistId, Command, Event, ItemFilter, JobEvent, JobKind, Reject,
    Sim, SimRules, Skill, save,
};
use std::sync::Arc;

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
        sim.check_invariants().unwrap();
    }
}

fn place(sim: &Sim, building: &str, x: u16, y: u16, material: &str) -> Command {
    Command::PlaceBlueprint {
        building: bld(sim, building),
        x,
        y,
        material: mat(sim, material),
    }
}

fn open_map(w: usize, h: usize, colonists: &[(usize, usize)]) -> Sim {
    let mut rows = vec![vec!['.'; w]; h];
    for &(x, y) in colonists {
        rows[y][x] = 'c';
    }
    let text: Vec<String> = rows.iter().map(|r| r.iter().collect()).collect();
    sim_from_ascii(&text.join("\n"), 1).unwrap()
}

/// A room with the interior x 3..=5, y 3..=5. The ring of granite walls is at x 2..=6,
/// y 2..=6. The door is at (4, 2). `skip` leaves out one wall tile. The roof covers the
/// interior only. The colonist stands at (0, 0).
fn walled_room(skip: Option<(u16, u16)>, roof: bool) -> Sim {
    let mut sim = open_map(9, 9, &[(0, 0)]);
    let (granite, plank, log) = (mat(&sim, "granite"), mat(&sim, "plank"), mat(&sim, "log"));
    let (wall, door, roof_b) = (bld(&sim, "wall"), bld(&sim, "door"), bld(&sim, "roof"));
    for i in 2..=6u16 {
        for (x, y) in [(i, 2), (i, 6), (2, i), (6, i)] {
            if Some((x, y)) == skip || sim.buildings().object_at(x, y).is_some() {
                continue;
            }
            if (x, y) == (4, 2) {
                assert!(sim.debug_build(door, x, y, plank));
            } else {
                assert!(sim.debug_build(wall, x, y, granite));
            }
        }
    }
    if roof {
        for y in 3..=5 {
            for x in 3..=5 {
                assert!(sim.debug_build(roof_b, x, y, log));
            }
        }
    }
    sim.step(&[]);
    sim
}

// ---- rooms ----

#[test]
fn r020_closed_area_with_walls_door_and_roof_is_one_room() {
    let sim = walled_room(None, true);
    assert_eq!(sim.rooms().len(), 1);
    let room = sim.room_at(4, 4).unwrap();
    assert_eq!(room.len(), 9);
    assert_eq!(room.border.len(), 12);
    assert!(
        sim.room_at(2, 2).is_none(),
        "a wall tile is not in the room"
    );
    assert!(sim.room_at(0, 0).is_none());
    assert_eq!(
        room.border
            .iter()
            .filter(|b| b.kind == BorderKind::Door)
            .count(),
        1
    );
}

#[test]
fn r020_one_missing_wall_tile_is_not_a_room() {
    let sim = walled_room(Some((6, 4)), true);
    assert!(sim.rooms().is_empty());
    assert!(sim.room_at(4, 4).is_none());
}

#[test]
fn r020_an_area_without_a_roof_on_each_tile_is_not_a_room() {
    let sim = walled_room(None, false);
    assert!(sim.rooms().is_empty());
    let mut sim = walled_room(None, true);
    assert_eq!(sim.rooms().len(), 1);
    // Take one roof tile away: the room is gone after the next detection.
    sim.debug_remove_building(4, 4, BuildLayer::Roof);
    sim.step(&[]);
    assert!(sim.rooms().is_empty());
}

#[test]
fn r020_a_room_has_at_most_max_tiles_tiles() {
    let t = BUILTIN_TEXTS;
    let make = |max: u32| {
        let rooms = t
            .rooms
            .replace("max_tiles: 400", &format!("max_tiles: {max}"));
        let rules =
            SimRules::load_texts(t.materials, t.jobs, t.buildings, t.recipes, &rooms).unwrap();
        let mut sim = walled_room(None, true);
        sim.debug_set_rules(Arc::new(rules));
        sim.step(&[]);
        sim
    };
    assert_eq!(make(9).rooms().len(), 1);
    assert!(make(8).rooms().is_empty());
}

#[test]
fn r020_a_roof_needs_a_wall_or_rock_within_6_tiles() {
    let mut sim = open_map(20, 3, &[(0, 1)]);
    let granite = mat(&sim, "granite");
    let wall = bld(&sim, "wall");
    assert!(sim.debug_build(wall, 0, 0, granite));
    sim.drain_job_events();
    let roof = |x| place(&sim, "roof", x, 1, "log");
    sim.step(&[roof(6), roof(7)]);
    let rejected: Vec<_> = sim
        .drain_job_events()
        .into_iter()
        .filter_map(|e| match e {
            JobEvent::Rejected(r) => Some(r),
            _ => None,
        })
        .collect();
    assert_eq!(
        rejected,
        vec![Reject::NoRoofSupport],
        "x = 7 is 7 tiles away"
    );
    assert_eq!(sim.buildings().blueprints().count(), 1);
    assert!(sim.roof_supported(6, 1) && !sim.roof_supported(7, 1));
}

#[test]
fn r020_room_detection_runs_only_in_the_tick_after_a_structural_change() {
    let mut sim = open_map(9, 9, &[(0, 0)]);
    assert_eq!(sim.room_detections(), 0);
    run(&mut sim, 5);
    assert_eq!(sim.room_detections(), 0, "no change, no detection");
    let (granite, wall) = (mat(&sim, "granite"), bld(&sim, "wall"));
    assert!(sim.debug_build(wall, 4, 4, granite));
    assert!(sim.rooms_pending());
    assert_eq!(sim.room_detections(), 0, "not in the tick of the change");
    sim.step(&[]);
    assert_eq!(sim.room_detections(), 1);
    assert!(!sim.rooms_pending());
    run(&mut sim, 10);
    assert_eq!(sim.room_detections(), 1);
    assert!(sim.set_wall(1, 1, true));
    run(&mut sim, 3);
    assert_eq!(sim.room_detections(), 2);
    // Two changes before one tick are one detection.
    assert!(sim.debug_build(wall, 5, 5, granite));
    assert!(sim.debug_build(wall, 6, 6, granite));
    run(&mut sim, 3);
    assert_eq!(sim.room_detections(), 3);
}

#[test]
fn r020_a_finished_construction_job_starts_the_detection_in_the_next_tick() {
    let mut sim = open_map(9, 3, &[(0, 1)]);
    let granite = mat(&sim, "granite");
    sim.debug_spawn_item(granite, 4, 1, 1);
    sim.step(&[place(&sim, "wall", 5, 1, "granite")]);
    let mut seen_pending = false;
    for _ in 0..3_000 {
        let before = sim.room_detections();
        sim.step(&[]);
        if sim.buildings().object_at(5, 1).is_some() && !seen_pending {
            seen_pending = true;
            assert_eq!(
                sim.room_detections(),
                before,
                "not in the tick of the change"
            );
            assert!(sim.rooms_pending());
            sim.step(&[]);
            assert_eq!(sim.room_detections(), before + 1);
            break;
        }
    }
    assert!(seen_pending, "the wall was built");
}

#[test]
fn r021_room_insulation_is_the_mean_of_the_border_tiles_with_a_why_chain() {
    let sim = walled_room(None, true);
    let ins = sim.room_insulation(4, 4).unwrap();
    let granite = mat(&sim, "granite");
    let plank = mat(&sim, "plank");
    // 11 granite wall tiles at 60 and 1 plank door at 48: 708 / 12 = 59.
    assert_eq!(ins.total, 59);
    assert_eq!(ins.parts.len(), 2);
    let wall = ins
        .parts
        .iter()
        .find(|p| p.kind == BorderKind::Wall)
        .unwrap();
    assert_eq!((wall.material, wall.tiles, wall.each), (granite, 11, 60));
    let door = ins
        .parts
        .iter()
        .find(|p| p.kind == BorderKind::Door)
        .unwrap();
    assert_eq!((door.material, door.tiles, door.each), (plank, 1, 48));
    let sum: u32 = ins.parts.iter().map(|p| p.tiles * p.each).sum();
    let tiles: u32 = ins.parts.iter().map(|p| p.tiles).sum();
    assert_eq!(ins.total, sum / tiles);
}

#[test]
fn r021_natural_rock_and_map_walls_count_as_border_tiles() {
    // A room in a rock corner: the rock border has the insulation of granite (60).
    let mut sim = sim_from_ascii("RRRRR\nR...R\nRRRRR\nc....", 1).unwrap();
    let (log, roof) = (mat(&sim, "log"), bld(&sim, "roof"));
    for x in 1..=3 {
        assert!(sim.debug_build(roof, x, 1, log));
    }
    sim.step(&[]);
    let room = sim.room_at(2, 1).unwrap();
    assert_eq!(room.len(), 3);
    assert_eq!(room.rock_border, 8);
    let ins = &room.insulation;
    assert_eq!(ins.total, 60);
    assert_eq!(ins.parts[0].kind, BorderKind::NaturalRock);

    let mut sim = sim_from_ascii("#####\n#...#\n#####\nc....", 1).unwrap();
    for x in 1..=3 {
        assert!(sim.debug_build(roof, x, 1, log));
    }
    sim.step(&[]);
    let room = sim.room_at(2, 1).unwrap();
    assert_eq!(room.insulation.parts[0].kind, BorderKind::MapWall);
    assert_eq!(room.insulation.total, 60, "a bare wall tile is granite");
}

#[test]
fn room_beauty_is_a_sum_of_named_parts() {
    let mut sim = walled_room(None, true);
    let (floor, bed) = (bld(&sim, "floor"), bld(&sim, "bed"));
    let (granite, plank) = (mat(&sim, "granite"), mat(&sim, "plank"));
    for y in 3..=5 {
        for x in 3..=5 {
            assert!(sim.debug_build(floor, x, y, granite));
        }
    }
    assert!(sim.debug_build(bed, 3, 3, plank));
    sim.step(&[]);
    let beauty = sim.room_beauty(4, 4).unwrap();
    let total: i32 = beauty.parts.iter().map(|p| p.tiles as i32 * p.each).sum();
    assert_eq!(beauty.total, total);
    let floor_part = beauty
        .parts
        .iter()
        .find(|p| p.cause == BeautyCause::Floor)
        .unwrap();
    // Floor building beauty 1 plus granite beauty 6, on 9 tiles.
    assert_eq!((floor_part.tiles, floor_part.each), (9, 7));
    let furniture = beauty
        .parts
        .iter()
        .find(|p| p.cause == BeautyCause::Furniture)
        .unwrap();
    // Bed beauty 2 plus plank beauty 4.
    assert_eq!((furniture.tiles, furniture.each), (1, 6));
    // 63 floor + 6 bed + 66 granite walls (0 + 6) + 5 plank door (1 + 4).
    assert_eq!(beauty.total, 140);
    assert_eq!(beauty.per_tile, 15);
}

// ---- construction ----

#[test]
fn granite_wall_uses_granite_items_and_has_material_granite() {
    let mut sim = open_map(9, 3, &[(0, 1)]);
    let granite = mat(&sim, "granite");
    sim.debug_spawn_item(granite, 6, 1, 1);
    sim.step(&[place(&sim, "wall", 5, 1, "granite")]);
    assert_eq!(sim.buildings().blueprints().count(), 1);
    assert!(sim.map().passable(5, 1), "a blueprint does not block");
    for _ in 0..3_000 {
        sim.step(&[]);
        sim.check_invariants().unwrap();
        if sim.buildings().object_at(5, 1).is_some() {
            break;
        }
    }
    let wall = sim.buildings().object_at(5, 1).expect("the wall is built");
    assert_eq!(wall.material, granite);
    assert_eq!(wall.def, bld(&sim, "wall"));
    assert_eq!(wall.units, 4);
    assert_eq!(sim.buildings().blueprints().count(), 0);
    assert!(!sim.map().passable(5, 1), "the wall blocks");
    assert_eq!(sim.items().total_units(granite), 2, "6 - 4");
    assert_eq!(sim.material_in_construction(granite), 4);
    assert!(sim.jobs().is_empty());
}

#[test]
fn a_wall_that_splits_a_corridor_splits_the_regions_and_the_paths() {
    let mut sim = sim_from_ascii("c.....", 1).unwrap();
    let id = ids(&sim)[0];
    assert_eq!(sim.regions().id_at(0, 0), sim.regions().id_at(5, 0));
    sim.step(&[Command::MoveColonist { id, x: 5, y: 0 }]);
    assert!(sim.colonist(id).unwrap().path.contains(&(3, 0)));
    let (granite, wall) = (mat(&sim, "granite"), bld(&sim, "wall"));
    assert!(sim.debug_build(wall, 3, 0, granite));
    assert_ne!(sim.regions().id_at(0, 0), sim.regions().id_at(5, 0));
    assert_eq!(sim.regions().id_at(3, 0), 0);
    run(&mut sim, 300);
    let c = sim.colonist(id).unwrap();
    assert!(c.tile().0 < 3, "the colonist stays on its side");
    assert!(!sim.map().passable(3, 0));
}

#[test]
fn a_door_does_not_block_walking_but_closes_a_room() {
    let sim = walled_room(None, true);
    assert!(sim.map().passable(4, 2), "the door tile is passable");
    assert_eq!(
        sim.regions().id_at(4, 2),
        sim.regions().id_at(4, 4),
        "colonists can pass the door"
    );
    assert_eq!(sim.regions().id_at(4, 2), sim.regions().id_at(0, 0));
}

#[test]
fn building_on_a_colonist_moves_the_colonist_off_the_tile() {
    let mut sim = sim_from_ascii("c..", 1).unwrap();
    let id = ids(&sim)[0];
    let (granite, wall) = (mat(&sim, "granite"), bld(&sim, "wall"));
    assert!(sim.debug_build(wall, 0, 0, granite));
    let c = sim.colonist(id).unwrap();
    assert_ne!(c.tile(), (0, 0));
    assert!(sim.map().passable(i32::from(c.x), i32::from(c.y)));
    sim.check_invariants().unwrap();
}

#[test]
fn blueprints_are_checked_when_the_command_arrives() {
    let mut sim = sim_from_ascii("c.T~R\n.....", 1).unwrap();
    let iron_ore = mat(&sim, "iron_ore");
    let wall = bld(&sim, "wall");
    sim.step(&[
        Command::PlaceBlueprint {
            building: wall,
            x: 1,
            y: 0,
            material: iron_ore,
        },
        place(&sim, "wall", 2, 0, "granite"),
        place(&sim, "wall", 3, 0, "granite"),
        place(&sim, "wall", 4, 0, "granite"),
        place(&sim, "wall", 9, 9, "granite"),
        Command::PlaceBlueprint {
            building: 99,
            x: 0,
            y: 1,
            material: 0,
        },
        place(&sim, "wall", 1, 1, "granite"),
        place(&sim, "wall", 1, 1, "granite"),
    ]);
    let rejected: Vec<Reject> = sim
        .drain_job_events()
        .into_iter()
        .filter_map(|e| match e {
            JobEvent::Rejected(r) => Some(r),
            _ => None,
        })
        .collect();
    assert_eq!(
        rejected,
        vec![
            Reject::BadMaterial,
            Reject::TileBlocked,
            Reject::TileBlocked,
            Reject::TileBlocked,
            Reject::OutOfMap,
            Reject::UnknownBuilding,
            Reject::TileOccupied,
        ]
    );
    assert_eq!(sim.buildings().blueprints().count(), 1);
    assert_eq!(sim.jobs().len(), 1);
}

#[test]
fn a_floor_and_an_object_can_share_a_tile() {
    let mut sim = open_map(5, 1, &[(0, 0)]);
    let granite = mat(&sim, "granite");
    sim.debug_spawn_item(granite, 10, 1, 0);
    sim.step(&[
        place(&sim, "floor", 3, 0, "granite"),
        place(&sim, "wall", 3, 0, "granite"),
    ]);
    assert!(sim.drain_job_events().is_empty());
    run(&mut sim, 4_000);
    assert!(sim.buildings().floor_at(3, 0).is_some());
    assert!(sim.buildings().object_at(3, 0).is_some());
    assert_eq!(sim.items().total_units(granite), 10 - 4 - 1);
}

#[test]
fn construction_takes_material_from_a_stockpile() {
    let mut sim = open_map(10, 4, &[(0, 0)]);
    let granite = mat(&sim, "granite");
    sim.step(&[Command::CreateStockpile {
        x0: 8,
        y0: 3,
        x1: 9,
        y1: 3,
        filter: ItemFilter::All,
    }]);
    sim.debug_spawn_item(granite, 5, 8, 3);
    sim.step(&[place(&sim, "wall", 2, 0, "granite")]);
    run(&mut sim, 4_000);
    let wall = sim.buildings().object_at(2, 0).unwrap();
    assert_eq!(wall.material, granite);
    let rest: u32 = sim
        .items()
        .iter()
        .filter(|(_, i)| i.material == granite)
        .map(|(_, i)| i.count)
        .sum();
    assert_eq!(rest, 1);
}

#[test]
fn construction_waits_for_material_and_starts_when_it_arrives() {
    let mut sim = open_map(8, 1, &[(0, 0)]);
    let granite = mat(&sim, "granite");
    sim.step(&[place(&sim, "wall", 4, 0, "granite")]);
    run(&mut sim, 200);
    assert!(sim.buildings().object_at(4, 0).is_none());
    assert_eq!(sim.buildings().blueprints().count(), 1);
    sim.debug_spawn_item(granite, 4, 7, 0);
    // A new item does not wake the board; the retry timer does.
    run(&mut sim, 3_000);
    assert!(sim.buildings().object_at(4, 0).is_some());
}

#[test]
fn a_half_filled_blueprint_keeps_its_delivered_units_when_cancelled() {
    let mut sim = open_map(8, 1, &[(0, 0)]);
    let granite = mat(&sim, "granite");
    sim.debug_spawn_item(granite, 2, 1, 0);
    sim.step(&[place(&sim, "wall", 5, 0, "granite")]);
    run(&mut sim, 400);
    let delivered = sim
        .buildings()
        .blueprint(5, 0, BuildLayer::Object)
        .unwrap()
        .delivered;
    assert_eq!(delivered, 2, "the colonist brought all it could");
    assert_eq!(sim.material_in_construction(granite), 2);
    sim.step(&[Command::CancelBlueprint {
        x: 5,
        y: 0,
        layer: BuildLayer::Object,
    }]);
    run(&mut sim, 10);
    assert_eq!(sim.buildings().blueprints().count(), 0);
    assert_eq!(sim.material_in_construction(granite), 0);
    assert_eq!(sim.items().total_units(granite), 2);
    assert!(sim.jobs().iter().all(|(_, j)| j.kind != JobKind::Construct));
}

#[test]
fn construction_trains_the_construction_skill() {
    let mut sim = open_map(6, 1, &[(0, 0)]);
    let id = ids(&sim)[0];
    let granite = mat(&sim, "granite");
    sim.debug_spawn_item(granite, 1, 1, 0);
    sim.step(&[place(&sim, "floor", 3, 0, "granite")]);
    run(&mut sim, 1_500);
    assert!(sim.buildings().floor_at(3, 0).is_some());
    let xp = sim.colonist(id).unwrap().skill_xp[Skill::Construction.index()];
    assert_eq!(xp, sim.rules().jobs.xp_per_job);
    assert_eq!(sim.priority(id, JobKind::Construct), Some(3));
    let speed = sim.work_speed(id, JobKind::Construct).unwrap();
    assert_eq!(speed.parts.len(), 2, "base plus the construction level");
}

#[test]
fn priority_off_stops_construction() {
    let mut sim = open_map(6, 1, &[(0, 0)]);
    sim.debug_set_needs_enabled(false);
    let id = ids(&sim)[0];
    let granite = mat(&sim, "granite");
    sim.debug_spawn_item(granite, 1, 1, 0);
    sim.step(&[Command::SetPriority {
        colonist: id,
        job: JobKind::Construct,
        priority: 0,
    }]);
    sim.step(&[place(&sim, "floor", 3, 0, "granite")]);
    run(&mut sim, 1_000);
    assert!(sim.buildings().floor_at(3, 0).is_none());
    sim.step(&[Command::SetPriority {
        colonist: id,
        job: JobKind::Construct,
        priority: 2,
    }]);
    run(&mut sim, 1_500);
    assert!(sim.buildings().floor_at(3, 0).is_some());
}

// ---- deconstruction ----

#[test]
fn deconstruction_gives_back_part_of_the_material() {
    let mut sim = open_map(8, 1, &[(0, 0)]);
    let granite = mat(&sim, "granite");
    let wall = bld(&sim, "wall");
    assert!(sim.debug_build(wall, 4, 0, granite));
    assert_eq!(sim.items().total_units(granite), 0);
    sim.step(&[Command::Deconstruct {
        x: 4,
        y: 0,
        layer: BuildLayer::Object,
    }]);
    // A second order for the same building is refused.
    sim.step(&[Command::Deconstruct {
        x: 4,
        y: 0,
        layer: BuildLayer::Object,
    }]);
    assert!(
        sim.drain_job_events()
            .contains(&JobEvent::Rejected(Reject::AlreadyOrdered))
    );
    run(&mut sim, 2_000);
    assert!(sim.buildings().object_at(4, 0).is_none());
    assert!(sim.map().passable(4, 0), "the tile is free again");
    assert_eq!(sim.items().total_units(granite), 2, "50 percent of 4");
    assert_eq!(sim.material_in_construction(granite), 0);
    assert!(sim.jobs().is_empty());
}

#[test]
fn deconstructing_a_wall_opens_the_room() {
    let mut sim = walled_room(None, true);
    assert_eq!(sim.rooms().len(), 1);
    sim.step(&[Command::Deconstruct {
        x: 6,
        y: 4,
        layer: BuildLayer::Object,
    }]);
    run(&mut sim, 3_000);
    assert!(sim.buildings().object_at(6, 4).is_none());
    assert!(sim.rooms().is_empty());
}

// ---- workshop ----

#[test]
fn a_workshop_makes_planks_from_logs() {
    let mut sim = open_map(8, 3, &[(0, 1)]);
    let (granite, log, plank) = (mat(&sim, "granite"), mat(&sim, "log"), mat(&sim, "plank"));
    let workshop = bld(&sim, "workshop");
    assert!(sim.debug_build(workshop, 4, 1, granite));
    sim.debug_spawn_item(log, 4, 1, 0);
    let recipe = sim.rules().recipe_index("plank").unwrap();
    sim.step(&[Command::OrderCraft {
        x: 4,
        y: 1,
        recipe,
        count: 2,
    }]);
    let mut crafted = 0;
    for _ in 0..4_000 {
        sim.step(&[]);
        sim.check_invariants().unwrap();
        crafted += sim
            .drain_events()
            .iter()
            .filter(|e| matches!(e, Event::Crafted { .. }))
            .count();
    }
    assert_eq!(crafted, 2);
    assert_eq!(sim.items().total_units(plank), 4);
    assert_eq!(
        sim.items().total_units(log) + sim.material_in_construction(log),
        2,
        "2 of 4 logs are left"
    );
    assert!(sim.buildings().object_at(4, 1).unwrap().orders.is_empty());
    assert!(sim.jobs().is_empty());
}

#[test]
fn a_workshop_makes_firewood_and_rejects_bad_orders() {
    let mut sim = open_map(8, 3, &[(0, 1)]);
    let (granite, log) = (mat(&sim, "granite"), mat(&sim, "log"));
    let firewood = mat(&sim, "firewood");
    let workshop = bld(&sim, "workshop");
    assert!(sim.debug_build(workshop, 4, 1, granite));
    sim.debug_spawn_item(log, 1, 1, 0);
    let recipe = sim.rules().recipe_index("firewood").unwrap();
    sim.step(&[
        Command::OrderCraft {
            x: 4,
            y: 1,
            recipe,
            count: 0,
        },
        Command::OrderCraft {
            x: 1,
            y: 1,
            recipe,
            count: 1,
        },
        Command::OrderCraft {
            x: 4,
            y: 1,
            recipe: 77,
            count: 1,
        },
        Command::OrderCraft {
            x: 4,
            y: 1,
            recipe,
            count: 1,
        },
    ]);
    let rejected: Vec<Reject> = sim
        .drain_job_events()
        .into_iter()
        .filter_map(|e| match e {
            JobEvent::Rejected(r) => Some(r),
            _ => None,
        })
        .collect();
    assert_eq!(
        rejected,
        vec![Reject::BadCount, Reject::NoWorkshop, Reject::UnknownRecipe]
    );
    run(&mut sim, 2_000);
    assert_eq!(sim.items().total_units(firewood), 2);
}

#[test]
fn cancelling_a_craft_order_removes_the_job() {
    let mut sim = open_map(8, 3, &[(0, 1)]);
    let granite = mat(&sim, "granite");
    let workshop = bld(&sim, "workshop");
    assert!(sim.debug_build(workshop, 4, 1, granite));
    let recipe = sim.rules().recipe_index("plank").unwrap();
    sim.step(&[Command::OrderCraft {
        x: 4,
        y: 1,
        recipe,
        count: 3,
    }]);
    assert_eq!(sim.jobs().len(), 1);
    sim.step(&[Command::CancelCraft { x: 4, y: 1, recipe }]);
    assert!(sim.jobs().is_empty());
    assert!(sim.buildings().object_at(4, 1).unwrap().orders.is_empty());
}

// ---- save, hash, invariants ----

#[test]
fn buildings_blueprints_and_bins_are_in_the_hash_and_the_save() {
    let mut sim = open_map(10, 3, &[(0, 1), (1, 1)]);
    let (granite, log) = (mat(&sim, "granite"), mat(&sim, "log"));
    let before = sim.state_hash();
    sim.debug_spawn_item(granite, 12, 2, 1);
    sim.debug_spawn_item(log, 3, 3, 1);
    let workshop = bld(&sim, "workshop");
    assert!(sim.debug_build(workshop, 8, 1, granite));
    let plank = sim.rules().recipe_index("plank").unwrap();
    sim.step(&[
        place(&sim, "wall", 5, 0, "granite"),
        place(&sim, "floor", 6, 2, "granite"),
        Command::OrderCraft {
            x: 8,
            y: 1,
            recipe: plank,
            count: 2,
        },
    ]);
    assert_ne!(sim.state_hash(), before);
    for _ in 0..350 {
        sim.step(&[]);
    }
    let bytes = save::to_bytes(&sim);
    let mut loaded = save::from_bytes(&bytes).unwrap();
    assert_eq!(loaded.state_hash(), sim.state_hash());
    assert_eq!(loaded.rooms(), sim.rooms());
    assert_eq!(loaded.buildings(), sim.buildings());
    for _ in 0..1_500 {
        sim.step(&[]);
        loaded.step(&[]);
        loaded.check_invariants().unwrap();
    }
    assert_eq!(
        loaded.state_hash(),
        sim.state_hash(),
        "same run after a load"
    );
    assert_eq!(save::FORMAT_VERSION, 12);
}

#[test]
fn a_save_with_a_building_outside_the_map_is_refused() {
    let mut sim = open_map(4, 1, &[(0, 0)]);
    let (granite, wall) = (mat(&sim, "granite"), bld(&sim, "wall"));
    assert!(sim.debug_build(wall, 2, 0, granite));
    let bytes = save::to_bytes(&sim);
    assert!(save::from_bytes(&bytes).is_ok());
    // Every one-byte change is an error or a valid state, and never a panic.
    for i in sim::save::HEADER_LEN..bytes.len() {
        let mut bad = bytes.clone();
        bad[i] ^= 0x5A;
        let _ = save::from_bytes(&bad);
    }
}

/// Seeded random construction. Colonists build and cancel; no material appears or
/// vanishes.
fn random_build_run(seed: u64) {
    let mut sim = open_map(16, 16, &[(1, 1), (2, 1), (3, 1)]);
    let mut rng = Pcg32::from_seed(seed.wrapping_mul(7919).wrapping_add(11));
    let (granite, log, plank) = (mat(&sim, "granite"), mat(&sim, "log"), mat(&sim, "plank"));
    let materials = [granite, log];
    let mut supply = [0u64; 2];
    for k in 0..6 {
        let m = materials[k % 2];
        let n = 5 + rng.below(30);
        sim.debug_spawn_item(m, n, rng.below(16) as u16, 8 + rng.below(8) as u16);
        supply[k % 2] += u64::from(n);
    }
    sim.step(&[Command::CreateStockpile {
        x0: 12,
        y0: 12,
        x1: 15,
        y1: 13,
        filter: ItemFilter::All,
    }]);
    let kinds = ["wall", "door", "floor", "roof", "bed", "hearth", "workshop"];
    for tick in 0..2_400u32 {
        let mut commands = Vec::new();
        if tick % 20 == 0 {
            let name = kinds[rng.below(kinds.len() as u32) as usize];
            let building = sim.rules().building_index(name).unwrap();
            let fitting: Vec<u16> = materials
                .iter()
                .copied()
                .filter(|&m| sim.rules().material_fits(building, m))
                .collect();
            if !fitting.is_empty() {
                commands.push(Command::PlaceBlueprint {
                    building,
                    x: rng.below(16) as u16,
                    y: rng.below(8) as u16,
                    material: fitting[rng.below(fitting.len() as u32) as usize],
                });
            }
        }
        if tick % 130 == 7 {
            let planned: Vec<_> = sim
                .buildings()
                .blueprints()
                .map(|(x, y, l, _)| (x, y, l))
                .collect();
            if !planned.is_empty() {
                let (x, y, layer) = planned[rng.below(planned.len() as u32) as usize];
                commands.push(Command::CancelBlueprint { x, y, layer });
            }
        }
        sim.step(&commands);
        sim.check_invariants().unwrap();
        if tick % 40 == 0 {
            for (k, &m) in materials.iter().enumerate() {
                let have = sim.items().total_units(m) + sim.material_in_construction(m);
                assert_eq!(have, supply[k], "seed {seed} tick {tick}: material {m}");
            }
            assert_eq!(sim.items().total_units(plank), 0);
        }
    }
    for (k, &m) in materials.iter().enumerate() {
        let have = sim.items().total_units(m) + sim.material_in_construction(m);
        assert_eq!(have, supply[k], "seed {seed}: final material {m}");
    }
    assert!(
        !sim.buildings().is_empty(),
        "seed {seed}: something was built"
    );
}

#[test]
fn construction_neither_makes_nor_destroys_material_seeded() {
    for seed in 1..=8 {
        random_build_run(seed);
    }
}

#[test]
fn a_seeded_construction_run_is_deterministic() {
    let hash = |seed| {
        let mut sim = open_map(12, 6, &[(0, 0), (1, 0)]);
        let granite = mat(&sim, "granite");
        sim.debug_spawn_item(granite, 30, 3, 3);
        let mut rng = Pcg32::from_seed(seed);
        for tick in 0..1_500u32 {
            let cmds = if tick % 50 == 0 {
                vec![place(
                    &sim,
                    "wall",
                    rng.below(12) as u16,
                    rng.below(6) as u16,
                    "granite",
                )]
            } else {
                Vec::new()
            };
            sim.step(&cmds);
        }
        sim.state_hash()
    };
    assert_eq!(hash(5), hash(5));
    assert_ne!(hash(5), hash(6));
}

#[test]
fn a_colonist_on_a_work_tile_survives_a_neighbor_wall_and_the_run_stays_valid() {
    // Two colonists build walls in a tight corridor; nobody gets trapped inside a wall.
    let mut sim = open_map(12, 1, &[(0, 0), (11, 0)]);
    let granite = mat(&sim, "granite");
    sim.debug_spawn_item(granite, 12, 5, 0);
    sim.step(&[
        place(&sim, "wall", 3, 0, "granite"),
        place(&sim, "wall", 8, 0, "granite"),
    ]);
    run(&mut sim, 5_000);
    for (_, c) in sim.colonists().iter() {
        assert!(sim.map().passable(i32::from(c.x), i32::from(c.y)));
    }
}
