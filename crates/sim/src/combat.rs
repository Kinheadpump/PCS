//! Melee combat (S13, R-032): drafting, move and attack orders, blows, wounds, and the death of
//! creatures.
//!
//! MVP combat is melee only. A blow does `base × (100% + 10% × combat level)` plus a roll of 0% to
//! 20% of the base. A hit of 10 or more damage adds the condition "wounded". There are no body
//! parts. The base damage of a colonist is the fist damage in `creatures.ron`; the weapons of S12
//! plug in at `colonist_base_damage`.
//!
//! A drafted colonist takes no job and no need action. It obeys a move order in the same tick
//! (`Command::DraftMove`). An attack order (`Command::Attack`) also works on a colonist that is
//! not drafted: the colonist hunts the creature, and goes back to work when it is dead or gone
//! (R-032, hunting). Colonists that are not drafted strike back at a hostile creature next to them.

use crate::colonist::ColonistId;
use crate::creature::{CombatEvent, CombatReject, Creature, CreatureId, Mode, Role, cheb};
use crate::job::AbortReason;
use crate::life::DeathCause;
use crate::skill::{self, Skill};
use crate::{Command, NeedAct, PathRequest, Sim};

/// Damage of a blow (R-032): the base damage with the combat level added in percent, plus a roll
/// of `roll` percent of the base (`roll` is 0 to 20).
pub fn melee_damage(base: u32, level: u32, level_pct: u32, roll: u32) -> u32 {
    let scaled = u64::from(base) * u64::from(100 + level_pct * level) / 100;
    let extra = u64::from(base) * u64::from(roll) / 100;
    u32::try_from(scaled + extra).unwrap_or(u32::MAX)
}

impl Sim {
    /// The base damage of a colonist (R-032). Weapons plug in here in S12.
    pub fn colonist_base_damage(&self, _id: ColonistId) -> u32 {
        self.rules.creature.fist_damage
    }

    /// The smallest and the largest damage that a colonist can do with one blow.
    pub fn colonist_damage_range(&self, id: ColonistId) -> Option<(u32, u32)> {
        let level = u32::from(self.skill_level(id, Skill::Combat)?);
        let r = &self.rules.creature;
        let base = self.colonist_base_damage(id);
        Some((
            melee_damage(base, level, r.level_pct, 0),
            melee_damage(base, level, r.level_pct, r.roll_pct),
        ))
    }

    /// True if the colonist is drafted or hunts under an attack order.
    pub fn colonist_under_orders(&self, id: ColonistId) -> bool {
        self.colonists
            .get(id)
            .is_some_and(|c| c.drafted || c.foe.is_some())
    }

    pub(crate) fn combat_command(&mut self, command: &Command) {
        match *command {
            Command::Draft { colonist, drafted } => self.set_drafted(colonist, drafted),
            Command::DraftMove { colonist, x, y } => self.draft_move(colonist, x, y),
            Command::Attack { colonist, creature } => self.attack_order(colonist, creature),
            _ => {}
        }
    }

    fn combat_reject(&mut self, why: CombatReject) {
        self.combat_events.push(CombatEvent::Rejected(why));
    }

    /// Stop the job, the need action, and the walk of a colonist that takes orders.
    fn take_orders(&mut self, id: ColonistId) {
        self.abort_job(id, AbortReason::JobGone, false);
        self.cancel_act(id, 0);
        self.path_queue.retain(|r| r.colonist != id);
        if let Some(c) = self.colonists.get_mut(id) {
            c.path.clear();
            c.target = None;
            c.progress = 0;
        }
    }

    /// The colonist goes back to work.
    fn release_orders(&mut self, id: ColonistId) {
        let tick = self.tick;
        if let Some(c) = self.colonists.get_mut(id) {
            c.drafted = false;
            c.foe = None;
            c.next_search = tick;
            c.need_next = tick;
        }
    }

