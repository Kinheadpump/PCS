//! Alerts: short warnings for the player (TECHNICAL_DESIGN §3.3).
//!
//! The alert list is a pure function of the state. `Sim::alerts` makes it when the
//! interface asks, so an alert is never out of date and is not part of the saved state.
//! Sprint 5 had two alerts: low fuel in a hearth and a cold period. Sprint 6 adds hunger,
//! hypothermia, heatstroke, low mood, and death.

use crate::Sim;
use crate::colonist::ColonistId;
use crate::life::DeathCause;
use crate::time::TICKS_PER_HOUR;

/// The kind of an alert, with its data.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AlertKind {
    /// A hearth has little fuel or none. `hours` is the fuel that is left, in burning hours.
    LowFuel {
        /// Hearth tile column.
        x: u16,
        /// Hearth tile row.
        y: u16,
        /// Fuel left, in burning hours.
        hours: u32,
    },
    /// The weather offset is at or below the cold period limit, now or in the forecast.
    ColdPeriod {
        /// 0 if the cold period is now, 1 to 3 if the forecast shows it.
        days_ahead: u8,
        /// The weather offset on that day, in tenths of a degree.
        offset: i16,
    },
    /// A colonist is hungry and does not eat, or is starving (R-011).
    Hunger {
        /// The colonist.
        colonist: ColonistId,
        /// Tile column of the colonist.
        x: u16,
        /// Tile row of the colonist.
        y: u16,
        /// True if hunger is 0.
        starving: bool,
    },
    /// A colonist has hypothermia (R-013).
    Hypothermia {
        /// The colonist.
        colonist: ColonistId,
        /// Tile column of the colonist.
        x: u16,
        /// Tile row of the colonist.
        y: u16,
        /// The hypothermia value.
        value: i32,
    },
    /// A colonist has heatstroke (R-013).
    Heatstroke {
        /// The colonist.
        colonist: ColonistId,
        /// Tile column of the colonist.
        x: u16,
        /// Tile row of the colonist.
        y: u16,
        /// The heatstroke value.
        value: i32,
    },
    /// A colonist has a mood below the low limit (R-015).
    LowMood {
        /// The colonist.
        colonist: ColonistId,
        /// Tile column of the colonist.
        x: u16,
        /// Tile row of the colonist.
        y: u16,
        /// The mood.
        mood: i32,
    },
    /// A colonist died lately (R-016).
    Death {
        /// Tile column of the death.
        x: u16,
        /// Tile row of the death.
        y: u16,
        /// The cause.
        cause: DeathCause,
        /// Game hours since the death.
        hours_ago: u32,
    },
}

/// How urgent an alert is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum AlertLevel {
    /// Good to know.
    Info,
    /// The player should act soon.
    Warning,
    /// Colonists are in danger or dead.
    Danger,
}

/// One alert.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Alert {
    /// What the alert is about.
    pub kind: AlertKind,
}

impl Alert {
    /// The tile that the alert is about, for "jump to". `None` for a global alert.
    pub fn tile(&self) -> Option<(u16, u16)> {
        match self.kind {
            AlertKind::LowFuel { x, y, .. }
            | AlertKind::Hunger { x, y, .. }
            | AlertKind::Hypothermia { x, y, .. }
            | AlertKind::Heatstroke { x, y, .. }
            | AlertKind::LowMood { x, y, .. }
            | AlertKind::Death { x, y, .. } => Some((x, y)),
            AlertKind::ColdPeriod { .. } => None,
        }
    }

    /// The urgency of the alert.
    pub fn level(&self) -> AlertLevel {
        match self.kind {
            AlertKind::ColdPeriod { .. } | AlertKind::LowFuel { .. } => AlertLevel::Info,
            AlertKind::LowMood { .. } => AlertLevel::Warning,
            AlertKind::Hunger { starving, .. } => {
                if starving {
                    AlertLevel::Danger
                } else {
                    AlertLevel::Warning
                }
            }
            AlertKind::Hypothermia { .. }
            | AlertKind::Heatstroke { .. }
            | AlertKind::Death { .. } => AlertLevel::Danger,
        }
    }
}

impl Sim {
    /// The alerts now. Order: the cold period, the low fuel alerts in tile order (row, then
    /// column), then the colonist alerts (hunger, hypothermia, heatstroke, low mood) in
    /// colonist order, then the death alerts, oldest first.
    pub fn alerts(&self) -> Vec<Alert> {
        let mut list = Vec::new();
        let limit = self.rules.climate.cold_period_offset;
        let today = self.weather_today();
        let cold = if today.offset <= limit {
            Some((0u8, today.offset))
        } else {
            self.forecast()
                .iter()
                .enumerate()
                .find(|(_, d)| d.offset <= limit)
                .map(|(i, d)| (i as u8 + 1, d.offset))
        };
        if let Some((days_ahead, offset)) = cold {
            list.push(Alert {
                kind: AlertKind::ColdPeriod { days_ahead, offset },
            });
        }
        let mut hearths = self.hearth_tiles();
        hearths.sort_unstable_by_key(|&(x, y)| (y, x));
        let low = self.rules.climate.low_fuel_alert_hours;
        for (x, y) in hearths {
            let hours = self.hearth_fuel(x, y);
            if hours < low {
                list.push(Alert {
                    kind: AlertKind::LowFuel { x, y, hours },
                });
            }
        }
        if self.needs_active() {
            self.life_alerts(&mut list);
        }
        list
    }

    fn life_alerts(&self, list: &mut Vec<Alert>) {
        let n = &self.rules.needs;
        let eat_at = i32::try_from(n.hunger_eat_at).unwrap_or(0) * crate::life::MILLI;
        for (id, c) in self.colonists.iter() {
            let (x, y) = c.tile();
            let eating = matches!(c.act, Some(crate::NeedAct::Eat { .. }));
            if c.hunger <= 0 || (c.hunger <= eat_at && !eating) {
                list.push(Alert {
                    kind: AlertKind::Hunger {
                        colonist: id,
                        x,
                        y,
                        starving: c.hunger <= 0,
                    },
                });
            }
            if c.hypothermia > 0 {
                list.push(Alert {
                    kind: AlertKind::Hypothermia {
                        colonist: id,
                        x,
                        y,
                        value: c.hypothermia,
                    },
                });
            }
            if c.heatstroke > 0 {
                list.push(Alert {
                    kind: AlertKind::Heatstroke {
                        colonist: id,
                        x,
                        y,
                        value: c.heatstroke,
                    },
                });
            }
            if let Some(mood) = self.mood(id)
                && mood.total < i32::from(n.mood_low_below)
            {
                list.push(Alert {
                    kind: AlertKind::LowMood {
                        colonist: id,
                        x,
                        y,
                        mood: mood.total,
                    },
                });
            }
        }
        let window = u64::from(n.alert_death_hours) * TICKS_PER_HOUR;
        for g in self.graveyard() {
            let age = self.tick.saturating_sub(g.tick);
            if age < window {
                list.push(Alert {
                    kind: AlertKind::Death {
                        x: g.x,
                        y: g.y,
                        cause: g.cause,
                        hours_ago: u32::try_from(age / TICKS_PER_HOUR).unwrap_or(u32::MAX),
                    },
                });
            }
        }
    }
}
