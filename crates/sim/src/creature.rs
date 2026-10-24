//! Creatures (S13, R-031, R-071): game animals, beasts, lair beasts, the horrors of awake
//! threats, and raiders.
//!
//! Creatures live in an arena with stable IDs. Game animals walk around and flee when hurt.
//! Beasts come from the map edge when the daily beast roll hits (S14), sleep in lairs until
//! Notice is high enough, and come out of a site when a colonist opens its seal. Raiders come with
//! a raid (S18): they attack colonists, break doors and walls, and carry away items.
//!
//! Wandering and fleeing use single steps and no path search. A creature that chases or goes to
//! a place asks for a path with the path budget that the colonists leave (P-04). It waits for the
//! next tick if the budget is used.

use crate::build::BuildLayer;
use crate::colonist::{ColonistId, step_progress_needed};
use crate::loader::{Issues, parse};
use crate::time::TICKS_PER_DAY;
use crate::{PATH_BUDGET_NODES, PATH_BUDGET_SEARCHES, Sim};
use base::content::{BuildingKind, ContentError, MaterialDef};
use base::rng::mix;
use base::{Arena, Id, Pcg32};
use serde::{Deserialize, Serialize};

/// Typed ID of a creature.
pub type CreatureId = Id<Creature>;

/// Tag of the creature random stream.
const TAG_CREATURE: u64 = 0x4352_4541_5455_5245;

/// What a creature is for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Role {
    /// A game animal. It is harmless unless its damage is more than 0 and it is hurt.
    Game,
    /// A beast: it attacks colonists.
    Beast,
    /// A creature of a sealed site. It attacks colonists.
    Horror,
    /// A fighter of a raid.
    Raider,
}

impl Role {
    /// True if the creature attacks colonists on its own.
    pub fn hostile(self) -> bool {
        !matches!(self, Role::Game)
    }
}

/// A kind of creature (`creatures.ron`).
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreatureKind {
    /// String ID.
    pub id: String,
    /// Display name.
    pub name: String,
    /// What it is for.
    pub role: Role,
    /// Health at the start.
    pub health: i32,
    /// Base damage of a blow. 0: harmless.
    pub damage: u32,
    /// Ticks between two blows.
    pub attack_ticks: u32,
    /// Progress units each tick. A colonist has 100.
    pub speed: u16,
    /// Sight in tiles.
    pub sight: u32,
    /// Lowest level that picks this beast or horror.
    pub min_level: u32,
    /// Units of meat that it leaves.
    pub meat: u32,
    /// Units of hide that it leaves.
    pub hide: u32,
}

/// Numbers for creatures and melee (`creatures.ron`).
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreatureRules {
    /// Game animals for each 1000 passable tiles at the start.
    pub game_per_1000: u32,
    /// Fewest game animals at the start.
    pub game_min: u32,
    /// Most game animals at the start.
    pub game_max: u32,
    /// No game animal starts nearer than this to a colonist.
    pub game_safe_distance: u32,
    /// A new game animal comes every this many game days while there are too few.
    pub regrow_days: u32,
    /// Number of lairs.
    pub lair_count: u32,
    /// Smallest distance of a lair from the colonists.
    pub lair_min_distance: u32,
    /// Beasts in each lair.
    pub lair_beasts: u32,
    /// No beast arrives and no lair wakes in this many first days.
    pub beast_quiet_days: u32,
    /// Notice for 1 more level of a beast.
    pub beast_level_div: u32,
    /// Levels for 1 more beast in a group.
    pub group_div: u32,
    /// Largest group.
    pub group_max: u32,
    /// Tiles that a creature wanders from its home.
    pub wander_radius: u32,
    /// Ticks between two wander steps.
    pub wander_ticks: u32,
    /// Ticks of fleeing after a hit.
    pub flee_ticks: u32,
    /// Ticks between two decisions.
    pub think_ticks: u32,
    /// Ticks before a chasing creature asks for a new path.
    pub repath_ticks: u32,
    /// Sight of a beast that hunts.
    pub active_sight: u32,
    /// Base damage of a colonist with no weapon.
    pub fist_damage: u32,
    /// Percent of the base damage for each combat level (R-032).
    pub level_pct: u32,
    /// Largest roll in percent of the base damage (R-032).
    pub roll_pct: u32,
    /// A hit of this damage or more adds the condition "wounded" (R-032).
    pub wound_damage: u32,
    /// Ticks between the blows of a colonist.
    pub colonist_attack_ticks: u32,
    /// Combat experience for a hit.
    pub combat_xp: u32,
    /// Hunting experience for the kill of a game animal.
    pub hunting_xp: u32,
    /// Ticks that raiders have to leave the map.
    pub leave_ticks: u32,
    /// Material key of meat.
    pub meat: String,
    /// Material key of hide.
    pub hide: String,
    /// The kinds, in the order of the file.
    pub kinds: Vec<CreatureKind>,
    /// Material index of meat, or `None` if the content has none.
    #[serde(skip)]
    pub meat_material: Option<u16>,
    /// Material index of hide, or `None` if the content has none.
    #[serde(skip)]
    pub hide_material: Option<u16>,
}

impl CreatureRules {
    /// Index of the kind with a string ID.
    pub fn kind_index(&self, id: &str) -> Option<u8> {
        self.kinds
            .iter()
            .position(|k| k.id == id)
            .and_then(|i| u8::try_from(i).ok())
    }

    /// Indices of the kinds of a role that a level picks, in file order.
    pub fn kinds_of(&self, role: Role, level: u32) -> Vec<u8> {
        self.kinds
            .iter()
            .enumerate()
            .filter(|(_, k)| k.role == role && k.min_level <= level)
            .filter_map(|(i, _)| u8::try_from(i).ok())
            .collect()
    }
}

