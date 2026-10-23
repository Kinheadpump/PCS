//! The ledger: stock, trend, and days remaining for food and fuel (GAME_DESIGN §5).
//!
//! The stock and the use rate are sums of named parts (why chain, D-021). The days remaining
//! are the stock divided by the use rate. The trend comes from daily snapshots of the stock.

use crate::Sim;
use crate::item::ItemLoc;
use crate::loader::{Issues, parse};
use base::content::ContentError;
use serde::{Deserialize, Serialize};

/// Numbers for the ledger (`ledger.ron`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LedgerRules {
    /// Daily snapshots that the trend uses.
    pub history_days: u32,
    /// Food stock below this many days remaining is low.
    pub food_low_days: u32,
    /// Fuel stock below this many days remaining is low.
    pub fuel_low_days: u32,
}

impl LedgerRules {
    /// Rules for the empty default rules.
    pub fn empty() -> Self {
        Self {
            history_days: 5,
            food_low_days: 0,
            fuel_low_days: 0,
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawLedger {
    history_days: u32,
    food_low_days: u32,
    fuel_low_days: u32,
}

/// Load and check `ledger.ron`.
pub fn load_ledger_rules(file: &str, text: &str) -> Result<LedgerRules, ContentError> {
    let raw: RawLedger = parse(file, text)?;
    let mut issues = Issues::new(file);
    if raw.history_days < 2 || raw.history_days > 100 {
        issues.add("history_days", "must be from 2 to 100");
    }
    issues.finish(LedgerRules {
        history_days: raw.history_days,
        food_low_days: raw.food_low_days,
        fuel_low_days: raw.fuel_low_days,
    })
}

/// The ledger state. It is part of the saved state.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LedgerState {
    /// Food stock at the start of the last days, oldest first.
    pub(crate) food_hist: Vec<u64>,
    /// Fuel stock (burning hours) at the start of the last days, oldest first.
    pub(crate) fuel_hist: Vec<u64>,
    /// Food units that rotted since the last day entry.
    pub(crate) rotted_day: u64,
}

/// What a part of a ledger line is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LedgerCause {
    /// Food of one material: units times nutrition.
    FoodStock {
        /// Material index.
        material: u16,
    },
    /// Burning hours stored in the hearths.
    FuelInHearths,
    /// Burning hours of fuel items that lie around or are carried.
    FuelItems,
    /// The colonists eat: their number times the hunger per day.
    ColonistsEat,
    /// The traits change the hunger of some colonists.
    TraitAdjust,
    /// The hearths burn 24 hours each day.
    HearthsBurn,
}

/// One named part of a stock or a use rate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LedgerPart {
    /// The cause.
    pub cause: LedgerCause,
    /// The value. The parts add up to the total.
    pub value: i64,
}

/// One line of the ledger: food or fuel.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LedgerLine {
    /// The stock now. Food: nutrition points. Fuel: burning hours.
    pub stock: u64,
    /// The parts of the stock.
    pub stock_parts: Vec<LedgerPart>,
    /// The use for each game day, in the unit of the stock.
    pub use_per_day: u64,
    /// The parts of the use.
    pub use_parts: Vec<LedgerPart>,
    /// The change of the stock for each game day over the last days. `None` without history.
    pub trend_per_day: Option<i64>,
    /// Days remaining at the use rate, in tenths of a day. `None` if nothing is used.
    pub days_remaining_tenths: Option<u64>,
    /// Days remaining at the trend, in tenths of a day. `None` if the stock does not fall.
    pub days_by_trend_tenths: Option<u64>,
    /// True if the days remaining are below the low limit.
    pub low: bool,
}

/// The ledger of food and fuel.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ledger {
    /// Food.
    pub food: LedgerLine,
    /// Fuel.
    pub fuel: LedgerLine,
}

/// The change for each day between the first and the last daily snapshot.
fn trend(hist: &[u64]) -> Option<i64> {
    let (first, last) = (*hist.first()?, *hist.last()?);
    let days = i64::try_from(hist.len()).unwrap_or(i64::MAX) - 1;
    if days < 1 {
        return None;
    }
    Some((i64::try_from(last).unwrap_or(i64::MAX) - i64::try_from(first).unwrap_or(0)) / days)
}

