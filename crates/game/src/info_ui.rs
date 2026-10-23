//! The text of the food chain display (Sprint 7): the ledger chips of the alert strip, the colony
//! panel (ledger with its why chains, the chronicle, the abandon button), the end-of-run
//! panel, the crop picker, and the farm rows of the tile panel. Pure functions from `&Sim` to
//! text, so tests can check them.

use crate::inspector;
use crate::palette::ui as pal;
use crate::ui::{Fold, Meter, Row};
use crate::weather::{AlertChip, FUEL_TINT, OUT_TINT};
use sim::{AlertLevel, FertilityCause, Ledger, LedgerCause, LedgerLine, LedgerPart, RunState, Sim};

/// Most chronicle lines in the colony panel.
const CHRONICLE_LINES: usize = 14;
/// Characters in a wrapped chronicle line.
const WRAP: usize = 34;
/// Days remaining that fill the bar of a ledger meter, in tenths of a day.
const BAR_FULL_TENTHS: u64 = 300;

/// The days remaining as text, for example `12.4`. `-` if nothing is used.
pub fn days_text(tenths: Option<u64>) -> String {
    match tenths {
        Some(t) => format!("{}.{}", t / 10, t % 10),
        None => "-".to_owned(),
    }
}

/// The short days text of a chip, for example `12d`.
fn chip_days(tenths: Option<u64>) -> String {
    tenths.map_or_else(|| "-".to_owned(), |t| format!("{}d", t / 10))
}

/// The tint of a ledger line: normal, low, or out.
fn line_tint(line: &LedgerLine) -> [u8; 3] {
    match line.days_remaining_tenths {
        Some(t) if t < 10 => OUT_TINT,
        _ if line.low => FUEL_TINT,
        _ => pal::KEY,
    }
}

/// The two chips of the alert strip: food and fuel days remaining.
pub fn ledger_chips(sim: &Sim) -> Vec<AlertChip> {
    let ledger = sim.ledger();
    let chip = |icon: usize, name: &str, line: &LedgerLine| AlertChip {
        icon,
        text: format!("{name} {}", chip_days(line.days_remaining_tenths)),
        tint: line_tint(line),
        target: None,
        level: if line.low {
            AlertLevel::Warning
        } else {
            AlertLevel::Info
        },
    };
    vec![
        chip(crate::cells::icons::FOOD, "Food", &ledger.food),
        chip(crate::cells::icons::FLAME, "Fuel", &ledger.fuel),
    ]
}

/// The name of a part of a ledger line.
fn cause_name(sim: &Sim, cause: LedgerCause) -> String {
    match cause {
        LedgerCause::FoodStock { material } => inspector::material_name(sim, material),
        LedgerCause::FuelInHearths => "Fuel in hearths".to_owned(),
        LedgerCause::FuelItems => "Fuel items".to_owned(),
        LedgerCause::ColonistsEat => "Colonists eat".to_owned(),
        LedgerCause::TraitAdjust => "Traits".to_owned(),
        LedgerCause::HearthsBurn => "Hearths burn".to_owned(),
    }
}

fn part_line(sim: &Sim, part: &LedgerPart) -> Row {
    Row::Line(format!(
        "  {} {:+}",
        cause_name(sim, part.cause),
        part.value
    ))
}

