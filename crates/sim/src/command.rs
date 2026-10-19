//! Commands: the only way for the interface to change the `Sim` (TECHNICAL_DESIGN §3.3).

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
}
