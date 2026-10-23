//! Food spoilage (R-005).
//!
//! Each food stack has a rot value in thousandths (`Item::rot`). Each game hour the rot
//! grows by the rot rate of the material times the rate factor of the temperature on the
//! tile of the stack (`spoilage.ron`). Frozen food (below 0 degrees C) does not rot. At the
//! rotten limit the stack is rotten: colonists do not eat it and no job takes it. A rotten
//! stack that nobody holds ends at the next game hour.

use crate::Sim;
use crate::item::{Item, ItemId, ItemLoc};
use crate::loader::{Issues, parse};
use crate::time::{HOURS_PER_DAY, TICKS_PER_HOUR};
use base::content::ContentError;
use serde::Deserialize;

/// Numbers for spoilage (`spoilage.ron`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpoilRules {
    /// The rot value of a rotten stack, in thousandths.
    pub rotten_at: u32,
    /// Rate bands: (upper limit in tenths of a degree, exclusive; factor in per-mille).
    pub bands: Vec<(i16, u32)>,
    /// The rate factor at and above the last limit, in per-mille.
    pub top_factor: u32,
    /// The rate of smoked food in percent of the normal rate. A later sprint uses it.
    pub smoked_pct: u32,
}

impl SpoilRules {
    /// Rules without bands. Food never rots. For the empty default rules.
    pub fn empty() -> Self {
        Self {
            rotten_at: 100_000,
            bands: Vec::new(),
            top_factor: 0,
            smoked_pct: 100,
        }
    }

    /// The rate factor in per-mille for a temperature in tenths of a degree (R-005).
    pub fn factor(&self, temp: i16) -> u32 {
        self.bands
            .iter()
            .find(|(limit, _)| temp < *limit)
            .map_or(self.top_factor, |(_, factor)| *factor)
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawSpoil {
    rotten_at: u32,
    bands: Vec<(i16, u32)>,
    top_factor: u32,
    smoked_pct: u32,
}

/// Load and check `spoilage.ron`.
pub fn load_spoil_rules(file: &str, text: &str) -> Result<SpoilRules, ContentError> {
    let raw: RawSpoil = parse(file, text)?;
    let mut issues = Issues::new(file);
    if raw.rotten_at == 0 || raw.rotten_at > 10_000_000 {
        issues.add("rotten_at", "must be from 1 to 10000000");
    }
    if raw.bands.windows(2).any(|w| w[0].0 >= w[1].0) {
        issues.add("bands", "the limits must grow");
    }
    if raw
        .bands
        .iter()
        .map(|b| b.1)
        .chain([raw.top_factor])
        .any(|f| f > 10_000)
    {
        issues.add("bands", "a factor must be at most 10000 per-mille");
    }
    if raw.smoked_pct > 100 {
        issues.add("smoked_pct", "must be at most 100");
    }
    issues.finish(SpoilRules {
        rotten_at: raw.rotten_at,
        bands: raw.bands,
        top_factor: raw.top_factor,
        smoked_pct: raw.smoked_pct,
    })
}

/// The rot that a stack gains in a day, in thousandths: the rotten limit times the rot rate
/// (per-mille of the limit for each day at factor 1000) times the factor (per-mille).
pub fn rot_per_day(rules: &SpoilRules, rot_rate: u16, factor: u32) -> u32 {
    let per_day =
        u64::from(rules.rotten_at) * u64::from(rot_rate) * u64::from(factor) / 1000 / 1000;
    u32::try_from(per_day).unwrap_or(u32::MAX)
}

/// The share of the day total that falls in the game hour `hour` (0 to 23). The shares of
/// the 24 hours add up to the day total.
pub fn rot_hour_share(per_day: u32, hour: u32) -> u32 {
    let share = |h: u64| u64::from(per_day) * h / 24;
    let h = u64::from(hour % 24);
    u32::try_from(share(h + 1) - share(h)).unwrap_or(u32::MAX)
}

/// What a part of a rot rate is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RotCause {
    /// The rot rate of the material, in per-mille of the limit for each day.
    BaseRate,
    /// The rate factor of the temperature on the tile, in per-mille.
    Temperature,
}

/// One named part of a rot rate. The rate is the product of the parts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RotPart {
    /// What the part is.
    pub cause: RotCause,
    /// Its value.
    pub value: u32,
}

/// Why a stack rots at its speed (R-005, why chain).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RotReport {
    /// Rot value now, in thousandths.
    pub rot: u32,
    /// Rot value of a rotten stack.
    pub rotten_at: u32,
    /// The temperature on the tile of the stack, in tenths of a degree.
    pub temperature: i16,
    /// The parts of the rate: the base rate and the temperature factor.
    pub parts: Vec<RotPart>,
    /// Rot that the stack gains in a day, in thousandths.
    pub per_day: u32,
    /// Game hours until the stack is rotten at this rate. `None` if it does not rot now.
    pub hours_left: Option<u32>,
}

impl Sim {
    /// The temperature around a stack, in tenths of a degree: the temperature of its tile,
    /// or of the tile of the colonist who carries it.
    pub fn item_temperature(&self, item: &Item) -> Option<i16> {
        let (x, y) = match item.loc {
            ItemLoc::Tile(x, y) => (x, y),
            ItemLoc::Carried(who) => self.colonists.get(who)?.tile(),
        };
        Some(self.room_temp(x, y).unwrap_or_else(|| self.outdoor_temp()))
    }

