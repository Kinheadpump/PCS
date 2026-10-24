//! Rival colonies and raids (S18, R-070 to R-072).
//!
//! Local generation gives up to 3 rival colonies (`worldgen::EmbarkRival`). The hostility of a
//! rival colony is a sum of named contributions (history, wealth, Notice, and the result of past
//! raids). After the quiet days each rival colony rolls once a game day for a raid. A scout comes
//! to the map edge 1 game day before the raiders. When the raiders arrive, the raid waits for the
//! player: pay the tribute or fight (R-072).
//!
//! In a fight the raiders are creatures (S13, `creature.rs`): they walk in from the entry tile,
//! attack colonists, break doors and walls, and carry away the best stacks. A raider that dies
//! reports to `raid_fighters_down` and drops its loot. The raid ends at once when half of the
//! raiders are down. The other hooks are `raid_entry` and `raid_break_ticks` (time to break a door
//! or a wall).

use crate::Sim;
use crate::build::BuildLayer;
use crate::chronicle::RunState;
use crate::item::{ItemId, ItemLoc};
use crate::time::TICKS_PER_DAY;
use base::Pcg32;
use base::content::BuildingKind;
use base::rng::mix;
use serde::{Deserialize, Serialize};
use worldgen::EmbarkRival;
use worldgen::embark::{EXIT_E, EXIT_N, EXIT_S, EXIT_W};

/// Most hostility change from past raids, up or down.
const ADJUST_LIMIT: i32 = 100;
/// Tag of the raid random stream.
const TAG_RAID: u64 = 0x5241_4944_5f53_3138;

/// A rival colony. Part of the state.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RivalColony {
    /// Site ID of the settlement in the history.
    pub site: u32,
    /// Owner civilization ID.
    pub civ: u32,
    /// Culture of the owner.
    pub culture: u8,
    /// Fighters (R-070).
    pub strength: u8,
    /// The map edge that the raiders come from: 0 north, 1 west, 2 east, 3 south.
    pub edge: u8,
    /// Hostility from the history (R-070).
    pub base_hostility: u8,
    /// Hostility change from tributes and fights (R-072). From -100 to 100.
    pub adjust: i32,
    /// Game day of the last raid.
    pub last_raid_day: Option<u64>,
    /// Raids so far.
    pub raids: u32,
}

/// What a part of the hostility is made of.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HostilityCause {
    /// Wars with the owner of the embark area (R-070).
    History,
    /// Wealth of the colony.
    Wealth,
    /// Notice of the colony.
    Notice,
    /// Tributes and fights of earlier raids (R-072).
    Raids,
    /// Friends, weddings, and grudges between the colonists and the people of the rival colony
    /// (R-047).
    Ties,
}

/// One contribution to the hostility.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HostilityPart {
    /// What it is.
    pub cause: HostilityCause,
    /// Value that it adds. Can be negative.
    pub value: i32,
}

/// The hostility of a rival colony and its parts. `total` is the sum of the parts, from 0 to 100.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hostility {
    /// The sum, limited to 0 to 100.
    pub total: u32,
    /// The parts: history, wealth, Notice, raids, then ties.
    pub parts: Vec<HostilityPart>,
}

/// Where a raid is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RaidPhase {
    /// The scout is at the map edge. The raiders come when the lead time ends.
    Scouting,
    /// The raiders are here. The player chooses (R-072). The raid does not move.
    Awaiting,
    /// The fight (R-071).
    Fighting,
}

/// The result of a raid.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RaidOutcome {
    /// The colony paid the tribute.
    Paid,
    /// Half of the raiders were down: the colony won.
    Repelled,
    /// The raiders carried away items and left.
    Plundered,
    /// The raiders left with nothing.
    Left,
}