/// Load and check `creatures.ron`. A missing meat or hide material is not a problem: the
/// creatures then leave nothing of that kind.
pub fn load_creature_rules(
    file: &str,
    text: &str,
    materials: &[MaterialDef],
) -> Result<CreatureRules, ContentError> {
    let mut r: CreatureRules = parse(file, text)?;
    let mut issues = Issues::new(file);
    if r.kinds.is_empty() || r.kinds.len() > 64 {
        issues.add("kinds", "must hold 1 to 64 kinds");
    }
    for (i, k) in r.kinds.iter().enumerate() {
        if r.kinds[..i].iter().any(|o| o.id == k.id) {
            issues.add(&k.id, "duplicate ID");
        }
        if k.health <= 0 || k.speed == 0 {
            issues.add(&k.id, "health and speed must be 1 or more");
        }
        if k.damage > 0 && k.attack_ticks == 0 {
            issues.add(&k.id, "a creature with damage needs attack_ticks");
        }
    }
    if r.think_ticks == 0 || r.wander_ticks == 0 || r.repath_ticks == 0 {
        issues.add("think_ticks", "the tick numbers must be 1 or more");
    }
    if r.beast_level_div == 0 || r.group_div == 0 || r.group_max == 0 {
        issues.add(
            "beast_level_div",
            "the divisors and group_max must be 1 or more",
        );
    }
    if r.roll_pct > 100 || r.game_min > r.game_max {
        issues.add(
            "roll_pct",
            "roll_pct must be 100 or less and game_min at most game_max",
        );
    }
    let find = |key: &str| {
        materials
            .iter()
            .position(|m| m.key == key)
            .and_then(|i| u16::try_from(i).ok())
    };
    r.meat_material = find(&r.meat);
    r.hide_material = find(&r.hide);
    issues.finish(r)
}

/// What a creature does now.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Mode {
    /// It walks around its home. A beast that is not driven stays at home.
    Wander,
    /// It goes for a colonist.
    Chase,
    /// It runs away from the tile of the blow until `mode_until`.
    Flee,
    /// A raider goes to the entry tile and leaves the map.
    Leave,
}

/// A creature. Part of the state.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Creature {
    /// Kind index (`creatures.ron` order).
    pub kind: u8,
    /// Tile column.
    pub x: u16,
    /// Tile row.
    pub y: u16,
    /// Progress toward the next tile of the path.
    pub progress: u16,
    /// Remaining path tiles in reverse order: the next tile is the last element.
    pub path: Vec<(u16, u16)>,
    /// Goal tile of the path.
    pub goal: Option<(u16, u16)>,
    /// Health. The creature dies at 0.
    pub health: i32,
    /// The place where it lives.
    pub home: (u16, u16),
    /// What it does.
    pub mode: Mode,
    /// Tick when `Flee` or `Leave` ends.
    pub mode_until: u64,
    /// True for a beast that hunts colonists. A beast in a lair sleeps until it is driven.
    pub driven: bool,
    /// True for a beast of a lair.
    pub lair: bool,
    /// The raid that the creature belongs to, or 0.
    pub raid: u32,
    /// The stack that a raider carries: material and units.
    pub loot: Option<(u16, u32)>,
    /// The colonist that it chases.
    pub victim: Option<ColonistId>,
    /// The tile of the last blow that it took, for the flight.
    pub threat: (u16, u16),
    /// Tick of the next decision.
    pub next_think: u64,
    /// Tick when it can strike again.
    pub next_attack: u64,
    /// Tick of the last path request.
    pub last_path: u64,
    /// A raider that breaks a door or a wall: the tile and the ticks left.
    pub breaking: Option<((u16, u16), u32)>,
    /// A raider that goes to break a wall: the tile to stand on and the wall tile.
    pub breach: Option<((u16, u16), (u16, u16))>,
}

impl Creature {
    /// A creature on a tile.
    pub fn new(kind: u8, health: i32, x: u16, y: u16) -> Self {
        Self {
            kind,
            x,
            y,
            progress: 0,
            path: Vec::new(),
            goal: None,
            health,
            home: (x, y),
            mode: Mode::Wander,
            mode_until: 0,
            driven: false,
            lair: false,
            raid: 0,
            loot: None,
            victim: None,
            threat: (x, y),
            next_think: 0,
            next_attack: 0,
            last_path: 0,
            breaking: None,
            breach: None,
        }
    }

    /// The tile that the creature stands on.
    pub fn tile(&self) -> (u16, u16) {
        (self.x, self.y)
    }

