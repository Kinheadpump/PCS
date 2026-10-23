//! Content of the inspector panels and the work panel. Pure: it reads `&Sim` and builds
//! rows of text. Each value that the player sees is a sum of named parts (why chain).

use crate::build_ui;
use crate::cells::icons as ic;
use crate::palette::Rgb;
use crate::tools::priority_text;
use crate::ui::{Meter, Row};
use crate::weather;
use base::content::BuildingKind;
use sim::skill::Skill;
use sim::{
    AbortReason, BuildLayer, Building, ColonistId, ComfortCause, Condition, HealthCause,
    ItemFilter, ItemLoc, JobEvent, JobKind, JobStep, MoodCause, NeedAct, Sex, Sim, SpeedCause,
    SpeedModCause, SpeedPart, StockpileId,
};

/// Color of a bar that is fine.
const GOOD: Rgb = [104, 200, 120];
/// Color of a bar that needs attention.
const WARN: Rgb = [240, 176, 72];
/// Color of a bar in danger.
const BAD: Rgb = [236, 90, 84];
/// Color of a bar that is neither good nor bad (the mood in the middle).
const NEUTRAL: Rgb = [150, 196, 236];

/// Display name of a colonist.
pub fn colonist_name(id: ColonistId) -> String {
    format!("Colonist {}", id.index())
}

/// Display name of a material index.
pub fn material_name(sim: &Sim, material: u16) -> String {
    sim.rules()
        .materials
        .get(usize::from(material))
        .map_or_else(|| "unknown".to_owned(), |m| m.name.clone())
}

/// Text of a step of a job.
pub fn step_text(step: JobStep) -> &'static str {
    match step {
        JobStep::GoToWork => "going to the work tile",
        JobStep::Work { .. } => "working",
        JobStep::GoToItem => "going to the item",
        JobStep::GoToDest => "carrying to the stockpile",
    }
}

/// Name of the cause of a part of a work speed.
pub fn cause_name(cause: SpeedCause) -> String {
    match cause {
        SpeedCause::Base => "Base".to_owned(),
        SpeedCause::SkillLevel { skill, level } => format!("{} level {level}", skill.name()),
    }
}

/// A signed percent, for example `+5` or `-10`.
pub fn signed(pct: i32) -> String {
    format!("{pct:+}")
}

/// The lines of a why chain: one for each named part.
pub fn speed_part_lines(parts: &[SpeedPart]) -> Vec<String> {
    parts
        .iter()
        .map(|p| format!("  {} {}", cause_name(p.cause), signed(p.pct)))
        .collect()
}

/// Name of a trait by its index.
fn trait_name(sim: &Sim, index: u8) -> String {
    sim.rules()
        .traits
        .get(usize::from(index))
        .map_or_else(|| "Trait".to_owned(), |t| t.name.clone())
}

/// Name of the cause of a mood modifier.
pub fn mood_cause_name(sim: &Sim, cause: MoodCause) -> String {
    match cause {
        MoodCause::Base => "Base".to_owned(),
        MoodCause::Hungry => "Hungry".to_owned(),
        MoodCause::Starving => "Starving".to_owned(),
        MoodCause::Tired => "Tired".to_owned(),
        MoodCause::Exhausted => "Exhausted".to_owned(),
        MoodCause::Cold => "Cold".to_owned(),
        MoodCause::Hot => "Hot".to_owned(),
        MoodCause::Wounded => "Wounded".to_owned(),
        MoodCause::RoomBeauty { per_tile } => format!("Room beauty ({per_tile:+})"),
        MoodCause::Memory { key } => sim
            .rules()
            .needs
            .memories
            .get(usize::from(key))
            .map_or_else(|| "Memory".to_owned(), |m| m.name.clone()),
        _ => "Other".to_owned(),
    }
}

/// Name of the cause of a part of the lower limit of the comfort band.
fn comfort_cause_name(sim: &Sim, cause: ComfortCause) -> String {
    match cause {
        ComfortCause::Base => "Base".to_owned(),
        ComfortCause::Clothes { material } => material_name(sim, material),
        ComfortCause::Trait { index } => trait_name(sim, index),
    }
}

/// Name of the cause of a part of a work speed that comes from life.
fn speed_mod_name(sim: &Sim, cause: SpeedModCause) -> String {
    match cause {
        SpeedModCause::HighMood => "High mood".to_owned(),
        SpeedModCause::LowMood => "Low mood".to_owned(),
        SpeedModCause::Exhausted => "Exhausted".to_owned(),
        SpeedModCause::Trait { index } => trait_name(sim, index),
        _ => "Other".to_owned(),
    }
}

/// The rows of a work speed: the total, then the named parts (skill, mood, exhaustion, traits).
fn speed_rows(sim: &Sim, id: ColonistId, kind: JobKind, rows: &mut Vec<Row>) {
    let Some(speed) = sim.effective_work_speed(id, kind) else {
        return;
    };
    rows.push(Row::Pair(
        kind.name().to_owned(),
        format!("{}%", speed.total_pct),
    ));
    rows.extend(
        speed_part_lines(&speed.base.parts)
            .into_iter()
            .map(Row::Line),
    );
    for m in &speed.mods {
        rows.push(Row::Line(format!(
            "  {} {}",
            speed_mod_name(sim, m.cause),
            signed(m.pct)
        )));
    }
}

