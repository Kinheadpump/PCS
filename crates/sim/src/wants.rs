//! Wants and personality (S20, R-033, R-034).
//!
//! Each colonist of a new colony or a migrant group has a social trait (introvert or
//! extrovert) and one life goal. Each game hour the sim checks the wants. A met want gives a
//! timed mood memory, and an unmet want gives a bad one. The memories are in `needs.ron`, so
//! the mood why chain shows each of them by name. Colonists of a scenario map have no social
//! trait and no goal, so they have no want memories.

use crate::Sim;
use crate::colonist::{ColonistId, NeedAct};
use crate::loader::{Issues, parse};
use crate::needs::NeedRules;
use crate::skill::Skill;
use base::content::{BuildingKind, ContentError};
use base::{Pcg32, rng::mix};
use serde::{Deserialize, Serialize};

/// Tag of the random stream that picks personalities.
const TAG_WANTS: u64 = 0x5741_4e54_5350_4552;

/// The social trait of a colonist (R-033).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Social {
    /// No social trait.
    Neutral,
    /// Wants time alone and dislikes crowds.
    Introvert,
    /// Wants company.
    Extrovert,
}

impl Social {
    /// Display name.
    pub fn name(self) -> &'static str {
        match self {
            Social::Neutral => "Neutral",
            Social::Introvert => "Introvert",
            Social::Extrovert => "Extrovert",
        }
    }

    /// The value that the colonist stores.
    pub(crate) fn code(self) -> u8 {
        self as u8
    }

    /// The social trait for a stored value.
    pub(crate) fn from_code(code: u8) -> Self {
        match code {
            1 => Social::Introvert,
            2 => Social::Extrovert,
            _ => Social::Neutral,
        }
    }
}

/// What a life goal asks for.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub enum GoalKind {
    /// The colonist slept in a room with exactly one bed.
    PrivateRoom,
    /// A skill of the given level, by display name.
    Skill(String, u8),
    /// The colony food lasts this many days.
    Food(u32),
}

/// One life goal in `wants.ron`.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GoalDef {
    /// String ID.
    pub id: String,
    /// Display name.
    pub name: String,
    /// What the goal asks for.
    pub kind: GoalKind,
}

/// Indices of the memories of `needs.ron` that the wants give.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WantMemories {
    /// `had_company`.
    pub had_company: u8,
    /// `lonely`.
    pub lonely: u8,
    /// `time_alone`.
    pub time_alone: u8,
    /// `crowded`.
    pub crowded: u8,
    /// `goal_met`.
    pub goal_met: u8,
    /// `goal_unmet`.
    pub goal_unmet: u8,
}

/// Raw `wants.ron` with the linked memory indices.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WantRules {
    /// Tile distance in which a colonist is near another one.
    pub company_radius: u16,
    /// Percent of new colonists that are introverts.
    pub introvert_pct: u32,
    /// Percent of new colonists that are extroverts.
    pub extrovert_pct: u32,
    /// Hours alone in a row until an extrovert is lonely.
    pub lonely_after_hours: u32,
    /// Colonists near that make an introvert feel crowded.
    pub crowd_count: u32,
    /// Hours of a crowd in a row until an introvert is crowded.
    pub crowd_after_hours: u32,
    /// Hours that a goal can stay unmet before it costs mood.
    pub goal_patience_hours: u32,
    /// Life goals, in file order.
    pub goals: Vec<GoalDef>,
    /// Linked memory indices.
    #[serde(skip)]
    pub memories: WantMemories,
}

impl WantRules {
    /// Index of the goal with a string ID.
    pub fn goal_index(&self, id: &str) -> Option<u8> {
        self.goals
            .iter()
            .position(|g| g.id == id)
            .and_then(|i| u8::try_from(i).ok())
    }
}

/// Load and check `wants.ron`. `needs` gives the memory keys.
pub fn load_want_rules(
    file: &str,
    text: &str,
    needs: &NeedRules,
) -> Result<WantRules, ContentError> {
    let mut r: WantRules = parse(file, text)?;
    let mut issues = Issues::new(file);
    if r.goals.is_empty() || r.goals.len() > 200 {
        issues.add("goals", "must hold 1 to 200 goals");
    }
    for (i, g) in r.goals.iter().enumerate() {
        if r.goals[..i].iter().any(|o| o.id == g.id) {
            issues.add(&g.id, "duplicate ID");
        }
        if let GoalKind::Skill(name, level) = &g.kind {
            if !Skill::ALL.iter().any(|s| s.name() == name) {
                issues.add(&g.id, "unknown skill");
            }
            if *level == 0 {
                issues.add(&g.id, "the skill level must be 1 or more");
            }
        }
    }
    if r.introvert_pct + r.extrovert_pct > 100 {
        issues.add(
            "introvert_pct",
            "the percent values must sum to 100 or less",
        );
    }
    if r.company_radius == 0 || r.crowd_count == 0 {
        issues.add(
            "company_radius",
            "company_radius and crowd_count must be 1 or more",
        );
    }
    let mut find = |key: &str| -> u8 {
        needs.memory_index(key).unwrap_or_else(|| {
            issues.add(key, "the memory is not in needs.ron");
            0
        })
    };
    r.memories = WantMemories {
        had_company: find("had_company"),
        lonely: find("lonely"),
        time_alone: find("time_alone"),
        crowded: find("crowded"),
        goal_met: find("goal_met"),
        goal_unmet: find("goal_unmet"),
    };
    issues.finish(r)
}