    /// True if the stack is rotten (R-005).
    pub fn is_rotten(&self, item: &Item) -> bool {
        item.rot >= self.rules.spoil.rotten_at
    }

    /// Why a stack rots at its speed. `None` if the stack does not exist or does not rot.
    pub fn rot_report(&self, id: ItemId) -> Option<RotReport> {
        let item = self.items.get(id)?;
        let rate = self
            .rules
            .materials
            .get(usize::from(item.material))?
            .rot_rate;
        if rate == 0 {
            return None;
        }
        let temperature = self.item_temperature(item)?;
        let factor = self.rules.spoil.factor(temperature);
        let per_day = rot_per_day(&self.rules.spoil, rate, factor);
        let left = self.rules.spoil.rotten_at.saturating_sub(item.rot);
        let hours_left = (per_day > 0).then(|| {
            u32::try_from(u64::from(left) * HOURS_PER_DAY / u64::from(per_day)).unwrap_or(u32::MAX)
        });
        Some(RotReport {
            rot: item.rot,
            rotten_at: self.rules.spoil.rotten_at,
            temperature,
            parts: vec![
                RotPart {
                    cause: RotCause::BaseRate,
                    value: u32::from(rate),
                },
                RotPart {
                    cause: RotCause::Temperature,
                    value: factor,
                },
            ],
            per_day,
            hours_left,
        })
    }

    /// Put a rot value on a stack at once. For scenario tests and debug tools.
    pub fn debug_set_rot(&mut self, id: ItemId, rot: u32) -> bool {
        let rot = rot.min(self.rules.spoil.rotten_at);
        self.items.set_rot(id, rot)
    }

    /// Each game hour: the rot of all food stacks grows. Rotten stacks that nobody holds
    /// end, and the ledger counts their units.
    pub(crate) fn spoil_tick(&mut self) {
        if !self.tick.is_multiple_of(TICKS_PER_HOUR) {
            return;
        }
        let hour = u32::try_from((self.tick / TICKS_PER_HOUR) % HOURS_PER_DAY).unwrap_or(0);
        let outdoor = self.outdoor_temp();
        let limit = self.rules.spoil.rotten_at;
        let mut rotten: Vec<ItemId> = Vec::new();
        let mut updates: Vec<(ItemId, u32)> = Vec::new();
        for (id, item) in self.items.iter() {
            let rate = self
                .rules
                .materials
                .get(usize::from(item.material))
                .map_or(0, |m| m.rot_rate);
            if rate == 0 {
                continue;
            }
            let (x, y) = match item.loc {
                ItemLoc::Tile(x, y) => (x, y),
                ItemLoc::Carried(who) => match self.colonists.get(who) {
                    Some(c) => c.tile(),
                    None => continue,
                },
            };
            let temp = self.room_temp(x, y).unwrap_or(outdoor);
            let factor = self.rules.spoil.factor(temp);
            let per_day = rot_per_day(&self.rules.spoil, rate, factor);
            let rot = item
                .rot
                .saturating_add(rot_hour_share(per_day, hour))
                .min(limit);
            if rot != item.rot {
                updates.push((id, rot));
            }
            if rot >= limit && item.reserved_by.is_none() && matches!(item.loc, ItemLoc::Tile(..)) {
                rotten.push(id);
            }
        }
        for (id, rot) in updates {
            self.items.set_rot(id, rot);
        }
        for id in rotten {
            if let Some(item) = self.items.remove(id) {
                self.ledger.rotted_day += u64::from(item.count);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rules() -> SpoilRules {
        SpoilRules {
            rotten_at: 100_000,
            bands: vec![(0, 0), (80, 250), (250, 1000)],
            top_factor: 2000,
            smoked_pct: 10,
        }
    }

    #[test]
    fn r005_factor_bands() {
        let r = rules();
        assert_eq!(r.factor(-1), 0, "below 0 C: frozen");
        assert_eq!(r.factor(0), 250);
        assert_eq!(r.factor(79), 250);
        assert_eq!(r.factor(80), 1000);
        assert_eq!(r.factor(249), 1000);
        assert_eq!(r.factor(250), 2000);
    }

    #[test]
    fn r005_food_rots_four_times_faster_at_20c_than_at_5c() {
        let r = rules();
        let day = |temp: i16| -> u64 {
            let per_day = rot_per_day(&r, 100, r.factor(temp));
            (0..24).map(|h| u64::from(rot_hour_share(per_day, h))).sum()
        };
        assert_eq!(day(200), 4 * day(50));
        assert_eq!(day(-10), 0, "-1 C: no rot");
        assert!(day(200) > 0);
    }

    #[test]
    fn hour_shares_add_up_to_the_day_total() {
        for per_day in [0u32, 1, 23, 24, 25, 999, 10_000] {
            let sum: u32 = (0..24).map(|h| rot_hour_share(per_day, h)).sum();
            assert_eq!(sum, per_day);
        }
    }

    #[test]
    fn spoil_file_checks() {
        let ok =
            "(rotten_at: 100000, bands: [(0, 0), (80, 250)], top_factor: 1000, smoked_pct: 10)";
        assert!(load_spoil_rules("spoilage.ron", ok).is_ok());
        let bad = ok.replace("(80, 250)", "(0, 250)");
        let err = load_spoil_rules("spoilage.ron", &bad)
            .unwrap_err()
            .to_string();
        assert!(
            err.contains("spoilage.ron") && err.contains("bands"),
            "{err}"
        );
    }
}