/// The priorities as lines of two job types each, so the panel stays narrow.
pub fn priorities_lines(priorities: &[u8; JobKind::COUNT]) -> Vec<String> {
    JobKind::ALL
        .chunks(2)
        .map(|kinds| {
            kinds
                .iter()
                .map(|k| format!("{} {}", k.name(), priority_text(priorities[k.index()])))
                .collect::<Vec<_>>()
                .join("  ")
        })
        .collect()
}

/// Text of what a colonist does because of a need or a breakdown, if it does something like that.
pub fn need_activity(sim: &Sim, c: &sim::Colonist) -> Option<String> {
    if c.break_until > sim.tick() {
        return Some("On a break".to_owned());
    }
    Some(match c.act? {
        NeedAct::Eat { progress: None, .. } => "Walking to food".to_owned(),
        NeedAct::Eat { .. } => "Eating".to_owned(),
        NeedAct::Wear { item, .. } => sim.items().get(item).map_or_else(
            || "Getting clothes".to_owned(),
            |i| format!("Getting {}", material_name(sim, i.material).to_lowercase()),
        ),
        NeedAct::Sleep { asleep: false, .. } => "Walking to bed".to_owned(),
        NeedAct::Sleep { bed: Some(_), .. } => "Sleeping in a bed".to_owned(),
        NeedAct::Sleep { bed: None, .. } => "Sleeping on the ground".to_owned(),
    })
}

/// Color of a need bar: red at 0, amber at or below the limit, green above it.
fn need_tint(value: i32, warn_at: i32) -> Rgb {
    if value <= 0 {
        BAD
    } else if value <= warn_at {
        WARN
    } else {
        GOOD
    }
}

/// The condition tags, one for each condition, with the icon and the color.
fn condition_rows(sim: &Sim, id: ColonistId, c: &sim::Colonist, rows: &mut Vec<Row>) {
    for cond in sim.conditions(id).unwrap_or_default() {
        let (icon, tint, text) = match cond {
            Condition::Starving => (ic::FOOD, BAD, cond.name().to_owned()),
            Condition::Hypothermia => (
                ic::THERMOMETER,
                weather::COLD_TINT,
                format!("{} {}", cond.name(), c.hypothermia),
            ),
            Condition::Heatstroke => (ic::SUN, BAD, format!("{} {}", cond.name(), c.heatstroke)),
            Condition::Exhausted => (ic::MOON, BAD, cond.name().to_owned()),
            Condition::Wounded => (ic::HEART, BAD, cond.name().to_owned()),
        };
        rows.push(Row::Tag(icon, text, tint));
    }
}

/// Name of the cause of a part of the hourly health change.
fn health_cause_name(cause: HealthCause) -> &'static str {
    cause.name()
}

/// The rows of the needs, the mood, and the health: bars with icons, and the why chains as
/// lines that the colonist panel puts behind folds.
fn life_rows(sim: &Sim, id: ColonistId, c: &sim::Colonist, rows: &mut Vec<Row>) {
    let n = &sim.rules().needs;
    if !n.enabled {
        return;
    }
    let meter = |icon, tip: &str, permille, tint, text, fold| {
        Row::Meter(Meter {
            icon,
            tip: tip.to_owned(),
            permille,
            tint,
            text,
            fold,
        })
    };
    let ratio = |v: i32, max: u32| v.saturating_mul(1000) / i32::try_from(max.max(1)).unwrap_or(1);
    let eat_at = i32::try_from(n.hunger_eat_at).unwrap_or(0) * 1000;
    let sleep_at = i32::try_from(n.rest_sleep_at).unwrap_or(0) * 1000;
    condition_rows(sim, id, c, rows);
    rows.push(meter(
        ic::FOOD,
        "Hunger",
        ratio(c.hunger, n.hunger_max.saturating_mul(1000)),
        need_tint(c.hunger, eat_at),
        (c.hunger.max(0) / 1000).to_string(),
        None,
    ));
    rows.push(meter(
        ic::MOON,
        "Rest",
        ratio(c.rest, n.rest_max.saturating_mul(1000)),
        need_tint(c.rest, sleep_at),
        (c.rest.max(0) / 1000).to_string(),
        None,
    ));
    if let Some(mood) = sim.mood(id) {
        let tint = if mood.total < i32::from(n.mood_break_below) {
            BAD
        } else if mood.total < i32::from(n.mood_low_below) {
            WARN
        } else if mood.total >= i32::from(n.mood_high_from) {
            GOOD
        } else {
            NEUTRAL
        };
        let span = i32::from(n.mood_max) - i32::from(n.mood_min);
        rows.push(meter(
            ic::MOOD,
            "Mood",
            (mood.total - i32::from(n.mood_min)) * 1000 / span.max(1),
            tint,
            mood.total.to_string(),
            Some(false),
        ));
        for part in &mood.parts {
            rows.push(Row::Line(format!(
                "  {} {}",
                mood_cause_name(sim, part.cause),
                signed(part.value)
            )));
        }
        if mood.sum != mood.total {
            rows.push(Row::Line(format!(
                "  Sum {}, limited to {}",
                mood.sum, mood.total
            )));
        }
    }
    if let Some(report) = sim.health_report(id) {
        let health = report.health;
        let tint = if health * 4 < report.max {
            BAD
        } else if health * 5 < report.max * 3 {
            WARN
        } else {
            GOOD
        };
        rows.push(meter(
            ic::HEART,
            "Health",
            health.saturating_mul(1000) / report.max.max(1),
            tint,
            format!("{health} {:+}/h", report.per_hour),
            Some(false),
        ));
        for part in &report.parts {
            rows.push(Row::Line(format!(
                "  {} {:+}/h",
                health_cause_name(part.cause),
                part.per_hour
            )));
        }
    }
    if let Some(band) = sim.comfort_band(id) {
        rows.push(Row::Section(format!(
            "Comfort {} to {}",
            weather::tenths_text(band.low),
            weather::temp_text(band.high)
        )));
        for part in &band.parts {
            rows.push(Row::Line(format!(
                "  {} {}",
                comfort_cause_name(sim, part.cause),
                weather::signed_text(part.tenths)
            )));
        }
    }
}