fn line(
    stock_parts: Vec<LedgerPart>,
    use_parts: Vec<LedgerPart>,
    hist: &[u64],
    low_days: u32,
) -> LedgerLine {
    let stock = u64::try_from(stock_parts.iter().map(|p| p.value).sum::<i64>()).unwrap_or(0);
    let use_per_day = u64::try_from(use_parts.iter().map(|p| p.value).sum::<i64>()).unwrap_or(0);
    let trend_per_day = trend(hist);
    let days_remaining_tenths = (use_per_day > 0).then(|| stock.saturating_mul(10) / use_per_day);
    let days_by_trend_tenths = trend_per_day
        .filter(|&t| t < 0)
        .map(|t| stock.saturating_mul(10) / t.unsigned_abs());
    let low = days_remaining_tenths.is_some_and(|d| d < u64::from(low_days) * 10);
    LedgerLine {
        stock,
        stock_parts,
        use_per_day,
        use_parts,
        trend_per_day,
        days_remaining_tenths,
        days_by_trend_tenths,
        low,
    }
}

impl Sim {
    /// The food stock as parts: for each food material, units times nutrition. Rotten food
    /// does not count.
    fn food_stock_parts(&self) -> Vec<LedgerPart> {
        let n = &self.rules.needs;
        n.foods
            .iter()
            .map(|f| {
                let units: u64 = self
                    .items
                    .iter()
                    .filter(|(_, i)| i.material == f.material && !self.is_rotten(i))
                    .map(|(_, i)| u64::from(i.count))
                    .sum();
                LedgerPart {
                    cause: LedgerCause::FoodStock {
                        material: f.material,
                    },
                    value: i64::try_from(units * u64::from(f.nutrition)).unwrap_or(i64::MAX),
                }
            })
            .filter(|p| p.value > 0)
            .collect()
    }

    fn fuel_stock_parts(&self) -> Vec<LedgerPart> {
        let in_hearths: u64 = self
            .hearth_tiles()
            .into_iter()
            .map(|(x, y)| u64::from(self.hearth_fuel(x, y)))
            .sum();
        let in_items: u64 = self
            .items
            .iter()
            .filter(|(_, i)| matches!(i.loc, ItemLoc::Tile(..) | ItemLoc::Carried(_)))
            .filter_map(|(_, i)| {
                self.rules
                    .fuel
                    .iter()
                    .find(|f| f.material == i.material)
                    .map(|f| u64::from(i.count) * u64::from(f.hours))
            })
            .sum();
        vec![
            LedgerPart {
                cause: LedgerCause::FuelInHearths,
                value: i64::try_from(in_hearths).unwrap_or(i64::MAX),
            },
            LedgerPart {
                cause: LedgerCause::FuelItems,
                value: i64::try_from(in_items).unwrap_or(i64::MAX),
            },
        ]
    }

    fn food_use_parts(&self) -> Vec<LedgerPart> {
        let n = &self.rules.needs;
        let per_day = i64::from(n.hunger_per_day);
        let mut base = 0i64;
        let mut adjust = 0i64;
        for (_, c) in self.colonists.iter() {
            let pct = c
                .traits
                .iter()
                .filter_map(|&t| self.rules.traits.get(usize::from(t)))
                .fold(100i64, |acc, t| acc * i64::from(t.hunger_rate_pct) / 100);
            base += per_day;
            adjust += per_day * pct / 100 - per_day;
        }
        vec![
            LedgerPart {
                cause: LedgerCause::ColonistsEat,
                value: base,
            },
            LedgerPart {
                cause: LedgerCause::TraitAdjust,
                value: adjust,
            },
        ]
    }

    /// The ledger of food and fuel now, with the why chain (D-021).
    pub fn ledger(&self) -> Ledger {
        let r = &self.rules.ledger;
        let hearths = i64::try_from(self.hearth_tiles().len()).unwrap_or(0);
        Ledger {
            food: line(
                self.food_stock_parts(),
                self.food_use_parts(),
                &self.ledger.food_hist,
                r.food_low_days,
            ),
            fuel: line(
                self.fuel_stock_parts(),
                vec![LedgerPart {
                    cause: LedgerCause::HearthsBurn,
                    value: hearths * 24,
                }],
                &self.ledger.fuel_hist,
                r.fuel_low_days,
            ),
        }
    }

    /// Store the stock of today in the history. Called each game day.
    pub(crate) fn ledger_snapshot(&mut self) {
        let now = self.ledger();
        let keep = self.rules.ledger.history_days.max(2) as usize;
        for (hist, value) in [
            (&mut self.ledger.food_hist, now.food.stock),
            (&mut self.ledger.fuel_hist, now.fuel.stock),
        ] {
            hist.push(value);
            if hist.len() > keep {
                let extra = hist.len() - keep;
                hist.drain(..extra);
            }
        }
    }
}
