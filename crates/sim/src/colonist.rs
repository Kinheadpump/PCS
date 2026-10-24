//! Colonists: tile position, sub-tile progress, and the current path.

use crate::item::ItemId;
use crate::job::{ActiveJob, JobKind};
use crate::skill::SKILL_COUNT;
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

/// The sex of a colonist (GAME_DESIGN 7.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Sex {
    /// Female.
    Female,
    /// Male.
    Male,
}

/// A mood memory: a timed mood modifier from `needs.ron` that ends at a tick.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Memory {
    /// Index in the memory list of `needs.ron`.
    pub key: u8,
    /// The memory ends at this tick.
    pub until: u64,
}

/// The action that a need gives to a colonist. It replaces work until it ends.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum NeedAct {
    /// Walk to a food stack, then eat one unit (R-011).
    Eat {
        /// The reserved food stack.
        item: ItemId,
        /// The tile of the stack.
        stand: (u16, u16),
        /// Eating time so far in ticks. `None` while the colonist walks.
        progress: Option<u32>,
    },
    /// Walk to a clothes item, then wear it (R-001).
    Wear {
        /// The reserved clothes stack.
        item: ItemId,
        /// The tile of the stack.
        stand: (u16, u16),
    },
    /// Walk to a warm room, because it is too cold or too hot outside.
    Shelter {
        /// The room tile to walk to.
        stand: (u16, u16),
    },
    /// Walk to a bed, then sleep until rest is full (R-012).
    Sleep {
        /// The reserved bed tile. `None`: the colonist sleeps where it is.
        bed: Option<(u16, u16)>,
        /// True when the colonist sleeps.
        asleep: bool,
    },
}

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
    /// Experience points of each skill, in `Skill::index` order (R-010).
    pub skill_xp: [u32; SKILL_COUNT],
    /// Work priority for each job type, in `JobKind::index` order: 1 (highest) to 4,
    /// or 0 for "off". `Sim` gives new colonists the default from `jobs.ron`.
    pub priorities: [u8; JobKind::COUNT],
    /// The job that the colonist does now.
    pub job: Option<ActiveJob>,
    /// The item stack that the colonist carries.
    pub carrying: Option<ItemId>,
    /// The colonist looks for a new job from this tick on.
    pub next_search: u64,
    /// Age in years (GAME_DESIGN 7.2).
    pub age: u8,
    /// Sex.
    pub sex: Sex,
    /// Race index. All colonists have race 0 in the MVP (ROADMAP 7.1, slice A).
    pub race: u8,
    /// Trait indices (`traits.ron` order). One trait in the MVP.
    pub traits: Vec<u8>,
    /// Hunger in thousandths of a unit (R-011). 100 000 is full.
    pub hunger: i32,
    /// Rest in thousandths of a unit (R-012). 100 000 is full.
    pub rest: i32,
    /// Health in whole units (R-016). The colonist dies at 0.
    pub health: i32,
    /// Hypothermia value (R-013).
    pub hypothermia: i32,
    /// Heatstroke value (R-013).
    pub heatstroke: i32,
    /// True while the colonist has a wound (R-016).
    pub wounded: bool,
    /// Hours of sleep in a bed since the wound began.
    pub wound_bed_hours: u32,
    /// Material index of the worn clothes.
    pub clothes: Option<u16>,
    /// The need action now.
    pub act: Option<NeedAct>,
    /// The colonist looks for food or a bed again from this tick on.
    pub need_next: u64,
    /// Timed mood modifiers (R-014).
    pub memories: Vec<Memory>,
    /// The colonist does no work before this tick (breakdown or walk-around, R-015).
    pub break_until: u64,
    /// Hours in a row with a breakdown mood (R-015).
    pub low_mood_hours: u32,
    /// Background line index plus 1 (`migrants.ron`). 0: no line (S15).
    pub background: u8,
    /// True while the colonist is drafted: no job and no need action (R-032, S13).
    pub drafted: bool,
    /// The creature that the colonist attacks under an order (R-032, S13).
    pub foe: Option<crate::creature::CreatureId>,
    /// The colonist can strike again from this tick on.
    pub attack_ready: u64,
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
            skill_xp: [0; SKILL_COUNT],
            priorities: [0; JobKind::COUNT],
            job: None,
            carrying: None,
            next_search: 0,
            age: 30,
            sex: Sex::Female,
            race: 0,
            traits: Vec::new(),
            hunger: 0,
            rest: 0,
            health: 0,
            hypothermia: 0,
            heatstroke: 0,
            wounded: false,
            wound_bed_hours: 0,
            clothes: None,
            act: None,
            need_next: 0,
            memories: Vec::new(),
            break_until: 0,
            low_mood_hours: 0,
            background: 0,
            drafted: false,
            foe: None,
            attack_ready: 0,
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