    /// Position in 1/1000 tile units for drawing.
    pub fn position_milli(&self) -> (i32, i32) {
        let (x, y) = (i32::from(self.x) * 1000, i32::from(self.y) * 1000);
        match self.path.last() {
            Some(&(nx, ny)) => {
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

/// The creatures and their random stream. Part of the state.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct CreatureState {
    pub(crate) rng: Pcg32,
    pub(crate) creatures: Arena<Creature>,
    /// Game animals at the start: the regrowth stops at this number.
    pub(crate) game_target: u32,
    /// The lairs: tile columns and rows.
    pub(crate) lairs: Vec<(u16, u16)>,
    /// True after the lair beasts were woken.
    pub(crate) lairs_awake: bool,
    /// Hostile creatures killed.
    pub(crate) killed: u32,
    /// Game animals killed.
    pub(crate) hunted: u32,
}

impl CreatureState {
    pub(crate) fn new(seed: u64) -> Self {
        Self {
            rng: Pcg32::from_seed(mix(seed ^ TAG_CREATURE)),
            creatures: Arena::new(),
            game_target: 0,
            lairs: Vec::new(),
            lairs_awake: false,
            killed: 0,
            hunted: 0,
        }
    }
}

/// A thing that happened with creatures or in a fight. The caller takes it with
/// `Sim::drain_combat_events`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CombatEvent {
    /// A blow hit someone.
    Hit {
        /// Tile of the victim.
        x: u16,
        /// Tile of the victim.
        y: u16,
        /// True if a colonist struck.
        by_colonist: bool,
        /// The damage.
        damage: u32,
        /// True if the blow wounded a colonist.
        wounded: bool,
    },
    /// A creature died.
    CreatureDied {
        /// Kind index.
        kind: u8,
        /// Tile column.
        x: u16,
        /// Tile row.
        y: u16,
        /// The colonist that struck the last blow, if any.
        by: Option<ColonistId>,
    },
    /// A beast came to the map edge (R-031).
    BeastArrived {
        /// Entry tile column.
        x: u16,
        /// Entry tile row.
        y: u16,
        /// Number of beasts.
        count: u32,
        /// Level of the beasts.
        level: u32,
    },
    /// The beasts of the lairs woke because Notice is high (R-031).
    LairsWoke {
        /// Number of beasts.
        count: u32,
    },
    /// The creatures of a threat came out of a site (R-031).
    ThreatReleased {
        /// Site ID.
        site: u32,
        /// Number of creatures.
        count: u32,
        /// Strength level.
        level: u32,
    },
    /// A raider broke a wall or a door (R-071).
    Breached {
        /// Tile column.
        x: u16,
        /// Tile row.
        y: u16,
    },
    /// A raider carries away a stack (R-071).
    RaiderLooted {
        /// Tile column of the stack.
        x: u16,
        /// Tile row of the stack.
        y: u16,
    },
    /// A command was refused.
    Rejected(CombatReject),
}

/// Why a combat command was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CombatReject {
    /// The colonist does not exist.
    NoColonist,
    /// The creature does not exist.
    NoCreature,
    /// The colonist is not drafted.
    NotDrafted,
    /// The goal tile cannot be reached.
    NoPath,
    /// The colonist is a child (R-037).
    TooYoung,
}

/// The numbers of a kind that the fight code needs, copied out of the content.
#[derive(Clone, Copy)]
pub(crate) struct Stats {
    pub(crate) role: Role,
    pub(crate) damage: u32,
    pub(crate) attack_ticks: u32,
    pub(crate) speed: u16,
    pub(crate) sight: u32,
}

/// A wall to break: its score, its distance from the raider, the tile to stand on, and the wall.
type Breach = (u32, u32, (u16, u16), (u16, u16));

/// Distance in tiles with diagonal steps: the larger of the two differences.
pub(crate) fn cheb(a: (u16, u16), b: (u16, u16)) -> u32 {
    (i32::from(a.0) - i32::from(b.0))
        .unsigned_abs()
        .max((i32::from(a.1) - i32::from(b.1)).unsigned_abs())
}

const NEIGHBORS: [(i32, i32); 8] = [
    (0, -1),
    (-1, 0),
    (1, 0),
    (0, 1),
    (-1, -1),
    (1, -1),
    (-1, 1),
    (1, 1),
];

impl Sim {
    /// All creatures, in slot order.
    pub fn creatures(&self) -> &Arena<Creature> {
        &self.creature.creatures
    }

    /// One creature.
    pub fn creature(&self, id: CreatureId) -> Option<&Creature> {
        self.creature.creatures.get(id)
    }

    /// The content of a kind.
    pub fn creature_kind(&self, kind: u8) -> Option<&CreatureKind> {
        self.rules.creature.kinds.get(usize::from(kind))
    }

    /// The lairs of the map: tile columns and rows.
    pub fn lairs(&self) -> &[(u16, u16)] {
        &self.creature.lairs
    }

    /// True after the beasts of the lairs woke.
    pub fn lairs_awake(&self) -> bool {
        self.creature.lairs_awake
    }

    /// Hostile creatures that colonists killed, and game animals that they hunted.
    pub fn creature_kills(&self) -> (u32, u32) {
        (self.creature.killed, self.creature.hunted)
    }

    /// Take the combat events that the last ticks made.
    pub fn drain_combat_events(&mut self) -> Vec<CombatEvent> {
        std::mem::take(&mut self.combat_events)
    }

    pub(crate) fn stats(&self, kind: u8) -> Option<Stats> {
        self.rules
            .creature
            .kinds
            .get(usize::from(kind))
            .map(|k| Stats {
                role: k.role,
                damage: k.damage,
                attack_ticks: k.attack_ticks,
                speed: k.speed,
                sight: k.sight,
            })
    }

    /// Put a new creature on a tile. `None` if the kind or the tile is wrong.
    pub(crate) fn spawn_creature(&mut self, kind: u8, x: u16, y: u16) -> Option<CreatureId> {
        let health = self.rules.creature.kinds.get(usize::from(kind))?.health;
        if !self.map.passable(i32::from(x), i32::from(y)) {
            return None;
        }
        Some(
            self.creature
                .creatures
                .insert(Creature::new(kind, health, x, y)),
        )
    }

    /// The first passable tile near a tile, the tile itself first.
    fn spawn_tile_near(&self, x: u16, y: u16) -> Option<(u16, u16)> {
        if self.map.passable(i32::from(x), i32::from(y)) {
            return Some((x, y));
        }
        self.nearest_free_tile(x, y)
    }

    /// A random passable tile in the region of the colonists, at least `min` tiles from every
    /// colonist and at least `from_center` tiles from their center.
    fn random_far_tile(&mut self, min: u32, from_center: u32) -> Option<(u16, u16)> {
        let home = self.colonists.iter().next().map(|(_, c)| c.tile())?;
        let center = self.notice_home();
        let (w, h) = (self.map.width(), self.map.height());
        for _ in 0..400 {
            let x = self.creature.rng.below(w) as u16;
            let y = self.creature.rng.below(h) as u16;
            if !self.map.passable(i32::from(x), i32::from(y))
                || !self.regions.connected(
                    (i32::from(x), i32::from(y)),
                    (i32::from(home.0), i32::from(home.1)),
                )
                || cheb((x, y), center) < from_center
                || self
                    .colonists
                    .iter()
                    .any(|(_, c)| cheb(c.tile(), (x, y)) < min)
            {
                continue;
            }
            return Some((x, y));
        }
        None
    }

