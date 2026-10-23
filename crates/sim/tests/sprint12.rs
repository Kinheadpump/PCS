//! Scenario tests for Sprint 12: tools (R-008), knowledge (R-042), and the production chains
//! of `GAME_DESIGN.md` §11.3.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use sim::scenario::sim_from_ascii;
use sim::{Command, JobEvent, Reject, Sim};

fn open_map(w: usize, h: usize, colonists: &[(usize, usize)]) -> String {
    let mut rows = vec![vec!['.'; w]; h];
    for &(x, y) in colonists {
        rows[y][x] = 'c';
    }
    let text: Vec<String> = rows.iter().map(|r| r.iter().collect()).collect();
    text.join("\n")
}

fn mat(sim: &Sim, key: &str) -> u16 {
    sim.rules().material_index(key).unwrap()
}

fn bld(sim: &Sim, key: &str) -> u16 {
    sim.rules().building_index(key).unwrap()
}

fn recipe(sim: &Sim, key: &str) -> u16 {
    sim.rules().recipe_index(key).unwrap()
}

fn rejected(sim: &mut Sim) -> Vec<Reject> {
    sim.drain_job_events()
        .into_iter()
        .filter_map(|e| match e {
            JobEvent::Rejected(r) => Some(r),
            _ => None,
        })
        .collect()
}

fn order(x: u16, y: u16, recipe: u16, count: u32) -> Command {
    Command::OrderCraft {
        x,
        y,
        recipe,
        count,
    }
}

// ---- R-042 knowledge ----

#[test]
fn r042_a_locked_recipe_cannot_be_ordered_until_its_knowledge_is_learned() {
    let mut sim = sim_from_ascii(&open_map(8, 3, &[(0, 1)]), 1).unwrap();
    let (granite, kiln) = (mat(&sim, "granite"), bld(&sim, "kiln"));
    assert!(sim.debug_build(kiln, 4, 1, granite));
    let charcoal = recipe(&sim, "charcoal");
    assert!(!sim.recipe_known(charcoal));
    sim.drain_job_events();
    sim.step(&[order(4, 1, charcoal, 1)]);
    assert_eq!(rejected(&mut sim), vec![Reject::RecipeLocked]);
    assert!(sim.buildings().object_at(4, 1).unwrap().orders.is_empty());
    sim.debug_learn("charcoal");
    assert!(sim.recipe_known(charcoal));
    sim.step(&[order(4, 1, charcoal, 1)]);
    assert!(rejected(&mut sim).is_empty());
    assert_eq!(sim.buildings().object_at(4, 1).unwrap().orders.len(), 1);
}

#[test]
fn r042_a_recipe_without_a_lock_is_known_from_the_start() {
    let sim = sim_from_ascii(&open_map(4, 3, &[(0, 1)]), 1).unwrap();
    for key in [
        "plank",
        "firewood",
        "stone_axe",
        "stone_pick",
        "stone_block",
    ] {
        assert!(sim.recipe_known(recipe(&sim, key)), "{key}");
    }
    assert!(sim.knowledge().next().is_none());
}

// ---- GAME_DESIGN §11.3 production chains ----

/// Build a workshop at (4, 1), put the inputs on its tile, order one run, and run the sim.
/// Return the units of each output material.
fn run_chain(workshop: &str, recipe_key: &str, knowledge: &[&str], inputs: &[(&str, u32)]) -> Sim {
    let mut sim = sim_from_ascii(&open_map(8, 3, &[(0, 1)]), 1).unwrap();
    let granite = mat(&sim, "granite");
    let shop = bld(&sim, workshop);
    assert!(sim.debug_build(shop, 4, 1, granite));
    for key in knowledge {
        sim.debug_learn(key);
    }
    for &(key, count) in inputs {
        let m = mat(&sim, key);
        sim.debug_spawn_item(m, count, 1, 0);
    }
    sim.drain_job_events();
    let recipe = recipe(&sim, recipe_key);
    sim.step(&[order(4, 1, recipe, 1)]);
    assert!(rejected(&mut sim).is_empty(), "{recipe_key} was refused");
    for _ in 0..4_000 {
        sim.step(&[]);
        sim.check_invariants().unwrap();
    }
    sim
}

