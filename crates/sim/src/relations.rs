//! Relationships and bonds (S21, R-035, R-036).
//!
//! Two colonists that are near each other get a relation. The opinion of the relation is the
//! sum of five named parts: time together, shared sleep, quarrels, temperament, and chemistry.
//! The opinion gives a standing: acquaintance, friend, rival, or enemy. Two free adults with a
//! high opinion become partners. The standing changes the mood through timed memories in
//! `needs.ron`, so the mood why chain shows each of them by name. A relation ends when one of
//! the two colonists dies or leaves.

use crate::Sim;
use crate::colonist::{ColonistId, NeedAct};
use crate::loader::{Issues, parse};
use crate::needs::NeedRules;
use crate::wants::Social;
use base::content::ContentError;
use base::rng::mix;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Tag of the chemistry numbers.
const TAG_RELATION: u64 = 0x5245_4c41_5449_4f4e;

/// What two colonists are to each other (R-035).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Standing {
    /// Neither friend nor foe.
    Acquaintance,
    /// A friend: the opinion is `friend_at` or more.
    Friend,
    /// A rival: the opinion is `rival_at` or less.
    Rival,
    /// An enemy: the opinion is `enemy_at` or less.
    Enemy,
    /// A partner.
    Partner,
}

impl Standing {
    /// Display name.
    pub fn name(self) -> &'static str {
        match self {
            Standing::Acquaintance => "Acquaintance",
            Standing::Friend => "Friend",
            Standing::Rival => "Rival",
            Standing::Enemy => "Enemy",
            Standing::Partner => "Partner",
        }
    }
}

/// Why an opinion has a part of its value (D-021).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OpinionCause {
    /// Hours awake near each other.
    Together {
        /// The hours.
        hours: u32,
    },
    /// Hours asleep near each other.
    SharedSleep {
        /// The hours.
        hours: u32,
    },
    /// Hours near each other while one of them was in a bad mood.
    Quarrels {
        /// The hours.
        hours: u32,
    },
    /// The social traits of the two.
    Temperament,
    /// A fixed number of the pair.
    Chemistry,
}

/// One named part of an opinion.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OpinionPart {
    /// The cause.
    pub cause: OpinionCause,
    /// Opinion points.
    pub value: i32,
}

/// The relation of two colonists with the why chain, for the UI.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RelationReport {
    /// The sum of all parts.
    pub opinion: i32,
    /// The standing.
    pub standing: Standing,
    /// The named parts.
    pub parts: Vec<OpinionPart>,
}

/// A change of standing, for the caller (`Sim::drain_relation_events`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RelationEvent {
    /// The first colonist.
    pub a: ColonistId,
    /// The second colonist.
    pub b: ColonistId,
    /// The old standing.
    pub from: Standing,
    /// The new standing.
    pub to: Standing,
}

/// Raw `relations.ron`, with the linked memory indices.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RelationRules {
    /// Tile distance in which two colonists are near each other.
    pub near_radius: u16,
    /// Hours together for 1 opinion point.
    pub together_hours_per_point: u32,
    /// Most points from time together.
    pub together_max: i32,
    /// Hours of shared sleep for 1 opinion point.
    pub sleep_hours_per_point: u32,
    /// Most points from shared sleep.
    pub sleep_max: i32,
    /// A colonist with a mood below this makes quarrels.
    pub quarrel_mood_below: i32,
    /// Hours of quarrels for 1 point less.
    pub quarrel_hours_per_point: u32,
    /// Most points that quarrels take away.
    pub quarrel_max: i32,
    /// Points for the same social trait.
    pub same_trait: i32,
    /// Points for an introvert and an extrovert.
    pub opposite_trait: i32,
    /// Chemistry goes from minus this to plus this.
    pub chemistry_max: i32,
    /// Opinion for a friend.
    pub friend_at: i32,
    /// Opinion for a rival.
    pub rival_at: i32,
    /// Opinion for an enemy.
    pub enemy_at: i32,
    /// Opinion for a partner.
    pub partner_at: i32,
    /// Age at which a colonist can have a partner.
    pub partner_min_age: u8,
    /// Linked memory indices.
    #[serde(skip)]
    pub memories: RelationMemories,
}

/// Indices of the memories of `needs.ron` that relations give.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RelationMemories {
    /// `near_friend`.
    pub near_friend: u8,
    /// `near_rival`.
    pub near_rival: u8,
    /// `near_enemy`.
    pub near_enemy: u8,
    /// `friend_died`.
    pub friend_died: u8,
    /// `partner_died`.
    pub partner_died: u8,
    /// `new_partner`.
    pub new_partner: u8,
}