/// The state of a life goal, for the UI.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GoalReport {
    /// String ID of the goal.
    pub id: String,
    /// Display name.
    pub name: String,
    /// True when the goal is met now.
    pub met: bool,
    /// Progress toward the goal (skill level, days of food, or 0/1).
    pub progress: u32,
    /// The value that the goal asks for.
    pub target: u32,
    /// Hours in a row that the goal was not met.
    pub unmet_hours: u32,
    /// Hours of patience before an unmet goal costs mood.
    pub patience_hours: u32,
}

/// The wants of a colonist, for the UI.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Wants {
    /// Social trait.
    pub social: Social,
    /// Hours in a row without company.
    pub alone_hours: u32,
    /// Hours in a row in a crowd.
    pub crowd_hours: u32,
    /// The life goal, if the colonist has one.
    pub goal: Option<GoalReport>,
}

impl Sim {
    /// The tile distance test of "near": at most `company_radius` tiles on both axes.
    fn near_count(&self, id: ColonistId) -> u32 {
        let radius = i32::from(self.rules.wants.company_radius);
        let Some(me) = self.colonists.get(id) else {
            return 0;
        };
        let count = self
            .colonists
            .iter()
            .filter(|(other, c)| {
                *other != id
                    && (i32::from(c.x) - i32::from(me.x)).abs() <= radius
                    && (i32::from(c.y) - i32::from(me.y)).abs() <= radius
            })
            .count();
        u32::try_from(count).unwrap_or(u32::MAX)
    }

    /// Number of beds in the room that holds a tile. 0 if the tile is in no room.
    pub fn beds_in_room(&self, x: u16, y: u16) -> usize {
        let Some(room) = self.room_at(x, y) else {
            return 0;
        };
        self.buildings
            .object_tiles(|def| self.rules.is_kind(def, BuildingKind::Bed))
            .into_iter()
            .filter(|&(bx, by)| {
                self.room_at(bx, by)
                    .is_some_and(|r| r.tiles.first() == room.tiles.first())
            })
            .count()
    }

    /// The progress, the target, and the state of the life goal of a colonist.
    fn goal_progress(
        &self,
        id: ColonistId,
        kind: &GoalKind,
        food_tenths: Option<u64>,
    ) -> (u32, u32) {
        let Some(c) = self.colonists.get(id) else {
            return (0, 1);
        };
        match kind {
            GoalKind::PrivateRoom => (u32::from(c.sleep_room_beds == 1), 1),
            GoalKind::Skill(name, level) => {
                let level_now = Skill::ALL
                    .iter()
                    .find(|s| s.name() == name)
                    .and_then(|&s| self.skill_level(id, s))
                    .unwrap_or(0);
                (
                    u32::from(level_now).min(u32::from(*level)),
                    u32::from(*level),
                )
            }
            GoalKind::Food(days) => {
                // A colony that uses no food has enough of it.
                let now = food_tenths.map_or(*days, |t| u32::try_from(t / 10).unwrap_or(u32::MAX));
                (now.min(*days), *days)
            }
        }
    }

    fn goal_report_of(&self, id: ColonistId, food_tenths: Option<u64>) -> Option<GoalReport> {
        let c = self.colonists.get(id)?;
        let def = self
            .rules
            .wants
            .goals
            .get(usize::from(c.goal.checked_sub(1)?))?;
        let (progress, target) = self.goal_progress(id, &def.kind, food_tenths);
        Some(GoalReport {
            id: def.id.clone(),
            name: def.name.clone(),
            met: progress >= target,
            progress,
            target,
            unmet_hours: c.goal_unmet_hours,
            patience_hours: self.rules.wants.goal_patience_hours,
        })
    }

    /// The social trait and life goal of a colonist (R-033).
    pub fn wants(&self, id: ColonistId) -> Option<Wants> {
        let c = self.colonists.get(id)?;
        let food = self.ledger().food.days_remaining_tenths;
        Some(Wants {
            social: Social::from_code(c.social),
            alone_hours: u32::from(c.alone_hours),
            crowd_hours: u32::from(c.crowd_hours),
            goal: self.goal_report_of(id, food),
        })
    }