/// The rows for the selected colonist. Empty if the colonist does not exist.
pub fn colonist_rows(sim: &Sim, id: ColonistId) -> Vec<Row> {
    let Some(c) = sim.colonist(id) else {
        return Vec::new();
    };
    let mut rows = vec![Row::Title(colonist_name(id))];
    rows.push(Row::Pair(
        "Position".to_owned(),
        format!("({}, {})", c.x, c.y),
    ));
    if let Some((tx, ty)) = c.target {
        rows.push(Row::Pair("Walking to".to_owned(), format!("({tx}, {ty})")));
    }
    if let Some(text) = need_activity(sim, c) {
        rows.push(Row::Pair("Job".to_owned(), text));
    } else {
        match c.job {
            Some(job) => {
                rows.push(Row::Pair(
                    "Job".to_owned(),
                    format!("{} at ({}, {})", job.kind.name(), job.stand.0, job.stand.1),
                ));
                rows.push(Row::Pair("Step".to_owned(), step_text(job.step).to_owned()));
            }
            None => rows.push(Row::Pair("Job".to_owned(), "idle".to_owned())),
        }
    }
    let sex = match c.sex {
        Sex::Female => "female",
        Sex::Male => "male",
    };
    rows.push(Row::Pair("Age".to_owned(), format!("{}, {sex}", c.age)));
    if !c.traits.is_empty() {
        let names: Vec<String> = c.traits.iter().map(|&t| trait_name(sim, t)).collect();
        rows.push(Row::Pair("Trait".to_owned(), names.join(", ")));
    }
    let carrying = c
        .carrying
        .and_then(|item| sim.items().get(item))
        .map_or_else(
            || "nothing".to_owned(),
            |i| format!("{} x{}", material_name(sim, i.material), i.count),
        );
    rows.push(Row::Pair("Carrying".to_owned(), carrying));
    for (i, line) in priorities_lines(&c.priorities).into_iter().enumerate() {
        let key = if i == 0 { "Priorities" } else { "" };
        rows.push(Row::Pair(key.to_owned(), line));
    }

    life_rows(sim, id, c, &mut rows);

    rows.push(Row::Section("Skills".to_owned()));
    let max_level = sim.rules().jobs.max_level;
    for skill in Skill::ALL {
        let level = sim.skill_level(id, skill).unwrap_or(0);
        let xp = c.skill_xp[skill.index()];
        let value = if level >= max_level {
            format!("L{level} (max)  {xp} xp")
        } else {
            format!("L{level}  {xp} xp")
        };
        rows.push(Row::Pair(skill.name().to_owned(), value));
    }

    rows.push(Row::Section("Work speed".to_owned()));
    for kind in JobKind::ALL {
        speed_rows(sim, id, kind, &mut rows);
    }
    if let Some(cap) = sim.carry_capacity(id) {
        rows.push(Row::Pair("Carries".to_owned(), format!("{cap} units")));
    }
    rows
}

/// The stockpile zone that holds a tile, if any.
pub fn zone_at(sim: &Sim, x: u16, y: u16) -> Option<StockpileId> {
    sim.stockpiles()
        .iter()
        .find(|(_, s)| s.contains(x, y))
        .map(|(id, _)| id)
}

/// Text of a stockpile filter.
pub fn filter_text(sim: &Sim, filter: &ItemFilter, all: &[u16]) -> String {
    match filter {
        ItemFilter::All => "everything".to_owned(),
        ItemFilter::Only(list) if list.is_empty() => "nothing".to_owned(),
        ItemFilter::Only(list) if list.len() <= 2 => list
            .iter()
            .map(|&m| material_name(sim, m))
            .collect::<Vec<_>>()
            .join(", "),
        ItemFilter::Only(list) => format!("{} of {} materials", list.len(), all.len()),
    }
}