    fn set_drafted(&mut self, id: ColonistId, drafted: bool) {
        let Some(c) = self.colonists.get(id) else {
            self.combat_reject(CombatReject::NoColonist);
            return;
        };
        if c.drafted == drafted {
            return;
        }
        if drafted && c.age < self.rules.family.adult_age {
            self.combat_reject(CombatReject::TooYoung);
            return;
        }
        if drafted {
            self.take_orders(id);
            if let Some(c) = self.colonists.get_mut(id) {
                c.drafted = true;
            }
        } else {
            self.release_orders(id);
        }
    }

    fn draft_move(&mut self, id: ColonistId, x: u16, y: u16) {
        let Some(c) = self.colonists.get(id) else {
            self.combat_reject(CombatReject::NoColonist);
            return;
        };
        if !c.drafted {
            self.combat_reject(CombatReject::NotDrafted);
            return;
        }
        let (sx, sy) = (i32::from(c.x), i32::from(c.y));
        if !self
            .regions
            .connected((sx, sy), (i32::from(x), i32::from(y)))
        {
            self.combat_reject(CombatReject::NoPath);
            return;
        }
        self.path_queue.retain(|r| r.colonist != id);
        self.path_queue.push_back(PathRequest {
            colonist: id,
            goal: (x, y),
        });
        if let Some(c) = self.colonists.get_mut(id) {
            c.foe = None;
            c.target = Some((x, y));
        }
    }

    fn attack_order(&mut self, id: ColonistId, creature: CreatureId) {
        if !self.colonists.contains(id) {
            self.combat_reject(CombatReject::NoColonist);
            return;
        }
        let Some(target) = self.creature.creatures.get(creature).map(Creature::tile) else {
            self.combat_reject(CombatReject::NoCreature);
            return;
        };
        let from = self.colonists.get(id).map(|c| c.tile());
        let reachable = from.is_some_and(|f| {
            self.regions.connected(
                (i32::from(f.0), i32::from(f.1)),
                (i32::from(target.0), i32::from(target.1)),
            )
        });
        if !reachable {
            self.combat_reject(CombatReject::NoPath);
            return;
        }
        let drafted = self.colonists.get(id).is_some_and(|c| c.drafted);
        if !drafted {
            self.take_orders(id);
        } else {
            self.path_queue.retain(|r| r.colonist != id);
        }
        if let Some(c) = self.colonists.get_mut(id) {
            c.foe = Some(creature);
            c.target = None;
        }
    }

