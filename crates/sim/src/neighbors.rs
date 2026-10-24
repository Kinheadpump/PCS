//! People of other colonies (S24, R-041, R-042, R-043).
//!
//! Each rival colony has a few people. Their age, sex, and chemistry come from the seed, so the
//! state holds only the ties. Each game day a calm rival colony can have a contact: one colonist
//! meets one person. The opinion of a tie is the sum of three named parts: contact, chemistry,
//! and the grudge of the colonist against the rival colony. A free adult colonist whose opinion
//! of a person is high weds that person, and the person joins the colony. A colonist who loses a
//! partner, a friend, or kin in a raid holds a grudge against the raiding colony. Friends and
//! weddings lower the hostility of the rival colony, and grudges raise it.

use crate::Sim;
use crate::colonist::{Colonist, ColonistId, Sex};
use crate::life::DeathCause;
use crate::loader::{Issues, parse};
use crate::needs::NeedRules;
use crate::relations::{Relation, Standing};
use crate::time::TICKS_PER_DAY;
use base::content::ContentError;
use base::rng::mix;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Tag of the neighbor numbers.
const TAG_NEIGHBOR: u64 = 0x4e45_4947_4842_4f52;

/// Why a tie has a part of its opinion (D-021).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NeighborCause {
    /// Days of contact.
    Contact {
        /// The days.
        days: u32,
    },
    /// A fixed number of the pair.
    Chemistry,
    /// The grudge of the colonist against the rival colony.
    Grudge,
}

/// One named part of the opinion of a tie.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NeighborPart {
    /// The cause.
    pub cause: NeighborCause,
    /// Opinion points.
    pub value: i32,
}

/// A person of a rival colony.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NeighborPerson {
    /// Age in years.
    pub age: u8,
    /// Sex.
    pub sex: Sex,
    /// True when the person wed a colonist and left the rival colony.
    pub wed: bool,
}

/// The tie of a colonist with a person of a rival colony, with the why chain, for the UI.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NeighborTie {
    /// Index of the rival colony.
    pub rival: u16,
    /// The person in the rival colony.
    pub slot: u8,
    /// The person.
    pub person: NeighborPerson,
    /// The standing of the tie.
    pub standing: Standing,
    /// The sum of the parts.
    pub opinion: i32,
    /// The named parts: contact, chemistry, grudge.
    pub parts: Vec<NeighborPart>,
}

/// An event of the people of other colonies, for the caller (`Sim::drain_neighbor_events`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NeighborEvent {
    /// A colonist met a person of a rival colony.
    Met {
        /// The colonist.
        colonist: ColonistId,
        /// Index of the rival colony.
        rival: u16,
        /// The person.
        slot: u8,
    },
    /// The standing of a tie changed.
    Standing {
        /// The colonist.
        colonist: ColonistId,
        /// Index of the rival colony.
        rival: u16,
        /// The person.
        slot: u8,
        /// The old standing.
        from: Standing,
        /// The new standing.
        to: Standing,
    },
    /// A person of a rival colony wed a colonist and joined the colony.
    Wed {
        /// The colonist.
        colonist: ColonistId,
        /// The new colonist.
        partner: ColonistId,
        /// Index of the rival colony.
        rival: u16,
    },
    /// A colonist got a grudge against a rival colony after a death in a raid.
    Grudge {
        /// The colonist.
        colonist: ColonistId,
        /// Index of the rival colony.
        rival: u16,
        /// The grudge points that were added.
        amount: u32,
    },
}