/// A raid that is under way. Part of the state.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Raid {
    /// Raid number from 1.
    pub id: u32,
    /// Index of the rival colony.
    pub rival: u16,
    /// Number of raiders (R-071).
    pub fighters: u32,
    /// Raiders that are dead or down.
    pub down: u32,
    /// Where the raid is.
    pub phase: RaidPhase,
    /// Tick of the scout.
    pub scout_tick: u64,
    /// Tick when the raiders arrive.
    pub arrive_tick: u64,
    /// Map-edge tile where the scout and the raiders come in.
    pub entry: (u16, u16),
    /// Tick when an unanswered raid starts as a fight. Used in `Awaiting`.
    pub deadline: u64,
    /// Value of the tribute (R-072).
    pub tribute_due: u64,
    /// Tick when the fight began.
    pub fight_tick: u64,
    /// True after the raiders got the order to leave.
    pub leaving: bool,
    /// Stacks carried away.
    pub stacks_taken: u32,
    /// Value carried away.
    pub value_taken: u64,
}

/// A raid that ended. Part of the state.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RaidRecord {
    /// Raid number.
    pub id: u32,
    /// Index of the rival colony.
    pub rival: u16,
    /// Game day of the end.
    pub day: u64,
    /// Number of raiders.
    pub fighters: u32,
    /// The result.
    pub outcome: RaidOutcome,
    /// Value carried away.
    pub value_taken: u64,
}

/// Rival colonies and raids. Part of the state.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct RaidState {
    pub(crate) rivals: Vec<RivalColony>,
    rng: Pcg32,
    next_id: u32,
    pub(crate) active: Option<Raid>,
    pub(crate) log: Vec<RaidRecord>,
}

impl RaidState {
    pub(crate) fn new(seed: u64) -> Self {
        Self {
            rivals: Vec::new(),
            rng: Pcg32::from_seed(mix(seed ^ TAG_RAID)),
            next_id: 1,
            active: None,
            log: Vec::new(),
        }
    }

    pub(crate) fn with_rivals(seed: u64, rivals: &[EmbarkRival]) -> Self {
        let mut state = Self::new(seed);
        state.rivals = rivals
            .iter()
            .map(|r| RivalColony {
                site: r.site,
                civ: r.civ,
                culture: r.culture,
                strength: r.strength,
                edge: r.edge,
                base_hostility: r.base_hostility,
                adjust: 0,
                last_raid_day: None,
                raids: 0,
            })
            .collect();
        state
    }
}

/// Why the sim refused a raid command.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RaidReject {
    /// No raid waits for an answer.
    NoChoice,
    /// The goods list has an item that does not exist, is not on a tile, is reserved, is in the
    /// list twice, or has more units than the stack.
    BadGoods,
    /// The goods are worth less than the tribute.
    TooLittle,
}

/// A thing that happened with a raid. The caller takes it with `Sim::drain_raid_events`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RaidEvent {
    /// The scout came to the map edge. The warning alert starts here.
    ScoutSeen {
        /// Raid number.
        raid: u32,
        /// Index of the rival colony.
        rival: u16,
        /// Entry tile column.
        x: u16,
        /// Entry tile row.
        y: u16,
        /// Number of raiders.
        fighters: u32,
        /// Tick when the raiders arrive.
        arrives_tick: u64,
    },
    /// The raiders arrived. The interface pauses the game and asks the player (R-072).
    Arrived {
        /// Raid number.
        raid: u32,
        /// Index of the rival colony.
        rival: u16,
        /// Entry tile column.
        x: u16,
        /// Entry tile row.
        y: u16,
        /// Number of raiders.
        fighters: u32,
        /// Value of the tribute.
        tribute_due: u64,
    },
    /// The fight began. `auto` is true if the player did not answer in time.
    FightBegan {
        /// Raid number.
        raid: u32,
        /// True if the time ran out.
        auto: bool,
    },
    /// The raiders carried away a stack.
    Looted {
        /// Raid number.
        raid: u32,
        /// Tile column of the stack.
        x: u16,
        /// Tile row of the stack.
        y: u16,
        /// Material index.
        material: u16,
        /// Units.
        count: u32,
        /// Value.
        value: u64,
    },
    /// The raid ended.
    Ended {
        /// Raid number.
        raid: u32,
        /// Index of the rival colony.
        rival: u16,
        /// The result.
        outcome: RaidOutcome,
        /// Hostility of the rival colony after the raid.
        hostility: u32,
    },
    /// A raid command was refused.
    Rejected(RaidReject),
}

