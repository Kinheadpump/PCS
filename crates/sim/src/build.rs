//! Buildings, blueprints, construction, deconstruction, and workshop orders (Sprint 4).
//!
//! A building lives on one of three layers of a tile: the floor, the object (wall, door,
//! bed, hearth, workshop), or the roof. The player places a blueprint. A construction job
//! brings the material units of one material to the blueprint, then works. The finished
//! building has that material, and it holds the units. Deconstruction gives back part of
//! them. Construction never makes or destroys material: `Buildings::material_units` and the
//! items together stay the same.
//!
//! A construction job and a craft job both carry stacks from the world to a target that
//! keeps the units (a blueprint or a workshop bin), one trip for each stack, then work at
//! a tile next to the target.

use crate::Sim;
use crate::colonist::ColonistId;
use crate::item::{ItemId, ItemLoc};
use crate::job::{AbortReason, ActiveJob, Job, JobKind, JobStep, NEIGHBORS, Reject, octile};
use crate::{Command, Event};
use base::content::BuildingKind;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// The layer of a tile that a building uses.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum BuildLayer {
    /// Floor covering.
    Floor,
    /// Wall, door, bed, hearth, workshop.
    Object,
    /// Roof.
    Roof,
}

impl BuildLayer {
    /// The layer that a building kind uses.
    pub fn of(kind: BuildingKind) -> Self {
        match kind {
            BuildingKind::Floor => BuildLayer::Floor,
            BuildingKind::Roof => BuildLayer::Roof,
            _ => BuildLayer::Object,
        }
    }

    /// Code stored in `Job::arg`.
    pub fn code(self) -> u16 {
        self as u16
    }

    /// The layer with this code.
    pub fn from_code(code: u16) -> Option<Self> {
        match code {
            0 => Some(BuildLayer::Floor),
            1 => Some(BuildLayer::Object),
            2 => Some(BuildLayer::Roof),
            _ => None,
        }
    }
}

/// An order for a workshop: make a recipe a number of times.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkOrder {
    /// Recipe index (`recipes.ron` order).
    pub recipe: u16,
    /// Runs left. Always 1 or more.
    pub remaining: u32,
}

/// A finished building.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Building {
    /// Building type index (`buildings.ron` order).
    pub def: u16,
    /// Material index. The building is made from this material.
    pub material: u16,
    /// Material units that the building holds. Deconstruction gives part of them back.
    pub units: u32,
    /// Workshop only: input units that colonists brought, sorted by material.
    pub bin: Vec<(u16, u32)>,
    /// Workshop only: the orders, in the order they were made.
    pub orders: Vec<WorkOrder>,
    /// Hearth only: the fuel, in burning hours (R-022).
    pub fuel_hours: u32,
}

/// A planned building. Colonists bring the units, then build.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Blueprint {
    /// Building type index.
    pub building: u16,
    /// Material index chosen by the player.
    pub material: u16,
    /// Material units delivered so far.
    pub delivered: u32,
}

/// All buildings and blueprints of the colony, on sparse tile maps.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Buildings {
    floors: BTreeMap<(u16, u16), Building>,
    objects: BTreeMap<(u16, u16), Building>,
    roofs: BTreeMap<(u16, u16), Building>,
    blueprints: BTreeMap<(u16, u16, BuildLayer), Blueprint>,
}

impl Buildings {
    fn layer(&self, layer: BuildLayer) -> &BTreeMap<(u16, u16), Building> {
        match layer {
            BuildLayer::Floor => &self.floors,
            BuildLayer::Object => &self.objects,
            BuildLayer::Roof => &self.roofs,
        }
    }

    fn layer_mut(&mut self, layer: BuildLayer) -> &mut BTreeMap<(u16, u16), Building> {
        match layer {
            BuildLayer::Floor => &mut self.floors,
            BuildLayer::Object => &mut self.objects,
            BuildLayer::Roof => &mut self.roofs,
        }
    }

    /// The finished building on a layer of a tile.
    pub fn get(&self, x: u16, y: u16, layer: BuildLayer) -> Option<&Building> {
        self.layer(layer).get(&(x, y))
    }

    /// The object (wall, door, bed, hearth, workshop) on a tile.
    pub fn object_at(&self, x: u16, y: u16) -> Option<&Building> {
        self.objects.get(&(x, y))
    }

    /// The floor building on a tile.
    pub fn floor_at(&self, x: u16, y: u16) -> Option<&Building> {
        self.floors.get(&(x, y))
    }

    /// The roof building on a tile.
    pub fn roof_at(&self, x: u16, y: u16) -> Option<&Building> {
        self.roofs.get(&(x, y))
    }

