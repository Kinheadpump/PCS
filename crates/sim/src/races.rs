//! Races (S25, R-048, R-049).
//!
//! A race is a set of content values: the speed of ageing, the age of an elder, the comfort
//! band, and the weight among migrants. Each pair of different races has a rule for the
//! opinion, for partners, and for offspring. Races have no spells.

use crate::Sim;
use crate::colonist::ColonistId;
use crate::loader::{Issues, parse};
use base::content::ContentError;
use serde::Deserialize;

/// Which race a child of a pair gets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChildRace {
    /// The race of the mother.
    Mother,
    /// The race of the father.
    Father,
    /// The race of one of the parents, from a hash.
    Either,
    /// A fixed race.
    Race(u8),
}

/// One race.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RaceDef {
    /// Key in the content files.
    pub key: String,
    /// Display name.
    pub name: String,
    /// Days of one age year, as a percent of the base value of `family.ron`.
    pub age_scale_pct: u32,
    /// Years that the race adds to the age of an elder. Can be negative.
    pub elder_age_delta: i16,
    /// Tenths of a degree that the race adds to the lower limit of the comfort band.
    pub comfort_low_delta: i16,
    /// Weight among migrants and the people of rival colonies. 0 means never.
    pub migrant_weight: u32,
}

/// The rule for a pair of different races. `a` is smaller than `b`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RacePairRule {
    /// The first race.
    pub a: u8,
    /// The second race.
    pub b: u8,
    /// Points that the pair adds to the opinion of a relation.
    pub opinion: i32,
    /// True if the two can become partners.
    pub partner: bool,
    /// True if the two can have children.
    pub offspring: bool,
    /// The race of the child.
    pub child: ChildRace,
}

/// The rule for a pair of races, as `Sim::race_pair` returns it.
pub type RacePair = RacePairRule;

/// The loaded `races.ron`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RaceRules {
    /// The races. The first is the race of the start colonists.
    pub races: Vec<RaceDef>,
    /// The rules of the pairs of different races.
    pub pairs: Vec<RacePairRule>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawPair {
    a: String,
    b: String,
    opinion: i32,
    partner: bool,
    offspring: bool,
    child: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawRaces {
    races: Vec<RaceDef>,
    pairs: Vec<RawPair>,
}

/// Load and check `races.ron`.
pub fn load_race_rules(file: &str, text: &str) -> Result<RaceRules, ContentError> {
    let raw: RawRaces = parse(file, text)?;
    let mut issues = Issues::new(file);
    let n = raw.races.len();
    if n == 0 || n > 8 {
        issues.add("races", "need 1 to 8 races");
    }
    for (i, r) in raw.races.iter().enumerate() {
        if raw.races[..i].iter().any(|o| o.key == r.key) {
            issues.add(&r.key, "the race key is used two times");
        }
        if r.age_scale_pct == 0 {
            issues.add(&r.key, "age_scale_pct must be 1 or more");
        }
    }
    let index = |key: &str| raw.races.iter().position(|r| r.key == key);
    let mut pairs: Vec<RacePairRule> = Vec::new();
    for p in &raw.pairs {
        let (Some(a), Some(b)) = (index(&p.a), index(&p.b)) else {
            issues.add(&p.a, "the pair names a race that is not in the file");
            continue;
        };
        if a == b {
            issues.add(&p.a, "a pair needs two different races");
            continue;
        }
        let child = match p.child.as_str() {
            "mother" => ChildRace::Mother,
            "father" => ChildRace::Father,
            "either" => ChildRace::Either,
            key => match index(key) {
                Some(i) => ChildRace::Race(u8::try_from(i).unwrap_or(0)),
                None => {
                    issues.add(&p.child, "the child race is not in the file");
                    continue;
                }
            },
        };
        let (a, b) = (a.min(b), a.max(b));
        let (a, b) = (u8::try_from(a).unwrap_or(0), u8::try_from(b).unwrap_or(0));
        if pairs.iter().any(|q| (q.a, q.b) == (a, b)) {
            issues.add(&p.a, "the pair is listed two times");
            continue;
        }
        pairs.push(RacePairRule {
            a,
            b,
            opinion: p.opinion,
            partner: p.partner,
            offspring: p.offspring,
            child,
        });
    }
    if pairs.len() != n * n.saturating_sub(1) / 2 {
        issues.add("pairs", "every pair of different races needs one rule");
    }
    pairs.sort_by_key(|p| (p.a, p.b));
    issues.finish(RaceRules {
        races: raw.races,
        pairs,
    })
}

impl RaceRules {
    /// The index of a race at a weight roll. Race 0 if all weights are 0.
    pub fn pick(&self, roll: u64) -> u8 {
        let total: u64 = self.races.iter().map(|r| u64::from(r.migrant_weight)).sum();
        if total == 0 {
            return 0;
        }
        let mut at = roll % total;
        for (i, r) in self.races.iter().enumerate() {
            let w = u64::from(r.migrant_weight);
            if at < w {
                return u8::try_from(i).unwrap_or(0);
            }
            at -= w;
        }
        0
    }
}

impl Sim {
    /// The rule for two races (R-049). Two people of the same race follow the default rule.
    pub fn race_pair(&self, a: u8, b: u8) -> RacePair {
        let (lo, hi) = (a.min(b), a.max(b));
        if lo == hi {
            return RacePairRule {
                a: lo,
                b: hi,
                opinion: 0,
                partner: true,
                offspring: true,
                child: ChildRace::Mother,
            };
        }
        self.rules
            .races
            .pairs
            .iter()
            .find(|p| (p.a, p.b) == (lo, hi))
            .copied()
            .unwrap_or(RacePairRule {
                a: lo,
                b: hi,
                opinion: 0,
                partner: false,
                offspring: false,
                child: ChildRace::Mother,
            })
    }

    /// The race of a colonist.
    pub fn race_of(&self, id: ColonistId) -> Option<u8> {
        self.colonists.get(id).map(|c| c.race)
    }

    /// Days of one age year of a race (R-037, R-048).
    pub fn race_age_days(&self, race: u8) -> u32 {
        let pct = self
            .rules
            .races
            .races
            .get(usize::from(race))
            .map_or(100, |r| r.age_scale_pct);
        (self.rules.family.days_per_age_year * pct / 100).max(1)
    }

    /// Age at which a colonist of a race is an elder.
    pub fn race_elder_age(&self, race: u8) -> u8 {
        let delta = self
            .rules
            .races
            .races
            .get(usize::from(race))
            .map_or(0, |r| r.elder_age_delta);
        let age = i32::from(self.rules.family.elder_age) + i32::from(delta);
        u8::try_from(age.clamp(0, 255)).unwrap_or(u8::MAX)
    }

    /// Set the race of a colonist. For scenario tests and debug tools.
    pub fn debug_set_race(&mut self, id: ColonistId, race: u8) {
        if let Some(c) = self.colonists.get_mut(id) {
            c.race = race;
        }
    }
}