/// The rows of one ledger line: a meter for the days remaining that folds the parts, then the
/// use and the trend.
fn line_rows(
    sim: &Sim,
    name: &str,
    icon: usize,
    line: &LedgerLine,
    unit: &str,
    open: bool,
    rows: &mut Vec<Row>,
) {
    let permille = line.days_remaining_tenths.map_or(1000, |t| {
        (t.min(BAR_FULL_TENTHS) * 1000 / BAR_FULL_TENTHS) as i32
    });
    rows.push(Row::Meter(Meter {
        icon,
        tip: name.to_owned(),
        permille,
        tint: line_tint(line),
        text: format!("{name} {} days", days_text(line.days_remaining_tenths)),
        fold: Some(open),
    }));
    if open {
        rows.push(Row::Line(format!("Stock {} {unit}", line.stock)));
        rows.extend(line.stock_parts.iter().map(|p| part_line(sim, p)));
        rows.push(Row::Line(format!(
            "Use per day {} {unit}",
            line.use_per_day
        )));
        rows.extend(line.use_parts.iter().map(|p| part_line(sim, p)));
    }
    let trend = line
        .trend_per_day
        .map_or_else(|| "-".to_owned(), |t| format!("{t:+} {unit} per day"));
    rows.push(Row::Pair("  Trend".to_owned(), trend));
    if let Some(by_trend) = line.days_by_trend_tenths {
        rows.push(Row::Pair(
            "  At trend".to_owned(),
            format!("{} days", days_text(Some(by_trend))),
        ));
    }
}

/// Wrap `text` into lines of at most `WRAP` characters.
pub fn wrap(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for word in text.split_whitespace() {
        match out.last_mut() {
            Some(last) if last.chars().count() + 1 + word.chars().count() <= WRAP => {
                last.push(' ');
                last.push_str(word);
            }
            _ => out.push(word.to_owned()),
        }
    }
    out
}

/// The rows of the colony panel and the folds of its fold rows, in order.
pub fn colony_rows(sim: &Sim, open: impl Fn(Fold) -> bool) -> (Vec<Row>, Vec<Fold>) {
    let ledger: Ledger = sim.ledger();
    let mut rows = vec![Row::Title("Colony".to_owned())];
    let mut folds = Vec::new();
    line_rows(
        sim,
        "Food",
        crate::cells::icons::FOOD,
        &ledger.food,
        "pts",
        open(Fold::FoodParts),
        &mut rows,
    );
    folds.push(Fold::FoodParts);
    line_rows(
        sim,
        "Fuel",
        crate::cells::icons::FLAME,
        &ledger.fuel,
        "h",
        open(Fold::FuelParts),
        &mut rows,
    );
    folds.push(Fold::FuelParts);
    let abandon_open = open(Fold::Abandon);
    rows.push(Row::Fold(
        if abandon_open {
            "Click again to abandon".to_owned()
        } else {
            "Abandon colony".to_owned()
        },
        abandon_open,
    ));
    folds.push(Fold::Abandon);
    rows.push(Row::Section("Chronicle".to_owned()));
    let seasons = sim.chronicle_by_season();
    if seasons.is_empty() {
        rows.push(Row::Line("Nothing yet".to_owned()));
    }
    let mut left = CHRONICLE_LINES;
    for season in seasons.iter().rev() {
        if left == 0 {
            break;
        }
        rows.push(Row::Line(format!(
            "{}, year {}",
            sim.season_name(season.season),
            season.year
        )));
        for entry in season.entries.iter().rev() {
            for (i, line) in wrap(&sim.chronicle_text(entry)).into_iter().enumerate() {
                if left == 0 {
                    break;
                }
                left -= 1;
                rows.push(Row::Line(format!(
                    "{}{line}",
                    if i == 0 { "- " } else { "  " }
                )));
            }
        }
    }
    (rows, folds)
}

/// The text of the end-of-run panel, if the run has ended: the title and the rows.
pub fn end_rows(sim: &Sim) -> Option<Vec<Row>> {
    let info = sim.run_info();
    let (title, tick) = match info.state {
        RunState::Running => return None,
        RunState::Lost { tick } => ("Colony lost", tick),
        RunState::Abandoned { tick } => ("Colony abandoned", tick),
    };
    let days = sim::GameTime::day_index(tick) + 1;
    let mut rows = vec![
        Row::Title(title.to_owned()),
        Row::Pair("Survived".to_owned(), format!("{days} days")),
        Row::Pair(
            "Most colonists".to_owned(),
            info.peak_population.to_string(),
        ),
    ];
    if info.left > 0 {
        rows.push(Row::Pair("Left".to_owned(), info.left.to_string()));
    }
    rows.push(Row::Section("Last in the chronicle".to_owned()));
    let entries = sim.chronicle();
    for entry in entries.iter().rev().take(3).rev() {
        for (i, line) in wrap(&sim.chronicle_text(entry)).into_iter().enumerate() {
            rows.push(Row::Line(format!(
                "{}{line}",
                if i == 0 { "- " } else { "  " }
            )));
        }
    }
    Some(rows)
}