    /// The blueprint on a layer of a tile.
    pub fn blueprint(&self, x: u16, y: u16, layer: BuildLayer) -> Option<&Blueprint> {
        self.blueprints.get(&(x, y, layer))
    }

    /// All finished buildings: floors, then objects, then roofs, each in `(x, y)` order.
    pub fn iter(&self) -> impl Iterator<Item = (u16, u16, BuildLayer, &Building)> {
        [BuildLayer::Floor, BuildLayer::Object, BuildLayer::Roof]
            .into_iter()
            .flat_map(move |layer| {
                self.layer(layer)
                    .iter()
                    .map(move |(&(x, y), b)| (x, y, layer, b))
            })
    }

    /// All blueprints in `(x, y, layer)` order.
    pub fn blueprints(&self) -> impl Iterator<Item = (u16, u16, BuildLayer, &Blueprint)> {
        self.blueprints.iter().map(|(&(x, y, l), b)| (x, y, l, b))
    }

    /// Number of finished buildings.
    pub fn len(&self) -> usize {
        self.floors.len() + self.objects.len() + self.roofs.len()
    }

    /// True if there are no finished buildings.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Material units that blueprints and buildings hold: delivered units, building units,
    /// and workshop bins. Items on tiles and carried items are not counted.
    pub fn material_units(&self, material: u16) -> u64 {
        let in_blueprints: u64 = self
            .blueprints
            .values()
            .filter(|b| b.material == material)
            .map(|b| u64::from(b.delivered))
            .sum();
        let in_buildings: u64 = self
            .iter()
            .map(|(_, _, _, b)| {
                let own = if b.material == material {
                    u64::from(b.units)
                } else {
                    0
                };
                let bin: u64 = b
                    .bin
                    .iter()
                    .filter(|(m, _)| *m == material)
                    .map(|(_, n)| u64::from(*n))
                    .sum();
                own + bin
            })
            .sum();
        in_blueprints + in_buildings
    }

    /// Insert a finished building. Return false if the slot is not empty.
    fn insert(&mut self, x: u16, y: u16, layer: BuildLayer, building: Building) -> bool {
        let map = self.layer_mut(layer);
        if map.contains_key(&(x, y)) {
            return false;
        }
        map.insert((x, y), building);
        true
    }

    /// Set the fuel of the hearth on a tile. Other buildings are not changed.
    pub(crate) fn set_fuel(&mut self, x: u16, y: u16, hours: u32) {
        if let Some(b) = self.objects.get_mut(&(x, y)) {
            b.fuel_hours = hours;
        }
    }

    /// Tiles of the objects that satisfy a test on the building type.
    pub(crate) fn object_tiles(&self, mut keep: impl FnMut(u16) -> bool) -> Vec<(u16, u16)> {
        self.objects
            .iter()
            .filter(|(_, b)| keep(b.def))
            .map(|(&t, _)| t)
            .collect()
    }

    /// Tiles of all roofs, in `(x, y)` order.
    pub(crate) fn roof_tiles(&self) -> Vec<(u16, u16)> {
        self.roofs.keys().copied().collect()
    }
}

/// The best stack for a delivery, with its tile.
type Plan = ((u16, u16), Option<(u16, u16)>, Option<ItemId>);

impl Sim {
    // ---- queries ----

    /// All buildings and blueprints.
    pub fn buildings(&self) -> &Buildings {
        &self.buildings
    }

    /// Material units held by blueprints, buildings, and workshop bins. Together with the
    /// item units of the material, this number stays the same while colonists build
    /// (they only change when material enters or leaves the colony).
    pub fn material_in_construction(&self, material: u16) -> u64 {
        self.buildings.material_units(material)
    }

    /// True if a roof on the tile has a supporting wall or natural rock within the roof
    /// support radius (A-008).
    pub fn roof_supported(&self, x: u16, y: u16) -> bool {
        let r = self.rules.rooms.roof_support_radius as i32;
        let (cx, cy) = (i32::from(x), i32::from(y));
        for ty in cy - r..=cy + r {
            for tx in cx - r..=cx + r {
                if self.is_roof_support(tx, ty) {
                    return true;
                }
            }
        }
        false
    }

    fn is_roof_support(&self, x: i32, y: i32) -> bool {
        if !self.map.in_bounds(x, y) {
            return false;
        }
        if self
            .map
            .rock(x, y)
            .is_some_and(|r| r != worldgen::RockKind::None)
            || self.map.wall(x, y)
        {
            return true;
        }
        self.buildings
            .object_at(x as u16, y as u16)
            .is_some_and(|b| self.rules.is_kind(b.def, BuildingKind::Wall))
    }

    // ---- commands ----