/// The rows for a tile: terrain, designation, items, and stockpile.
pub fn tile_rows(sim: &Sim, x: u16, y: u16, haulable: &[u16]) -> Vec<Row> {
    let (ix, iy) = (i32::from(x), i32::from(y));
    let map = sim.map();
    let mut rows = vec![Row::Title(format!("Tile ({x}, {y})"))];
    let ground = map.ground(ix, iy).map_or_else(
        || "outside the map".to_owned(),
        |g| format!("{g:?}").to_lowercase(),
    );
    rows.push(Row::Pair("Ground".to_owned(), capitalise(&ground)));
    let rock = map.rock(ix, iy).map(|r| format!("{r:?}"));
    rows.push(Row::Pair(
        "Rock".to_owned(),
        rock.filter(|r| r != "None")
            .unwrap_or_else(|| "-".to_owned()),
    ));
    let ore = map.ore(ix, iy).map(|o| format!("{o:?}"));
    rows.push(Row::Pair(
        "Ore".to_owned(),
        ore.filter(|o| o != "None")
            .unwrap_or_else(|| "-".to_owned()),
    ));
    rows.push(Row::Pair(
        "Tree".to_owned(),
        if map.tree(ix, iy) { "yes" } else { "-" }.to_owned(),
    ));
    rows.push(Row::Pair(
        "Walkable".to_owned(),
        if map.passable(ix, iy) { "yes" } else { "no" }.to_owned(),
    ));
    temperature_rows(sim, x, y, &mut rows);

    let designation = match sim.designation(x, y) {
        Some(kind) => match sim
            .jobs()
            .find(kind, x, y)
            .and_then(|j| sim.jobs().get(j))
            .and_then(|j| j.claimed_by)
        {
            Some(who) => format!("{}, claimed by {}", kind.name(), colonist_name(who)),
            None => format!("{}, waiting", kind.name()),
        },
        None => "-".to_owned(),
    };
    rows.push(Row::Pair("Designation".to_owned(), designation));
    building_rows(sim, x, y, &mut rows);

    let items: Vec<_> = sim
        .items()
        .at_tile(x, y)
        .iter()
        .filter_map(|&id| sim.items().get(id))
        .collect();
    if items.is_empty() {
        rows.push(Row::Pair("Items".to_owned(), "-".to_owned()));
    } else {
        rows.push(Row::Section("Items".to_owned()));
        for item in items {
            let mut text = format!("{} x{}", material_name(sim, item.material), item.count);
            if item.to_haul {
                text.push_str(", haul");
            }
            if let Some(who) = item.reserved_by {
                text.push_str(&format!(", reserved by {}", colonist_name(who)));
            }
            if matches!(item.loc, ItemLoc::Carried(_)) {
                text.push_str(", carried");
            }
            rows.push(Row::Line(text));
        }
    }

    match zone_at(sim, x, y).and_then(|id| sim.stockpiles().get(id).map(|s| (id, s))) {
        Some((id, zone)) => {
            rows.push(Row::Pair(
                "Stockpile".to_owned(),
                format!(
                    "zone {}, {}x{}",
                    id.index(),
                    zone.x1 - zone.x0 + 1,
                    zone.y1 - zone.y0 + 1
                ),
            ));
            rows.push(Row::Pair(
                "Accepts".to_owned(),
                filter_text(sim, &zone.filter, haulable),
            ));
        }
        None => rows.push(Row::Pair("Stockpile".to_owned(), "-".to_owned())),
    }
    rows.extend(crate::info_ui::farm_tile_rows(sim, x, y));
    room_rows(sim, x, y, &mut rows);
    rows
}

/// Name of a finished building: its material and its type, for example `Granite wall`.
pub fn building_label(sim: &Sim, building: &Building) -> String {
    let kind = sim
        .rules()
        .building(building.def)
        .map_or_else(|| "building".to_owned(), |d| d.name.to_lowercase());
    capitalise(&format!(
        "{} {kind}",
        material_name(sim, building.material).to_lowercase()
    ))
}

/// The layers of a tile with the row name of each.
const LAYER_ROWS: [(BuildLayer, &str); 3] = [
    (BuildLayer::Object, "Building"),
    (BuildLayer::Floor, "Floor"),
    (BuildLayer::Roof, "Roof"),
];

/// Name of the section that holds the named parts of the temperature. The tile panel puts them
/// behind a fold.
pub const TEMP_PARTS: &str = "Temperature parts";

/// Rows for the temperature of a tile and the ice: the value, then the named parts (a why chain)
/// whose sum is the value.
pub fn temperature_rows(sim: &Sim, x: u16, y: u16, rows: &mut Vec<Row>) {
    let Some(t) = sim.tile_temperature(x, y) else {
        return;
    };
    rows.push(Row::Pair(
        "Temperature".to_owned(),
        weather::temp_text(i32::from(t.total)),
    ));
    if sim.ice_at(x, y) {
        rows.push(Row::Pair("Ice".to_owned(), "yes".to_owned()));
    }
    rows.push(Row::Section(TEMP_PARTS.to_owned()));
    rows.push(Row::Line(format!("  {}", weather::place_name(t.place))));
    for part in &t.parts {
        rows.push(Row::Line(format!(
            "  {} {}",
            part.cause.name(),
            weather::signed_text(part.tenths)
        )));
    }
}