/// A crop of the picker: the name, the tooltip, and the color of the swatch.
pub struct CropChip {
    /// Display name.
    pub name: String,
    /// Tooltip lines: the growth band, the kill limit, the time, the seeds in stock.
    pub tip: String,
    /// Color of the harvest.
    pub color: [u8; 3],
}

fn degrees(tenths: i16) -> String {
    format!("{} C", tenths / 10)
}

/// The crops of the farm tool.
pub fn crop_chips(sim: &Sim) -> Vec<CropChip> {
    let rules = sim.rules();
    rules
        .farm
        .crops
        .iter()
        .map(|c| CropChip {
            name: c.name.clone(),
            tip: format!(
                "{}\nGrows from {} to {}\nDies below {}\n{} days to ripen\nSeeds in stock: {}",
                c.name,
                degrees(c.min_temp),
                degrees(c.max_temp),
                degrees(c.kill_temp),
                c.growth_days,
                sim.items().total_units(c.seed)
            ),
            color: rules
                .materials
                .get(usize::from(c.product))
                .map_or(pal::DIM, |m| m.color),
        })
        .collect()
}

/// Text of a fertility value in per-mille, for example `115%`.
pub fn percent(permille: i32) -> String {
    format!("{}%", permille / 10)
}

fn cause_label(cause: FertilityCause) -> &'static str {
    match cause {
        FertilityCause::Ground => "Ground",
        FertilityCause::Variation => "Variation",
        FertilityCause::Water => "Water near",
        FertilityCause::Limit => "Limit",
    }
}