/// Load and check `relations.ron`. `needs` gives the memory keys.
pub fn load_relation_rules(
    file: &str,
    text: &str,
    needs: &NeedRules,
) -> Result<RelationRules, ContentError> {
    let mut r: RelationRules = parse(file, text)?;
    let mut issues = Issues::new(file);
    if r.near_radius == 0
        || r.together_hours_per_point == 0
        || r.sleep_hours_per_point == 0
        || r.quarrel_hours_per_point == 0
    {
        issues.add(
            "near_radius",
            "the radius and the hour divisors must be 1 or more",
        );
    }
    if !(r.enemy_at < r.rival_at && r.rival_at < 0 && 0 < r.friend_at && r.friend_at < r.partner_at)
    {
        issues.add(
            "enemy_at",
            "need enemy_at < rival_at < 0 < friend_at < partner_at",
        );
    }
    if r.together_max < 0 || r.sleep_max < 0 || r.quarrel_max < 0 || r.chemistry_max < 0 {
        issues.add("together_max", "the limits must be 0 or more");
    }
    let mut find = |key: &str| -> u8 {
        needs.memory_index(key).unwrap_or_else(|| {
            issues.add(key, "the memory is not in needs.ron");
            0
        })
    };
    r.memories = RelationMemories {
        near_friend: find("near_friend"),
        near_rival: find("near_rival"),
        near_enemy: find("near_enemy"),
        friend_died: find("friend_died"),
        partner_died: find("partner_died"),
        new_partner: find("new_partner"),
    };
    issues.finish(r)
}

/// One relation. The key is the pair of colonist IDs in order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Relation {
    /// Hours awake near each other.
    pub together_hours: u32,
    /// Hours asleep near each other.
    pub sleep_hours: u32,
    /// Hours of quarrels.
    pub quarrel_hours: u32,
    /// The chemistry of the pair.
    pub chemistry: i32,
    /// True when the two are partners.
    pub partner: bool,
    /// The standing at the last hourly check.
    pub standing: Standing,
}

/// The relations of the colony.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelationState {
    /// The seed of the chemistry numbers.
    pub seed: u64,
    /// All relations, by the pair of colonist IDs (smaller ID first).
    pub map: BTreeMap<(ColonistId, ColonistId), Relation>,
}

impl RelationState {
    /// An empty state.
    pub fn new(seed: u64) -> Self {
        Self {
            seed,
            map: BTreeMap::new(),
        }
    }
}

fn pair(a: ColonistId, b: ColonistId) -> (ColonistId, ColonistId) {
    if a <= b { (a, b) } else { (b, a) }
}

/// The standing for an opinion.
fn standing_of(r: &RelationRules, opinion: i32, partner: bool) -> Standing {
    if partner {
        Standing::Partner
    } else if opinion <= r.enemy_at {
        Standing::Enemy
    } else if opinion <= r.rival_at {
        Standing::Rival
    } else if opinion >= r.friend_at {
        Standing::Friend
    } else {
        Standing::Acquaintance
    }
}

/// The named parts of the opinion of a relation.
fn parts_of(r: &RelationRules, rel: &Relation, a: Social, b: Social) -> Vec<OpinionPart> {
    let up = |hours: u32, per: u32, max: i32| {
        i32::try_from(hours / per.max(1))
            .unwrap_or(i32::MAX)
            .min(max)
    };
    let temperament = match (a, b) {
        (Social::Neutral, _) | (_, Social::Neutral) => 0,
        (x, y) if x == y => r.same_trait,
        _ => r.opposite_trait,
    };
    vec![
        OpinionPart {
            cause: OpinionCause::Together {
                hours: rel.together_hours,
            },
            value: up(
                rel.together_hours,
                r.together_hours_per_point,
                r.together_max,
            ),
        },
        OpinionPart {
            cause: OpinionCause::SharedSleep {
                hours: rel.sleep_hours,
            },
            value: up(rel.sleep_hours, r.sleep_hours_per_point, r.sleep_max),
        },
        OpinionPart {
            cause: OpinionCause::Quarrels {
                hours: rel.quarrel_hours,
            },
            value: -up(rel.quarrel_hours, r.quarrel_hours_per_point, r.quarrel_max),
        },
        OpinionPart {
            cause: OpinionCause::Temperament,
            value: temperament,
        },
        OpinionPart {
            cause: OpinionCause::Chemistry,
            value: rel.chemistry,
        },
    ]
}