    /// A game animal, a lair, and the lair beasts for a new colony on a generated map.
    pub(crate) fn creature_start(&mut self) {
        let r = self.rules.creature.clone();
        let passable = (0..self.map.height() as i32)
            .flat_map(|y| (0..self.map.width() as i32).map(move |x| (x, y)))
            .filter(|&(x, y)| self.map.passable(x, y))
            .count() as u32;
        let target = (passable / 1000 * r.game_per_1000).clamp(r.game_min, r.game_max);
        self.creature.game_target = target;
        let game = r.kinds_of(Role::Game, u32::MAX);
        if !game.is_empty() {
            for _ in 0..target {
                self.spawn_game_animal(&game, r.game_safe_distance);
            }
        }
        let beasts = r.kinds_of(Role::Beast, 1);
        if beasts.is_empty() {
            return;
        }
        for _ in 0..r.lair_count {
            let Some((lx, ly)) = self.random_far_tile(0, r.lair_min_distance) else {
                break;
            };
            if self
                .creature
                .lairs
                .iter()
                .any(|&l| cheb(l, (lx, ly)) < r.lair_min_distance / 2)
            {
                continue;
            }
            self.creature.lairs.push((lx, ly));
            for _ in 0..r.lair_beasts {
                let kind = beasts[self.creature.rng.below(beasts.len() as u32) as usize];
                if let Some(t) = self.spawn_tile_near(lx, ly)
                    && let Some(id) = self.spawn_creature(kind, t.0, t.1)
                    && let Some(c) = self.creature.creatures.get_mut(id)
                {
                    c.home = (lx, ly);
                    c.lair = true;
                }
            }
        }
    }

    fn spawn_game_animal(&mut self, kinds: &[u8], safe: u32) -> bool {
        let Some((x, y)) = self.random_far_tile(safe, 0) else {
            return false;
        };
        let kind = kinds[self.creature.rng.below(kinds.len() as u32) as usize];
        self.spawn_creature(kind, x, y).is_some()
    }

    /// The daily work: new game animals, and the lair beasts wake when Notice is high.
    fn creature_daily(&mut self) {
        let r = self.rules.creature.clone();
        let day = self.tick / TICKS_PER_DAY;
        let game = r.kinds_of(Role::Game, u32::MAX);
        let count = self
            .creature
            .creatures
            .iter()
            .filter(|(_, c)| {
                self.rules
                    .creature
                    .kinds
                    .get(usize::from(c.kind))
                    .is_some_and(|k| k.role == Role::Game)
            })
            .count() as u32;
        if !game.is_empty()
            && count < self.creature.game_target
            && day.is_multiple_of(u64::from(r.regrow_days.max(1)))
        {
            self.spawn_game_animal(&game, r.game_safe_distance);
        }
        if !self.creature.lairs_awake
            && day > u64::from(r.beast_quiet_days)
            && self.lairs_active()
            && !self.creature.lairs.is_empty()
        {
            self.creature.lairs_awake = true;
            let mut woke = 0;
            for (_, c) in self.creature.creatures.iter_mut() {
                if c.lair && !c.driven {
                    c.driven = true;
                    woke += 1;
                }
            }
            self.combat_events
                .push(CombatEvent::LairsWoke { count: woke });
        }
    }

    /// Level of the beasts that Notice makes (R-031).
    pub fn beast_level(&self) -> u32 {
        1 + self.notice_total() / self.rules.creature.beast_level_div.max(1)
    }

    /// A group of beasts comes in at the map edge (R-031). Called when the daily roll hits.
    pub(crate) fn spawn_beasts(&mut self) {
        let r = self.rules.creature.clone();
        let day = self.tick / TICKS_PER_DAY;
        if day <= u64::from(r.beast_quiet_days) || self.colonists.is_empty() {
            return;
        }
        let level = self.beast_level();
        let kinds = r.kinds_of(Role::Beast, level);
        if kinds.is_empty() {
            return;
        }
        let edge = self.creature.rng.below(4) as u8;
        let Some(entry) = self.creature_edge_tile(edge) else {
            return;
        };
        let count = (1 + level / r.group_div.max(1)).min(r.group_max);
        let mut made = 0;
        for _ in 0..count {
            let kind = kinds[self.creature.rng.below(kinds.len() as u32) as usize];
            if let Some(id) = self.spawn_creature(kind, entry.0, entry.1)
                && let Some(c) = self.creature.creatures.get_mut(id)
            {
                c.driven = true;
                c.mode = Mode::Chase;
                made += 1;
            }
        }
        if made > 0 {
            self.combat_events.push(CombatEvent::BeastArrived {
                x: entry.0,
                y: entry.1,
                count: made,
                level,
            });
        }
    }

    fn creature_edge_tile(&mut self, edge: u8) -> Option<(u16, u16)> {
        for home in self.home_tiles() {
            for k in 0..4u8 {
                let e = usize::from((edge + k) % 4);
                let len = if e == worldgen::embark::EXIT_N || e == worldgen::embark::EXIT_S {
                    self.map.width()
                } else {
                    self.map.height()
                };
                let offset = self.creature.rng.below(len) as i32;
                if let Some(t) = self.edge_scan(e, offset, home) {
                    return Some(t);
                }
            }
        }
        None
    }