fn units(sim: &Sim, key: &str) -> u64 {
    sim.items().total_units(mat(sim, key))
}

#[test]
fn chain_log_to_charcoal() {
    let sim = run_chain("kiln", "charcoal", &["charcoal"], &[("log", 2)]);
    assert_eq!((units(&sim, "log"), units(&sim, "charcoal")), (0, 2));
}

#[test]
fn chain_rock_to_stone_block() {
    let sim = run_chain("workshop", "stone_block", &[], &[("granite", 2)]);
    assert_eq!(units(&sim, "stone_block"), 1);
}

#[test]
fn chain_clay_to_brick() {
    let sim = run_chain(
        "kiln",
        "brick",
        &["fired_clay"],
        &[("clay", 2), ("firewood", 1)],
    );
    assert_eq!(units(&sim, "brick"), 2);
}

#[test]
fn chain_iron_ore_and_charcoal_to_iron_bar() {
    let sim = run_chain(
        "smelter",
        "iron_bar",
        &["smelting"],
        &[("iron_ore", 2), ("charcoal", 1)],
    );
    assert_eq!(units(&sim, "iron"), 1);
}

#[test]
fn chain_iron_bar_to_iron_tool() {
    let sim = run_chain(
        "forge",
        "iron_axe",
        &["iron_tools"],
        &[("iron", 2), ("plank", 1)],
    );
    assert_eq!(units(&sim, "iron_axe"), 1);
    let sim = run_chain(
        "forge",
        "iron_pick",
        &["iron_tools"],
        &[("iron", 2), ("plank", 1)],
    );
    assert_eq!(units(&sim, "iron_pick"), 1);
}

#[test]
fn chain_stone_block_and_log_to_stone_tool() {
    let sim = run_chain("forge", "stone_axe", &[], &[("stone_block", 1), ("log", 1)]);
    assert_eq!(units(&sim, "stone_axe"), 1);
}

#[test]
fn chain_crop_and_firewood_to_smoked_food() {
    let sim = run_chain(
        "kitchen",
        "smoked_barley",
        &["smoked_food"],
        &[("barley", 2), ("firewood", 1)],
    );
    assert_eq!(units(&sim, "smoked_food"), 2);
}

#[test]
fn chain_fiber_to_simple_clothes() {
    let sim = run_chain("workshop", "clothes", &[], &[("fiber", 3)]);
    assert_eq!(units(&sim, "simple_clothes"), 1);
}

#[test]
fn chain_log_to_plank_and_to_firewood() {
    let sim = run_chain("workshop", "plank", &[], &[("log", 1)]);
    assert_eq!(units(&sim, "plank"), 2);
    let sim = run_chain("workshop", "firewood", &[], &[("log", 1)]);
    assert_eq!(units(&sim, "firewood"), 2);
}

// ---- R-008 tools ----

use base::content::ToolKind;
use sim::{ColonistId, JobKind};

fn only_colonist(sim: &Sim) -> ColonistId {
    sim.colonists().iter().next().unwrap().0
}

fn tool_of(sim: &Sim, kind: ToolKind) -> Option<sim::Tool> {
    sim.colonists().get(only_colonist(sim)).unwrap().tools[kind.index()]
}

fn run(sim: &mut Sim, ticks: u32) {
    for _ in 0..ticks {
        sim.step(&[]);
        sim.check_invariants().unwrap();
    }
}

fn bare_handed(ascii: &str) -> Sim {
    let mut sim = sim_from_ascii(ascii, 1).unwrap();
    let id = only_colonist(&sim);
    sim.debug_clear_tools(id);
    sim
}

fn designate(kind: JobKind, x: u16, y: u16) -> Command {
    Command::Designate { kind, x, y }
}