    /// The colonists fight (R-032), in slot order. A colonist strikes the creature of its order,
    /// or the hostile creature next to it, and walks toward the creature of its order.
    pub(crate) fn combat_tick(&mut self) {
        if self.creature.creatures.is_empty()
            && !self.colonists.iter().any(|(_, c)| c.foe.is_some())
        {
            return;
        }
        let ids: Vec<ColonistId> = self.colonists.iter().map(|(id, _)| id).collect();
        let tick = self.tick;
        for id in ids {
            let Some(c) = self.colonists.get(id) else {
                continue;
            };
            let (tile, foe, ready, drafted) = (c.tile(), c.foe, c.attack_ready, c.drafted);
            let asleep = matches!(c.act, Some(NeedAct::Sleep { asleep: true, .. }));
            let foe_tile = foe.map(|f| self.creature.creatures.get(f).map(Creature::tile));
            if foe.is_some() && foe_tile == Some(None) {
                // The creature is dead or gone.
                if drafted {
                    if let Some(c) = self.colonists.get_mut(id) {
                        c.foe = None;
                    }
                } else {
                    self.release_orders(id);
                }
                continue;
            }
            if asleep {
                continue;
            }
            let target = match (foe, foe_tile.flatten()) {
                (Some(f), Some(t)) => Some((f, t)),
                _ => self.adjacent_hostile(tile),
            };
            let Some((cid, ct)) = target else {
                continue;
            };
            if self.can_strike(tile, ct) {
                if tick >= ready {
                    self.colonist_strike(id, cid);
                }
                // A colonist next to its foe stands still.
                if foe.is_some()
                    && let Some(c) = self.colonists.get_mut(id)
                    && !c.path.is_empty()
                    && c.progress == 0
                {
                    c.path.clear();
                    c.target = None;
                }
                continue;
            }
            if foe.is_none() {
                continue;
            }
            let wait = u64::from(self.rules.creature.repath_ticks.max(1));
            let due = self.colonists.get(id).is_some_and(|c| {
                c.path.is_empty()
                    || (c.target != Some(ct) && (tick + u64::from(id.index())).is_multiple_of(wait))
            });
            if !due {
                continue;
            }
            let connected = self.regions.connected(
                (i32::from(tile.0), i32::from(tile.1)),
                (i32::from(ct.0), i32::from(ct.1)),
            );
            if !connected {
                // The creature went where the colonist cannot follow.
                if drafted {
                    if let Some(c) = self.colonists.get_mut(id) {
                        c.foe = None;
                    }
                } else {
                    self.release_orders(id);
                }
                continue;
            }
            if self
                .path_queue
                .iter()
                .any(|r| r.colonist == id && r.goal == ct)
            {
                continue;
            }
            self.path_queue.retain(|r| r.colonist != id);
            self.path_queue.push_back(PathRequest {
                colonist: id,
                goal: ct,
            });
            if let Some(c) = self.colonists.get_mut(id) {
                c.target = Some(ct);
            }
        }
    }

    /// The hostile creature next to a tile with the least health. Ties go to the lower ID.
    fn adjacent_hostile(&self, tile: (u16, u16)) -> Option<(CreatureId, (u16, u16))> {
        let mut best: Option<(i32, CreatureId, (u16, u16))> = None;
        for (id, c) in self.creature.creatures.iter() {
            let hostile = self
                .stats(c.kind)
                .is_some_and(|s| s.role.hostile() || c.mode == Mode::Chase);
            if !hostile || cheb(tile, c.tile()) > 1 || !self.can_strike(tile, c.tile()) {
                continue;
            }
            if best.is_none_or(|(h, ..)| c.health < h) {
                best = Some((c.health, id, c.tile()));
            }
        }
        best.map(|(_, id, t)| (id, t))
    }

    fn colonist_strike(&mut self, id: ColonistId, target: CreatureId) {
        let r = self.rules.creature.clone();
        let level = u32::from(self.skill_level(id, Skill::Combat).unwrap_or(0));
        let roll = self.creature.rng.below(r.roll_pct + 1);
        let damage = melee_damage(self.colonist_base_damage(id), level, r.level_pct, roll);
        let tick = self.tick;
        let job_rules = self.rules.jobs;
        if let Some(c) = self.colonists.get_mut(id) {
            c.attack_ready = tick + u64::from(r.colonist_attack_ticks);
            let cap = skill::max_xp(&job_rules);
            let xp = &mut c.skill_xp[Skill::Combat.index()];
            *xp = xp.saturating_add(r.combat_xp).min(cap);
        }
        self.hurt_creature(target, damage, Some(id));
    }

