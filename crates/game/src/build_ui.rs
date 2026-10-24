//! Build tool: pure logic for the building picker, the material picker, drag shapes,
//! blueprint commands, and removal commands. No `macroquad` calls, so tests run without
//! a window. The sim checks every command again; these checks only keep the player from
//! sending orders that would be refused, and they color the ghost tiles.

use crate::tools::Area;
use base::content::BuildingKind;
use sim::{BuildLayer, Command, JobKind, JobStep, Reject, Sim, SimRules};

/// How a drag places a building.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shape {
    /// One tile: the tile where the drag starts.
    Point,
    /// A straight line along the longer side of the drag.
    Line,
    /// A filled rectangle.
    Rect,
}

/// The drag shape of a building kind.
pub fn shape_of(kind: BuildingKind) -> Shape {
    match kind {
        BuildingKind::Wall => Shape::Line,
        BuildingKind::Floor | BuildingKind::Roof => Shape::Rect,
        BuildingKind::Door | BuildingKind::Bed | BuildingKind::Hearth | BuildingKind::Workshop => {
            Shape::Point
        }
    }
}

/// Text that tells how to drag, for the build panel.
#[cfg(test)]
pub fn shape_hint(shape: Shape) -> &'static str {
    match shape {
        Shape::Point => "Click a tile",
        Shape::Line => "Drag a line",
        Shape::Rect => "Drag a rectangle",
    }
}

/// The tiles of a drag from `a` to `b`, in row-major order.
pub fn drag_tiles(shape: Shape, a: (u16, u16), b: (u16, u16)) -> Vec<(u16, u16)> {
    match shape {
        Shape::Point => vec![a],
        Shape::Rect => Area::from_corners(a, b).tiles().collect(),
        Shape::Line => {
            let (dx, dy) = (a.0.abs_diff(b.0), a.1.abs_diff(b.1));
            let end = if dx >= dy { (b.0, a.1) } else { (a.0, b.1) };
            Area::from_corners(a, end).tiles().collect()
        }
    }
}

/// The building and the material that the build tool places.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BuildState {
    building: u16,
    /// The last material of each building type, or `None` for the default.
    chosen: Vec<Option<u16>>,
}

impl BuildState {
    /// The state at the start: the first building type.
    pub fn new(rules: &SimRules) -> Self {
        Self {
            building: 0,
            chosen: vec![None; rules.buildings.len()],
        }
    }

    /// The chosen building type.
    pub fn building(&self) -> u16 {
        self.building
    }

    /// Choose a building type by index. An index outside the list changes nothing.
    pub fn pick_building(&mut self, rules: &SimRules, index: usize) {
        if index < rules.buildings.len() {
            self.building = u16::try_from(index).unwrap_or(0);
        }
    }

    /// Choose a material for the chosen building. A material that does not fit changes
    /// nothing.
    pub fn pick_material(&mut self, rules: &SimRules, material: u16) {
        if rules.material_fits(self.building, material)
            && let Some(slot) = self.chosen.get_mut(usize::from(self.building))
        {
            *slot = Some(material);
        }
    }

    /// The material for the chosen building: the player's choice, else the first fitting
    /// material that the colony has, else the first fitting material. `None` if no
    /// material fits.
    pub fn material(&self, sim: &Sim) -> Option<u16> {
        let rules = sim.rules();
        let list = fitting_materials(rules, self.building);
        let chosen = self
            .chosen
            .get(usize::from(self.building))
            .copied()
            .flatten()
            .filter(|m| list.contains(m));
        chosen
            .or_else(|| {
                list.iter()
                    .copied()
                    .find(|&m| sim.items().total_units(m) > 0)
            })
            .or_else(|| list.first().copied())
    }
}

/// The materials that a building type can be made from, in file order.
pub fn fitting_materials(rules: &SimRules, building: u16) -> Vec<u16> {
    (0..rules.materials.len())
        .filter_map(|m| u16::try_from(m).ok())
        .filter(|&m| rules.material_fits(building, m))
        .collect()
}

