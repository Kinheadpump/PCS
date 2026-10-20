//! Step 13: history (TECHNICAL_DESIGN §8.1, GAME_DESIGN §9.2).
//!
//! The history runs on the region graph in 1-year steps. It makes civilizations, settlements,
//! sites, artifacts, and events, and four cell overlays: owner, road, depletion, and sanctity.
//! It has its own seed and hash array, so the world hashes of steps 1 to 12 never change
//! (D-047). It reads `Regions.deposits` (the repaired overlay), not the raw deposits (D-050).
//! Its numbers are in `history.ron`, `cultures.ron`, and `event_templates.ron`. They are
//! embedded with `include_str!` and parsed once.

mod road;
mod sim;

use crate::world::{LAYER_STAGE_COUNT, REGION_STAGE_COUNT, STAGE_COUNT, WorldData};
use base::Grid;
use base::content::{
    ContentError, CultureDef, HistoryRules, load_cultures, load_event_templates, load_history_rules,
};
use base::hash::Fnv1a;
use base::rng::mix;
use std::sync::OnceLock;

const HISTORY_RON: &str = include_str!("../../../../assets/content/history.ron");
const CULTURES_RON: &str = include_str!("../../../../assets/content/cultures.ron");
const TEMPLATES_RON: &str = include_str!("../../../../assets/content/event_templates.ron");

/// Value of an event index that means "no event".
pub const NO_EVENT: u32 = u32::MAX;
/// Value of a year that means "never".
pub const NEVER: u16 = u16::MAX;

/// Number of history hashes. They have their own array (D-047, D-050).
pub const HISTORY_STAGE_COUNT: usize = 2;

/// A part of the history output with its own hash.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HistoryStage {
    /// Civilizations, settlements, sites, artifacts, and events.
    Records,
    /// Owner, road, depletion, and sanctity overlays.
    Overlays,
}

impl HistoryStage {
    /// All parts in order.
    pub const ALL: [HistoryStage; HISTORY_STAGE_COUNT] =
        [HistoryStage::Records, HistoryStage::Overlays];

    /// Short stable name, used in golden files and tool output.
    pub fn name(self) -> &'static str {
        const NAMES: [&str; HISTORY_STAGE_COUNT] = ["history_records", "history_overlays"];
        NAMES[self as usize]
    }
}

/// Fixed tag of the history seed. It continues the tags of steps 1 to 12.
fn history_tag() -> u64 {
    0x574F_524C_4400_0000 + (STAGE_COUNT + LAYER_STAGE_COUNT + REGION_STAGE_COUNT) as u64
}

/// The 10 MVP event types (TECHNICAL_DESIGN §8.1). Each one writes a map result (R-040).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum EventKind {
    /// A civilization starts. Result: civilization state.
    FoundCiv,
    /// A civilization founds a settlement. Result: settlement site, owner overlay.
    FoundSettlement,
    /// A civilization opens a mine. Result: mine site.
    OpenMine,
    /// A mine has no more ore. Result: depletion overlay.
    MineDepleted,
    /// A civilization builds a road. Result: road overlay.
    BuildRoad,
    /// Two civilizations fight. Result: battlefield site.
    Battle,
    /// A settlement is destroyed. Result: the settlement site becomes a ruin.
    Raze,
    /// A civilization ends. Result: civilization state, its settlements become ruins.
    CivCollapse,
    /// An artifact is lost. Result: artifact location in a site.
    ArtifactLost,
    /// A mine is sealed. Result: sealed site with a dormant threat.
    Seal,
}

impl EventKind {
    /// Number of event types.
    pub const COUNT: usize = 10;
    /// All event types in order. The order feeds `event_templates.ron`.
    pub const ALL: [EventKind; EventKind::COUNT] = [
        EventKind::FoundCiv,
        EventKind::FoundSettlement,
        EventKind::OpenMine,
        EventKind::MineDepleted,
        EventKind::BuildRoad,
        EventKind::Battle,
        EventKind::Raze,
        EventKind::CivCollapse,
        EventKind::ArtifactLost,
        EventKind::Seal,
    ];

    /// Stable name. It equals the `kind` field in `event_templates.ron`.
    pub fn name(self) -> &'static str {
        const NAMES: [&str; EventKind::COUNT] = [
            "FoundCiv",
            "FoundSettlement",
            "OpenMine",
            "MineDepleted",
            "BuildRoad",
            "Battle",
            "Raze",
            "CivCollapse",
            "ArtifactLost",
            "Seal",
        ];
        NAMES[self as usize]
    }
}