    /// The social trait of a colonist.
    pub fn social(&self, id: ColonistId) -> Option<Social> {
        Some(Social::from_code(self.colonists.get(id)?.social))
    }

    /// Pick a social trait and a goal for a colonist from a salt. The roll uses its own random
    /// stream, so other systems keep their numbers.
    pub(crate) fn roll_wants(&mut self, id: ColonistId, salt: u64) {
        let r = &self.rules.wants;
        if r.goals.is_empty() {
            return;
        }
        let mut rng = Pcg32::from_seed(mix(salt ^ TAG_WANTS));
        let pick = rng.below(100);
        let social = if pick < r.introvert_pct {
            Social::Introvert
        } else if pick < r.introvert_pct + r.extrovert_pct {
            Social::Extrovert
        } else {
            Social::Neutral
        };
        let goal = rng.below(u32::try_from(r.goals.len()).unwrap_or(1));
        if let Some(c) = self.colonists.get_mut(id) {
            c.social = social.code();
            c.goal = u8::try_from(goal + 1).unwrap_or(0);
        }
    }

    /// Each game hour: the company counters, the social memories, and the life goal.
    /// Colonists run in slot order.
    pub(crate) fn wants_tick(&mut self) {
        let rules = self.rules.clone();
        let r = &rules.wants;
        let ids: Vec<ColonistId> = self.colonists.iter().map(|(id, _)| id).collect();
        let food = self
            .colonists
            .iter()
            .any(|(_, c)| c.goal > 0)
            .then(|| self.ledger().food.days_remaining_tenths)
            .flatten();
        for id in ids {
            let Some(c) = self.colonists.get(id) else {
                continue;
            };
            let asleep = matches!(c.act, Some(NeedAct::Sleep { asleep: true, .. }));
            let social = Social::from_code(c.social);
            let near = self.near_count(id);
            let mut gives: Vec<u8> = Vec::new();
            let (mut alone, mut crowd) = (c.alone_hours, c.crowd_hours);
            if !asleep {
                match social {
                    Social::Extrovert => {
                        if near > 0 {
                            alone = 0;
                            gives.push(r.memories.had_company);
                        } else {
                            alone = alone.saturating_add(1);
                            if u32::from(alone) >= r.lonely_after_hours {
                                gives.push(r.memories.lonely);
                            }
                        }
                    }
                    Social::Introvert => {
                        if near == 0 {
                            crowd = 0;
                            gives.push(r.memories.time_alone);
                        } else if near >= r.crowd_count {
                            crowd = crowd.saturating_add(1);
                            if u32::from(crowd) >= r.crowd_after_hours {
                                gives.push(r.memories.crowded);
                            }
                        } else {
                            crowd = 0;
                        }
                    }
                    Social::Neutral => {}
                }
            }
            let mut unmet = c.goal_unmet_hours;
            if let Some(g) = self.goal_report_of(id, food) {
                if g.met {
                    unmet = 0;
                    gives.push(r.memories.goal_met);
                } else {
                    unmet = unmet.saturating_add(1);
                    if unmet >= r.goal_patience_hours {
                        gives.push(r.memories.goal_unmet);
                    }
                }
            }
            if let Some(c) = self.colonists.get_mut(id) {
                c.alone_hours = alone;
                c.crowd_hours = crowd;
                c.goal_unmet_hours = unmet;
            }
            for key in gives {
                self.give_memory(id, key);
            }
        }
    }

    /// Set the social trait of a colonist. For scenario tests and debug tools.
    pub fn debug_set_social(&mut self, id: ColonistId, social: Social) {
        if let Some(c) = self.colonists.get_mut(id) {
            c.social = social.code();
        }
    }

    /// Set the life goal of a colonist by string ID, or none. Return false for an unknown ID.
    pub fn debug_set_goal(&mut self, id: ColonistId, goal: Option<&str>) -> bool {
        let value = match goal {
            None => 0,
            Some(g) => match self.rules.wants.goal_index(g) {
                Some(i) => i + 1,
                None => return false,
            },
        };
        match self.colonists.get_mut(id) {
            Some(c) => {
                c.goal = value;
                c.goal_unmet_hours = 0;
                true
            }
            None => false,
        }
    }

    /// Set the number of beds in the room where the colonist last slept in a bed.
    pub fn debug_set_sleep_room(&mut self, id: ColonistId, beds: u8) {
        if let Some(c) = self.colonists.get_mut(id) {
            c.sleep_room_beds = beds;
        }
    }
}