/// The warning of an incoming raid (the scout is at the edge).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RaidWarning {
    /// Raid number.
    pub raid: u32,
    /// Index of the rival colony.
    pub rival: u16,
    /// Entry tile column.
    pub x: u16,
    /// Entry tile row.
    pub y: u16,
    /// Number of raiders.
    pub fighters: u32,
    /// Ticks until the raiders arrive.
    pub ticks_left: u64,
}

/// A stack that the player can give as tribute.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TributeGood {
    /// The stack.
    pub item: ItemId,
    /// Material index.
    pub material: u16,
    /// Units in the stack.
    pub count: u32,
    /// Value of one unit.
    pub unit_value: u32,
}

impl Sim {
    /// The rival colonies, nearest first.
    pub fn rivals(&self) -> &[RivalColony] {
        &self.raid.rivals
    }

    /// The hostility of a rival colony with its parts (R-070): history + wealth / 1,000 +
    /// Notice / 2, with the result of earlier raids, to 0 and 100.
    pub fn rival_hostility(&self, index: usize) -> Option<Hostility> {
        let rival = self.raid.rivals.get(index)?;
        let r = &self.rules.raid;
        let wealth = self.wealth_value() / u64::from(r.wealth_div.max(1));
        let notice = u64::from(self.notice_total()) / u64::from(r.notice_div.max(1));
        let parts = vec![
            HostilityPart {
                cause: HostilityCause::History,
                value: i32::from(rival.base_hostility),
            },
            HostilityPart {
                cause: HostilityCause::Wealth,
                value: i32::try_from(wealth).unwrap_or(i32::MAX),
            },
            HostilityPart {
                cause: HostilityCause::Notice,
                value: i32::try_from(notice).unwrap_or(i32::MAX),
            },
            HostilityPart {
                cause: HostilityCause::Raids,
                value: rival.adjust,
            },
            HostilityPart {
                cause: HostilityCause::Ties,
                value: self.neighbor_ties_value(index),
            },
        ];
        let sum = parts
            .iter()
            .fold(0i64, |a, p| a.saturating_add(i64::from(p.value)));
        Some(Hostility {
            total: u32::try_from(sum.clamp(0, 100)).unwrap_or(0),
            parts,
        })
    }

    /// The daily raid chance of a rival colony, in per-mille: hostility / 20 percent, to 5
    /// percent (R-071). 0 for a wrong index.
    pub fn raid_chance_permille(&self, index: usize) -> u32 {
        let r = &self.rules.raid;
        let Some(h) = self.rival_hostility(index) else {
            return 0;
        };
        (h.total * 1000 / (r.chance_div.max(1) * 100)).min(r.max_pct * 10)
    }

    /// The number of raiders that a rival colony sends now: `strength × hostility / 100`, at
    /// least `min_fighters` (R-071). 0 for a wrong index.
    pub fn raid_size(&self, index: usize) -> u32 {
        let (Some(rival), Some(h)) = (self.raid.rivals.get(index), self.rival_hostility(index))
        else {
            return 0;
        };
        (u32::from(rival.strength) * h.total / 100).max(self.rules.raid.min_fighters)
    }

    /// The raid that is under way, if any.
    pub fn raid(&self) -> Option<&Raid> {
        self.raid.active.as_ref()
    }

    /// The raids that ended, oldest first.
    pub fn raid_log(&self) -> &[RaidRecord] {
        &self.raid.log
    }

    /// The warning while the scout waits at the edge. `None` in other phases.
    pub fn raid_warning(&self) -> Option<RaidWarning> {
        let raid = self.raid.active.as_ref()?;
        (raid.phase == RaidPhase::Scouting).then(|| RaidWarning {
            raid: raid.id,
            rival: raid.rival,
            x: raid.entry.0,
            y: raid.entry.1,
            fighters: raid.fighters,
            ticks_left: raid.arrive_tick.saturating_sub(self.tick),
        })
    }