/// One event. It holds no strings: text comes from templates when a player reads it.
/// IDs are 1-based (0 means none).
///
/// | Kind | actors | place | extra |
/// |---|---|---|---|
/// | FoundCiv | civ | capital region | culture |
/// | FoundSettlement | civ, settlement | site | region |
/// | OpenMine | civ | site | mineral (1-based) + richness × 256 |
/// | MineDepleted | civ | site | richness |
/// | BuildRoad | civ, settlement, settlement | first region | path length |
/// | Battle | attacker, defender | site | defender region |
/// | Raze | attacker, defender, settlement | site | region |
/// | CivCollapse | civ | capital region | ruined settlements |
/// | ArtifactLost | civ, artifact | site | artifact kind |
/// | Seal | civ | site | threat level |
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Event {
    /// Year of the event.
    pub year: u16,
    /// Event type.
    pub kind: EventKind,
    /// Up to 3 actor IDs.
    pub actors: [u32; 3],
    /// Site ID or region ID, by kind (see the table).
    pub place: u32,
    /// Extra value, by kind.
    pub extra: u32,
}

/// A historical people. A civilization is alive while `collapsed` is `NEVER`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Civ {
    /// ID from 1.
    pub id: u32,
    /// Index into the culture list.
    pub culture: u8,
    /// Year of the start.
    pub founded: u16,
    /// Year of the collapse (`NEVER` if alive).
    pub collapsed: u16,
    /// Region of the first settlement.
    pub capital: u16,
    /// Drive: tendency to found settlements (0 to 100).
    pub expansion: u8,
    /// Drive: tendency to fight (0 to 100).
    pub aggression: u8,
    /// Wealth and stability (0 to 100).
    pub prosperity: u8,
    /// Index of the `FoundCiv` event.
    pub found_event: u32,
    /// Index of the `CivCollapse` event (`NO_EVENT` if alive).
    pub end_event: u32,
}

/// A settlement. It has one site. A destroyed settlement keeps its site as a ruin.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Settlement {
    /// ID from 1.
    pub id: u32,
    /// Owner civilization.
    pub civ: u32,
    /// Region.
    pub region: u16,
    /// Cell x.
    pub x: u16,
    /// Cell y.
    pub y: u16,
    /// Year of the founding.
    pub founded: u16,
    /// Year of the end (`NEVER` if it still stands).
    pub ended: u16,
    /// Site ID.
    pub site: u32,
}

/// The kind of a site (GAME_DESIGN §3.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum SiteKind {
    /// A living settlement.
    Settlement,
    /// A mine, working or old.
    Mine,
    /// A place of a battle.
    Battlefield,
    /// A destroyed or abandoned settlement.
    Ruin,
    /// A sealed mine with a dormant threat.
    Sealed,
}

impl SiteKind {
    /// Number of site kinds.
    pub const COUNT: usize = 5;
    /// All kinds in order.
    pub const ALL: [SiteKind; SiteKind::COUNT] = [
        SiteKind::Settlement,
        SiteKind::Mine,
        SiteKind::Battlefield,
        SiteKind::Ruin,
        SiteKind::Sealed,
    ];

    /// Short stable name.
    pub fn name(self) -> &'static str {
        const NAMES: [&str; SiteKind::COUNT] =
            ["settlement", "mine", "battlefield", "ruin", "sealed"];
        NAMES[self as usize]
    }
}

/// A place that history made. This is the site record that local map generation reads (S11).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Site {
    /// ID from 1.
    pub id: u32,
    /// Current kind.
    pub kind: SiteKind,
    /// Civilization that made the site.
    pub civ: u32,
    /// Region.
    pub region: u16,
    /// Cell x.
    pub x: u16,
    /// Cell y.
    pub y: u16,
    /// Year the site was made.
    pub year: u16,
    /// Index of the event that made the site.
    pub event: u32,
    /// Year the site ended: a ruin, a closed mine, or a sealed mine (`NEVER` if it did not).
    pub ended: u16,
    /// Index of the event that ended the site (`Raze`, `CivCollapse`, `MineDepleted`, `Seal`).
    pub cause: u32,
    /// Mineral index plus 1 of a mine (0 for other sites).
    pub mineral: u8,
    /// Richness of a mine (1 to 3), or the threat of a sealed site (1 to 3).
    pub level: u8,
}