    /// The creatures of a threat that just woke come out of the site (R-031).
    pub(crate) fn spawn_threat_creatures(&mut self, index: usize) {
        let Some(t) = self.threats.get(index) else {
            return;
        };
        let (site, x, y) = (t.site, t.x, t.y);
        let level = self.threat_strength(index).unwrap_or(1);
        let r = self.rules.creature.clone();
        let kinds = r.kinds_of(Role::Horror, level);
        if kinds.is_empty() {
            return;
        }
        let count = (1 + level / r.group_div.max(1)).min(r.group_max);
        let mut made = 0;
        for _ in 0..count {
            let kind = kinds[self.creature.rng.below(kinds.len() as u32) as usize];
            let Some((tx, ty)) = self.spawn_tile_near(x, y) else {
                break;
            };
            if let Some(id) = self.spawn_creature(kind, tx, ty)
                && let Some(c) = self.creature.creatures.get_mut(id)
            {
                c.home = (x, y);
                c.driven = true;
                c.mode = Mode::Chase;
                made += 1;
            }
        }
        if made > 0 {
            self.combat_events.push(CombatEvent::ThreatReleased {
                site,
                count: made,
                level,
            });
        }
    }

    /// The raiders of a raid that starts to fight come in at the entry tile (R-071).
    pub(crate) fn spawn_raiders(&mut self, raid: u32, fighters: u32, entry: (u16, u16)) {
        let kinds = self.rules.creature.kinds_of(Role::Raider, u32::MAX);
        let Some(&kind) = kinds.first() else {
            return;
        };
        for _ in 0..fighters {
            if let Some(id) = self.spawn_creature(kind, entry.0, entry.1)
                && let Some(c) = self.creature.creatures.get_mut(id)
            {
                c.raid = raid;
                c.driven = true;
                c.home = entry;
            }
        }
    }

    /// Number of creatures that belong to a raid.
    pub fn raiders_on_map(&self, raid: u32) -> usize {
        self.creature
            .creatures
            .iter()
            .filter(|(_, c)| c.raid == raid)
            .count()
    }

    /// The raiders of a raid go to the entry tile and leave (R-071).
    pub(crate) fn recall_raiders(&mut self, raid: u32) {
        let until = self.tick + u64::from(self.rules.creature.leave_ticks);
        for (_, c) in self.creature.creatures.iter_mut() {
            if c.raid == raid && c.mode != Mode::Leave {
                c.mode = Mode::Leave;
                c.mode_until = until;
                c.path.clear();
                c.goal = None;
                c.progress = 0;
                c.victim = None;
                c.breaking = None;
                c.breach = None;
                c.next_think = 0;
            }
        }
    }

    /// Take the raiders of a raid off the map. Their loot is lost.
    pub(crate) fn remove_raiders(&mut self, raid: u32) {
        let ids: Vec<CreatureId> = self
            .creature
            .creatures
            .iter()
            .filter(|(_, c)| c.raid == raid)
            .map(|(id, _)| id)
            .collect();
        for id in ids {
            self.creature.creatures.remove(id);
        }
    }

    /// Run all creatures for one tick (after the path requests of the colonists).
    pub(crate) fn creature_tick(&mut self) {
        if self.tick > 0 && self.tick.is_multiple_of(TICKS_PER_DAY) {
            self.creature_daily();
        }
        if self.creature.creatures.is_empty() {
            return;
        }
        let ids: Vec<CreatureId> = self.creature.creatures.iter().map(|(id, _)| id).collect();
        for id in ids {
            self.creature_act(id);
        }
    }

    fn creature_act(&mut self, id: CreatureId) {
        let Some(mut c) = self.creature.creatures.get(id).cloned() else {
            return;
        };
        let Some(stats) = self.stats(c.kind) else {
            return;
        };
        let keep = self.creature_run(id, &mut c, stats);
        if !keep {
            self.creature.creatures.remove(id);
        } else if let Some(slot) = self.creature.creatures.get_mut(id) {
            *slot = c;
        }
    }

    /// One tick of one creature. Return false if it leaves the map.
    fn creature_run(&mut self, id: CreatureId, c: &mut Creature, stats: Stats) -> bool {
        let tick = self.tick;
        if let Some((tile, left)) = c.breaking {
            if left > 1 {
                c.breaking = Some((tile, left - 1));
                return true;
            }
            c.breaking = None;
            self.remove_building(tile.0, tile.1, BuildLayer::Object);
            self.combat_events.push(CombatEvent::Breached {
                x: tile.0,
                y: tile.1,
            });
            c.breach = None;
            c.next_think = tick;
        }
        self.creature_walk(c, stats);
        if c.breaking.is_some() {
            return true;
        }
        match stats.role {
            Role::Game => self.game_think(id, c, stats),
            Role::Beast | Role::Horror => self.beast_think(id, c, stats),
            Role::Raider => return self.raider_think(id, c, stats),
        }
        true
    }

    /// Move along the path. A raider stops in front of a door and breaks it first.
    fn creature_walk(&mut self, c: &mut Creature, stats: Stats) {
        let Some(&next) = c.path.last() else {
            return;
        };
        if !self.map.passable(i32::from(next.0), i32::from(next.1)) {
            c.path.clear();
            c.goal = None;
            c.progress = 0;
            return;
        }
        if stats.role == Role::Raider
            && c.mode != Mode::Leave
            && let Some(ticks) = self.door_break_ticks(next.0, next.1)
        {
            c.breaking = Some((next, ticks.max(1)));
            return;
        }
        c.progress = c.progress.saturating_add(stats.speed);
        let need = step_progress_needed(c.tile(), next);
        if c.progress >= need {
            c.progress -= need;
            c.x = next.0;
            c.y = next.1;
            c.path.pop();
            if c.path.is_empty() {
                c.progress = 0;
                c.goal = None;
            }
        }
    }

    /// Ticks to break the door on a tile, or `None` if the tile has no door.
    fn door_break_ticks(&self, x: u16, y: u16) -> Option<u32> {
        let b = self.buildings.get(x, y, BuildLayer::Object)?;
        let def = self.rules.building(b.def)?;
        (def.kind == BuildingKind::Door)
            .then(|| self.raid_break_ticks(x, y))
            .flatten()
    }