/// The fuel row of a hearth: the burning hours that are left and the capacity.
pub fn hearth_fuel_text(sim: &Sim, x: u16, y: u16) -> String {
    let fuel = sim.hearth_fuel(x, y);
    let capacity = sim.rules().climate.hearth_fuel_capacity_hours;
    if fuel == 0 {
        "none".to_owned()
    } else {
        format!("{fuel} of {capacity} h")
    }
}

/// Rows for the buildings and blueprints of a tile: one row for each layer that has one.
pub fn building_rows(sim: &Sim, x: u16, y: u16, rows: &mut Vec<Row>) {
    let buildings = sim.buildings();
    for (layer, name) in LAYER_ROWS {
        if let Some(b) = buildings.get(x, y, layer) {
            let mut text = building_label(sim, b);
            if build_ui::deconstruct_ordered(sim, x, y, layer) {
                text.push_str(", taken down");
            }
            rows.push(Row::Pair(name.to_owned(), text));
        }
        if let Some(bp) = buildings.blueprint(x, y, layer) {
            let def = sim.rules().building(bp.building);
            let cost = def.map_or(0, |d| d.cost);
            let pct = build_ui::blueprint_progress(sim, x, y, layer).unwrap_or(0);
            rows.push(Row::Pair(
                "Blueprint".to_owned(),
                format!(
                    "{} {}, {}/{} units, {pct}%",
                    capitalise(&material_name(sim, bp.material).to_lowercase()),
                    def.map_or("building".to_owned(), |d| d.name.to_lowercase()),
                    bp.delivered,
                    cost
                ),
            ));
        }
    }
    if let Some(b) = buildings.object_at(x, y)
        && sim.rules().is_kind(b.def, BuildingKind::Hearth)
    {
        rows.push(Row::Pair("Fuel".to_owned(), hearth_fuel_text(sim, x, y)));
    }
    if let Some(b) = buildings.object_at(x, y)
        && build_ui::is_workshop(sim.rules(), b.def)
    {
        for order in &b.orders {
            let name = sim
                .rules()
                .recipe(order.recipe)
                .map_or("?", |r| r.name.as_str());
            rows.push(Row::Line(format!("  Order: {name} x{}", order.remaining)));
        }
    }
}

/// Rows for the room that holds the tile: its size, the insulation and the beauty with
/// their named parts (why chains), and the roof support. Nothing if the tile is not in a room.
pub fn room_rows(sim: &Sim, x: u16, y: u16, rows: &mut Vec<Row>) {
    let Some(room) = sim.room_at(x, y) else {
        return;
    };
    let (insulation, beauty) = (&room.insulation, &room.beauty);
    rows.push(Row::Section("Room".to_owned()));
    rows.push(Row::Pair("Tiles".to_owned(), room.tiles.len().to_string()));
    rows.push(Row::Pair(
        "Insulation".to_owned(),
        format!("{} of 100", insulation.total),
    ));
    for part in &insulation.parts {
        rows.push(Row::Line(format!(
            "  {} {} x{} @{}",
            part.kind.name(),
            material_name(sim, part.material),
            part.tiles,
            part.each
        )));
    }
    rows.push(Row::Pair(
        "Beauty".to_owned(),
        format!("{} ({:+} per tile)", beauty.total, beauty.per_tile),
    ));
    for part in &beauty.parts {
        let what = part
            .building
            .and_then(|b| sim.rules().building(b))
            .map_or(part.cause.name().to_owned(), |d| d.name.clone());
        rows.push(Row::Line(format!(
            "  {} {} x{} {:+}",
            what,
            material_name(sim, part.material),
            part.tiles,
            part.each
        )));
    }
    let supported = room
        .tiles
        .iter()
        .filter(|&&(tx, ty)| sim.roof_supported(tx, ty))
        .count();
    rows.push(Row::Pair(
        "Roof support".to_owned(),
        format!("{supported} of {} tiles", room.tiles.len()),
    ));
    if room.rock_border > 0 {
        rows.push(Row::Pair(
            "Rock border".to_owned(),
            format!("{} tiles", room.rock_border),
        ));
    }
}

/// One recipe line of the workshop panel.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecipeInfo {
    /// Recipe index (`recipes.ron` order).
    pub recipe: u16,
    /// Name and the number of runs ordered.
    pub label: String,
    /// Inputs, outputs, and work time.
    pub detail: String,
    /// The keep target now (0 for none).
    pub keep: u32,
}

/// The building type index of the workshop on a tile, if there is one.
pub fn workshop_at(sim: &Sim, x: u16, y: u16) -> Option<u16> {
    sim.buildings()
        .object_at(x, y)
        .map(|b| b.def)
        .filter(|&d| build_ui::is_workshop(sim.rules(), d))
}