/// A lost artifact. It lies in a site.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Artifact {
    /// ID from 1.
    pub id: u32,
    /// Civilization that owned it.
    pub civ: u32,
    /// Year it was lost.
    pub year: u16,
    /// Site where it lies.
    pub site: u32,
    /// Kind (0 to 3). The local map picks the item.
    pub kind: u8,
    /// Index of the `ArtifactLost` event.
    pub event: u32,
}

/// Number of hard limits of the history.
pub const LIMIT_COUNT: usize = 5;
/// Names of the hard limits, in the order of `History::skipped`.
pub const LIMIT_NAMES: [&str; LIMIT_COUNT] =
    ["civs", "settlements", "sites", "artifacts", "events"];

/// The generated history. Immutable after generation.
#[derive(Clone, Debug)]
pub struct History {
    /// History seed.
    pub seed: u64,
    /// Length in years.
    pub years: u32,
    /// Hash of each part, indexed by `HistoryStage`.
    pub hashes: [u64; HISTORY_STAGE_COUNT],
    /// Civilizations. The index is the ID minus 1.
    pub civs: Vec<Civ>,
    /// Settlements. The index is the ID minus 1.
    pub settlements: Vec<Settlement>,
    /// Sites. The index is the ID minus 1.
    pub sites: Vec<Site>,
    /// Artifacts. The index is the ID minus 1.
    pub artifacts: Vec<Artifact>,
    /// Events. The years do not decrease.
    pub events: Vec<Event>,
    /// Owner civilization of each cell at the end (0 for none).
    pub owner: Grid<u16>,
    /// Road overlay: number of roads over the cell (saturates at 255).
    pub road: Grid<u8>,
    /// Depletion overlay: 0 untouched to 255 mined out.
    pub depletion: Grid<u8>,
    /// Sanctity overlay: positive near sealed sites, negative near battlefields.
    pub sanctity: Grid<i8>,
    /// Number of alive civilizations at the end of each year.
    pub alive_by_year: Vec<u8>,
    /// Number of rolls skipped at a hard limit, in the order of `LIMIT_NAMES`.
    pub skipped: [u32; LIMIT_COUNT],
}

impl History {
    /// Civilization by ID.
    pub fn civ(&self, id: u32) -> Option<&Civ> {
        usize::try_from(id)
            .ok()
            .and_then(|i| i.checked_sub(1))
            .and_then(|i| self.civs.get(i))
    }

    /// Settlement by ID.
    pub fn settlement(&self, id: u32) -> Option<&Settlement> {
        usize::try_from(id)
            .ok()
            .and_then(|i| i.checked_sub(1))
            .and_then(|i| self.settlements.get(i))
    }

    /// Site by ID.
    pub fn site(&self, id: u32) -> Option<&Site> {
        usize::try_from(id)
            .ok()
            .and_then(|i| i.checked_sub(1))
            .and_then(|i| self.sites.get(i))
    }

    /// Number of alive civilizations at the end.
    pub fn alive_civs(&self) -> usize {
        self.civs.iter().filter(|c| c.collapsed == NEVER).count()
    }

    /// Number of events of one type.
    pub fn count(&self, kind: EventKind) -> usize {
        self.events.iter().filter(|e| e.kind == kind).count()
    }

    /// Region of an event, if it has one.
    pub fn event_region(&self, e: &Event) -> Option<u16> {
        match e.kind {
            EventKind::FoundCiv | EventKind::BuildRoad | EventKind::CivCollapse => {
                u16::try_from(e.place).ok()
            }
            _ => self.site(e.place).map(|s| s.region),
        }
    }

    fn civ_name(&self, id: u32, cultures: &[CultureDef]) -> String {
        let culture = self
            .civ(id)
            .and_then(|c| cultures.get(usize::from(c.culture)))
            .map_or("?", |c| c.name.as_str());
        format!("{culture}-{id}")
    }

    /// Text of an event from the templates. Returns an empty string if the embedded
    /// content is bad.
    pub fn render(&self, e: &Event) -> String {
        let Ok(t) = history_tables() else {
            return String::new();
        };
        let Some(template) = t.templates.get(e.kind as usize) else {
            return String::new();
        };
        let culture = self
            .civ(e.actors[0])
            .and_then(|c| t.cultures.get(usize::from(c.culture)))
            .map_or("?", |c| c.name.as_str());
        let region = self.event_region(e).unwrap_or(0);
        let second = match e.kind {
            EventKind::Battle | EventKind::Raze => self.civ_name(e.actors[1], &t.cultures),
            _ => String::new(),
        };
        let extra = match e.kind {
            EventKind::BuildRoad => e.extra,
            _ => 0,
        };
        let mut out = template.clone();
        let names: [(&str, String); 7] = [
            ("{year}", e.year.to_string()),
            ("{civ}", self.civ_name(e.actors[0], &t.cultures)),
            ("{culture}", culture.to_string()),
            ("{civ2}", second),
            ("{site}", format!("site {}", e.place)),
            ("{region}", region.to_string()),
            ("{extra}", extra.to_string()),
        ];
        for (name, value) in names {
            out = out.replace(name, &value);
        }
        out
    }
}