    /// Ask for a path with the budget that is left in this tick. Return false if there is no
    /// budget or no path.
    fn creature_path(&mut self, c: &mut Creature, goal: (u16, u16)) -> bool {
        if self.path_stats.searches >= PATH_BUDGET_SEARCHES
            || self.path_stats.expanded >= PATH_BUDGET_NODES
        {
            return false;
        }
        let start = if c.progress > 0 {
            c.path.last().copied().unwrap_or_else(|| c.tile())
        } else {
            c.tile()
        };
        let result = self
            .finder
            .find(&self.map, start, goal, &mut self.scratch_path);
        self.path_stats.searches += 1;
        self.path_stats.expanded += result.expanded;
        c.last_path = self.tick;
        if !result.found {
            c.path.clear();
            c.goal = None;
            c.progress = 0;
            return false;
        }
        let carried = (c.progress > 0).then(|| c.path.last().copied()).flatten();
        c.path.clear();
        c.path.extend_from_slice(&self.scratch_path);
        if let Some(t) = carried {
            c.path.push(t);
        }
        c.goal = Some(goal);
        true
    }

    /// True if a blow can pass from tile `a` to the neighbor tile `b`.
    pub(crate) fn can_strike(&self, a: (u16, u16), b: (u16, u16)) -> bool {
        let (dx, dy) = (
            i32::from(b.0) - i32::from(a.0),
            i32::from(b.1) - i32::from(a.1),
        );
        if dx == 0 && dy == 0 {
            return true;
        }
        dx.abs() <= 1
            && dy.abs() <= 1
            && crate::path::can_step(&self.map, i32::from(a.0), i32::from(a.1), dx, dy)
    }

    /// The colonist nearest to a tile within `range` tiles. Ties go to the lower ID.
    pub(crate) fn nearest_colonist(
        &self,
        at: (u16, u16),
        range: u32,
    ) -> Option<(ColonistId, (u16, u16))> {
        let mut best: Option<(u32, ColonistId, (u16, u16))> = None;
        for (id, col) in self.colonists.iter() {
            let d = cheb(at, col.tile());
            if d <= range && best.is_none_or(|(bd, ..)| d < bd) {
                best = Some((d, id, col.tile()));
            }
        }
        best.map(|(_, id, t)| (id, t))
    }

    /// One random step to a free neighbor tile that stays in `radius` of the home.
    fn wander_step(&mut self, c: &mut Creature, radius: u32) {
        let first = self.creature.rng.below(8) as usize;
        for k in 0..8 {
            let (dx, dy) = NEIGHBORS[(first + k) % 8];
            let (nx, ny) = (i32::from(c.x) + dx, i32::from(c.y) + dy);
            if !self.map.passable(nx, ny)
                || !crate::path::can_step(&self.map, i32::from(c.x), i32::from(c.y), dx, dy)
            {
                continue;
            }
            let t = (nx as u16, ny as u16);
            if cheb(t, c.home) <= radius {
                c.path = vec![t];
                c.progress = 0;
                c.goal = None;
                return;
            }
        }
    }

    /// One step to the free neighbor tile that is farthest from the threat.
    fn flee_step(&mut self, c: &mut Creature) {
        let mut best: Option<(u32, (u16, u16))> = None;
        for (dx, dy) in NEIGHBORS {
            let (nx, ny) = (i32::from(c.x) + dx, i32::from(c.y) + dy);
            if !self.map.passable(nx, ny)
                || !crate::path::can_step(&self.map, i32::from(c.x), i32::from(c.y), dx, dy)
            {
                continue;
            }
            let t = (nx as u16, ny as u16);
            let d = cheb(t, c.threat);
            if best.is_none_or(|(bd, _)| d > bd) {
                best = Some((d, t));
            }
        }
        if let Some((_, t)) = best {
            c.path = vec![t];
            c.progress = 0;
            c.goal = None;
        }
    }

    fn game_think(&mut self, _id: CreatureId, c: &mut Creature, stats: Stats) {
        let tick = self.tick;
        let r = self.rules.creature.clone();
        match c.mode {
            Mode::Flee if tick < c.mode_until => {
                if c.path.is_empty() {
                    self.flee_step(c);
                }
                return;
            }
            Mode::Chase if tick < c.mode_until => {
                self.chase(c, stats, r.active_sight);
                return;
            }
            _ => {}
        }
        c.mode = Mode::Wander;
        if c.path.is_empty() && tick >= c.next_think {
            self.wander_step(c, r.wander_radius);
            let wait = u64::from(r.wander_ticks);
            c.next_think = tick + wait + u64::from(self.creature.rng.below(r.wander_ticks.max(1)));
        }
    }

    fn beast_think(&mut self, _id: CreatureId, c: &mut Creature, stats: Stats) {
        let tick = self.tick;
        let r = self.rules.creature.clone();
        if !c.driven {
            // A beast in a lair sleeps. A wild beast of the map walks around its home.
            if !c.lair && c.path.is_empty() && tick >= c.next_think {
                self.wander_step(c, r.wander_radius);
                c.next_think = tick + u64::from(r.wander_ticks);
            }
            return;
        }
        c.mode = Mode::Chase;
        let moved = self.chase(c, stats, r.active_sight);
        if !moved && c.path.is_empty() && tick >= c.next_think {
            self.wander_step(c, r.wander_radius);
            c.next_think = tick + u64::from(r.wander_ticks);
        }
    }

    /// Go for the victim or the nearest colonist in sight, and strike when next to it. Return
    /// true if the creature has a victim.
    fn chase(&mut self, c: &mut Creature, stats: Stats, sight: u32) -> bool {
        let tick = self.tick;
        let r_think = u64::from(self.rules.creature.think_ticks);
        let victim = c
            .victim
            .and_then(|v| self.colonists.get(v).map(|col| (v, col.tile())));
        let victim = match victim {
            Some(v) => Some(v),
            None if tick >= c.next_think || c.victim.is_some() => {
                c.next_think = tick + r_think;
                let range = sight.max(stats.sight);
                self.nearest_colonist(c.tile(), range)
            }
            None => None,
        };
        let Some((vid, vt)) = victim else {
            c.victim = None;
            return false;
        };
        c.victim = Some(vid);
        if self.can_strike(c.tile(), vt) && c.progress == 0 {
            c.path.clear();
            c.goal = None;
            if stats.damage > 0 && tick >= c.next_attack {
                self.creature_strike(c, stats, vid);
            }
            return true;
        }
        let stale =
            c.goal != Some(vt) && tick >= c.last_path + u64::from(self.rules.creature.repath_ticks);
        if (c.path.is_empty() || stale)
            && self.regions.connected(
                (i32::from(c.x), i32::from(c.y)),
                (i32::from(vt.0), i32::from(vt.1)),
            )
        {
            self.creature_path(c, vt);
        }
        true
    }