/// Check that a blueprint can go on a tile now. This is the check of the sim.
pub fn can_place(sim: &Sim, building: u16, material: u16, x: u16, y: u16) -> Result<(), Reject> {
    let rules = sim.rules();
    let def = rules.building(building).ok_or(Reject::UnknownBuilding)?;
    if !rules.material_fits(building, material) {
        return Err(Reject::BadMaterial);
    }
    let (ix, iy) = (i32::from(x), i32::from(y));
    let map = sim.map();
    if !map.in_bounds(ix, iy) {
        return Err(Reject::OutOfMap);
    }
    let layer = BuildLayer::of(def.kind);
    let water = map.ground(ix, iy) == Some(worldgen::Ground::Water);
    let rock = map
        .rock(ix, iy)
        .is_none_or(|r| r != worldgen::RockKind::None);
    if water || rock || (layer != BuildLayer::Roof && (map.tree(ix, iy) || map.wall(ix, iy))) {
        return Err(Reject::TileBlocked);
    }
    let b = sim.buildings();
    if b.get(x, y, layer).is_some() || b.blueprint(x, y, layer).is_some() {
        return Err(Reject::TileOccupied);
    }
    if def.kind == BuildingKind::Roof && !sim.roof_supported(x, y) {
        return Err(Reject::NoRoofSupport);
    }
    Ok(())
}

/// The commands for a finished build drag: one blueprint for each tile where the building
/// can go.
pub fn place_commands(
    sim: &Sim,
    building: u16,
    material: u16,
    tiles: &[(u16, u16)],
) -> Vec<Command> {
    tiles
        .iter()
        .filter(|&&(x, y)| can_place(sim, building, material, x, y).is_ok())
        .map(|&(x, y)| Command::PlaceBlueprint {
            building,
            material,
            x,
            y,
        })
        .collect()
}

/// What the remove tool does on one tile.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RemoveAction {
    /// Take a blueprint away.
    CancelBlueprint(BuildLayer),
    /// Order the deconstruction of a building.
    Deconstruct(BuildLayer),
    /// Take back the deconstruction order of a building.
    CancelDeconstruct(BuildLayer),
}

/// The layers of a tile, from the one that the remove tool takes first.
const REMOVE_ORDER: [BuildLayer; 3] = [BuildLayer::Object, BuildLayer::Roof, BuildLayer::Floor];

/// What the remove tool does on a tile, if anything. Blueprints go first (all layers).
/// A finished building comes next: the object, then the roof, then the floor. A building
/// that already has a deconstruction order gets the order taken back.
pub fn remove_action(sim: &Sim, x: u16, y: u16) -> Option<RemoveAction> {
    let b = sim.buildings();
    for layer in REMOVE_ORDER {
        if b.blueprint(x, y, layer).is_some() {
            return Some(RemoveAction::CancelBlueprint(layer));
        }
    }
    for layer in REMOVE_ORDER {
        if b.get(x, y, layer).is_some() {
            return Some(if deconstruct_ordered(sim, x, y, layer) {
                RemoveAction::CancelDeconstruct(layer)
            } else {
                RemoveAction::Deconstruct(layer)
            });
        }
    }
    None
}

/// True if a deconstruction order exists for the building on the layer.
pub fn deconstruct_ordered(sim: &Sim, x: u16, y: u16, layer: BuildLayer) -> bool {
    sim.jobs()
        .find_arg(JobKind::Deconstruct, x, y, layer.code())
        .is_some()
}

/// The commands for a finished remove drag.
pub fn remove_commands(sim: &Sim, area: Area) -> Vec<Command> {
    area.tiles()
        .filter_map(|(x, y)| {
            remove_action(sim, x, y).map(|a| match a {
                RemoveAction::CancelBlueprint(layer) => Command::CancelBlueprint { x, y, layer },
                RemoveAction::Deconstruct(layer) => Command::Deconstruct { x, y, layer },
                RemoveAction::CancelDeconstruct(layer) => {
                    Command::CancelDeconstruct { x, y, layer }
                }
            })
        })
        .collect()
}

/// Number of tiles of the area where the remove tool does something.
pub fn count_removable(sim: &Sim, area: Area) -> u32 {
    area.tiles()
        .filter(|&(x, y)| remove_action(sim, x, y).is_some())
        .count() as u32
}