    /// True while the raiders wait for the answer of the player (R-072). The interface pauses
    /// the game and shows the choice.
    pub fn raid_choice_pending(&self) -> bool {
        self.raid
            .active
            .as_ref()
            .is_some_and(|r| r.phase == RaidPhase::Awaiting)
    }

    /// Value of the tribute of the raid that waits for an answer.
    pub fn tribute_due(&self) -> Option<u64> {
        let raid = self.raid.active.as_ref()?;
        (raid.phase == RaidPhase::Awaiting).then_some(raid.tribute_due)
    }

    /// The stacks that the player can give, in item order: stacks on tiles, with value, with no
    /// reservation.
    pub fn tribute_goods(&self) -> Vec<TributeGood> {
        self.items
            .iter()
            .filter_map(|(item, s)| {
                let unit_value = u32::try_from(self.material_value(s.material)).unwrap_or(u32::MAX);
                let free = matches!(s.loc, ItemLoc::Tile(..)) && s.reserved_by.is_none();
                (free && unit_value > 0).then_some(TributeGood {
                    item,
                    material: s.material,
                    count: s.count,
                    unit_value,
                })
            })
            .collect()
    }

    /// Total value of the goods that the player can give.
    pub fn tribute_goods_value(&self) -> u64 {
        self.tribute_goods()
            .iter()
            .map(|g| u64::from(g.count) * u64::from(g.unit_value))
            .sum()
    }

    /// True if the colony has enough goods for the tribute. The player cannot pay if not (R-072).
    pub fn can_pay_tribute(&self) -> bool {
        self.tribute_due()
            .is_some_and(|due| self.tribute_goods_value() >= due)
    }

    /// Take the raid events that the last ticks made.
    pub fn drain_raid_events(&mut self) -> Vec<RaidEvent> {
        std::mem::take(&mut self.raid_events)
    }

    /// S13 hook: the entry tile of the raid. The raiders come in here.
    pub fn raid_entry(&self) -> Option<(u16, u16)> {
        self.raid.active.as_ref().map(|r| r.entry)
    }

    /// S13 hook: the melee reports `count` more raiders dead or down. When half of the raiders are
    /// down, the raid ends at once as `Repelled`.
    pub fn raid_fighters_down(&mut self, count: u32) {
        let Some(raid) = self.raid.active.as_mut() else {
            return;
        };
        raid.down = raid.down.saturating_add(count).min(raid.fighters);
    }

    /// S13 hook: ticks that one raider needs to break the door or wall on a tile (R-071). The time
    /// comes from the hardness of the material. `None` if the tile has no wall or door.
    pub fn raid_break_ticks(&self, x: u16, y: u16) -> Option<u32> {
        let b = self.buildings.get(x, y, BuildLayer::Object)?;
        let def = self.rules.building(b.def)?;
        if !matches!(def.kind, BuildingKind::Wall | BuildingKind::Door) {
            return None;
        }
        let hardness = self.rules.materials.get(usize::from(b.material))?.hardness;
        Some(u32::from(hardness).saturating_mul(self.rules.raid.break_ticks_per_hardness))
    }

    /// Value of all items on tiles or carried, and all buildings.
    pub(crate) fn wealth_value(&self) -> u64 {
        let items: u64 = self
            .items
            .iter()
            .map(|(_, i)| u64::from(i.count) * self.material_value(i.material))
            .sum();
        let buildings: u64 = self
            .buildings
            .iter()
            .map(|(_, _, _, b)| u64::from(b.units) * self.material_value(b.material))
            .sum();
        items + buildings
    }