/// The history tables. Parsed once from the embedded files.
#[derive(Debug)]
pub struct HistoryTables {
    /// Numbers of the simulation.
    pub rules: HistoryRules,
    /// Cultures.
    pub cultures: Vec<CultureDef>,
    /// Event text templates, in `EventKind` order.
    pub templates: Vec<String>,
}

static TABLES: OnceLock<Result<HistoryTables, ContentError>> = OnceLock::new();

fn build_tables() -> Result<HistoryTables, ContentError> {
    let rules = load_history_rules("history.ron", HISTORY_RON)?;
    let cultures = load_cultures("cultures.ron", CULTURES_RON)?;
    let kinds = EventKind::ALL.map(EventKind::name);
    let templates = load_event_templates("event_templates.ron", TEMPLATES_RON, &kinds)?;
    Ok(HistoryTables {
        rules,
        cultures,
        templates,
    })
}

/// The history tables. Parsed from the embedded files on the first call.
pub fn history_tables() -> Result<&'static HistoryTables, &'static ContentError> {
    TABLES.get_or_init(build_tables).as_ref()
}

/// Check all embedded content tables of `worldgen`: rocks, minerals, and history.
/// Call it at world generation start (`try_generate_world` does it).
pub fn check_content() -> Result<(), ContentError> {
    crate::geology::check_tables()?;
    history_tables().map(|_| ()).map_err(Clone::clone)
}

/// Generate the history of a world. `years` is `None` for the default length. A value is
/// clamped to the range in `history.ron`. The embedded files are checked first.
pub fn generate_history(world: &WorldData, years: Option<u32>) -> Result<History, ContentError> {
    let tables = history_tables().map_err(Clone::clone)?;
    let rules = &tables.rules;
    let years = years
        .unwrap_or(rules.years_default)
        .clamp(rules.years_min, rules.years_max);
    let seed = mix(world.seed ^ history_tag());
    Ok(sim::run(world, tables, seed, years))
}

/// Feed the records and the overlays into the two history hashes.
pub(crate) fn hash_history(h: &History) -> [u64; HISTORY_STAGE_COUNT] {
    let mut r = Fnv1a::new();
    r.write(&h.seed.to_le_bytes());
    r.write_u32(h.years);
    for c in &h.civs {
        r.write_u32(c.id);
        r.write(&[c.culture, c.expansion, c.aggression, c.prosperity]);
        r.write(&c.founded.to_le_bytes());
        r.write(&c.collapsed.to_le_bytes());
        r.write(&c.capital.to_le_bytes());
    }
    for s in &h.settlements {
        r.write_u32(s.id);
        r.write_u32(s.civ);
        r.write_u32(s.site);
        for v in [s.region, s.x, s.y, s.founded, s.ended] {
            r.write(&v.to_le_bytes());
        }
    }
    for s in &h.sites {
        r.write_u32(s.id);
        r.write_u32(s.civ);
        r.write_u32(s.event);
        r.write_u32(s.cause);
        r.write(&[s.kind as u8, s.mineral, s.level]);
        for v in [s.region, s.x, s.y, s.year, s.ended] {
            r.write(&v.to_le_bytes());
        }
    }
    for a in &h.artifacts {
        r.write_u32(a.id);
        r.write_u32(a.civ);
        r.write_u32(a.site);
        r.write_u32(a.event);
        r.write(&[a.kind]);
        r.write(&a.year.to_le_bytes());
    }
    for e in &h.events {
        r.write(&e.year.to_le_bytes());
        r.write(&[e.kind as u8]);
        for a in e.actors {
            r.write_u32(a);
        }
        r.write_u32(e.place);
        r.write_u32(e.extra);
    }
    let mut o = Fnv1a::new();
    for v in h.owner.as_slice() {
        o.write(&v.to_le_bytes());
    }
    o.write(h.road.as_slice());
    o.write(h.depletion.as_slice());
    for v in h.sanctity.as_slice() {
        o.write(&v.to_le_bytes());
    }
    [r.finish(), o.finish()]
}