/// Raw `neighbors.ron`, with the linked memory indices.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NeighborRules {
    /// People in each rival colony.
    pub people_per_rival: u8,
    /// Youngest age of a person.
    pub person_age_min: u8,
    /// Oldest age of a person.
    pub person_age_max: u8,
    /// A rival colony with a hostility above this has no contact.
    pub contact_hostility_max: u32,
    /// Daily chance of a contact, in 1,000.
    pub contact_permille: u32,
    /// Days of contact for 1 opinion point.
    pub contact_days_per_point: u32,
    /// Most points from contact.
    pub contact_max: i32,
    /// Chemistry goes from minus this to plus this.
    pub chemistry_max: i32,
    /// Opinion for a friend.
    pub friend_at: i32,
    /// Opinion for a rival.
    pub rival_at: i32,
    /// Opinion for an enemy.
    pub enemy_at: i32,
    /// Opinion for a wedding.
    pub partner_at: i32,
    /// Age at which a colonist can wed.
    pub partner_min_age: u8,
    /// A wedding needs a hostility of this or less.
    pub wed_hostility_max: u32,
    /// Grudge points for the loss of a partner.
    pub grudge_partner: u32,
    /// Grudge points for the loss of a friend.
    pub grudge_friend: u32,
    /// Grudge points for the loss of a parent or a child.
    pub grudge_kin: u32,
    /// Most grudge points of one colonist against one colony.
    pub grudge_max: u32,
    /// Days for the loss of 1 grudge point.
    pub grudge_decay_days: u32,
    /// Hostility that each friend takes away.
    pub friend_hostility: u32,
    /// Hostility that each wedding takes away.
    pub wed_hostility: u32,
    /// Grudge points for 1 hostility.
    pub grudge_hostility_div: u32,
    /// The Ties part of the hostility stays between minus and plus this.
    pub ties_max: i32,
    /// Linked memory indices.
    #[serde(skip)]
    pub memories: NeighborMemories,
}

/// Indices of the memories of `needs.ron` that the people of other colonies give.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NeighborMemories {
    /// `met_neighbor`.
    pub met: u8,
    /// `grudge_raid`.
    pub grudge: u8,
    /// `neighbor_wed`.
    pub wed: u8,
}

/// Load and check `neighbors.ron`. `needs` gives the memory keys.
pub fn load_neighbor_rules(
    file: &str,
    text: &str,
    needs: &NeedRules,
) -> Result<NeighborRules, ContentError> {
    let mut r: NeighborRules = parse(file, text)?;
    let mut issues = Issues::new(file);
    if r.people_per_rival == 0
        || r.contact_days_per_point == 0
        || r.grudge_decay_days == 0
        || r.grudge_hostility_div == 0
    {
        issues.add(
            "people_per_rival",
            "the people and the divisors must be 1 or more",
        );
    }
    if r.person_age_min < r.partner_min_age || r.person_age_min > r.person_age_max {
        issues.add(
            "person_age_min",
            "need partner_min_age <= person_age_min <= person_age_max",
        );
    }
    if !(r.enemy_at < r.rival_at && r.rival_at < 0 && 0 < r.friend_at && r.friend_at < r.partner_at)
    {
        issues.add(
            "enemy_at",
            "need enemy_at < rival_at < 0 < friend_at < partner_at",
        );
    }
    if r.contact_max < 0 || r.chemistry_max < 0 || r.ties_max < 0 || r.contact_permille > 1000 {
        issues.add("contact_max", "the limits must be 0 or more");
    }
    let mut find = |key: &str| -> u8 {
        needs.memory_index(key).unwrap_or_else(|| {
            issues.add(key, "the memory is not in needs.ron");
            0
        })
    };
    r.memories = NeighborMemories {
        met: find("met_neighbor"),
        grudge: find("grudge_raid"),
        wed: find("neighbor_wed"),
    };
    issues.finish(r)
}

/// One tie of a colonist with a person of a rival colony.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct Tie {
    /// Days of contact.
    contact_days: u32,
    /// The chemistry of the pair.
    chemistry: i32,
    /// The standing at the last daily check.
    standing: Standing,
}