    /// A creature strikes a colonist.
    pub(crate) fn creature_strike(&mut self, c: &mut Creature, stats: Stats, victim: ColonistId) {
        let roll = self.creature.rng.below(self.rules.creature.roll_pct + 1);
        let damage = stats.damage * (100 + roll) / 100;
        c.next_attack = self.tick + u64::from(stats.attack_ticks);
        self.hurt_colonist(victim, damage, false);
    }

    /// A raider: attack colonists, take the best stack, break doors and walls, and leave.
    fn raider_think(&mut self, _id: CreatureId, c: &mut Creature, stats: Stats) -> bool {
        let tick = self.tick;
        let entry = self.raid_entry();
        let active = self.raid.active.as_ref().map(|r| r.id);
        if (c.raid == 0 || active != Some(c.raid)) && c.mode != Mode::Leave {
            c.mode = Mode::Leave;
            c.mode_until = tick + u64::from(self.rules.creature.leave_ticks);
            c.path.clear();
            c.goal = None;
        }
        if c.mode == Mode::Leave {
            return self.raider_leave(c, entry);
        }
        if c.loot.is_some() {
            c.mode = Mode::Leave;
            c.mode_until = tick + u64::from(self.rules.creature.leave_ticks);
            c.path.clear();
            c.goal = None;
            return true;
        }
        // A raider next to a colonist strikes at once.
        if let Some((vid, vt)) = self.nearest_colonist(c.tile(), 1)
            && self.can_strike(c.tile(), vt)
        {
            c.path.clear();
            c.goal = None;
            c.progress = 0;
            if tick >= c.next_attack {
                self.creature_strike(c, stats, vid);
            }
            return true;
        }
        if let Some((stand, wall)) = c.breach {
            if self.raid_break_ticks(wall.0, wall.1).is_none() {
                c.breach = None;
            } else if c.tile() == stand {
                let ticks = self.raid_break_ticks(wall.0, wall.1).unwrap_or(1);
                c.breaking = Some((wall, ticks.max(1)));
                return true;
            } else {
                if c.path.is_empty() && tick >= c.next_think {
                    c.next_think = tick + u64::from(self.rules.creature.think_ticks);
                    if !self.creature_path(c, stand) {
                        c.breach = None;
                    }
                }
                return true;
            }
        }
        if tick < c.next_think {
            return true;
        }
        c.next_think = tick + u64::from(self.rules.creature.think_ticks);
        // Target: a colonist in sight, then the best stack, then any colonist.
        let sight = stats.sight;
        let colonist = self.nearest_colonist(c.tile(), sight);
        let target = match colonist {
            Some((_, t)) => Some((t, true)),
            None => match self.raid_loot_target(c.tile()) {
                Some(t) => Some((t, false)),
                None => self
                    .nearest_colonist(c.tile(), u32::MAX)
                    .map(|(_, t)| (t, true)),
            },
        };
        let Some((goal, is_colonist)) = target else {
            c.mode = Mode::Leave;
            c.mode_until = tick + u64::from(self.rules.creature.leave_ticks);
            return true;
        };
        if !is_colonist && c.tile() == goal {
            self.raider_take(c);
            return true;
        }
        let reachable = self.regions.connected(
            (i32::from(c.x), i32::from(c.y)),
            (i32::from(goal.0), i32::from(goal.1)),
        );
        if reachable {
            let stale = c.goal != Some(goal);
            if c.path.is_empty() || stale {
                self.creature_path(c, goal);
            }
            if !is_colonist && c.path.is_empty() && c.tile() == goal {
                self.raider_take(c);
            }
        } else if let Some(b) = self.breach_target(c.tile(), goal) {
            c.breach = Some(b);
            c.path.clear();
            c.goal = None;
            c.next_think = tick;
        } else {
            c.mode = Mode::Leave;
            c.mode_until = tick + u64::from(self.rules.creature.leave_ticks);
        }
        true
    }

    /// The raider walks to the entry tile and leaves. Return false when it is gone.
    fn raider_leave(&mut self, c: &mut Creature, entry: Option<(u16, u16)>) -> bool {
        let tick = self.tick;
        let exit = entry.unwrap_or(c.home);
        if tick >= c.mode_until || cheb(c.tile(), exit) <= 1 {
            return false;
        }
        let stale = c.goal != Some(exit);
        if (c.path.is_empty() || stale) && tick >= c.next_think {
            c.next_think = tick + u64::from(self.rules.creature.think_ticks);
            self.creature_path(c, exit);
        }
        true
    }

    /// The most valuable stack on a tile that nobody reserved: the tile. Ties go to the tile
    /// nearer to the raider, then to the item order.
    fn raid_loot_target(&self, from: (u16, u16)) -> Option<(u16, u16)> {
        let (cap, taken) = self
            .raid
            .active
            .as_ref()
            .map(|r| {
                (
                    u64::from(self.rules.raid.loot_per_raider) * u64::from(r.fighters),
                    u64::from(r.stacks_taken),
                )
            })
            .unwrap_or((0, 0));
        if taken >= cap {
            return None;
        }
        let mut best: Option<(u64, u32, (u16, u16))> = None;
        for g in self.tribute_goods() {
            let Some(s) = self.items.get(g.item) else {
                continue;
            };
            let crate::item::ItemLoc::Tile(x, y) = s.loc else {
                continue;
            };
            let value = u64::from(g.count) * u64::from(g.unit_value);
            let d = cheb(from, (x, y));
            let better = match best {
                None => true,
                Some((bv, bd, _)) => value > bv || (value == bv && d < bd),
            };
            if better {
                best = Some((value, d, (x, y)));
            }
        }
        best.map(|(_, _, t)| t)
    }