    pub(crate) fn apply_build_command(&mut self, command: &Command) {
        match *command {
            Command::PlaceBlueprint {
                building,
                x,
                y,
                material,
            } => self.place_blueprint(building, x, y, material),
            Command::CancelBlueprint { x, y, layer } => self.cancel_blueprint(x, y, layer),
            Command::Deconstruct { x, y, layer } => self.order_deconstruct(x, y, layer),
            Command::CancelDeconstruct { x, y, layer } => self.cancel_deconstruct(x, y, layer),
            Command::OrderCraft {
                x,
                y,
                recipe,
                count,
            } => self.order_craft(x, y, recipe, count),
            Command::CancelCraft { x, y, recipe } => self.cancel_craft(x, y, recipe),
            _ => {}
        }
    }

    /// Check that a building can stand on the layer of a tile (not the roof support rule).
    fn tile_allows(&self, x: u16, y: u16, layer: BuildLayer) -> Result<(), Reject> {
        let (ix, iy) = (i32::from(x), i32::from(y));
        if !self.map.in_bounds(ix, iy) {
            return Err(Reject::OutOfMap);
        }
        let water = self.map.ground(ix, iy) == Some(worldgen::Ground::Water);
        let rock = self
            .map
            .rock(ix, iy)
            .is_none_or(|r| r != worldgen::RockKind::None);
        if water || rock || self.has_ditch(x, y) {
            return Err(Reject::TileBlocked);
        }
        if layer != BuildLayer::Roof && (self.map.tree(ix, iy) || self.map.wall(ix, iy)) {
            return Err(Reject::TileBlocked);
        }
        Ok(())
    }

    fn place_blueprint(&mut self, building: u16, x: u16, y: u16, material: u16) {
        let rules = self.rules.clone();
        let Some(def) = rules.building(building) else {
            self.reject(Reject::UnknownBuilding);
            return;
        };
        if !rules.material_fits(building, material) {
            self.reject(Reject::BadMaterial);
            return;
        }
        let layer = BuildLayer::of(def.kind);
        if let Err(reason) = self.tile_allows(x, y, layer) {
            self.reject(reason);
            return;
        }
        if self.buildings.get(x, y, layer).is_some()
            || self.buildings.blueprint(x, y, layer).is_some()
        {
            self.reject(Reject::TileOccupied);
            return;
        }
        if def.kind == BuildingKind::Roof && !self.roof_supported(x, y) {
            self.reject(Reject::NoRoofSupport);
            return;
        }
        self.buildings.blueprints.insert(
            (x, y, layer),
            Blueprint {
                building,
                material,
                delivered: 0,
            },
        );
        self.add_job_arg(JobKind::Construct, x, y, layer.code());
    }

    fn cancel_blueprint(&mut self, x: u16, y: u16, layer: BuildLayer) {
        let Some(bp) = self.buildings.blueprints.remove(&(x, y, layer)) else {
            self.reject(Reject::NoBlueprint);
            return;
        };
        self.remove_job_arg(JobKind::Construct, x, y, layer.code());
        // The delivered units go back to the world on the tile.
        self.spawn_units(bp.material, bp.delivered, x, y);
    }

    fn order_deconstruct(&mut self, x: u16, y: u16, layer: BuildLayer) {
        if self.buildings.get(x, y, layer).is_none() {
            self.reject(Reject::NoBuilding);
        } else if self
            .jobs
            .find_arg(JobKind::Deconstruct, x, y, layer.code())
            .is_some()
        {
            self.reject(Reject::AlreadyOrdered);
        } else {
            self.add_job_arg(JobKind::Deconstruct, x, y, layer.code());
        }
    }

    fn cancel_deconstruct(&mut self, x: u16, y: u16, layer: BuildLayer) {
        if !self.remove_job_arg(JobKind::Deconstruct, x, y, layer.code()) {
            self.reject(Reject::NoBuilding);
        }
    }

    pub(crate) fn order_craft(&mut self, x: u16, y: u16, recipe: u16, count: u32) {
        let Some(def) = self.rules.recipe(recipe) else {
            self.reject(Reject::UnknownRecipe);
            return;
        };
        let workshop = def.workshop;
        if count == 0 {
            self.reject(Reject::BadCount);
            return;
        }
        let ok = self
            .buildings
            .object_at(x, y)
            .is_some_and(|b| b.def == workshop);
        if !ok {
            self.reject(Reject::NoWorkshop);
            return;
        }
        if let Some(b) = self.buildings.objects.get_mut(&(x, y)) {
            match b.orders.iter_mut().find(|o| o.recipe == recipe) {
                Some(o) => o.remaining = o.remaining.saturating_add(count).min(MAX_ORDER),
                None => b.orders.push(WorkOrder {
                    recipe,
                    remaining: count.min(MAX_ORDER),
                }),
            }
        }
        if self.jobs.find_arg(JobKind::Craft, x, y, recipe).is_none() {
            self.add_job_arg(JobKind::Craft, x, y, recipe);
        }
    }