/// The ties, grudges, and counts of the people of other colonies. Part of the state.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct NeighborState {
    /// Ties by colonist, rival colony, and person.
    ties: BTreeMap<(ColonistId, u16, u8), Tie>,
    /// Grudge points by colonist and rival colony.
    grudges: BTreeMap<(ColonistId, u16), u32>,
    /// Contacts so far, by rival colony. The hook for the trade of the diplomacy (Phase 5).
    contacts: BTreeMap<u16, u32>,
    /// People that wed a colonist, by rival colony and person.
    weds: BTreeSet<(u16, u8)>,
}

impl Sim {
    fn neighbor_roll(&self, a: u64, b: u64, c: u64) -> u64 {
        mix(self.relation_state.seed ^ TAG_NEIGHBOR ^ mix(a ^ mix(b ^ mix(c))))
    }

    /// A person of a rival colony (R-041). `None` for a wrong rival or person.
    pub fn neighbor_person(&self, rival: usize, slot: u8) -> Option<NeighborPerson> {
        let n = &self.rules.neighbors;
        if rival >= self.raid.rivals.len() || slot >= n.people_per_rival {
            return None;
        }
        let h = self.neighbor_roll(1, rival as u64, u64::from(slot));
        let span = u64::from(n.person_age_max - n.person_age_min) + 1;
        let age = n.person_age_min + u8::try_from((h >> 8) % span).unwrap_or(0);
        let sex = if (h >> 40) & 1 == 0 {
            Sex::Female
        } else {
            Sex::Male
        };
        let wed = self.neighbors.weds.contains(&(rival as u16, slot));
        Some(NeighborPerson { age, sex, wed })
    }

    fn neighbor_chemistry(&self, id: ColonistId, rival: u16, slot: u8) -> i32 {
        let max = self.rules.neighbors.chemistry_max;
        let roll = self.neighbor_roll(
            2,
            id.index() as u64,
            (u64::from(rival) << 8) | u64::from(slot),
        );
        let span = u64::try_from(2 * max + 1).unwrap_or(1);
        i32::try_from(roll % span).unwrap_or(0) - max
    }

    fn tie_parts(&self, key: (ColonistId, u16, u8), tie: &Tie) -> Vec<NeighborPart> {
        let n = &self.rules.neighbors;
        let contact = i32::try_from(tie.contact_days / n.contact_days_per_point.max(1))
            .unwrap_or(i32::MAX)
            .min(n.contact_max);
        let grudge = self
            .neighbors
            .grudges
            .get(&(key.0, key.1))
            .copied()
            .unwrap_or(0);
        vec![
            NeighborPart {
                cause: NeighborCause::Contact {
                    days: tie.contact_days,
                },
                value: contact,
            },
            NeighborPart {
                cause: NeighborCause::Chemistry,
                value: tie.chemistry,
            },
            NeighborPart {
                cause: NeighborCause::Grudge,
                value: -i32::try_from(grudge).unwrap_or(i32::MAX),
            },
        ]
    }

    fn tie_standing(&self, opinion: i32) -> Standing {
        let n = &self.rules.neighbors;
        if opinion <= n.enemy_at {
            Standing::Enemy
        } else if opinion <= n.rival_at {
            Standing::Rival
        } else if opinion >= n.friend_at {
            Standing::Friend
        } else {
            Standing::Acquaintance
        }
    }

    fn tie_report(&self, key: (ColonistId, u16, u8), tie: &Tie) -> NeighborTie {
        let parts = self.tie_parts(key, tie);
        let opinion = parts.iter().map(|p| p.value).sum();
        NeighborTie {
            rival: key.1,
            slot: key.2,
            person: self
                .neighbor_person(usize::from(key.1), key.2)
                .unwrap_or(NeighborPerson {
                    age: 0,
                    sex: Sex::Female,
                    wed: false,
                }),
            standing: self.tie_standing(opinion),
            opinion,
            parts,
        }
    }

    /// The ties of a colonist with the people of rival colonies, with the why chain, in order
    /// of rival colony and person.
    pub fn neighbor_ties(&self, id: ColonistId) -> Vec<NeighborTie> {
        self.neighbors
            .ties
            .iter()
            .filter(|(k, _)| k.0 == id)
            .map(|(&k, t)| self.tie_report(k, t))
            .collect()
    }