fn units_text(sim: &Sim, list: &[(u16, u32)]) -> String {
    list.iter()
        .map(|&(m, n)| format!("{n} {}", material_name(sim, m)))
        .collect::<Vec<_>>()
        .join(" + ")
}

/// The text lines of the workshop panel: the name and the input bin.
pub fn workshop_lines(sim: &Sim, x: u16, y: u16) -> Vec<String> {
    let Some(b) = sim.buildings().object_at(x, y) else {
        return Vec::new();
    };
    let bin = if b.bin.is_empty() {
        "empty".to_owned()
    } else {
        units_text(sim, &b.bin)
    };
    vec![
        format!("{} at ({x}, {y})", building_label(sim, b)),
        format!("Input bin: {bin}"),
    ]
}

/// The recipes that the workshop on a tile can make, with the orders now.
pub fn workshop_recipes(sim: &Sim, x: u16, y: u16) -> Vec<RecipeInfo> {
    let Some(b) = sim.buildings().object_at(x, y) else {
        return Vec::new();
    };
    sim.rules()
        .recipes
        .iter()
        .enumerate()
        .filter(|(_, r)| r.workshop == b.def)
        .filter_map(|(i, r)| {
            let recipe = u16::try_from(i).ok()?;
            let ordered = b
                .orders
                .iter()
                .find(|o| o.recipe == recipe)
                .map_or(0, |o| o.remaining);
            let keep = sim.keep_target(x, y, recipe);
            let mut notes = Vec::new();
            if ordered > 0 {
                notes.push(format!("ordered {ordered}"));
            }
            if keep > 0 {
                notes.push(format!("keep {keep}, have {}", sim.keep_stock(recipe)));
            }
            let label = if notes.is_empty() {
                r.name.clone()
            } else {
                format!("{} ({})", r.name, notes.join(", "))
            };
            Some(RecipeInfo {
                recipe,
                label,
                detail: format!(
                    "{} to {}, {}",
                    units_text(sim, &r.inputs),
                    units_text(sim, &r.outputs),
                    build_ui::hours_text(r.work_ticks)
                ),
                keep,
            })
        })
        .collect()
}

/// Text for the status panel about a job event. `None` for events that need no message.
pub fn job_event_text(event: &JobEvent) -> Option<String> {
    match event {
        JobEvent::Completed { .. } => None,
        JobEvent::Aborted {
            colonist,
            kind,
            reason,
        } => Some(format!(
            "{} stopped {}: {}",
            colonist_name(*colonist),
            kind.name(),
            match reason {
                AbortReason::JobGone => "the order is gone",
                AbortReason::NoPath => "no path",
                AbortReason::NoMaterial => "no material",
            }
        )),
        JobEvent::Rejected(reject) => {
            Some(format!("Order refused: {}", build_ui::reject_text(*reject)))
        }
    }
}

fn capitalise(text: &str) -> String {
    let mut chars = text.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

/// One row of the work panel.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkRow {
    /// The colonist.
    pub id: ColonistId,
    /// The display name.
    pub name: String,
    /// The priority for each job type: 1 to 4, or 0 for off.
    pub priorities: [u8; JobKind::COUNT],
}