    /// The player pays the tribute with the chosen goods (R-072).
    pub(crate) fn raid_pay(&mut self, goods: &[(ItemId, u32)]) {
        let Some(due) = self.tribute_due() else {
            self.raid_events
                .push(RaidEvent::Rejected(RaidReject::NoChoice));
            return;
        };
        let mut seen: Vec<ItemId> = Vec::new();
        let mut value = 0u64;
        for &(id, count) in goods {
            let ok = !seen.contains(&id)
                && count > 0
                && self.items.get(id).is_some_and(|s| {
                    matches!(s.loc, ItemLoc::Tile(..))
                        && s.reserved_by.is_none()
                        && s.count >= count
                });
            let Some(s) = self.items.get(id).filter(|_| ok) else {
                self.raid_events
                    .push(RaidEvent::Rejected(RaidReject::BadGoods));
                return;
            };
            seen.push(id);
            value += u64::from(count) * self.material_value(s.material);
        }
        if value < due {
            self.raid_events
                .push(RaidEvent::Rejected(RaidReject::TooLittle));
            return;
        }
        for &(id, count) in goods {
            let whole = self.items.get(id).is_some_and(|s| s.count == count);
            if whole {
                self.items.remove(id);
            } else if let Some(part) = self.items.split_off(id, count) {
                self.items.remove(part);
            }
        }
        if let Some(raid) = self.raid.active.as_mut() {
            raid.value_taken = value;
        }
        self.raid_end(RaidOutcome::Paid);
    }

    /// The player chooses to fight (R-072).
    pub(crate) fn raid_fight(&mut self) {
        if !self.raid_choice_pending() {
            self.raid_events
                .push(RaidEvent::Rejected(RaidReject::NoChoice));
            return;
        }
        self.raid_begin_fight(false);
    }

    fn raid_begin_fight(&mut self, auto: bool) {
        let tick = self.tick;
        let Some(raid) = self.raid.active.as_mut() else {
            return;
        };
        raid.phase = RaidPhase::Fighting;
        raid.fight_tick = tick;
        let (id, fighters, entry) = (raid.id, raid.fighters, raid.entry);
        self.raid_events
            .push(RaidEvent::FightBegan { raid: id, auto });
        self.spawn_raiders(id, fighters, entry);
    }

    /// Called every tick after the tick number increased.
    pub(crate) fn raid_tick(&mut self) {
        if self.raid.active.is_some() {
            self.raid_advance();
        } else if self.tick > 0 && self.tick.is_multiple_of(TICKS_PER_DAY) {
            self.raid_daily();
        }
    }

    /// Each game day: every rival colony that is not on cooldown rolls for a raid (R-071). One
    /// raid at a time.
    fn raid_daily(&mut self) {
        let day = self.tick / TICKS_PER_DAY;
        let r = self.rules.raid;
        if day <= u64::from(r.quiet_days)
            || self.run.state != RunState::Running
            || self.colonists.is_empty()
        {
            return;
        }
        for i in 0..self.raid.rivals.len() {
            let cooling = self.raid.rivals[i]
                .last_raid_day
                .is_some_and(|d| day < d + u64::from(r.cooldown_days));
            let permille = self.raid_chance_permille(i);
            if cooling || permille == 0 {
                continue;
            }
            if self.raid.rng.chance(permille, 1000) && self.raid_schedule(i) {
                return;
            }
        }
    }

    /// Start a raid of rival colony `index`: the scout comes to a map-edge tile now. Return false
    /// if the map has no usable edge tile.
    pub(crate) fn raid_schedule(&mut self, index: usize) -> bool {
        let fighters = self.raid_size(index);
        let tribute_due = u64::from(self.rules.raid.tribute_per_raider) * u64::from(fighters);
        let Some(edge) = self.raid.rivals.get(index).map(|r| r.edge) else {
            return false;
        };
        let Some(entry) = self.raid_entry_tile(edge) else {
            return false;
        };
        let day = self.tick / TICKS_PER_DAY;
        let arrive_tick = self.tick + u64::from(self.rules.raid.scout_lead_ticks);
        let id = self.raid.next_id;
        self.raid.next_id += 1;
        let rival = u16::try_from(index).unwrap_or(u16::MAX);
        self.raid.active = Some(Raid {
            id,
            rival,
            fighters,
            down: 0,
            phase: RaidPhase::Scouting,
            scout_tick: self.tick,
            arrive_tick,
            entry,
            deadline: 0,
            tribute_due,
            fight_tick: 0,
            leaving: false,
            stacks_taken: 0,
            value_taken: 0,
        });
        if let Some(r) = self.raid.rivals.get_mut(index) {
            r.last_raid_day = Some(day);
            r.raids += 1;
        }
        self.raid_events.push(RaidEvent::ScoutSeen {
            raid: id,
            rival,
            x: entry.0,
            y: entry.1,
            fighters,
            arrives_tick: arrive_tick,
        });
        true
    }

