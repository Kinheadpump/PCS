//! Skills and work speed (R-010).
//!
//! Experience adds 1 level for each `xp_per_level` points, to `max_level`. Work speed is
//! the sum of named contributions: a base and a bonus for each skill level.

use base::content::JobRules;

/// A skill (GAME_DESIGN §7.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Skill {
    /// Mining rock.
    Mining,
    /// Cutting trees.
    Woodcutting,
    /// Building.
    Construction,
    /// Growing crops.
    Farming,
    /// Cooking.
    Cooking,
    /// Crafting.
    Crafting,
    /// Hunting.
    Hunting,
    /// Fighting.
    Combat,
    /// Healing.
    Medicine,
}

/// Number of skills.
pub const SKILL_COUNT: usize = 9;

impl Skill {
    /// All skills, in index order.
    pub const ALL: [Skill; SKILL_COUNT] = [
        Skill::Mining,
        Skill::Woodcutting,
        Skill::Construction,
        Skill::Farming,
        Skill::Cooking,
        Skill::Crafting,
        Skill::Hunting,
        Skill::Combat,
        Skill::Medicine,
    ];

    /// Index in the skill array of a colonist.
    pub fn index(self) -> usize {
        self as usize
    }

    /// Display name.
    pub fn name(self) -> &'static str {
        match self {
            Skill::Mining => "Mining",
            Skill::Woodcutting => "Woodcutting",
            Skill::Construction => "Construction",
            Skill::Farming => "Farming",
            Skill::Cooking => "Cooking",
            Skill::Crafting => "Crafting",
            Skill::Hunting => "Hunting",
            Skill::Combat => "Combat",
            Skill::Medicine => "Medicine",
        }
    }
}

/// Why a work speed has its value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpeedCause {
    /// Speed at skill level 0.
    Base,
    /// Bonus from the skill level.
    SkillLevel {
        /// The skill.
        skill: Skill,
        /// Its level.
        level: u8,
    },
}

/// One named contribution to a work speed, in percent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SpeedPart {
    /// The cause.
    pub cause: SpeedCause,
    /// Percent points that the cause adds.
    pub pct: i32,
}

/// A work speed: its parts and their sum.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkSpeed {
    /// Named contributions, in display order.
    pub parts: Vec<SpeedPart>,
    /// Sum of the parts, in percent (100 is normal speed).
    pub total_pct: i32,
}

/// Skill level for an experience value (R-010).
pub fn level_for_xp(xp: u32, rules: &JobRules) -> u8 {
    let level = xp / rules.xp_per_level.max(1);
    u8::try_from(level.min(u32::from(rules.max_level))).unwrap_or(rules.max_level)
}

/// Largest experience value of a skill. Experience stops at the maximum level.
pub fn max_xp(rules: &JobRules) -> u32 {
    u32::from(rules.max_level).saturating_mul(rules.xp_per_level)
}

/// Work speed of a colonist with the given skill and level (R-010). With no skill
/// (hauling) only the base counts. This is the only place that computes work speed.
pub fn work_speed(skill: Option<(Skill, u8)>, rules: &JobRules) -> WorkSpeed {
    let mut parts = vec![SpeedPart {
        cause: SpeedCause::Base,
        pct: rules.speed_base_pct,
    }];
    if let Some((skill, level)) = skill {
        parts.push(SpeedPart {
            cause: SpeedCause::SkillLevel { skill, level },
            pct: rules.speed_per_level_pct * i32::from(level),
        });
    }
    let total_pct = parts.iter().map(|p| p.pct).sum();
    WorkSpeed { parts, total_pct }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::SimRules;

    #[test]
    fn r010_level_four_gives_120_percent_with_a_why_chain() {
        let rules = SimRules::builtin().jobs;
        let speed = work_speed(Some((Skill::Mining, 4)), &rules);
        assert_eq!(speed.total_pct, 120);
        assert_eq!(speed.parts.len(), 2);
        assert_eq!(speed.parts[0].pct, 100);
        assert_eq!(speed.parts[1].pct, 20);
        assert_eq!(work_speed(None, &rules).total_pct, 100);
        assert_eq!(work_speed(Some((Skill::Mining, 0)), &rules).total_pct, 100);
        assert_eq!(work_speed(Some((Skill::Mining, 10)), &rules).total_pct, 150);
    }

    #[test]
    fn r010_one_thousand_xp_make_one_level_to_a_maximum_of_10() {
        let rules = SimRules::builtin().jobs;
        assert_eq!(level_for_xp(0, &rules), 0);
        assert_eq!(level_for_xp(999, &rules), 0);
        assert_eq!(level_for_xp(1_000, &rules), 1);
        assert_eq!(level_for_xp(4_500, &rules), 4);
        assert_eq!(level_for_xp(10_000, &rules), 10);
        assert_eq!(level_for_xp(u32::MAX, &rules), 10);
    }
}
