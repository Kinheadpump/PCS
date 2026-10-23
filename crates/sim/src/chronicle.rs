//! The chronicle and the end of the run (R-050).
//!
//! The chronicle is a list of entries. An entry holds a tick, a kind, and three numbers.
//! The text comes from `chronicle_templates.ron`: the code never builds free text. The run
//! ends when the last colonist is gone (dead or left) or when the player abandons the
//! colony.

use crate::Sim;
use crate::life::DeathCause;
use crate::loader::{Issues, parse};
use crate::time::{GameTime, Season, TICKS_PER_DAY, TICKS_PER_YEAR};
use base::content::ContentError;
use serde::{Deserialize, Serialize};

/// The kind of a chronicle entry. Each kind has a template in the content file.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChronicleKind {
    /// A colonist starved. Number 0: age.
    DeathStarvation,
    /// A colonist froze. Number 0: age.
    DeathHypothermia,
    /// A colonist died of heat. Number 0: age.
    DeathHeatstroke,
    /// A colonist died of wounds. Number 0: age.
    DeathWounds,
    /// A colonist died of an unknown cause. Number 0: age.
    DeathUnknown,
    /// A colonist left the colony. Number 0: age.
    Left,
    /// The first winter begins. Number 0: colonists. Number 1: days of food.
    WinterBegins,
    /// The first winter ends. Number 0: colonists alive. Number 1: deaths in the winter.
    WinterSurvived,
    /// The last colonist is gone.
    ColonyLost,
    /// The player abandoned the colony. Number 0: colonists.
    ColonyAbandoned,
    /// Frost killed crop plants. Number 0: how many.
    CropsKilled,
    /// The first harvest. Number 0: units.
    FirstHarvest,
    /// Food rotted. Number 0: units.
    FoodRotted,
}

impl ChronicleKind {
    /// All kinds, in index order.
    pub const ALL: [ChronicleKind; 13] = [
        ChronicleKind::DeathStarvation,
        ChronicleKind::DeathHypothermia,
        ChronicleKind::DeathHeatstroke,
        ChronicleKind::DeathWounds,
        ChronicleKind::DeathUnknown,
        ChronicleKind::Left,
        ChronicleKind::WinterBegins,
        ChronicleKind::WinterSurvived,
        ChronicleKind::ColonyLost,
        ChronicleKind::ColonyAbandoned,
        ChronicleKind::CropsKilled,
        ChronicleKind::FirstHarvest,
        ChronicleKind::FoodRotted,
    ];

    /// The key in the template file.
    pub fn key(self) -> &'static str {
        match self {
            ChronicleKind::DeathStarvation => "death_starvation",
            ChronicleKind::DeathHypothermia => "death_hypothermia",
            ChronicleKind::DeathHeatstroke => "death_heatstroke",
            ChronicleKind::DeathWounds => "death_wounds",
            ChronicleKind::DeathUnknown => "death_unknown",
            ChronicleKind::Left => "left",
            ChronicleKind::WinterBegins => "winter_begins",
            ChronicleKind::WinterSurvived => "winter_survived",
            ChronicleKind::ColonyLost => "colony_lost",
            ChronicleKind::ColonyAbandoned => "colony_abandoned",
            ChronicleKind::CropsKilled => "crops_killed",
            ChronicleKind::FirstHarvest => "first_harvest",
            ChronicleKind::FoodRotted => "food_rotted",
        }
    }

    /// The kind of the death entry for a cause.
    pub fn of_death(cause: DeathCause) -> Self {
        match cause {
            DeathCause::Starvation => ChronicleKind::DeathStarvation,
            DeathCause::Hypothermia => ChronicleKind::DeathHypothermia,
            DeathCause::Heatstroke => ChronicleKind::DeathHeatstroke,
            DeathCause::Wounds => ChronicleKind::DeathWounds,
            DeathCause::Unknown => ChronicleKind::DeathUnknown,
        }
    }
}

/// One entry of the chronicle.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    /// The tick of the event.
    pub tick: u64,
    /// The kind.
    pub kind: ChronicleKind,
    /// The numbers for the placeholders `{0}`, `{1}`, `{2}`.
    pub args: [i32; 3],
}

/// The entries of one season.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChronicleSeason {
    /// Year, from 1.
    pub year: u64,
    /// The season.
    pub season: Season,
    /// The entries, oldest first.
    pub entries: Vec<Entry>,
}