    fn cancel_craft(&mut self, x: u16, y: u16, recipe: u16) {
        let had = self.buildings.objects.get_mut(&(x, y)).is_some_and(|b| {
            let before = b.orders.len();
            b.orders.retain(|o| o.recipe != recipe);
            b.orders.len() != before
        });
        if !had {
            self.reject(Reject::NoWorkshop);
            return;
        }
        self.remove_job_arg(JobKind::Craft, x, y, recipe);
    }

    // ---- debug ----

    /// Put a finished building on a tile at once, with no blueprint and no work. For
    /// scenario tests and debug tools; the player has no such command. It checks the
    /// building, the material, and the tile like a blueprint, but not the roof support
    /// rule. Return true if the building stands now.
    pub fn debug_build(&mut self, building: u16, x: u16, y: u16, material: u16) -> bool {
        let rules = self.rules.clone();
        let Some(def) = rules.building(building) else {
            return false;
        };
        let layer = BuildLayer::of(def.kind);
        if !rules.material_fits(building, material)
            || self.tile_allows(x, y, layer).is_err()
            || self.buildings.get(x, y, layer).is_some()
            || self.buildings.blueprint(x, y, layer).is_some()
        {
            return false;
        }
        self.complete_building(x, y, layer, building, material, def.cost)
    }

    /// Take a finished building away at once. Its material units vanish. For scenario tests
    /// and debug tools; the player has no such command. Return true if there was a building.
    pub fn debug_remove_building(&mut self, x: u16, y: u16, layer: BuildLayer) -> bool {
        self.remove_building(x, y, layer).is_some()
    }

    // ---- finished buildings ----

    /// Put a finished building on its layer. A building that blocks walking moves the
    /// colonists and items off its tile first. The rooms are detected again in the next tick.
    fn complete_building(
        &mut self,
        x: u16,
        y: u16,
        layer: BuildLayer,
        building: u16,
        material: u16,
        units: u32,
    ) -> bool {
        let blocks = self
            .rules
            .building(building)
            .is_some_and(|d| d.blocks_walking);
        if blocks {
            self.evict_tile(x, y);
        }
        let inserted = self.buildings.insert(
            x,
            y,
            layer,
            Building {
                def: building,
                material,
                units,
                bin: Vec::new(),
                orders: Vec::new(),
                fuel_hours: 0,
            },
        );
        if !inserted {
            return false;
        }
        if blocks && self.map.set_solid(i32::from(x), i32::from(y), true) {
            self.passability_changed(x, y);
        }
        self.rooms_dirty = true;
        true
    }

    /// Take a finished building away and return it.
    pub(crate) fn remove_building(
        &mut self,
        x: u16,
        y: u16,
        layer: BuildLayer,
    ) -> Option<Building> {
        let building = self.buildings.layer_mut(layer).remove(&(x, y))?;
        let blocks = self
            .rules
            .building(building.def)
            .is_some_and(|d| d.blocks_walking);
        if blocks && self.map.set_solid(i32::from(x), i32::from(y), false) {
            self.passability_changed(x, y);
        }
        self.rooms_dirty = true;
        Some(building)
    }

    /// The nearest passable tile next to or near a tile, not the tile itself. The order is
    /// fixed: the four neighbors, then the eight neighbors, then growing rings.
    pub(crate) fn nearest_free_tile(&self, x: u16, y: u16) -> Option<(u16, u16)> {
        let (cx, cy) = (i32::from(x), i32::from(y));
        for ring in 1..=4i32 {
            for dy in -ring..=ring {
                for dx in -ring..=ring {
                    if dx.abs().max(dy.abs()) != ring {
                        continue;
                    }
                    let (tx, ty) = (cx + dx, cy + dy);
                    if self.map.passable(tx, ty) {
                        return Some((tx as u16, ty as u16));
                    }
                }
            }
        }
        None
    }

    /// Move the colonists and the items off a tile that a building will block.
    fn evict_tile(&mut self, x: u16, y: u16) {
        let Some(to) = self.nearest_free_tile(x, y) else {
            return;
        };
        let moved: Vec<ColonistId> = self
            .colonists
            .iter()
            .filter(|(_, c)| c.tile() == (x, y))
            .map(|(id, _)| id)
            .collect();
        for id in moved {
            if let Some(c) = self.colonists.get_mut(id) {
                c.x = to.0;
                c.y = to.1;
                c.path.clear();
                c.progress = 0;
                if let Some(goal) = c.target {
                    self.path_queue.retain(|r| r.colonist != id);
                    self.path_queue
                        .push_back(crate::PathRequest { colonist: id, goal });
                }
            }
        }
        let stacks: Vec<ItemId> = self.items.at_tile(x, y).to_vec();
        for id in stacks {
            self.items.relocate(id, to.0, to.1);
        }
    }