    /// A passable edge tile in the region of the colonists. Start on the given edge at a random
    /// place and go on along the edge, then try the next edges.
    pub(crate) fn raid_entry_tile(&mut self, edge: u8) -> Option<(u16, u16)> {
        for home in self.home_tiles() {
            for k in 0..4u8 {
                let e = usize::from((edge + k) % 4);
                let len = if e == EXIT_N || e == EXIT_S {
                    self.map.width()
                } else {
                    self.map.height()
                };
                let offset = self.raid.rng.below(len) as i32;
                if let Some(t) = self.edge_scan(e, offset, home) {
                    return Some(t);
                }
            }
        }
        None
    }

    /// One colonist tile for each region that holds colonists, in slot order. A colonist who is
    /// shut in a room is not the home of the colony: the entry tiles of creatures try the next
    /// region if the first has no edge tile.
    pub(crate) fn home_tiles(&self) -> Vec<(u16, u16)> {
        let mut seen: Vec<u32> = Vec::new();
        let mut homes = Vec::new();
        for (_, c) in self.colonists.iter() {
            let id = self.regions.id_at(i32::from(c.x), i32::from(c.y));
            if !seen.contains(&id) {
                seen.push(id);
                homes.push(c.tile());
            }
        }
        homes
    }

    /// The first passable tile along a map edge from `offset`, in the region of `home`.
    pub(crate) fn edge_scan(&self, e: usize, offset: i32, home: (u16, u16)) -> Option<(u16, u16)> {
        let (w, h) = (self.map.width() as i32, self.map.height() as i32);
        let len = if e == EXIT_N || e == EXIT_S { w } else { h };
        for j in 0..len {
            let t = (offset + j) % len;
            let (x, y) = match e {
                EXIT_N => (t, 0),
                EXIT_W => (0, t),
                EXIT_E => (w - 1, t),
                EXIT_S => (t, h - 1),
                _ => return None,
            };
            if self.map.passable(x, y)
                && self
                    .regions
                    .connected((x, y), (i32::from(home.0), i32::from(home.1)))
            {
                return Some((x as u16, y as u16));
            }
        }
        None
    }

    fn raid_advance(&mut self) {
        let Some(raid) = self.raid.active.as_ref() else {
            return;
        };
        match raid.phase {
            RaidPhase::Scouting => {
                if self.tick >= raid.arrive_tick {
                    let deadline = self.tick + u64::from(self.rules.raid.decision_ticks);
                    let event = RaidEvent::Arrived {
                        raid: raid.id,
                        rival: raid.rival,
                        x: raid.entry.0,
                        y: raid.entry.1,
                        fighters: raid.fighters,
                        tribute_due: raid.tribute_due,
                    };
                    if let Some(r) = self.raid.active.as_mut() {
                        r.phase = RaidPhase::Awaiting;
                        r.deadline = deadline;
                    }
                    self.raid_events.push(event);
                }
            }
            RaidPhase::Awaiting => {
                if self.tick >= raid.deadline {
                    self.raid_begin_fight(true);
                }
            }
            RaidPhase::Fighting => self.raid_fight_step(),
        }
    }