    /// Contacts with a rival colony so far.
    pub fn neighbor_contacts(&self, rival: usize) -> u32 {
        u16::try_from(rival)
            .ok()
            .and_then(|r| self.neighbors.contacts.get(&r))
            .copied()
            .unwrap_or(0)
    }

    /// People of a rival colony that wed a colonist.
    pub fn neighbor_weds(&self, rival: usize) -> u32 {
        let r = u16::try_from(rival).unwrap_or(u16::MAX);
        u32::try_from(self.neighbors.weds.iter().filter(|w| w.0 == r).count()).unwrap_or(0)
    }

    /// The grudge points of a colonist against a rival colony.
    pub fn neighbor_grudge(&self, id: ColonistId, rival: usize) -> u32 {
        u16::try_from(rival)
            .ok()
            .and_then(|r| self.neighbors.grudges.get(&(id, r)))
            .copied()
            .unwrap_or(0)
    }

    /// Take the events that the last ticks made.
    pub fn drain_neighbor_events(&mut self) -> Vec<NeighborEvent> {
        std::mem::take(&mut self.neighbor_events)
    }

    /// The Ties part of the hostility of a rival colony (R-043): grudges add, friends and
    /// weddings take away.
    pub(crate) fn neighbor_ties_value(&self, index: usize) -> i32 {
        let n = &self.rules.neighbors;
        let Ok(rival) = u16::try_from(index) else {
            return 0;
        };
        let to_i64 = |v: u32| i64::from(v);
        let grudge: u32 = self
            .neighbors
            .grudges
            .iter()
            .filter(|(k, _)| k.1 == rival)
            .fold(0u32, |a, (_, &g)| a.saturating_add(g));
        let friends = self
            .neighbors
            .ties
            .iter()
            .filter(|(k, _)| k.1 == rival)
            .filter(|(k, t)| self.tie_report(**k, t).standing == Standing::Friend)
            .count();
        let weds = self.neighbors.weds.iter().filter(|w| w.0 == rival).count();
        let value = to_i64(grudge / n.grudge_hostility_div.max(1))
            - friends as i64 * to_i64(n.friend_hostility)
            - weds as i64 * to_i64(n.wed_hostility);
        let max = i64::from(n.ties_max);
        i32::try_from(value.clamp(-max, max)).unwrap_or(0)
    }