    /// Put `count` units of a material on a tile as stacks of at most `stack_max`, flagged
    /// for hauling.
    pub(crate) fn spawn_units(&mut self, material: u16, count: u32, x: u16, y: u16) {
        let stack_max = self.rules.jobs.stack_max.max(1);
        let mut left = count;
        while left > 0 {
            let n = left.min(stack_max);
            self.spawn_item(material, n, x, y, true);
            left -= n;
        }
    }

    // ---- job board support ----

    /// Put a job with an argument on the board.
    pub(crate) fn add_job_arg(&mut self, kind: JobKind, x: u16, y: u16, arg: u16) {
        self.jobs.add_arg(kind, x, y, arg);
        self.wake_idle();
    }

    /// Remove a job with an argument from the board. Return true if there was one.
    fn remove_job_arg(&mut self, kind: JobKind, x: u16, y: u16, arg: u16) -> bool {
        match self.jobs.find_arg(kind, x, y, arg) {
            Some(job) => {
                self.jobs.remove(job);
                true
            }
            None => false,
        }
    }

    /// The nearest passable neighbor of a tile in a region, to stand on while working.
    pub(crate) fn work_stand(
        &self,
        region: u32,
        from: (u16, u16),
        x: u16,
        y: u16,
    ) -> Option<(u16, u16)> {
        let mut best: Option<((u16, u16), u32)> = None;
        for (dx, dy) in NEIGHBORS {
            let (nx, ny) = (i32::from(x) + dx, i32::from(y) + dy);
            if self.regions.id_at(nx, ny) != region {
                continue;
            }
            let tile = (nx as u16, ny as u16);
            let cost = octile(from, tile);
            if best.is_none_or(|(_, c)| cost < c) {
                best = Some((tile, cost));
            }
        }
        match best {
            Some((tile, _)) => Some(tile),
            // A passable target (floor, door, bed): the colonist can work on the tile itself.
            None if self.regions.id_at(i32::from(x), i32::from(y)) == region => Some((x, y)),
            None => None,
        }
    }

    /// The unreserved stack of a material that is nearest to a tile, in the region.
    /// Stacks in a stockpile come first. The stack index breaks ties.
    pub(crate) fn find_material(
        &self,
        region: u32,
        from: (u16, u16),
        material: u16,
    ) -> Option<ItemId> {
        let mut best: Option<((u8, u32, u32), ItemId)> = None;
        for (id, item) in self.items.iter() {
            let ItemLoc::Tile(ix, iy) = item.loc else {
                continue;
            };
            if item.material != material
                || item.reserved_by.is_some()
                || self.regions.id_at(i32::from(ix), i32::from(iy)) != region
            {
                continue;
            }
            let key = (
                u8::from(!self.is_stockpiled(id)),
                octile(from, (ix, iy)),
                id.index(),
            );
            if best.is_none_or(|(k, _)| key < k) {
                best = Some((key, id));
            }
        }
        best.map(|(_, id)| id)
    }

    /// The next material that the job needs, with the number of units: a missing input of
    /// a recipe, or the rest of a blueprint. `None` if nothing is missing.
    fn material_wanted(&self, job: &Job) -> Option<(u16, u32)> {
        match job.kind {
            JobKind::Construct => {
                let layer = BuildLayer::from_code(job.arg)?;
                let bp = self.buildings.blueprint(job.x, job.y, layer)?;
                let cost = self.rules.building(bp.building)?.cost;
                (bp.delivered < cost).then_some((bp.material, cost - bp.delivered))
            }
            JobKind::Craft => {
                let recipe = self.rules.recipe(job.arg)?;
                let building = self.buildings.object_at(job.x, job.y)?;
                recipe.inputs.iter().find_map(|&(material, count)| {
                    let have = building
                        .bin
                        .iter()
                        .find(|(m, _)| *m == material)
                        .map_or(0, |(_, n)| *n);
                    (have < count).then_some((material, count - have))
                })
            }
            _ => None,
        }
    }