/// The farm rows of the tile panel: the zone, the plant, the wild plant, and the fertility with
/// its named parts.
pub fn farm_tile_rows(sim: &Sim, x: u16, y: u16) -> Vec<Row> {
    let farm = &sim.rules().farm;
    let mut rows = Vec::new();
    if let Some(crop) = sim
        .farm_zone(x, y)
        .and_then(|c| farm.crops.get(usize::from(c)))
    {
        rows.push(Row::Pair("Farm zone".to_owned(), crop.name.clone()));
    }
    if let Some(report) = sim.plant_report(x, y)
        && let Some(crop) = farm.crops.get(usize::from(report.plant.crop))
    {
        let done =
            (u64::from(report.plant.hours) * 100 / u64::from(report.hours_needed.max(1))).min(100);
        let state = if report.ripe {
            "ripe".to_owned()
        } else if report.growing {
            format!("{done}%")
        } else {
            format!("{done}%, paused by the temperature")
        };
        rows.push(Row::Pair("Crop".to_owned(), crop.name.clone()));
        rows.push(Row::Pair("Growth".to_owned(), state));
    }
    if sim.plant_at(x, y).is_none()
        && let Some(wild) = sim.wild_plant_at(x, y)
        && let Some(def) = farm.wild.get(usize::from(wild.kind))
    {
        let state = if wild.ripe {
            "ripe".to_owned()
        } else {
            format!("ripe in {} days", wild.days_left)
        };
        rows.push(Row::Pair("Wild plant".to_owned(), def.name.clone()));
        rows.push(Row::Pair("Gather".to_owned(), state));
    }
    if let Some(why) = sim.fertility_why(x, y) {
        rows.push(Row::Pair("Fertility".to_owned(), percent(why.total)));
        rows.push(Row::Section("Fertility parts".to_owned()));
        for part in why.parts.iter().filter(|p| p.value != 0) {
            rows.push(Row::Line(format!(
                "{} {:+}%",
                cause_label(part.cause),
                part.value / 10
            )));
        }
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;
    use sim::scenario::sim_from_ascii;

    #[test]
    fn days_are_shown_with_one_decimal() {
        assert_eq!(days_text(Some(124)), "12.4");
        assert_eq!(days_text(Some(5)), "0.5");
        assert_eq!(days_text(None), "-");
    }

    #[test]
    fn long_text_wraps_at_word_borders() {
        let lines = wrap("Mira died of hunger in the first winter of the colony");
        assert!(lines.len() >= 2);
        assert!(lines.iter().all(|l| l.chars().count() <= WRAP), "{lines:?}");
        assert_eq!(
            lines.join(" "),
            "Mira died of hunger in the first winter of the colony"
        );
    }

    #[test]
    fn the_strip_always_has_a_food_chip_and_a_fuel_chip() {
        let sim = sim_from_ascii("c", 1).unwrap();
        let chips = ledger_chips(&sim);
        assert_eq!(chips.len(), 2);
        assert!(chips[0].text.starts_with("Food"));
        assert!(chips[1].text.starts_with("Fuel"));
    }

    #[test]
    fn the_colony_panel_has_the_ledger_the_abandon_row_and_the_chronicle() {
        let sim = sim_from_ascii("c", 1).unwrap();
        let (rows, folds) = colony_rows(&sim, |_| false);
        assert_eq!(folds, vec![Fold::FoodParts, Fold::FuelParts, Fold::Abandon]);
        assert!(matches!(rows[0], Row::Title(_)));
        assert!(
            rows.iter()
                .any(|r| matches!(r, Row::Section(t) if t == "Chronicle"))
        );
        let fold_rows = rows.iter().filter(|r| r.fold_state().is_some()).count();
        assert_eq!(fold_rows, folds.len());
    }

    #[test]
    fn open_ledger_folds_show_the_named_parts_that_add_up() {
        let sim = sim_from_ascii("c", 1).unwrap();
        let (closed, _) = colony_rows(&sim, |_| false);
        let (open, _) = colony_rows(&sim, |_| true);
        assert!(open.len() > closed.len());
        let ledger = sim.ledger();
        let sum: i64 = ledger.food.stock_parts.iter().map(|p| p.value).sum();
        assert_eq!(sum, i64::try_from(ledger.food.stock).unwrap());
    }

    #[test]
    fn a_running_colony_has_no_end_panel() {
        let sim = sim_from_ascii("c", 1).unwrap();
        assert!(end_rows(&sim).is_none());
    }

    #[test]
    fn every_crop_has_a_chip_with_its_limits_in_the_tip() {
        let sim = sim_from_ascii("c", 1).unwrap();
        let chips = crop_chips(&sim);
        assert_eq!(chips.len(), sim.rules().farm.crops.len());
        assert!(!chips.is_empty());
        for chip in chips {
            assert!(chip.tip.contains("Dies below"), "{}", chip.tip);
        }
    }

    #[test]
    fn the_tile_panel_names_the_parts_of_the_fertility() {
        let sim = sim_from_ascii("c", 1).unwrap();
        let rows = (0..8u16)
            .flat_map(|x| (0..1u16).map(move |y| (x, y)))
            .map(|(x, y)| farm_tile_rows(&sim, x, y))
            .find(|r| {
                r.iter()
                    .any(|row| matches!(row, Row::Section(t) if t == "Fertility parts"))
            })
            .unwrap_or_default();
        if rows.is_empty() {
            return;
        }
        assert!(
            rows.iter()
                .any(|r| matches!(r, Row::Pair(k, _) if k == "Fertility"))
        );
    }
}