/// The progress of a blueprint in percent: the first half is the delivered material, the
/// second half is the work. `None` if there is no blueprint on the layer.
pub fn blueprint_progress(sim: &Sim, x: u16, y: u16, layer: BuildLayer) -> Option<u32> {
    let bp = sim.buildings().blueprint(x, y, layer)?;
    let rules = sim.rules();
    let cost = rules.building(bp.building)?.cost.max(1);
    if bp.delivered < cost {
        return Some(bp.delivered * 50 / cost);
    }
    let need = (rules.build_ticks(bp.building, bp.material) * 100).max(1);
    let done = sim
        .jobs()
        .find_arg(JobKind::Construct, x, y, layer.code())
        .and_then(|id| sim.jobs().get(id))
        .and_then(|j| j.claimed_by)
        .and_then(|who| sim.colonist(who))
        .and_then(|c| c.job)
        .map_or(0, |job| match job.step {
            JobStep::Work { progress } => progress,
            _ => 0,
        });
    Some(50 + (done.min(need) * 50 / need))
}

/// Work time in game hours with one decimal, for example `3.4 h`. 100 ticks are one hour.
pub fn hours_text(ticks: u32) -> String {
    format!("{}.{} h", ticks / 100, ticks % 100 / 10)
}

/// The line under the building picker: the cost and the work time.
pub fn cost_text(rules: &SimRules, building: u16, material: u16) -> String {
    let Some(def) = rules.building(building) else {
        return String::new();
    };
    format!(
        "{} x {}, {}",
        def.cost,
        rules
            .materials
            .get(usize::from(material))
            .map_or("?", |m| m.name.as_str()),
        hours_text(rules.build_ticks(building, material))
    )
}

/// Text for a refused build order or a tile that is not possible.
pub fn reject_text(reject: Reject) -> &'static str {
    match reject {
        Reject::OutOfMap => "outside the map",
        Reject::NothingToCut => "no tree there",
        Reject::NothingToMine => "no rock there",
        Reject::NoItems => "no items there",
        Reject::BadZone => "bad zone corners",
        Reject::ZoneOverlap => "the zone overlaps another zone",
        Reject::UnknownTarget => "unknown target",
        Reject::BadPriority => "bad priority",
        Reject::BadDesignation => "not a designation",
        Reject::UnknownBuilding => "unknown building",
        Reject::BadMaterial => "the building cannot use this material",
        Reject::TileBlocked => "water, rock, or a tree is in the way",
        Reject::TileOccupied => "something is already built there",
        Reject::NoRoofSupport => "a roof needs a wall within 6 tiles",
        Reject::RecipeLocked => "the colony does not know this recipe",
        Reject::NoBlueprint => "no blueprint there",
        Reject::NoBuilding => "no building there",
        Reject::AlreadyOrdered => "already ordered",
        Reject::UnknownRecipe => "unknown recipe",
        Reject::BadCount => "the count must be 1 or more",
        Reject::NoWorkshop => "no workshop there for this recipe",
        Reject::NotFarmable => "crops need soil, clay, or sand without trees or buildings",
        Reject::NoPlant => "no ripe wild plant there",
        Reject::UnknownCrop => "unknown crop",
    }
}