    /// Check a construction, deconstruction, or craft job in a region. Return the tile
    /// where the colonist goes first, the delivery tile, and the stack. A job that needs
    /// material starts at the stack. Other jobs start at the tile where the colonist works.
    pub(crate) fn eval_build(&self, region: u32, from: (u16, u16), job: &Job) -> Option<Plan> {
        match job.kind {
            JobKind::Construct => {
                let layer = BuildLayer::from_code(job.arg)?;
                self.buildings.blueprint(job.x, job.y, layer)?;
            }
            JobKind::Deconstruct => {
                let layer = BuildLayer::from_code(job.arg)?;
                self.buildings.get(job.x, job.y, layer)?;
            }
            JobKind::Craft => {
                let recipe = self.rules.recipe(job.arg)?;
                let building = self.buildings.object_at(job.x, job.y)?;
                if building.def != recipe.workshop
                    || !building
                        .orders
                        .iter()
                        .any(|o| o.recipe == job.arg && o.remaining > 0)
                {
                    return None;
                }
            }
            _ => return None,
        }
        match self.material_wanted(job) {
            Some((material, _)) => {
                let item = self.find_material(region, from, material)?;
                let ItemLoc::Tile(ix, iy) = self.items.get(item)?.loc else {
                    return None;
                };
                let drop = self.work_stand(region, (ix, iy), job.x, job.y)?;
                Some(((ix, iy), Some(drop), Some(item)))
            }
            None => {
                let stand = self.work_stand(region, from, job.x, job.y)?;
                Some((stand, None, None))
            }
        }
    }

    /// True if the target of a construction, deconstruction, or craft job is ready for work.
    pub(crate) fn build_target_ok(&self, job: &Job) -> bool {
        let Some(layer) = BuildLayer::from_code(job.arg) else {
            return job.kind == JobKind::Craft && self.craft_ready(job);
        };
        match job.kind {
            JobKind::Construct => {
                self.buildings.blueprint(job.x, job.y, layer).is_some()
                    && self.material_wanted(job).is_none()
            }
            JobKind::Deconstruct => self.buildings.get(job.x, job.y, layer).is_some(),
            JobKind::Craft => self.craft_ready(job),
            _ => false,
        }
    }

    fn craft_ready(&self, job: &Job) -> bool {
        let Some(recipe) = self.rules.recipe(job.arg) else {
            return false;
        };
        self.buildings.object_at(job.x, job.y).is_some_and(|b| {
            b.def == recipe.workshop
                && b.orders
                    .iter()
                    .any(|o| o.recipe == job.arg && o.remaining > 0)
        }) && self.material_wanted(job).is_none()
    }

    /// Work time of a construction, deconstruction, or craft job, in ticks at 100% speed.
    pub(crate) fn build_work_ticks(&self, job: &Job) -> u32 {
        let layer = BuildLayer::from_code(job.arg);
        match job.kind {
            JobKind::Construct => layer
                .and_then(|l| self.buildings.blueprint(job.x, job.y, l))
                .map_or(0, |bp| self.rules.build_ticks(bp.building, bp.material)),
            JobKind::Deconstruct => layer
                .and_then(|l| self.buildings.get(job.x, job.y, l))
                .map_or(0, |b| {
                    let pct = self
                        .rules
                        .building(b.def)
                        .map_or(100, |d| d.deconstruct_pct);
                    self.rules.build_ticks(b.def, b.material) * pct / 100
                }),
            JobKind::Craft => self.rules.recipe(job.arg).map_or(0, |r| r.work_ticks),
            _ => 0,
        }
    }

    /// True if a construction, deconstruction, or craft job has nothing to do any more.
    pub(crate) fn build_job_stale(&self, job: &Job) -> bool {
        match job.kind {
            JobKind::Construct => BuildLayer::from_code(job.arg)
                .is_none_or(|l| self.buildings.blueprint(job.x, job.y, l).is_none()),
            JobKind::Deconstruct => BuildLayer::from_code(job.arg)
                .is_none_or(|l| self.buildings.get(job.x, job.y, l).is_none()),
            JobKind::Craft => {
                let Some(recipe) = self.rules.recipe(job.arg) else {
                    return true;
                };
                self.buildings.object_at(job.x, job.y).is_none_or(|b| {
                    b.def != recipe.workshop
                        || !b
                            .orders
                            .iter()
                            .any(|o| o.recipe == job.arg && o.remaining > 0)
                })
            }
            _ => false,
        }
    }

    /// Units that the job still needs of the material of a stack.
    fn units_needed(&self, job: &Job, material: u16) -> u32 {
        if job.is_refuel() {
            return self.refuel_units_needed(job, material);
        }
        match self.material_wanted(job) {
            Some((m, n)) if m == material => n,
            _ => 0,
        }
    }

    /// Before a colonist picks a stack up for a construction or craft job: cut off the units
    /// that the job does not need. The cut part stays on the tile.
    pub(crate) fn trim_to_need(&mut self, active: &ActiveJob, item: ItemId) {
        let Some(job) = self.jobs.get(active.job).cloned() else {
            return;
        };
        let Some(stack) = self.items.get(item) else {
            return;
        };
        let (material, count) = (stack.material, stack.count);
        let need = self.units_needed(&job, material);
        if need > 0
            && count > need
            && let Some(rest) = self.items.split_off(item, count - need)
        {
            let flagged = self.items.get(rest).is_some_and(|i| i.to_haul);
            if flagged {
                self.queue_haul(rest);
            }
        }
    }