    /// Each game day: grudges fade, a calm rival colony can have a contact, standings update,
    /// and a tie with a high opinion ends in a wedding. Acts at the day boundary only.
    pub(crate) fn neighbors_day(&mut self) {
        if self.tick == 0 || !self.tick.is_multiple_of(TICKS_PER_DAY) || self.colonists.is_empty() {
            return;
        }
        let rules = self.rules.clone();
        let n = &rules.neighbors;
        let day = self.tick / TICKS_PER_DAY;
        if day.is_multiple_of(u64::from(n.grudge_decay_days)) {
            for g in self.neighbors.grudges.values_mut() {
                *g -= 1;
            }
            self.neighbors.grudges.retain(|_, g| *g > 0);
        }
        for r in 0..self.raid.rivals.len() {
            let Ok(rival) = u16::try_from(r) else {
                break;
            };
            let raiding = self.raid.active.as_ref().is_some_and(|a| a.rival == rival);
            let calm = self
                .rival_hostility(r)
                .is_some_and(|h| h.total <= n.contact_hostility_max);
            let roll = self.neighbor_roll(3, day, u64::from(rival));
            if raiding || !calm || roll % 1000 >= u64::from(n.contact_permille) {
                continue;
            }
            let adults: Vec<ColonistId> = self
                .colonists
                .iter()
                .filter(|(_, c)| c.age >= n.partner_min_age)
                .map(|(id, _)| id)
                .collect();
            let slots: Vec<u8> = (0..n.people_per_rival)
                .filter(|&s| !self.neighbors.weds.contains(&(rival, s)))
                .collect();
            if adults.is_empty() || slots.is_empty() {
                continue;
            }
            let colonist = adults[((roll >> 16) % adults.len() as u64) as usize];
            let slot = slots[((roll >> 32) % slots.len() as u64) as usize];
            let chemistry = self.neighbor_chemistry(colonist, rival, slot);
            let tie = self
                .neighbors
                .ties
                .entry((colonist, rival, slot))
                .or_insert(Tie {
                    contact_days: 0,
                    chemistry,
                    standing: Standing::Acquaintance,
                });
            tie.contact_days = tie.contact_days.saturating_add(1);
            *self.neighbors.contacts.entry(rival).or_insert(0) += 1;
            self.give_memory(colonist, n.memories.met);
            self.neighbor_events.push(NeighborEvent::Met {
                colonist,
                rival,
                slot,
            });
        }
        let keys: Vec<(ColonistId, u16, u8)> = self.neighbors.ties.keys().copied().collect();
        for key in keys {
            let Some(tie) = self.neighbors.ties.get(&key).copied() else {
                continue;
            };
            let report = self.tie_report(key, &tie);
            if tie.standing != report.standing {
                if let Some(t) = self.neighbors.ties.get_mut(&key) {
                    t.standing = report.standing;
                }
                self.neighbor_events.push(NeighborEvent::Standing {
                    colonist: key.0,
                    rival: key.1,
                    slot: key.2,
                    from: tie.standing,
                    to: report.standing,
                });
            }
            let calm = self
                .rival_hostility(usize::from(key.1))
                .is_some_and(|h| h.total <= n.wed_hostility_max);
            let single_adult = self
                .colonists
                .get(key.0)
                .is_some_and(|c| c.age >= n.partner_min_age)
                && self.partner_of(key.0).is_none();
            if report.opinion >= n.partner_at
                && calm
                && single_adult
                && !self.neighbors.weds.contains(&(key.1, key.2))
            {
                self.neighbor_wed(key, tie.chemistry);
            }
        }
    }

    /// The person of a tie weds the colonist and joins the colony.
    fn neighbor_wed(&mut self, key: (ColonistId, u16, u8), chemistry: i32) {
        let (id, rival, slot) = key;
        let Some(person) = self.neighbor_person(usize::from(rival), slot) else {
            return;
        };
        let Some(host) = self.colonists.get(id) else {
            return;
        };
        let (x, y) = (host.x, host.y);
        let mut c = Colonist::new(x, y);
        c.priorities = [self.rules.jobs.default_priority; crate::job::JobKind::COUNT];
        self.rules.needs.init_colonist(&mut c);
        c.age = person.age;
        c.sex = person.sex;
        let trait_count = self.rules.traits.len();
        let roll = self.neighbor_roll(4, u64::from(rival), u64::from(slot));
        if trait_count > 0 {
            c.traits = vec![u8::try_from(roll as usize % trait_count).unwrap_or(0)];
        }
        let partner = self.colonists.insert(c);
        self.roll_wants(partner, roll ^ self.tick);
        self.neighbors.weds.insert((rival, slot));
        self.neighbors
            .ties
            .retain(|k, _| !(k.1 == rival && k.2 == slot));
        let r = &self.rules.relations;
        let (together, max) = (r.together_hours_per_point, r.together_max);
        let pair = if id <= partner {
            (id, partner)
        } else {
            (partner, id)
        };
        self.relation_state.map.insert(
            pair,
            Relation {
                together_hours: together.saturating_mul(u32::try_from(max).unwrap_or(0)),
                sleep_hours: 0,
                quarrel_hours: 0,
                chemistry: chemistry.clamp(-r.chemistry_max, r.chemistry_max),
                partner: true,
                standing: Standing::Partner,
            },
        );
        if let Some(c) = self.colonists.get_mut(id) {
            c.bed = None;
        }
        let memory = self.rules.neighbors.memories.wed;
        self.give_memory(id, memory);
        self.give_memory(partner, memory);
        let count = i32::try_from(self.colonists.len()).unwrap_or(i32::MAX);
        self.chronicle_add(crate::ChronicleKind::Wedding, [count, 0, 0]);
        self.run.peak_population = self
            .run
            .peak_population
            .max(u32::try_from(self.colonists.len()).unwrap_or(u32::MAX));
        self.wake_idle();
        self.neighbor_events.push(NeighborEvent::Wed {
            colonist: id,
            partner,
            rival,
        });
    }