/// True if the building kind is a workshop.
pub fn is_workshop(rules: &SimRules, building: u16) -> bool {
    rules.is_kind(building, BuildingKind::Workshop)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sim::scenario::sim_from_ascii;

    fn index(sim: &Sim, key: &str) -> u16 {
        sim.rules().building_index(key).unwrap()
    }

    fn mat(sim: &Sim, key: &str) -> u16 {
        sim.rules().material_index(key).unwrap()
    }

    #[test]
    fn a_wall_drag_is_a_straight_line_along_the_longer_side() {
        let tiles = drag_tiles(Shape::Line, (2, 3), (6, 4));
        assert_eq!(tiles, vec![(2, 3), (3, 3), (4, 3), (5, 3), (6, 3)]);
        let tiles = drag_tiles(Shape::Line, (2, 3), (3, 7));
        assert_eq!(tiles, vec![(2, 3), (2, 4), (2, 5), (2, 6), (2, 7)]);
        assert_eq!(drag_tiles(Shape::Line, (5, 5), (5, 5)), vec![(5, 5)]);
        assert_eq!(drag_tiles(Shape::Line, (6, 3), (2, 3)).len(), 5);
    }

    #[test]
    fn a_floor_drag_is_a_filled_rectangle_and_a_point_is_one_tile() {
        assert_eq!(drag_tiles(Shape::Rect, (1, 1), (3, 2)).len(), 6);
        assert_eq!(drag_tiles(Shape::Point, (4, 4), (9, 9)), vec![(4, 4)]);
    }

    #[test]
    fn every_building_kind_has_a_shape() {
        for kind in BuildingKind::ALL {
            assert!(!shape_hint(shape_of(kind)).is_empty());
        }
        assert_eq!(shape_of(BuildingKind::Wall), Shape::Line);
        assert_eq!(shape_of(BuildingKind::Roof), Shape::Rect);
    }

    #[test]
    fn the_picker_keeps_one_material_for_each_building() {
        let sim = sim_from_ascii("....\n....", 1).unwrap();
        let rules = sim.rules();
        let mut state = BuildState::new(rules);
        let wall = index(&sim, "wall");
        let door = index(&sim, "door");
        state.pick_building(rules, usize::from(wall));
        let granite = mat(&sim, "granite");
        state.pick_material(rules, granite);
        assert_eq!(state.material(&sim), Some(granite));
        state.pick_building(rules, usize::from(door));
        assert_ne!(state.material(&sim), Some(granite), "a door needs wood");
        state.pick_material(rules, granite);
        assert_ne!(state.material(&sim), Some(granite), "granite does not fit");
        state.pick_building(rules, usize::from(wall));
        assert_eq!(state.material(&sim), Some(granite), "the choice is kept");
        state.pick_building(rules, 99);
        assert_eq!(state.building(), wall);
    }

    #[test]
    fn the_default_material_is_one_that_the_colony_has() {
        let mut sim = sim_from_ascii("....\n....", 1).unwrap();
        let slate = mat(&sim, "slate");
        sim.debug_spawn_item(slate, 5, 0, 0);
        let mut state = BuildState::new(sim.rules());
        let wall = index(&sim, "wall");
        state.pick_building(sim.rules(), usize::from(wall));
        assert_eq!(state.material(&sim), Some(slate));
    }

    #[test]
    fn only_fitting_materials_are_offered() {
        let sim = sim_from_ascii("..", 1).unwrap();
        let rules = sim.rules();
        let door = fitting_materials(rules, index(&sim, "door"));
        assert!(door.contains(&mat(&sim, "log")));
        assert!(!door.contains(&mat(&sim, "granite")));
        let wall = fitting_materials(rules, index(&sim, "wall"));
        assert!(wall.contains(&mat(&sim, "granite")) && wall.contains(&mat(&sim, "log")));
        assert!(fitting_materials(rules, 99).is_empty());
    }

    #[test]
    fn blueprints_are_not_placed_on_blocked_or_occupied_tiles() {
        let mut sim = sim_from_ascii(".T.R\n....", 1).unwrap();
        let wall = index(&sim, "wall");
        let granite = mat(&sim, "granite");
        assert_eq!(can_place(&sim, wall, granite, 0, 0), Ok(()));
        assert_eq!(
            can_place(&sim, wall, granite, 1, 0),
            Err(Reject::TileBlocked),
            "tree"
        );
        assert_eq!(
            can_place(&sim, wall, granite, 3, 0),
            Err(Reject::TileBlocked),
            "rock"
        );
        assert_eq!(
            can_place(&sim, wall, mat(&sim, "iron"), 0, 0),
            Err(Reject::BadMaterial)
        );
        assert!(sim.debug_build(wall, 0, 1, granite));
        assert_eq!(
            can_place(&sim, wall, granite, 0, 1),
            Err(Reject::TileOccupied)
        );
        let cmds = place_commands(&sim, wall, granite, &[(0, 0), (1, 0), (0, 1), (2, 0)]);
        assert_eq!(cmds.len(), 2);
        assert_eq!(
            cmds[0],
            Command::PlaceBlueprint {
                building: wall,
                material: granite,
                x: 0,
                y: 0
            }
        );
    }

    #[test]
    fn a_roof_needs_a_wall_near_it() {
        let mut sim = sim_from_ascii("....\n....", 1).unwrap();
        let roof = index(&sim, "roof");
        let wall = index(&sim, "wall");
        let log = mat(&sim, "log");
        assert_eq!(can_place(&sim, roof, log, 1, 1), Err(Reject::NoRoofSupport));
        assert!(sim.debug_build(wall, 0, 0, log));
        assert_eq!(can_place(&sim, roof, log, 1, 1), Ok(()));
    }

    #[test]
    fn a_placed_blueprint_command_makes_a_blueprint_in_the_sim() {
        let mut sim = sim_from_ascii("....\n....", 1).unwrap();
        let wall = index(&sim, "wall");
        let log = mat(&sim, "log");
        let cmds = place_commands(&sim, wall, log, &drag_tiles(Shape::Line, (0, 0), (3, 0)));
        assert_eq!(cmds.len(), 4);
        sim.step(&cmds);
        assert_eq!(sim.buildings().blueprints().count(), 4);
        assert!(sim.drain_job_events().is_empty());
    }

    #[test]
    fn remove_takes_blueprints_first_then_object_roof_floor() {
        let mut sim = sim_from_ascii(".....\n.....", 1).unwrap();
        let (wall, roof, floor) = (
            index(&sim, "wall"),
            index(&sim, "roof"),
            index(&sim, "floor"),
        );
        let log = mat(&sim, "log");
        assert_eq!(remove_action(&sim, 0, 0), None);
        assert!(sim.debug_build(floor, 1, 0, log));
        assert!(sim.debug_build(wall, 1, 0, log));
        assert!(sim.debug_build(roof, 1, 0, log));
        assert_eq!(
            remove_action(&sim, 1, 0),
            Some(RemoveAction::Deconstruct(BuildLayer::Object))
        );
        sim.step(&remove_commands(&sim, Area::from_corners((1, 0), (1, 0))));
        assert_eq!(
            remove_action(&sim, 1, 0),
            Some(RemoveAction::CancelDeconstruct(BuildLayer::Object))
        );
        assert!(sim.debug_remove_building(1, 0, BuildLayer::Object));
        assert_eq!(
            remove_action(&sim, 1, 0),
            Some(RemoveAction::Deconstruct(BuildLayer::Roof))
        );
        sim.step(&[Command::PlaceBlueprint {
            building: wall,
            material: log,
            x: 2,
            y: 0,
        }]);
        assert_eq!(
            remove_action(&sim, 2, 0),
            Some(RemoveAction::CancelBlueprint(BuildLayer::Object))
        );
        assert_eq!(count_removable(&sim, Area::from_corners((0, 0), (4, 1))), 2);
    }

    #[test]
    fn blueprint_progress_counts_material_first_and_work_second() {
        let mut sim = sim_from_ascii("....\n....", 1).unwrap();
        let wall = index(&sim, "wall");
        let log = mat(&sim, "log");
        assert_eq!(blueprint_progress(&sim, 0, 0, BuildLayer::Object), None);
        sim.step(&[Command::PlaceBlueprint {
            building: wall,
            material: log,
            x: 0,
            y: 0,
        }]);
        assert_eq!(blueprint_progress(&sim, 0, 0, BuildLayer::Object), Some(0));
    }

    #[test]
    fn hours_and_costs_are_written_with_one_decimal() {
        assert_eq!(hours_text(340), "3.4 h");
        assert_eq!(hours_text(95), "0.9 h");
        let sim = sim_from_ascii("..", 1).unwrap();
        let text = cost_text(sim.rules(), index(&sim, "wall"), mat(&sim, "log"));
        assert!(text.starts_with("4 x Log, "), "{text}");
        assert_eq!(cost_text(sim.rules(), 99, 0), "");
    }

    #[test]
    fn every_reject_has_text() {
        for r in [
            Reject::OutOfMap,
            Reject::TileBlocked,
            Reject::TileOccupied,
            Reject::NoRoofSupport,
            Reject::BadMaterial,
            Reject::NoBuilding,
            Reject::AlreadyOrdered,
            Reject::NoWorkshop,
            Reject::BadCount,
        ] {
            assert!(!reject_text(r).is_empty());
        }
    }
}