    /// A colonist arrives with a stack at the delivery tile: the units go to the blueprint
    /// or the workshop bin. Then the job plans its next trip or the work.
    pub(crate) fn deliver_item(&mut self, id: ColonistId, active: ActiveJob) {
        let Some(job) = self.jobs.get(active.job).cloned() else {
            self.abort_job(id, AbortReason::JobGone, false);
            return;
        };
        let Some(stack) = active.item.and_then(|i| self.items.get(i)) else {
            self.abort_job(id, AbortReason::JobGone, false);
            return;
        };
        let (material, count) = (stack.material, stack.count);
        let need = self.units_needed(&job, material);
        if need == 0 {
            // The job does not want this material any more: put the stack down.
            self.abort_job(id, AbortReason::JobGone, false);
            return;
        }
        let take = count.min(need);
        if let Some(item) = active.item {
            self.items.remove(item);
        }
        if let Some(c) = self.colonists.get_mut(id) {
            c.carrying = None;
        }
        self.store_units(&job, material, take);
        if count > take {
            let (cx, cy) = self.colonists.get(id).map(|c| c.tile()).unwrap_or_default();
            self.spawn_units(material, count - take, cx, cy);
        }
        self.retarget(id, active);
    }

    /// Add delivered units to the target of a job.
    fn store_units(&mut self, job: &Job, material: u16, count: u32) {
        match job.kind {
            JobKind::Construct => {
                if let Some(layer) = BuildLayer::from_code(job.arg)
                    && let Some(bp) = self.buildings.blueprints.get_mut(&(job.x, job.y, layer))
                {
                    bp.delivered += count;
                }
            }
            JobKind::Craft => {
                if let Some(b) = self.buildings.objects.get_mut(&(job.x, job.y)) {
                    match b.bin.iter_mut().find(|(m, _)| *m == material) {
                        Some((_, n)) => *n += count,
                        None => {
                            b.bin.push((material, count));
                            b.bin.sort_unstable();
                        }
                    }
                }
            }
            _ => {}
        }
    }

    /// Plan the next step of a claimed job after a delivery: the next stack, or the work.
    fn retarget(&mut self, id: ColonistId, active: ActiveJob) {
        let Some(job) = self.jobs.get(active.job).cloned() else {
            self.abort_job(id, AbortReason::JobGone, false);
            return;
        };
        let Some(from) = self.colonists.get(id).map(|c| c.tile()) else {
            return;
        };
        let region = self.regions.id_at(i32::from(from.0), i32::from(from.1));
        let Some((stand, dest, item)) = self.eval_build(region, from, &job) else {
            self.abort_job(id, AbortReason::NoMaterial, true);
            return;
        };
        if let Some(item) = item {
            self.items.set_reserved(item, Some(id));
        }
        if let Some(c) = self.colonists.get_mut(id) {
            c.job = Some(ActiveJob {
                step: if item.is_some() {
                    JobStep::GoToItem
                } else {
                    JobStep::GoToWork
                },
                stand,
                dest,
                item,
                ..active
            });
        }
        self.start_leg(id, stand);
    }

    /// The work of a construction, deconstruction, or craft job is done.
    pub(crate) fn complete_build_work(&mut self, id: ColonistId, active: ActiveJob, job: &Job) {
        let layer = BuildLayer::from_code(job.arg);
        match job.kind {
            JobKind::Construct => {
                if let Some(layer) = layer
                    && let Some(bp) = self.buildings.blueprints.remove(&(job.x, job.y, layer))
                {
                    self.complete_building(
                        job.x,
                        job.y,
                        layer,
                        bp.building,
                        bp.material,
                        bp.delivered,
                    );
                }
            }
            JobKind::Deconstruct => {
                if let Some(layer) = layer
                    && let Some(b) = self.remove_building(job.x, job.y, layer)
                {
                    self.give_back(&b, active.stand);
                    if layer == BuildLayer::Object {
                        let craft: Vec<_> = self
                            .jobs
                            .iter()
                            .filter(|(_, j)| {
                                j.kind == JobKind::Craft && (j.x, j.y) == (job.x, job.y)
                            })
                            .map(|(jid, _)| jid)
                            .collect();
                        for jid in craft {
                            self.jobs.remove(jid);
                        }
                    }
                }
            }
            JobKind::Craft => self.finish_craft(job, active.stand),
            _ => {}
        }
        self.finish_job(id, active, job.x, job.y);
        if job.kind == JobKind::Craft && !self.build_job_stale(job) {
            self.add_job_arg(JobKind::Craft, job.x, job.y, job.arg);
        }
    }