impl Sim {
    fn social_of(&self, id: ColonistId) -> Social {
        self.colonists
            .get(id)
            .map_or(Social::Neutral, |c| Social::from_code(c.social))
    }

    /// The relation of two colonists with the why chain (R-035). `None` if they have none.
    pub fn relation(&self, a: ColonistId, b: ColonistId) -> Option<RelationReport> {
        let rel = self.relation_state.map.get(&pair(a, b))?;
        let parts = parts_of(
            &self.rules.relations,
            rel,
            self.social_of(a),
            self.social_of(b),
        );
        let opinion = parts.iter().map(|p| p.value).sum();
        Some(RelationReport {
            opinion,
            standing: standing_of(&self.rules.relations, opinion, rel.partner),
            parts,
        })
    }

    /// All relations of a colonist: the other colonist, the standing, and the opinion. Sorted by
    /// the opinion, highest first, then by ID.
    pub fn relations_of(&self, id: ColonistId) -> Vec<(ColonistId, Standing, i32)> {
        let mut out: Vec<(ColonistId, Standing, i32)> = self
            .relation_state
            .map
            .keys()
            .filter_map(|&(x, y)| {
                let other = if x == id {
                    y
                } else if y == id {
                    x
                } else {
                    return None;
                };
                let r = self.relation(id, other)?;
                Some((other, r.standing, r.opinion))
            })
            .collect();
        out.sort_by(|p, q| q.2.cmp(&p.2).then(p.0.cmp(&q.0)));
        out
    }

    /// The partner of a colonist, if it has one.
    pub fn partner_of(&self, id: ColonistId) -> Option<ColonistId> {
        self.relation_state
            .map
            .iter()
            .find(|(k, rel)| rel.partner && (k.0 == id || k.1 == id))
            .map(|(k, _)| if k.0 == id { k.1 } else { k.0 })
    }

    /// Take the standing changes that the last ticks made.
    pub fn drain_relation_events(&mut self) -> Vec<RelationEvent> {
        std::mem::take(&mut self.relation_events)
    }

    fn chemistry_of(&self, a: ColonistId, b: ColonistId) -> i32 {
        let max = self.rules.relations.chemistry_max;
        let key = (u64::from(a.index()) << 32) | u64::from(b.index());
        let roll = mix(self.relation_state.seed ^ TAG_RELATION ^ mix(key));
        let span = u64::try_from(2 * max + 1).unwrap_or(1);
        i32::try_from(roll % span).unwrap_or(0) - max
    }

    /// Each game hour: update the relations, the standings, the partners, and the memories.
    /// Pairs run in ID order.
    pub(crate) fn relations_tick(&mut self) {
        let rules = self.rules.clone();
        let r = &rules.relations;
        let radius = i32::from(r.near_radius);
        // The state of each colonist: tile, asleep, bad mood.
        let list: Vec<(ColonistId, i32, i32, bool, bool)> = self
            .colonists
            .iter()
            .map(|(id, c)| {
                let asleep = matches!(c.act, Some(NeedAct::Sleep { asleep: true, .. }));
                let bad = self
                    .mood(id)
                    .is_some_and(|m| m.total < r.quarrel_mood_below);
                (id, i32::from(c.x), i32::from(c.y), asleep, bad)
            })
            .collect();
        for (i, &(a, ax, ay, a_asleep, a_bad)) in list.iter().enumerate() {
            for &(b, bx, by, b_asleep, b_bad) in &list[i + 1..] {
                if (ax - bx).abs() > radius || (ay - by).abs() > radius {
                    continue;
                }
                let key = pair(a, b);
                if !self.relation_state.map.contains_key(&key) {
                    let chemistry = self.chemistry_of(key.0, key.1);
                    self.relation_state.map.insert(
                        key,
                        Relation {
                            together_hours: 0,
                            sleep_hours: 0,
                            quarrel_hours: 0,
                            chemistry,
                            partner: false,
                            standing: Standing::Acquaintance,
                        },
                    );
                }
                let Some(rel) = self.relation_state.map.get_mut(&key) else {
                    continue;
                };
                if a_asleep && b_asleep {
                    rel.sleep_hours = rel.sleep_hours.saturating_add(1);
                } else if !a_asleep && !b_asleep {
                    rel.together_hours = rel.together_hours.saturating_add(1);
                    if a_bad || b_bad {
                        rel.quarrel_hours = rel.quarrel_hours.saturating_add(1);
                    }
                }
            }
        }
        self.pick_partners();
        // New standings, events, and the memories of colonists that are near a known colonist.
        let keys: Vec<(ColonistId, ColonistId)> = self.relation_state.map.keys().copied().collect();
        let mut gives: Vec<(ColonistId, u8)> = Vec::new();
        for key in keys {
            let Some(report) = self.relation(key.0, key.1) else {
                continue;
            };
            let Some(rel) = self.relation_state.map.get_mut(&key) else {
                continue;
            };
            if rel.standing != report.standing {
                self.relation_events.push(RelationEvent {
                    a: key.0,
                    b: key.1,
                    from: rel.standing,
                    to: report.standing,
                });
                rel.standing = report.standing;
            }
            let near = |x: ColonistId| list.iter().find(|e| e.0 == x);
            let (Some(ea), Some(eb)) = (near(key.0), near(key.1)) else {
                continue;
            };
            if (ea.1 - eb.1).abs() > radius || (ea.2 - eb.2).abs() > radius {
                continue;
            }
            let memory = match report.standing {
                Standing::Friend | Standing::Partner => r.memories.near_friend,
                Standing::Rival => r.memories.near_rival,
                Standing::Enemy => r.memories.near_enemy,
                Standing::Acquaintance => continue,
            };
            if !ea.3 {
                gives.push((key.0, memory));
            }
            if !eb.3 {
                gives.push((key.1, memory));
            }
        }
        for (id, key) in gives {
            self.give_memory(id, key);
        }
    }