    /// One tick of the fight (R-071). The raiders are creatures (`creature.rs`): the fight ends
    /// when half of them are down, or when all of them are gone from the map. They get the order to
    /// leave after `stay_ticks`, or when they carry away as many stacks as the rules allow.
    fn raid_fight_step(&mut self) {
        let r = self.rules.raid;
        let leave_ticks = u64::from(self.rules.creature.leave_ticks);
        let Some(raid) = self.raid.active.as_ref() else {
            return;
        };
        let (id, fighters, down, taken) = (raid.id, raid.fighters, raid.down, raid.stacks_taken);
        let stayed = self.tick >= raid.fight_tick + u64::from(r.stay_ticks);
        let leaving = raid.leaving;
        let overdue = self.tick >= raid.fight_tick + u64::from(r.stay_ticks) + leave_ticks;
        if down * 2 >= fighters {
            self.raid_end(RaidOutcome::Repelled);
            return;
        }
        let left_outcome = if taken > 0 {
            RaidOutcome::Plundered
        } else {
            RaidOutcome::Left
        };
        let loaded = taken >= r.loot_per_raider.saturating_mul(fighters - down);
        if !leaving && (stayed || loaded) {
            if let Some(raid) = self.raid.active.as_mut() {
                raid.leaving = true;
            }
            self.recall_raiders(id);
        }
        if self.raiders_on_map(id) == 0 || overdue {
            self.raid_end(left_outcome);
        }
    }

    /// End the raid: change the hostility (R-072), write the log, and tell the interface.
    fn raid_end(&mut self, outcome: RaidOutcome) {
        let r = self.rules.raid;
        let Some(raid) = self.raid.active.take() else {
            return;
        };
        if outcome != RaidOutcome::Repelled {
            // The raiders that are left are gone. After a won fight they walk off the map.
            self.remove_raiders(raid.id);
        }
        let to_i32 = |v: u32| i32::try_from(v).unwrap_or(i32::MAX);
        let mut change = 0i32;
        match outcome {
            RaidOutcome::Paid => change -= to_i32(r.pay_hostility),
            RaidOutcome::Repelled => change -= to_i32(r.win_hostility),
            RaidOutcome::Plundered | RaidOutcome::Left => {}
        }
        if outcome != RaidOutcome::Paid && raid.stacks_taken > 0 {
            change += to_i32(r.loot_hostility);
        }
        let index = usize::from(raid.rival);
        if let Some(rival) = self.raid.rivals.get_mut(index) {
            rival.adjust = (rival.adjust + change).clamp(-ADJUST_LIMIT, ADJUST_LIMIT);
        }
        self.crisis_survived();
        let hostility = self.rival_hostility(index).map_or(0, |h| h.total);
        self.raid.log.push(RaidRecord {
            id: raid.id,
            rival: raid.rival,
            day: self.tick / TICKS_PER_DAY,
            fighters: raid.fighters,
            outcome,
            value_taken: raid.value_taken,
        });
        self.raid_events.push(RaidEvent::Ended {
            raid: raid.id,
            rival: raid.rival,
            outcome,
            hostility,
        });
    }

    /// True if the raid state fits the map. Called after a load.
    pub(crate) fn raid_state_ok(&self) -> bool {
        let inside = |(x, y): (u16, u16)| self.map.in_bounds(i32::from(x), i32::from(y));
        let rivals_ok = self.raid.rivals.len() <= 8
            && self
                .raid
                .rivals
                .iter()
                .all(|r| r.edge < 4 && r.base_hostility <= 100);
        let active_ok = self.raid.active.as_ref().is_none_or(|a| {
            usize::from(a.rival) < self.raid.rivals.len() && a.down <= a.fighters && inside(a.entry)
        });
        rivals_ok && active_ok
    }

    // ---- debug hooks (scenario tests) ----

    /// Add a rival colony. For tests.
    pub fn debug_add_rival(&mut self, strength: u8, base_hostility: u8, edge: u8) {
        self.raid.rivals.push(RivalColony {
            site: 0,
            civ: 0,
            culture: 0,
            strength,
            edge: edge % 4,
            base_hostility: base_hostility.min(100),
            adjust: 0,
            last_raid_day: None,
            raids: 0,
        });
    }

    /// Start a raid of a rival colony now, as if the daily roll hit. For tests.
    pub fn debug_start_raid(&mut self, index: usize) -> bool {
        self.raid.active.is_none() && self.raid_schedule(index)
    }
}