#[test]
fn r008_a_cut_job_needs_an_axe() {
    let mut sim = bare_handed("c.T..");
    sim.step(&[designate(JobKind::Cut, 2, 0)]);
    run(&mut sim, 1_500);
    assert!(sim.map().tree(2, 0), "no axe: the tree stands");
}

#[test]
fn r008_a_mine_job_needs_a_pick_and_an_axe_does_not_help() {
    let mut sim = bare_handed("c.#..");
    let axe = mat(&sim, "stone_axe");
    sim.debug_spawn_item(axe, 1, 1, 0);
    sim.step(&[designate(JobKind::Mine, 2, 0)]);
    run(&mut sim, 1_500);
    assert!(
        !sim.map().passable(2, 0),
        "an axe does not mine, the wall stands"
    );
}

#[test]
fn r008_a_colonist_without_a_tool_takes_one_from_the_floor_or_a_stockpile() {
    let mut sim = bare_handed("c.T..");
    let axe = mat(&sim, "stone_axe");
    sim.debug_spawn_item(axe, 1, 4, 0);
    sim.step(&[designate(JobKind::Cut, 2, 0)]);
    run(&mut sim, 1_500);
    assert!(
        !sim.map().tree(2, 0),
        "the colonist fetched the axe and cut"
    );
    let tool = tool_of(&sim, ToolKind::Axe).unwrap();
    assert_eq!(tool.material, axe);
    assert_eq!(sim.items().total_units(axe), 0, "the axe left the floor");
}

#[test]
fn r008_each_completed_job_decreases_durability_by_one() {
    let mut sim = bare_handed("c.T.T");
    let axe = mat(&sim, "stone_axe");
    let id = only_colonist(&sim);
    sim.debug_set_tool(id, axe, 50);
    sim.step(&[designate(JobKind::Cut, 2, 0)]);
    run(&mut sim, 1_000);
    assert!(!sim.map().tree(2, 0));
    assert_eq!(tool_of(&sim, ToolKind::Axe).unwrap().durability, 49);
    sim.step(&[designate(JobKind::Cut, 4, 0)]);
    run(&mut sim, 1_000);
    assert_eq!(tool_of(&sim, ToolKind::Axe).unwrap().durability, 48);
}

#[test]
fn r008_a_tool_breaks_at_zero_durability() {
    let mut sim = bare_handed("c.T..");
    let axe = mat(&sim, "stone_axe");
    let id = only_colonist(&sim);
    sim.debug_set_tool(id, axe, 1);
    sim.step(&[designate(JobKind::Cut, 2, 0)]);
    run(&mut sim, 1_000);
    assert!(!sim.map().tree(2, 0), "the last job still finishes");
    assert!(tool_of(&sim, ToolKind::Axe).is_none(), "the axe broke");
}

#[test]
fn r008_a_new_tool_has_durability_of_hardness_times_ten() {
    let mut sim = bare_handed("c.T..");
    let axe = mat(&sim, "stone_axe");
    sim.debug_spawn_item(axe, 1, 1, 0);
    sim.step(&[designate(JobKind::Cut, 2, 0)]);
    run(&mut sim, 1_000);
    let hardness = u16::try_from(sim.rules().hardness(axe)).unwrap();
    assert_eq!(
        tool_of(&sim, ToolKind::Axe).unwrap().durability,
        hardness * 10 - 1,
        "full durability, minus the one job"
    );
}

#[test]
fn r008_a_dead_colonist_drops_the_tools() {
    let mut sim = bare_handed("c....");
    let (axe, pick) = (mat(&sim, "stone_axe"), mat(&sim, "stone_pick"));
    let id = only_colonist(&sim);
    sim.debug_set_tool(id, axe, 10);
    sim.debug_set_tool(id, pick, 10);
    sim.debug_kill(id);
    assert_eq!(sim.items().total_units(axe), 1);
    assert_eq!(sim.items().total_units(pick), 1);
}