/// Numbers and templates of the chronicle (`chronicle_templates.ron`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChronicleRules {
    /// The chronicle keeps at most this many entries.
    pub max_entries: usize,
    /// Season names: spring, summer, autumn, winter.
    pub seasons: Vec<String>,
    /// A day with at least this many rotten food units gets an entry.
    pub food_rotted_min: u32,
    /// One template for each kind, in `ChronicleKind::ALL` order.
    pub templates: Vec<String>,
}

impl ChronicleRules {
    /// Rules without text. For the empty default rules.
    pub fn empty() -> Self {
        Self {
            max_entries: 100,
            seasons: vec![String::new(); 4],
            food_rotted_min: u32::MAX,
            templates: vec![String::new(); ChronicleKind::ALL.len()],
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawTemplate {
    kind: String,
    text: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawChronicle {
    max_entries: usize,
    seasons: Vec<String>,
    food_rotted_min: u32,
    templates: Vec<RawTemplate>,
}

const PLACEHOLDERS: [&str; 6] = ["{0}", "{1}", "{2}", "{day}", "{season}", "{year}"];

/// Load and check `chronicle_templates.ron`.
pub fn load_chronicle_rules(file: &str, text: &str) -> Result<ChronicleRules, ContentError> {
    let raw: RawChronicle = parse(file, text)?;
    let mut issues = Issues::new(file);
    if raw.max_entries == 0 || raw.max_entries > 100_000 {
        issues.add("max_entries", "must be from 1 to 100000");
    }
    if raw.seasons.len() != 4 || raw.seasons.iter().any(String::is_empty) {
        issues.add("seasons", "need 4 names");
    }
    let mut templates = vec![String::new(); ChronicleKind::ALL.len()];
    let mut seen = vec![false; ChronicleKind::ALL.len()];
    for t in &raw.templates {
        let Some(i) = ChronicleKind::ALL.iter().position(|k| k.key() == t.kind) else {
            issues.add(&t.kind, "unknown kind");
            continue;
        };
        if seen[i] {
            issues.add(&t.kind, "duplicate template");
        }
        seen[i] = true;
        // Every brace must open a known placeholder.
        let mut rest = t.text.as_str();
        while let Some(pos) = rest.find('{') {
            rest = &rest[pos..];
            match PLACEHOLDERS.iter().find(|p| rest.starts_with(**p)) {
                Some(p) => rest = &rest[p.len()..],
                None => {
                    issues.add(&t.kind, "unknown placeholder");
                    break;
                }
            }
        }
        templates[i] = t.text.clone();
    }
    for (i, kind) in ChronicleKind::ALL.iter().enumerate() {
        if !seen[i] {
            issues.add(kind.key(), "no template for this kind");
        }
    }
    issues.finish(ChronicleRules {
        max_entries: raw.max_entries,
        seasons: raw.seasons,
        food_rotted_min: raw.food_rotted_min,
        templates,
    })
}

/// The legacy score (R-050).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Legacy {
    /// Full years that the colony lived.
    pub years_survived: u32,
    /// The most colonists at the same time.
    pub peak_population: u32,
    /// Winters, droughts, and raids that the colony lived through.
    pub crises_survived: u32,
    /// Lore fragments unlocked.
    pub knowledge_unlocked: u32,
}

/// The state of the run (R-050).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RunState {
    /// The colony lives.
    Running,
    /// The last colonist died or left (R-050).
    Lost {
        /// The tick of the end.
        tick: u64,
    },
    /// The player abandoned the colony.
    Abandoned {
        /// The tick of the end.
        tick: u64,
    },
}

/// The run: its state and the numbers for the end screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunInfo {
    /// The state.
    pub state: RunState,
    /// The most colonists that lived at the same time.
    pub peak_population: u32,
    /// Colonists who left the colony (they count as lost, R-050).
    pub left: u32,
    /// Crises that the colony lived through: winters, droughts, and raids (R-050).
    pub crises_survived: u32,
    /// Lore fragments that the colony unlocked (R-042). A later sprint counts them.
    pub knowledge_unlocked: u32,
}

impl Default for RunInfo {
    fn default() -> Self {
        Self {
            state: RunState::Running,
            peak_population: 0,
            left: 0,
            crises_survived: 0,
            knowledge_unlocked: 0,
        }
    }
}

/// The chronicle state. It is part of the saved state.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChronicleState {
    pub(crate) entries: Vec<Entry>,
}