/// The rows of the work panel, in colonist slot order.
pub fn work_rows(sim: &Sim) -> Vec<WorkRow> {
    sim.colonists()
        .iter()
        .map(|(id, c)| WorkRow {
            id,
            name: colonist_name(id),
            priorities: c.priorities,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use sim::Command;
    use sim::scenario::sim_from_ascii;

    fn pair<'a>(rows: &'a [Row], key: &str) -> Option<&'a str> {
        rows.iter().find_map(|r| match r {
            Row::Pair(k, v) if k == key => Some(v.as_str()),
            _ => None,
        })
    }

    fn first_colonist(sim: &Sim) -> ColonistId {
        sim.colonists().iter().next().map(|(id, _)| id).unwrap()
    }

    #[test]
    fn r011_the_life_rows_show_meters_tags_and_the_mood_why_chain() {
        let mut sim = sim_from_ascii("c..", 1).unwrap();
        let id = first_colonist(&sim);
        sim.debug_set_hunger(id, 0);
        let rows = colonist_rows(&sim, id);
        let meters = rows.iter().filter(|r| matches!(r, Row::Meter(_))).count();
        assert_eq!(meters, 4, "hunger, rest, mood, health");
        assert!(
            rows.iter()
                .any(|r| matches!(r, Row::Tag(_, t, _) if t == "Starving"))
        );
        assert!(
            rows.iter()
                .any(|r| matches!(r, Row::Section(t) if t.starts_with("Comfort")))
        );
        let lines: Vec<&str> = rows
            .iter()
            .filter_map(|r| match r {
                Row::Line(t) => Some(t.as_str()),
                _ => None,
            })
            .collect();
        for part in &sim.mood(id).unwrap().parts {
            let name = mood_cause_name(&sim, part.cause);
            assert!(lines.iter().any(|l| l.contains(&name)), "{name}");
        }
    }

    #[test]
    fn colonist_rows_list_position_job_carrying_skills_and_speed() {
        let sim = sim_from_ascii("c..", 1).unwrap();
        let id = first_colonist(&sim);
        let rows = colonist_rows(&sim, id);
        assert_eq!(rows[0], Row::Title("Colonist 0".to_owned()));
        assert_eq!(pair(&rows, "Position"), Some("(0, 0)"));
        assert_eq!(pair(&rows, "Job"), Some("idle"));
        assert_eq!(pair(&rows, "Carrying"), Some("nothing"));
        assert_eq!(pair(&rows, "Priorities"), Some("Cut 3  Mine 3"));
        for skill in Skill::ALL {
            assert_eq!(pair(&rows, skill.name()), Some("L0  0 xp"));
        }
        // A why chain for each job type: the total, then its named parts.
        assert_eq!(pair(&rows, "Cut"), Some("100%"));
        assert!(rows.contains(&Row::Line("  Base +100".to_owned())));
        assert!(rows.contains(&Row::Line("  Woodcutting level 0 +0".to_owned())));
    }

    #[test]
    fn r010_the_inspector_shows_level_four_as_120_percent_with_its_parts() {
        let mut sim = sim_from_ascii("cR", 1).unwrap();
        let id = first_colonist(&sim);
        sim.set_skill_xp(id, Skill::Mining, 4_000);
        let rows = colonist_rows(&sim, id);
        assert_eq!(pair(&rows, "Mining"), Some("L4  4000 xp"));
        assert_eq!(pair(&rows, "Mine"), Some("120%"));
        assert!(rows.contains(&Row::Line("  Mining level 4 +20".to_owned())));
        // The total is the sum of the part lines.
        let speed = sim.work_speed(id, JobKind::Mine).unwrap();
        assert_eq!(
            speed.parts.iter().map(|p| p.pct).sum::<i32>(),
            speed.total_pct
        );
    }

    #[test]
    fn a_missing_colonist_gives_no_rows() {
        let mut sim = sim_from_ascii("c", 1).unwrap();
        let id = first_colonist(&sim);
        let other = sim_from_ascii("cc", 1).unwrap();
        let second = other.colonists().iter().nth(1).map(|(i, _)| i).unwrap();
        assert!(colonist_rows(&sim, second).is_empty());
        sim.step(&[]);
        assert!(!colonist_rows(&sim, id).is_empty());
    }

    #[test]
    fn tile_rows_describe_terrain_and_a_designation() {
        let mut sim = sim_from_ascii("cTR", 1).unwrap();
        let rows = tile_rows(&sim, 1, 0, &[]);
        assert_eq!(rows[0], Row::Title("Tile (1, 0)".to_owned()));
        assert_eq!(pair(&rows, "Ground"), Some("Soil"));
        assert_eq!(pair(&rows, "Tree"), Some("yes"));
        assert_eq!(pair(&rows, "Rock"), Some("-"));
        assert_eq!(pair(&rows, "Designation"), Some("-"));
        let rock = tile_rows(&sim, 2, 0, &[]);
        assert_eq!(pair(&rock, "Rock"), Some("Granite"));
        assert_eq!(pair(&rock, "Walkable"), Some("no"));
        sim.step(&[Command::Designate {
            kind: JobKind::Cut,
            x: 1,
            y: 0,
        }]);
        let rows = tile_rows(&sim, 1, 0, &[]);
        let text = pair(&rows, "Designation").unwrap_or_default();
        assert!(text.starts_with("Cut"), "{text}");
    }

    #[test]
    fn tile_rows_list_items_and_the_stockpile() {
        let mut sim = sim_from_ascii("c....", 1).unwrap();
        let log = sim.rules().log;
        sim.debug_spawn_item(log, 4, 3, 0);
        sim.step(&[Command::CreateStockpile {
            x0: 2,
            y0: 0,
            x1: 4,
            y1: 0,
            filter: ItemFilter::All,
        }]);
        let all = crate::tools::haulable_materials(sim.rules());
        let rows = tile_rows(&sim, 3, 0, &all);
        assert!(rows.contains(&Row::Line("Log x4".to_owned())));
        assert_eq!(pair(&rows, "Stockpile"), Some("zone 0, 3x1"));
        assert_eq!(pair(&rows, "Accepts"), Some("everything"));
        let none = tile_rows(&sim, 0, 0, &all);
        assert_eq!(pair(&none, "Stockpile"), Some("-"));
        assert_eq!(pair(&none, "Items"), Some("-"));
    }

    fn index(sim: &Sim, key: &str) -> u16 {
        sim.rules().building_index(key).unwrap()
    }

    #[test]
    fn tile_rows_name_the_building_its_material_and_a_blueprint() {
        let mut sim = sim_from_ascii("....\n....", 1).unwrap();
        let granite = sim.rules().material_index("granite").unwrap();
        let wall = index(&sim, "wall");
        assert!(sim.debug_build(wall, 1, 0, granite));
        let rows = tile_rows(&sim, 1, 0, &[]);
        assert_eq!(pair(&rows, "Building"), Some("Granite wall"));
        sim.step(&[Command::PlaceBlueprint {
            building: wall,
            material: granite,
            x: 2,
            y: 0,
        }]);
        let rows = tile_rows(&sim, 2, 0, &[]);
        let text = pair(&rows, "Blueprint").unwrap_or_default();
        assert!(text.starts_with("Granite wall, 0/"), "{text}");
        assert!(text.ends_with("0%"), "{text}");
    }

    #[test]
    fn a_workshop_lists_its_recipes_and_orders() {
        let mut sim = sim_from_ascii("....", 1).unwrap();
        let workshop = index(&sim, "workshop");
        let granite = sim.rules().material_index("granite").unwrap();
        assert!(sim.debug_build(workshop, 1, 0, granite));
        assert_eq!(workshop_at(&sim, 1, 0), Some(workshop));
        assert_eq!(workshop_at(&sim, 0, 0), None);
        let recipes = workshop_recipes(&sim, 1, 0);
        assert_eq!(recipes.len(), 3);
        assert_eq!(recipes[0].label, "Make planks");
        assert_eq!(recipes[0].detail, "1 Log to 2 Plank, 2.0 h");
        sim.step(&[Command::OrderCraft {
            x: 1,
            y: 0,
            recipe: recipes[0].recipe,
            count: 3,
        }]);
        let recipes = workshop_recipes(&sim, 1, 0);
        assert_eq!(recipes[0].label, "Make planks (ordered 3)");
        let rows = tile_rows(&sim, 1, 0, &[]);
        assert!(rows.contains(&Row::Line("  Order: Make planks x3".to_owned())));
        assert_eq!(workshop_lines(&sim, 1, 0)[1], "Input bin: empty");
    }

    #[test]
    fn a_closed_roofed_room_shows_its_tiles_insulation_beauty_and_roof_support() {
        let mut sim = sim_from_ascii("......\n......\n......\n......\n......\n......", 1).unwrap();
        let granite = sim.rules().material_index("granite").unwrap();
        let (wall, roof) = (index(&sim, "wall"), index(&sim, "roof"));
        for y in 0..4u16 {
            for x in 0..4u16 {
                if x == 0 || y == 0 || x == 3 || y == 3 {
                    assert!(sim.debug_build(wall, x, y, granite));
                }
                assert!(sim.debug_build(roof, x, y, granite));
            }
        }
        sim.step(&[]);
        let mut rows = Vec::new();
        room_rows(&sim, 1, 1, &mut rows);
        assert_eq!(pair(&rows, "Tiles"), Some("4"));
        assert_eq!(pair(&rows, "Roof support"), Some("4 of 4 tiles"));
        let insulation = pair(&rows, "Insulation").unwrap_or_default();
        assert!(insulation.ends_with(" of 100"), "{insulation}");
        assert!(
            rows.iter()
                .any(|r| matches!(r, Row::Line(l) if l.contains("Granite") && l.contains('@'))),
            "the insulation parts must show: {rows:?}"
        );
        assert!(pair(&rows, "Beauty").is_some());
        let mut outside = Vec::new();
        room_rows(&sim, 5, 5, &mut outside);
        assert!(outside.is_empty());
    }

    #[test]
    fn building_rejects_have_their_own_text() {
        let text = job_event_text(&JobEvent::Rejected(sim::Reject::NoRoofSupport));
        assert_eq!(
            text.as_deref(),
            Some("Order refused: a roof needs a wall within 6 tiles")
        );
    }

    #[test]
    fn work_rows_have_one_row_per_colonist_with_its_priorities() {
        let mut sim = sim_from_ascii("cc", 1).unwrap();
        let id = first_colonist(&sim);
        sim.step(&[Command::SetPriority {
            colonist: id,
            job: JobKind::Mine,
            priority: 1,
        }]);
        let rows = work_rows(&sim);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].name, "Colonist 0");
        assert_eq!(rows[0].priorities, [3, 1, 3, 3, 3, 3, 3, 3, 3]);
        assert_eq!(rows[1].priorities, [3, 3, 3, 3, 3, 3, 3, 3, 3]);
    }

    #[test]
    fn filter_text_names_few_materials_and_counts_many() {
        let sim = sim_from_ascii("c", 1).unwrap();
        let all = crate::tools::haulable_materials(sim.rules());
        let log = sim.rules().log;
        assert_eq!(filter_text(&sim, &ItemFilter::All, &all), "everything");
        assert_eq!(filter_text(&sim, &ItemFilter::Only(vec![log]), &all), "Log");
        assert_eq!(
            filter_text(&sim, &ItemFilter::Only(vec![]), &all),
            "nothing"
        );
        assert_eq!(
            filter_text(&sim, &ItemFilter::Only(all[..4].to_vec()), &all),
            "4 of 7 materials"
        );
    }

    #[test]
    fn signed_percent_always_shows_a_sign() {
        assert_eq!(signed(5), "+5");
        assert_eq!(signed(0), "+0");
        assert_eq!(signed(-10), "-10");
    }
}