    /// Remove the ties and grudges of a colonist that is gone.
    pub(crate) fn neighbors_forget(&mut self, id: ColonistId) {
        self.neighbors.ties.retain(|k, _| k.0 != id);
        self.neighbors.grudges.retain(|k, _| k.0 != id);
    }

    /// A colonist died of wounds during a raid: the partner, the friends, and the kin of the dead
    /// get a grudge against the raiding colony (R-043).
    pub(crate) fn neighbors_grudge(
        &mut self,
        mourners: &[(ColonistId, Standing)],
        kin: &crate::family::Kin,
        cause: DeathCause,
    ) {
        let Some(rival) = self.raid.active.as_ref().map(|a| a.rival) else {
            return;
        };
        if cause != DeathCause::Wounds {
            return;
        }
        let n = self.rules.neighbors.clone();
        let mut amounts: BTreeMap<ColonistId, u32> = BTreeMap::new();
        for &(other, standing) in mourners {
            let a = match standing {
                Standing::Partner => n.grudge_partner,
                Standing::Friend => n.grudge_friend,
                _ => 0,
            };
            *amounts.entry(other).or_insert(0) += a;
        }
        for &other in kin.parents.iter().chain(&kin.children) {
            *amounts.entry(other).or_insert(0) += n.grudge_kin;
        }
        for (other, amount) in amounts {
            if amount == 0 || self.colonists.get(other).is_none() {
                continue;
            }
            let g = self.neighbors.grudges.entry((other, rival)).or_insert(0);
            *g = g.saturating_add(amount).min(n.grudge_max);
            self.give_memory(other, n.memories.grudge);
            self.neighbor_events.push(NeighborEvent::Grudge {
                colonist: other,
                rival,
                amount,
            });
        }
    }

    /// True if the ties fit the rival colonies and the rules. Called after a load.
    pub(crate) fn neighbors_state_ok(&self) -> bool {
        let rivals = self.raid.rivals.len();
        let people = self.rules.neighbors.people_per_rival;
        self.neighbors
            .ties
            .keys()
            .all(|k| usize::from(k.1) < rivals && k.2 < people)
            && self
                .neighbors
                .grudges
                .keys()
                .all(|k| usize::from(k.1) < rivals)
            && self
                .neighbors
                .contacts
                .keys()
                .all(|&r| usize::from(r) < rivals)
            && self
                .neighbors
                .weds
                .iter()
                .all(|w| usize::from(w.0) < rivals && w.1 < people)
    }

    // ---- debug hooks (scenario tests) ----

    /// Make or set a tie. For scenario tests and debug tools.
    pub fn debug_set_neighbor_tie(
        &mut self,
        id: ColonistId,
        rival: u16,
        slot: u8,
        contact_days: u32,
        chemistry: i32,
    ) {
        self.neighbors.ties.insert(
            (id, rival, slot),
            Tie {
                contact_days,
                chemistry,
                standing: Standing::Acquaintance,
            },
        );
    }

    /// Set the grudge of a colonist against a rival colony. For scenario tests and debug tools.
    pub fn debug_set_neighbor_grudge(&mut self, id: ColonistId, rival: u16, points: u32) {
        self.neighbors.grudges.insert((id, rival), points);
    }

    /// Kill a colonist with the cause "wounds", as a raider would. For scenario tests.
    pub fn debug_kill_wounded(&mut self, id: ColonistId) {
        self.kill(id, DeathCause::Wounds);
    }
}
