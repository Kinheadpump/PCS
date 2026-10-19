//! Colonists: tile position, sub-tile progress, and the current path.

use base::Id;
use serde::{Deserialize, Serialize};

/// Typed ID of a colonist.
pub type ColonistId = Id<Colonist>;

/// Number of colonists that a new colony starts with.
pub const START_COLONISTS: usize = 7;

/// Movement progress units for one straight step. A diagonal step needs 14/10 of it.
/// Progress per tick is [`WALK_PROGRESS_PER_TICK`]. These are provisional game numbers;
/// they move to `assets/content` when the content format has a place for them.
pub const STEP_PROGRESS_STRAIGHT: u16 = 1000;
/// Progress units a walking colonist gains each tick (a straight step takes 10 ticks).
pub const WALK_PROGRESS_PER_TICK: u16 = 100;

/// A colonist.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Colonist {
    /// Tile column of the tile that the colonist stands on.
    pub x: u16,
    /// Tile row of the tile that the colonist stands on.
    pub y: u16,
    /// Progress toward the next tile of the path, in `0..` progress units. 0 when the
    /// colonist stands still.
    pub progress: u16,
    /// Remaining path tiles in reverse order: the next tile is the last element.
    pub path: Vec<(u16, u16)>,
    /// Goal tile of the current move, if any.
    pub target: Option<(u16, u16)>,
}

impl Colonist {
    /// A colonist that stands on a tile.
    pub fn new(x: u16, y: u16) -> Self {
        Self {
            x,
            y,
            progress: 0,
            path: Vec::new(),
            target: None,
        }
    }

    /// The tile that the colonist stands on.
    pub fn tile(&self) -> (u16, u16) {
        (self.x, self.y)
    }

    /// The tile that the colonist walks to, if it walks.
    pub fn next_tile(&self) -> Option<(u16, u16)> {
        self.path.last().copied()
    }

    /// Position in 1/1000 tile units for drawing: tile center is `x * 1000`, the
    /// colonist moves toward the next tile by `progress / step * 1000`.
    pub fn position_milli(&self) -> (i32, i32) {
        let (x, y) = (i32::from(self.x) * 1000, i32::from(self.y) * 1000);
        match self.next_tile() {
            Some((nx, ny)) => {
                let need = step_progress_needed(self.tile(), (nx, ny)).max(1);
                let f = (i32::from(self.progress) * 1000 / i32::from(need)).min(1000);
                (
                    x + (i32::from(nx) - i32::from(self.x)) * f,
                    y + (i32::from(ny) - i32::from(self.y)) * f,
                )
            }
            None => (x, y),
        }
    }
}

/// Progress units needed to step from tile `a` to the adjacent tile `b`.
pub fn step_progress_needed(a: (u16, u16), b: (u16, u16)) -> u16 {
    if a.0 != b.0 && a.1 != b.1 {
        STEP_PROGRESS_STRAIGHT / 10 * 14
    } else {
        STEP_PROGRESS_STRAIGHT
    }
}
