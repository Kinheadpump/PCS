//! Events: what the `Sim` tells the interface (TECHNICAL_DESIGN §3.3).

use crate::colonist::ColonistId;

/// Something that happened in a tick. `Sim::drain_events` gives the events to the
/// caller. Events are not part of the saved state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    /// A move command or path request failed: the goal is not reachable.
    PathFailed {
        /// The colonist.
        colonist: ColonistId,
        /// Goal tile column.
        x: u16,
        /// Goal tile row.
        y: u16,
    },
    /// A colonist reached the goal of its move.
    ColonistArrived {
        /// The colonist.
        colonist: ColonistId,
        /// Tile column.
        x: u16,
        /// Tile row.
        y: u16,
    },
}