    /// Two free adults whose opinion is `partner_at` or more become partners. The pairs run in
    /// ID order, and a colonist has at most one partner.
    fn pick_partners(&mut self) {
        let r = self.rules.relations.clone();
        let keys: Vec<(ColonistId, ColonistId)> = self.relation_state.map.keys().copied().collect();
        for key in keys {
            let Some(report) = self.relation(key.0, key.1) else {
                continue;
            };
            if report.standing == Standing::Partner || report.opinion < r.partner_at {
                continue;
            }
            let adult = |x: ColonistId| {
                self.colonists
                    .get(x)
                    .is_some_and(|c| c.age >= r.partner_min_age)
            };
            if !adult(key.0)
                || !adult(key.1)
                || self.partner_of(key.0).is_some()
                || self.partner_of(key.1).is_some()
            {
                continue;
            }
            if let Some(rel) = self.relation_state.map.get_mut(&key) {
                rel.partner = true;
            }
            self.give_memory(key.0, r.memories.new_partner);
            self.give_memory(key.1, r.memories.new_partner);
        }
    }

    /// Remove all relations of a colonist that is gone. Return the other colonists with the
    /// standing that they had.
    pub(crate) fn relations_forget(&mut self, id: ColonistId) -> Vec<(ColonistId, Standing)> {
        let keys: Vec<(ColonistId, ColonistId)> = self
            .relation_state
            .map
            .keys()
            .filter(|k| k.0 == id || k.1 == id)
            .copied()
            .collect();
        let mut out = Vec::new();
        for key in keys {
            let other = if key.0 == id { key.1 } else { key.0 };
            if let Some(report) = self.relation(key.0, key.1) {
                out.push((other, report.standing));
            }
            self.relation_state.map.remove(&key);
        }
        out
    }

    /// The memories that a death gives to the friends and the partner of the dead.
    pub(crate) fn relations_mourn(&mut self, others: &[(ColonistId, Standing)]) {
        let m = self.rules.relations.memories;
        for &(other, standing) in others {
            match standing {
                Standing::Friend => self.give_memory(other, m.friend_died),
                Standing::Partner => self.give_memory(other, m.partner_died),
                _ => {}
            }
        }
    }

    /// Set the hours of a relation. For scenario tests and debug tools.
    pub fn debug_set_relation_hours(
        &mut self,
        a: ColonistId,
        b: ColonistId,
        together: u32,
        sleep: u32,
        quarrel: u32,
    ) {
        if let Some(rel) = self.relation_state.map.get_mut(&pair(a, b)) {
            rel.together_hours = together;
            rel.sleep_hours = sleep;
            rel.quarrel_hours = quarrel;
        }
    }

    /// Set the chemistry of a relation. For scenario tests and debug tools.
    pub fn debug_set_chemistry(&mut self, a: ColonistId, b: ColonistId, chemistry: i32) {
        if let Some(rel) = self.relation_state.map.get_mut(&pair(a, b)) {
            rel.chemistry = chemistry;
        }
    }

    /// Set the age of a colonist. For scenario tests and debug tools.
    pub fn debug_set_age(&mut self, id: ColonistId, age: u8) {
        if let Some(c) = self.colonists.get_mut(id) {
            c.age = age;
        }
    }
}