impl Sim {
    // ---- queries ----

    /// All entries, oldest first.
    pub fn chronicle(&self) -> &[Entry] {
        &self.chronicle.entries
    }

    /// The text of an entry, from the template of its kind.
    pub fn chronicle_text(&self, entry: &Entry) -> String {
        let rules = &self.rules.chronicle;
        let template = rules
            .templates
            .get(
                ChronicleKind::ALL
                    .iter()
                    .position(|&k| k == entry.kind)
                    .unwrap_or(0),
            )
            .map_or("", String::as_str);
        let t = GameTime::from_tick(entry.tick);
        let season = rules
            .seasons
            .get(season_index(t.season))
            .map_or("", String::as_str);
        template
            .replace("{0}", &entry.args[0].to_string())
            .replace("{1}", &entry.args[1].to_string())
            .replace("{2}", &entry.args[2].to_string())
            .replace("{day}", &t.day_of_season.to_string())
            .replace("{season}", season)
            .replace("{year}", &t.year.to_string())
    }

    /// The name of a season from the content file.
    pub fn season_name(&self, season: Season) -> &str {
        self.rules
            .chronicle
            .seasons
            .get(season_index(season))
            .map_or("", String::as_str)
    }

    /// The entries grouped by season, oldest first. Seasons without entries are left out.
    pub fn chronicle_by_season(&self) -> Vec<ChronicleSeason> {
        let mut out: Vec<ChronicleSeason> = Vec::new();
        for entry in &self.chronicle.entries {
            let t = GameTime::from_tick(entry.tick);
            match out.last_mut() {
                Some(last) if last.year == t.year && last.season == t.season => {
                    last.entries.push(*entry);
                }
                _ => out.push(ChronicleSeason {
                    year: t.year,
                    season: t.season,
                    entries: vec![*entry],
                }),
            }
        }
        out
    }

    /// The state of the run (R-050).
    pub fn run_state(&self) -> RunState {
        self.run.state
    }

    /// The legacy score of the run (R-050): years survived, peak population, crises survived, and
    /// knowledge unlocked. While the run lasts, the years count to now.
    pub fn legacy(&self) -> Legacy {
        let info = self.run_info();
        let end = match self.run.state {
            RunState::Running => self.tick,
            RunState::Lost { tick } | RunState::Abandoned { tick } => tick,
        };
        Legacy {
            years_survived: u32::try_from(end / crate::time::TICKS_PER_YEAR).unwrap_or(u32::MAX),
            peak_population: info.peak_population,
            crises_survived: info.crises_survived,
            knowledge_unlocked: info.knowledge_unlocked,
        }
    }

    /// True at the first tick of a game day (00:00) while the run lasts. The interface writes the
    /// ironman save then (R-050, D-032).
    pub fn ironman_save_due(&self) -> bool {
        self.run.state == RunState::Running
            && self.tick > 0
            && self.tick.is_multiple_of(TICKS_PER_DAY)
    }

    /// Count a crisis that the colony survived.
    pub(crate) fn crisis_survived(&mut self) {
        self.run.crises_survived = self.run.crises_survived.saturating_add(1);
    }

    /// The run state with the numbers for the end screen.
    pub fn run_info(&self) -> RunInfo {
        let alive = u32::try_from(self.colonists.len()).unwrap_or(u32::MAX);
        RunInfo {
            peak_population: self.run.peak_population.max(alive),
            ..self.run
        }
    }

    // ---- writes ----

    /// Add an entry. The oldest entries go when the list is full.
    pub(crate) fn chronicle_add(&mut self, kind: ChronicleKind, args: [i32; 3]) {
        let entry = Entry {
            tick: self.tick,
            kind,
            args,
        };
        self.chronicle.entries.push(entry);
        let cap = self.rules.chronicle.max_entries.max(1);
        if self.chronicle.entries.len() > cap {
            let extra = self.chronicle.entries.len() - cap;
            self.chronicle.entries.drain(..extra);
        }
    }

