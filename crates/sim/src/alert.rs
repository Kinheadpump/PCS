//! Alerts: short warnings for the player (TECHNICAL_DESIGN §3.3).
//!
//! The alert list is a pure function of the state. `Sim::alerts` makes it when the
//! interface asks, so an alert is never out of date and is not part of the saved state.
//! Sprint 5 has two alerts: low fuel in a hearth and a cold period.

use crate::Sim;

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
            AlertKind::LowFuel { x, y, .. } => Some((x, y)),
            AlertKind::ColdPeriod { .. } => None,
        }
    }
}

impl Sim {
    /// The alerts now. Order: the cold period first, then the low fuel alerts in tile
    /// order (row, then column).
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
        list
    }
}