    /// The raider takes the best stack of the tile it stands on.
    fn raider_take(&mut self, c: &mut Creature) {
        let (x, y) = c.tile();
        let mut best: Option<(u64, crate::item::ItemId)> = None;
        for g in self.tribute_goods() {
            let Some(s) = self.items.get(g.item) else {
                continue;
            };
            if s.loc != crate::item::ItemLoc::Tile(x, y) {
                continue;
            }
            let value = u64::from(g.count) * u64::from(g.unit_value);
            if best.is_none_or(|(v, _)| value > v) {
                best = Some((value, g.item));
            }
        }
        let Some((value, item)) = best else {
            return;
        };
        let Some(stack) = self.items.remove(item) else {
            return;
        };
        c.loot = Some((stack.material, stack.count));
        let mut raid_id = 0;
        if let Some(raid) = self.raid.active.as_mut() {
            raid.stacks_taken += 1;
            raid.value_taken += value;
            raid_id = raid.id;
        }
        self.raid_events.push(crate::raid::RaidEvent::Looted {
            raid: raid_id,
            x,
            y,
            material: stack.material,
            count: stack.count,
            value,
        });
        self.combat_events.push(CombatEvent::RaiderLooted { x, y });
        c.mode = Mode::Leave;
        c.mode_until = self.tick + u64::from(self.rules.creature.leave_ticks);
        c.path.clear();
        c.goal = None;
    }

    /// The wall or door to break to get from a tile to a goal that walls cut off: the tile to
    /// stand on and the wall. It is the wall with a reachable free neighbor that is nearest to
    /// the goal. Ties go to the lower tile in `(y, x)` order.
    fn breach_target(
        &self,
        from: (u16, u16),
        goal: (u16, u16),
    ) -> Option<((u16, u16), (u16, u16))> {
        let mut best: Option<Breach> = None;
        for (x, y, layer, b) in self.buildings.iter() {
            if layer != BuildLayer::Object {
                continue;
            }
            let Some(def) = self.rules.building(b.def) else {
                continue;
            };
            if !matches!(def.kind, BuildingKind::Wall | BuildingKind::Door) {
                continue;
            }
            let score = cheb((x, y), goal);
            let d_from = cheb((x, y), from);
            if best.is_some_and(|(s, d, ..)| (s, d) <= (score, d_from)) {
                continue;
            }
            for (dx, dy) in [(0i32, -1i32), (-1, 0), (1, 0), (0, 1)] {
                let (nx, ny) = (i32::from(x) + dx, i32::from(y) + dy);
                if self.map.passable(nx, ny)
                    && self
                        .regions
                        .connected((i32::from(from.0), i32::from(from.1)), (nx, ny))
                {
                    best = Some((score, d_from, (nx as u16, ny as u16), (x, y)));
                    break;
                }
            }
        }
        best.map(|(_, _, stand, wall)| (stand, wall))
    }

    /// Check the creatures of a loaded `Sim`: indices, tiles, and materials. A colonist or creature ID
    /// that is gone is no fault: the code looks IDs up and skips the missing ones.
    pub(crate) fn creature_state_ok(&self) -> bool {
        let inside = |t: (u16, u16)| self.map.in_bounds(i32::from(t.0), i32::from(t.1));
        let kinds = self.rules.creature.kinds.len();
        let materials = self.rules.materials.len();
        self.creature.lairs.iter().all(|&l| inside(l))
            && self.creature.creatures.iter().all(|(_, c)| {
                usize::from(c.kind) < kinds
                    && inside(c.tile())
                    && inside(c.home)
                    && inside(c.threat)
                    && c.path.iter().all(|&t| inside(t))
                    && c.goal.is_none_or(inside)
                    && c.breaking.is_none_or(|(t, _)| inside(t))
                    && c.breach.is_none_or(|(a, b)| inside(a) && inside(b))
                    && c.loot
                        .is_none_or(|(m, n)| usize::from(m) < materials && n > 0)
            })
    }

    // ---- debug hooks (scenario tests) ----

    /// Put a creature of a kind on a tile. Return its ID. For tests.
    pub fn debug_spawn_creature(&mut self, kind: &str, x: u16, y: u16) -> Option<CreatureId> {
        let k = self.rules.creature.kind_index(kind)?;
        self.spawn_creature(k, x, y)
    }

    /// Let a beast or a horror hunt colonists. For tests.
    pub fn debug_drive_creature(&mut self, id: CreatureId) {
        if let Some(c) = self.creature.creatures.get_mut(id) {
            c.driven = true;
            c.mode = Mode::Chase;
        }
    }

    /// Put a lair on a tile with `count` sleeping beasts of a kind. For tests.
    pub fn debug_add_lair(&mut self, kind: &str, x: u16, y: u16, count: u32) {
        self.creature.lairs.push((x, y));
        for _ in 0..count {
            if let Some(id) = self.debug_spawn_creature(kind, x, y)
                && let Some(c) = self.creature.creatures.get_mut(id)
            {
                c.lair = true;
            }
        }
    }

    /// Set the health of a creature. For tests.
    pub fn debug_set_creature_health(&mut self, id: CreatureId, health: i32) {
        if let Some(c) = self.creature.creatures.get_mut(id) {
            c.health = health;
        }
    }

    /// Make a group of beasts arrive now, as if the daily roll hit. For tests.
    pub fn debug_spawn_beasts(&mut self) {
        self.spawn_beasts();
    }
}
