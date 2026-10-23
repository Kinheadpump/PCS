//! Commands: the only way for the interface to change the `Sim` (TECHNICAL_DESIGN §3.3).

use crate::build::BuildLayer;
use crate::colonist::ColonistId;
use crate::job::JobKind;
use crate::stockpile::{ItemFilter, StockpileId};
use serde::{Deserialize, Serialize};

/// A command from the player, the interface, or a test. `Sim::step` applies the
/// commands at the start of a tick. Replay files hold commands with their tick.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Command {
    /// Debug command: walk a colonist to a tile. Only debug builds, scenario tests, and
    /// tools use it. The player has no such command. If the tile is not reachable,
    /// the same tick gives `Event::PathFailed`.
    MoveColonist {
        /// The colonist.
        id: ColonistId,
        /// Goal tile column.
        x: u16,
        /// Goal tile row.
        y: u16,
    },
    /// Designate a tile. `Cut` needs a tree, `Mine` needs a rock wall, `Haul` marks the
    /// items on the tile for hauling to a stockpile. A refused command gives
    /// `JobEvent::Rejected`.
    Designate {
        /// The designation type.
        kind: JobKind,
        /// Tile column.
        x: u16,
        /// Tile row.
        y: u16,
    },
    /// Order a ditch on a tile. A colonist digs it as a mine job and nobody walks on a ditch
    /// after that. The tile needs soil, sand, or clay with no tree, rock, building, or item. A
    /// refused command gives `JobEvent::Rejected`. `CancelDesignation` cancels the order.
    DigDitch {
        /// Tile column.
        x: u16,
        /// Tile row.
        y: u16,
    },
    /// Mark the room that contains a tile as a sick room, or take the mark away. Sick
    /// colonists sleep in a sick room if a bed is free there. A tile outside a room gives
    /// `JobEvent::Rejected`.
    SetSickRoom {
        /// Tile column.
        x: u16,
        /// Tile row.
        y: u16,
        /// True: mark the room. False: unmark it.
        on: bool,
    },
    /// Remove the cut or mine designation of a tile, and the haul order of its items that
    /// no colonist carries yet.
    CancelDesignation {
        /// Tile column.
        x: u16,
        /// Tile row.
        y: u16,
    },
    /// Make a stockpile zone. The corners are inclusive and can be in any order.
    CreateStockpile {
        /// First corner column.
        x0: u16,
        /// First corner row.
        y0: u16,
        /// Second corner column.
        x1: u16,
        /// Second corner row.
        y1: u16,
        /// Accepted materials.
        filter: ItemFilter,
    },
    /// Remove a stockpile zone. Its items stay on the tiles.
    RemoveStockpile {
        /// The stockpile.
        id: StockpileId,
    },
    /// Change the accepted materials of a stockpile.
    SetStockpileFilter {
        /// The stockpile.
        id: StockpileId,
        /// The new filter.
        filter: ItemFilter,
    },
    /// Set the work priority of a colonist for a job type: 1 (highest) to 4, or 0 for "off".
    SetPriority {
        /// The colonist.
        colonist: ColonistId,
        /// The job type.
        job: JobKind,
        /// The priority.
        priority: u8,
    },
    /// Place a blueprint for a building. Colonists bring units of `material` to the tile
    /// and build. A refused command gives `JobEvent::Rejected`.
    PlaceBlueprint {
        /// Building type index (`buildings.ron` order).
        building: u16,
        /// Tile column.
        x: u16,
        /// Tile row.
        y: u16,
        /// Material index. It must be in a category that the building accepts.
        material: u16,
    },
    /// Remove a blueprint. The units that colonists delivered fall on the tile.
    CancelBlueprint {
        /// Tile column.
        x: u16,
        /// Tile row.
        y: u16,
        /// The layer of the blueprint.
        layer: BuildLayer,
    },
    /// Order the deconstruction of a finished building. A colonist takes it down and part of
    /// the material units fall on the ground.
    Deconstruct {
        /// Tile column.
        x: u16,
        /// Tile row.
        y: u16,
        /// The layer of the building.
        layer: BuildLayer,
    },
    /// Cancel a deconstruction order that no colonist has finished.
    CancelDeconstruct {
        /// Tile column.
        x: u16,
        /// Tile row.
        y: u16,
        /// The layer of the building.
        layer: BuildLayer,
    },
    /// Order a workshop to make a recipe `count` more times.
    OrderCraft {
        /// Workshop tile column.
        x: u16,
        /// Workshop tile row.
        y: u16,
        /// Recipe index (`recipes.ron` order).
        recipe: u16,
        /// Number of runs. Must be 1 or more.
        count: u32,
    },
    /// Remove the order for a recipe from a workshop.
    CancelCraft {
        /// Workshop tile column.
        x: u16,
        /// Workshop tile row.
        y: u16,
        /// Recipe index.
        recipe: u16,
    },
    /// Mark a rectangle as a farm zone for a crop. Colonists sow seeds on its free tiles
    /// and harvest ripe plants. Tiles that cannot be farmed are skipped. The corners are
    /// inclusive and can be in any order.
    SetFarmZone {
        /// First corner column.
        x0: u16,
        /// First corner row.
        y0: u16,
        /// Second corner column.
        x1: u16,
        /// Second corner row.
        y1: u16,
        /// Crop index (`crops.ron` order).
        crop: u16,
    },
    /// Remove the farm zone from a rectangle. Plants stay on the tiles.
    ClearFarmZone {
        /// First corner column.
        x0: u16,
        /// First corner row.
        y0: u16,
        /// Second corner column.
        x1: u16,
        /// Second corner row.
        y1: u16,
    },
    /// Set the "keep N" order of a recipe at a workshop. The workshop makes the recipe until
    /// `target` units are in stock, and again when the stock falls below. A target of 0
    /// removes the order.
    SetKeep {
        /// Workshop tile column.
        x: u16,
        /// Workshop tile row.
        y: u16,
        /// Recipe index.
        recipe: u16,
        /// Units to keep in stock.
        target: u32,
    },
    /// The player abandons the colony. The run ends (R-050).
    Abandon,
}