    /// A colonist died or left: write the entry and check the end of the run (R-050).
    pub(crate) fn chronicle_gone(&mut self, kind: ChronicleKind, age: u8) {
        // The colonist is already removed: the count before was one more.
        let before = u32::try_from(self.colonists.len()).unwrap_or(u32::MAX) + 1;
        self.run.peak_population = self.run.peak_population.max(before);
        if kind == ChronicleKind::Left {
            self.run.left += 1;
        }
        self.chronicle_add(kind, [i32::from(age), 0, 0]);
        self.check_run_end();
    }

    /// The run ends when no colonist is left (R-050).
    pub(crate) fn check_run_end(&mut self) {
        if self.run.state == RunState::Running && self.colonists.is_empty() {
            self.run.state = RunState::Lost { tick: self.tick };
            self.chronicle_add(ChronicleKind::ColonyLost, [0, 0, 0]);
        }
    }

    /// The player abandons the colony (R-050).
    pub(crate) fn abandon(&mut self) {
        if self.run.state != RunState::Running {
            return;
        }
        let n = i32::try_from(self.colonists.len()).unwrap_or(i32::MAX);
        self.run.state = RunState::Abandoned { tick: self.tick };
        self.chronicle_add(ChronicleKind::ColonyAbandoned, [n, 0, 0]);
    }

    /// Each game day (the tick number is a multiple of 2400): the ledger snapshot, the peak
    /// population, the food that rotted, and the entries of the first winter.
    pub(crate) fn colony_daily_tick(&mut self) {
        if !self.tick.is_multiple_of(TICKS_PER_DAY) {
            return;
        }
        let alive = u32::try_from(self.colonists.len()).unwrap_or(u32::MAX);
        self.run.peak_population = self.run.peak_population.max(alive);
        self.ledger_snapshot();
        let rotted = std::mem::take(&mut self.ledger.rotted_day);
        if rotted >= u64::from(self.rules.chronicle.food_rotted_min) && rotted > 0 {
            let n = i32::try_from(rotted).unwrap_or(i32::MAX);
            self.chronicle_add(ChronicleKind::FoodRotted, [n, 0, 0]);
        }
        if self.run.state != RunState::Running {
            return;
        }
        let winter_start = (TICKS_PER_YEAR / 4) * 3;
        if self.tick == winter_start {
            let days = self.ledger().food.days_remaining_tenths.unwrap_or(0) / 10;
            self.chronicle_add(
                ChronicleKind::WinterBegins,
                [
                    i32::try_from(self.colonists.len()).unwrap_or(i32::MAX),
                    i32::try_from(days).unwrap_or(i32::MAX),
                    0,
                ],
            );
        } else if self.tick == TICKS_PER_YEAR {
            self.crisis_survived();
            let died = self
                .graveyard
                .iter()
                .filter(|g| g.tick >= winter_start && g.tick < TICKS_PER_YEAR)
                .count();
            self.chronicle_add(
                ChronicleKind::WinterSurvived,
                [
                    i32::try_from(self.colonists.len()).unwrap_or(i32::MAX),
                    i32::try_from(died).unwrap_or(i32::MAX),
                    0,
                ],
            );
        }
    }

    /// Check a loaded chronicle against the content.
    pub(crate) fn chronicle_state_ok(&self) -> bool {
        self.chronicle.entries.len() <= self.rules.chronicle.max_entries.max(1)
    }
}

fn season_index(season: Season) -> usize {
    match season {
        Season::Spring => 0,
        Season::Summer => 1,
        Season::Autumn => 2,
        Season::Winter => 3,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn templates_file_checks() {
        let text = include_str!("../../../assets/content/chronicle_templates.ron");
        let rules = load_chronicle_rules("chronicle_templates.ron", text).unwrap();
        assert_eq!(rules.templates.len(), ChronicleKind::ALL.len());
        let bad = text.replace("{0} units of food rotted.", "{9} units of food rotted.");
        let err = load_chronicle_rules("chronicle_templates.ron", &bad)
            .unwrap_err()
            .to_string();
        assert!(
            err.contains("food_rotted") && err.contains("placeholder"),
            "{err}"
        );
        let missing = text.replace("\"left\"", "\"went\"");
        let err = load_chronicle_rules("chronicle_templates.ron", &missing)
            .unwrap_err()
            .to_string();
        assert!(err.contains("left"), "{err}");
    }
}
