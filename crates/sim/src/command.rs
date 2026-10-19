//! Commands: the only way for the interface to change the `Sim` (TECHNICAL_DESIGN §3.3).

use crate::colonist::ColonistId;
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
}