    /// A creature takes damage. At 0 health it dies.
    pub(crate) fn hurt_creature(&mut self, id: CreatureId, damage: u32, by: Option<ColonistId>) {
        let attacker = by.and_then(|b| self.colonists.get(b).map(|c| c.tile()));
        let flee = u64::from(self.rules.creature.flee_ticks);
        let tick = self.tick;
        let Some(role) = self
            .creature
            .creatures
            .get(id)
            .and_then(|c| self.stats(c.kind))
            .map(|s| (s.role, s.damage))
        else {
            return;
        };
        let Some(c) = self.creature.creatures.get_mut(id) else {
            return;
        };
        c.health -= i32::try_from(damage).unwrap_or(i32::MAX);
        let (x, y, dead) = (c.x, c.y, c.health <= 0);
        self.combat_events.push(CombatEvent::Hit {
            x,
            y,
            by_colonist: true,
            damage,
            wounded: false,
        });
        if dead {
            self.creature_died(id, by);
            return;
        }
        let Some(c) = self.creature.creatures.get_mut(id) else {
            return;
        };
        match role {
            (Role::Game, 0) => {
                c.mode = Mode::Flee;
                c.mode_until = tick + flee;
                c.threat = attacker.unwrap_or((x, y));
                c.path.clear();
                c.progress = 0;
            }
            (Role::Game, _) => {
                c.mode = Mode::Chase;
                c.mode_until = tick + 2 * flee;
                c.victim = by;
            }
            _ => {
                c.driven = true;
                if c.victim.is_none() {
                    c.victim = by;
                }
            }
        }
    }

    /// A creature is dead: it leaves meat and hide, drops its loot, and counts as a fighter of
    /// its raid that is down.
    fn creature_died(&mut self, id: CreatureId, by: Option<ColonistId>) {
        let Some(c) = self.creature.creatures.remove(id) else {
            return;
        };
        let Some(kind) = self.rules.creature.kinds.get(usize::from(c.kind)).cloned() else {
            return;
        };
        let (x, y) = c.tile();
        if let Some(m) = self.rules.creature.meat_material {
            self.spawn_item(m, kind.meat, x, y, true);
        }
        if let Some(m) = self.rules.creature.hide_material {
            self.spawn_item(m, kind.hide, x, y, true);
        }
        if let Some((m, n)) = c.loot {
            self.spawn_item(m, n, x, y, true);
        }
        if c.raid != 0 && self.raid.active.as_ref().is_some_and(|r| r.id == c.raid) {
            self.raid_fighters_down(1);
        }
        if kind.role == Role::Game {
            self.creature.hunted += 1;
            if let Some(b) = by {
                let xp = self.rules.creature.hunting_xp;
                let cap = skill::max_xp(&self.rules.jobs);
                if let Some(col) = self.colonists.get_mut(b) {
                    let v = &mut col.skill_xp[Skill::Hunting.index()];
                    *v = v.saturating_add(xp).min(cap);
                }
            }
        } else {
            self.creature.killed += 1;
        }
        self.combat_events.push(CombatEvent::CreatureDied {
            kind: c.kind,
            x,
            y,
            by,
        });
    }

    /// A colonist takes damage (R-032). A hit of the wound damage or more adds the condition
    /// "wounded". At 0 health the colonist dies of wounds.
    pub(crate) fn hurt_colonist(&mut self, id: ColonistId, damage: u32, by_colonist: bool) {
        let wound = self.rules.creature.wound_damage;
        let Some(c) = self.colonists.get_mut(id) else {
            return;
        };
        c.health = (c.health - i32::try_from(damage).unwrap_or(i32::MAX)).max(0);
        let wounded = damage >= wound;
        if wounded && !c.wounded {
            c.wounded = true;
            c.wound_bed_hours = 0;
        }
        let (x, y, dead) = (c.x, c.y, c.health <= 0);
        self.combat_events.push(CombatEvent::Hit {
            x,
            y,
            by_colonist,
            damage,
            wounded,
        });
        if dead {
            self.kill(id, DeathCause::Wounds);
        }
    }

    // ---- debug hooks (scenario tests) ----

    /// Hit a colonist as a creature does. For tests.
    pub fn debug_hurt_colonist(&mut self, id: ColonistId, damage: u32) {
        self.hurt_colonist(id, damage, false);
    }

    /// Hit a creature as a colonist does. For tests.
    pub fn debug_hurt_creature(&mut self, id: CreatureId, damage: u32, by: Option<ColonistId>) {
        self.hurt_creature(id, damage, by);
    }
}