    /// Deconstruction: part of the material units and all of a workshop bin go to the
    /// world on the tile where the colonist worked.
    fn give_back(&mut self, building: &Building, at: (u16, u16)) {
        let pct = self
            .rules
            .building(building.def)
            .map_or(0, |d| d.return_pct);
        let units = (u64::from(building.units) * u64::from(pct) / 100) as u32;
        self.spawn_units(building.material, units, at.0, at.1);
        for &(material, count) in &building.bin {
            self.spawn_units(material, count, at.0, at.1);
        }
    }

    /// One run of a recipe is done: the inputs leave the bin and the outputs appear.
    fn finish_craft(&mut self, job: &Job, at: (u16, u16)) {
        let Some(recipe) = self.rules.recipe(job.arg).cloned() else {
            return;
        };
        if let Some(b) = self.buildings.objects.get_mut(&(job.x, job.y)) {
            for &(material, count) in &recipe.inputs {
                if let Some(slot) = b.bin.iter_mut().find(|(m, _)| *m == material) {
                    slot.1 = slot.1.saturating_sub(count);
                }
            }
            b.bin.retain(|&(_, n)| n > 0);
            if let Some(o) = b.orders.iter_mut().find(|o| o.recipe == job.arg) {
                o.remaining = o.remaining.saturating_sub(1);
            }
            b.orders.retain(|o| o.remaining > 0);
        }
        for &(material, count) in &recipe.outputs {
            self.spawn_units(material, count, at.0, at.1);
        }
        self.events.push(Event::Crafted {
            recipe: job.arg,
            x: job.x,
            y: job.y,
        });
    }

    // ---- checks ----

    /// Check the buildings of a loaded state against the map and the content.
    pub(crate) fn buildings_state_ok(&self) -> bool {
        let rules = &self.rules;
        let inside = |x: u16, y: u16| self.map.in_bounds(i32::from(x), i32::from(y));
        let material_ok = |m: u16| usize::from(m) < rules.materials.len();
        let built = self.buildings.iter().all(|(x, y, layer, b)| {
            inside(x, y)
                && rules
                    .building(b.def)
                    .is_some_and(|d| BuildLayer::of(d.kind) == layer)
                && material_ok(b.material)
                && b.units <= 1_000_000
                && b.fuel_hours <= 1_000_000
                && (b.fuel_hours == 0 || rules.is_kind(b.def, BuildingKind::Hearth))
                && b.bin.iter().all(|&(m, n)| material_ok(m) && n <= 1_000_000)
                && b.orders
                    .iter()
                    .all(|o| rules.recipe(o.recipe).is_some() && o.remaining > 0)
                && (rules.is_kind(b.def, BuildingKind::Workshop)
                    || (b.bin.is_empty() && b.orders.is_empty()))
        });
        let planned = self.buildings.blueprints().all(|(x, y, layer, bp)| {
            inside(x, y)
                && rules
                    .building(bp.building)
                    .is_some_and(|d| BuildLayer::of(d.kind) == layer && bp.delivered <= d.cost)
                && material_ok(bp.material)
        });
        built && planned
    }

    /// Check the building invariants. Return the first problem. For tests and the soak tool.
    pub(crate) fn check_building_invariants(&self) -> Result<(), String> {
        for (x, y, layer, bp) in self.buildings.blueprints() {
            if self.buildings.get(x, y, layer).is_some() {
                return Err(format!("tile {x},{y} has a blueprint and a building"));
            }
            if self
                .jobs
                .find_arg(JobKind::Construct, x, y, layer.code())
                .is_none()
            {
                return Err(format!("blueprint at {x},{y} has no construction job"));
            }
            let cost = self.rules.building(bp.building).map_or(0, |d| d.cost);
            if bp.delivered > cost {
                return Err(format!("blueprint at {x},{y} has more units than its cost"));
            }
        }
        for (_, c) in self.colonists.iter() {
            if !self.map.passable(i32::from(c.x), i32::from(c.y)) {
                return Err(format!(
                    "a colonist stands inside a wall at {},{}",
                    c.x, c.y
                ));
            }
        }
        for (x, y, layer, b) in self.buildings.iter() {
            let blocks = self.rules.building(b.def).is_some_and(|d| d.blocks_walking);
            if layer == BuildLayer::Object && blocks != self.map.solid(i32::from(x), i32::from(y)) {
                return Err(format!("building at {x},{y} and the map disagree"));
            }
        }
        Ok(())
    }
}

/// Largest number of runs in one workshop order.
const MAX_ORDER: u32 = 10_000;
