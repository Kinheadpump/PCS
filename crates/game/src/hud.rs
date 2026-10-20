//! The heads-up display: top bar, toolbar, work panel, build panel, inspectors, the stockpile
//! filter panel, and the workshop panel. It reads `&Sim` and returns `UiAction` values. It never
//! changes the sim (AGENTS.md code rules).
//!
//! The display shows no help text. Short tooltips show on hover. The explanations go to the
//! tutorial (later sprint).
//!
//! `Content::new` makes the text of every panel. `Content::sizes` gives the panel sizes to
//! `layout::place`, which gives the rectangles. A test checks that no two panels overlap.

use crate::build_ui::{self, BuildState};
use crate::camera::View;
use crate::cells;
use crate::icons;
use crate::inspector::{self, RecipeInfo};
use crate::layout::{self, Placed, Size, Sizes};
use crate::palette::ui as pal;
use crate::render::color as rgb_color;
use crate::tools::{self, Tool};
use crate::ui::{self, Fold, Row, Ui, UiAction};
use crate::weather;
use macroquad::prelude::{Color, draw_rectangle};
use sim::{ColonistId, Command, ItemFilter, JobKind, Sim, Speed, StockpileId};
use std::collections::BTreeSet;

/// Widest row of chips in the filter panel, in pixels.
const FILTER_WIDTH: f32 = 340.0;
/// Widest row of chips in the build panel, in pixels.
const BUILD_WIDTH: f32 = 420.0;
/// Longest text in an inspector panel, in characters.
const CLIP_CHARS: usize = 30;
/// Fill of a priority cell: off, then 1 (first) to 4.
const PRIORITY_FILL: [Color; 5] = [
    Color::new(0.13, 0.14, 0.19, 1.0),
    Color::new(0.16, 0.50, 0.32, 1.0),
    Color::new(0.16, 0.40, 0.32, 1.0),
    Color::new(0.17, 0.30, 0.30, 1.0),
    Color::new(0.20, 0.24, 0.28, 1.0),
];
/// Speed buttons: speed, icon, tooltip.
const SPEED_BUTTONS: [(Speed, usize, &str); 4] = [
    (Speed::Paused, cells::icons::PAUSE, "Pause (Space)"),
    (Speed::X1, cells::icons::PLAY, "Speed 1x (Q)"),
    (Speed::X3, cells::icons::FAST, "Speed 3x (E)"),
    (Speed::X6, cells::icons::FASTEST, "Speed 6x (R)"),
];

/// What the player has opened or closed in the display. It lives in the app, not in the sim.
#[derive(Clone, Debug, Default)]
pub struct UiState {
    /// The folds that are open.
    pub open: BTreeSet<Fold>,
    /// True if the debug lines show (tick, hash, FPS, map).
    pub debug: bool,
}

impl UiState {
    /// Open the fold if it is closed, close it if it is open.
    pub fn toggle(&mut self, fold: Fold) {
        if !self.open.remove(&fold) {
            self.open.insert(fold);
        }
    }
}

/// What the display needs to know for one frame.
pub struct HudInput<'a> {
    /// The simulation (read only).
    pub sim: &'a Sim,
    /// The viewport size.
    pub view: View,
    /// The active tool.
    pub tool: Tool,
    /// The building and the material of the build tool.
    pub build: &'a BuildState,
    /// True if the work panel is open.
    pub show_work: bool,
    /// True if the rooms overlay is on.
    pub show_rooms: bool,
    /// True if the temperature overlay is on.
    pub show_temp: bool,
    /// The selected colonist.
    pub selected: Option<ColonistId>,
    /// The selected tile.
    pub selected_tile: Option<(u16, u16)>,
    /// The tile under the mouse, if the mouse is over the map. Only the debug lines show it.
    pub hover_tile: Option<(u16, u16)>,
    /// The game speed.
    pub speed: Speed,
    /// Frames per second. Only the debug lines show it.
    pub fps: i32,
    /// Zoom in screen pixels per tile. Only the debug lines show it.
    pub zoom: f32,
    /// Name of the map preset. Only the debug lines show it.
    pub map_name: &'a str,
    /// Text of the last event.
    pub last_event: &'a str,
    /// The folds and the debug switch.
    pub state: &'a UiState,
}

/// `text`, cut to `max` characters with ".." at the end if it is longer.
fn shorten(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_owned();
    }
    let head: String = text.chars().take(max.saturating_sub(2)).collect();
    format!("{head}..")
}

/// The tooltip of a toolbar button, for example `Cut (T)`.
pub fn tool_label(tool: Tool) -> String {
    format!("{} ({})", tool.name(), tool.key_label())
}

/// The tooltip of the work panel button.
pub const WORK_LABEL: &str = "Work (P)";
/// The tooltip of the rooms overlay button.
pub const ROOMS_LABEL: &str = "Rooms (O)";
/// Tooltip of the temperature overlay button.
pub const TEMP_LABEL: &str = "Temperature (Y)";
/// The number of toolbar buttons after the tools: work, rooms, temperature.
pub const PANEL_BUTTONS: usize = 3;
/// Color of the rain icon.
const RAIN_COLOR: [u8; 3] = [124, 184, 255];

/// The command for a click on a priority cell.
pub fn priority_command(id: ColonistId, kind: JobKind, current: u8, backward: bool) -> Command {
    Command::SetPriority {
        colonist: id,
        job: kind,
        priority: tools::next_priority(current, backward),
    }
}

/// The filter after a click on a button of the filter panel. `button` 0 is "All".
pub fn filter_after_click(filter: &ItemFilter, materials: &[u16], button: usize) -> ItemFilter {
    match button.checked_sub(1).and_then(|i| materials.get(i)) {
        Some(&material) => tools::toggle_material(filter, material, materials),
        None => ItemFilter::All,
    }
}

/// The command for a click on a button of a recipe row: `button` 0 to 2 order 1, 5, or 10
/// more runs, 3 clears the order.
pub fn recipe_command(x: u16, y: u16, recipe: u16, button: usize) -> Command {
    let order = |count| Command::OrderCraft {
        x,
        y,
        recipe,
        count,
    };
    match button {
        0 => order(1),
        1 => order(5),
        2 => order(10),
        _ => Command::CancelCraft { x, y, recipe },
    }
}

/// The rows of the colonist panel: name, job, walking target, and what the colonist carries.
/// The skills and the work speeds (why chains) are folds.
fn colonist_panel_rows(rows: Vec<Row>, state: &UiState) -> (Vec<Row>, Vec<Fold>) {
    let mut out = Vec::new();
    let mut folds = Vec::new();
    // `Some(open)` while the rows of a fold go by.
    let mut inside: Option<bool> = None;
    for row in rows {
        match row {
            Row::Section(name) => {
                let fold = match name.as_str() {
                    "Skills" => Some(Fold::Skills),
                    "Work speed" => Some(Fold::WorkSpeed),
                    _ => None,
                };
                match fold {
                    Some(f) => {
                        let open = state.open.contains(&f);
                        out.push(Row::Fold(name, open));
                        folds.push(f);
                        inside = Some(open);
                    }
                    None => {
                        out.push(Row::Section(name));
                        inside = None;
                    }
                }
            }
            Row::Pair(key, _)
                if inside.is_none()
                    && matches!(key.as_str(), "Position" | "Step" | "Priorities" | "") => {}
            other => {
                if inside != Some(false) {
                    out.push(other);
                }
            }
        }
    }
    (out, folds)
}

/// The rows of the tile panel: only the values that say something. The named parts of the room
/// values (why chains) are a fold.
fn tile_panel_rows(rows: Vec<Row>, state: &UiState) -> (Vec<Row>, Vec<Fold>) {
    let mut out = Vec::new();
    let mut parts = Vec::new();
    let mut in_room = false;
    // `Some(open)` while the lines of the temperature fold go by.
    let mut in_temp: Option<bool> = None;
    let mut folds = Vec::new();
    for row in rows {
        if in_temp.is_some() && !matches!(row, Row::Line(_)) {
            in_temp = None;
        }
        match row {
            Row::Line(_) if in_temp.is_some() => {
                if in_temp == Some(true) {
                    out.push(row);
                }
            }
            Row::Section(ref name) if name == inspector::TEMP_PARTS => {
                let open = state.open.contains(&Fold::TempChain);
                out.push(Row::Fold("Temperature parts".to_owned(), open));
                folds.push(Fold::TempChain);
                in_temp = Some(open);
                in_room = false;
            }
            Row::Pair(_, ref v) if v == "-" => {}
            Row::Section(ref name) if name == "Room" => {
                in_room = true;
                out.push(row);
            }
            Row::Section(_) => {
                in_room = false;
                out.push(row);
            }
            Row::Line(ref l) if in_room && l.starts_with("  ") => parts.push(row),
            other => out.push(other),
        }
    }
    if !parts.is_empty() {
        let open = state.open.contains(&Fold::Details);
        out.push(Row::Fold("Details".to_owned(), open));
        folds.push(Fold::Details);
        if open {
            out.extend(parts);
        }
    }
    (out, folds)
}

/// Cut long texts, and cut rows from the end so that the panel is not taller than `max_h`.
fn fit_rows(mut rows: Vec<Row>, max_h: f32) -> Vec<Row> {
    for row in &mut rows {
        match row {
            Row::Pair(k, v) => {
                *k = shorten(k, CLIP_CHARS);
                *v = shorten(v, CLIP_CHARS);
            }
            Row::Line(t) | Row::Section(t) | Row::Fold(t, _) | Row::Title(t) => {
                *t = shorten(t, CLIP_CHARS + 6);
            }
        }
    }
    let mut cut = false;
    while rows.len() > 2 && ui::rows_layout((0.0, 0.0), &rows, true).panel.h > max_h {
        rows.pop();
        cut = true;
    }
    if cut && let Some(last) = rows.last_mut() {
        *last = Row::Line("...".to_owned());
    }
    rows
}

/// Text of the build panel.
struct BuildPanel {
    title: String,
    buildings: Vec<String>,
    active_building: usize,
    materials: Vec<String>,
    material_ids: Vec<u16>,
    material_colors: Vec<Color>,
    active_material: Option<u16>,
    lines: Vec<String>,
}

impl BuildPanel {
    fn new(h: &HudInput) -> Self {
        let rules = h.sim.rules();
        let building = h.build.building();
        let name = |b: u16| rules.building(b).map_or("?", |d| d.name.as_str());
        let buildings = (0..rules.buildings.len())
            .map(|i| name(u16::try_from(i).unwrap_or(0)).to_owned())
            .collect();
        let material_ids = build_ui::fitting_materials(rules, building);
        let materials = material_ids
            .iter()
            .map(|&m| inspector::material_name(h.sim, m))
            .collect();
        let material_colors = material_ids
            .iter()
            .map(|&m| {
                rules
                    .materials
                    .get(usize::from(m))
                    .map_or(rgb_color(pal::DIM, 255), |d| rgb_color(d.color, 255))
            })
            .collect();
        let active_material = h.build.material(h.sim);
        let mut lines = Vec::new();
        if rules.building(building).is_some() {
            match active_material {
                Some(m) => {
                    lines.push(build_ui::cost_text(rules, building, m));
                    lines.push(format!(
                        "In stock: {} {}",
                        h.sim.items().total_units(m),
                        inspector::material_name(h.sim, m)
                    ));
                }
                None => lines.push("No material fits".to_owned()),
            }
        }
        Self {
            title: format!("Build: {}", name(building)),
            buildings,
            active_building: usize::from(building),
            materials,
            material_ids,
            material_colors,
            active_material,
            lines,
        }
    }

    fn layout(&self, origin: (f32, f32)) -> ui::BuildLayout {
        ui::build_layout(
            origin,
            &self.title,
            &self.buildings,
            &self.materials,
            &self.lines,
            BUILD_WIDTH,
        )
    }
}

/// Text of the workshop panel.
struct WorkshopPanel {
    tile: (u16, u16),
    title: String,
    lines: Vec<String>,
    recipes: Vec<RecipeInfo>,
}

impl WorkshopPanel {
    fn new(sim: &Sim, tile: (u16, u16)) -> Option<Self> {
        inspector::workshop_at(sim, tile.0, tile.1)?;
        let mut lines = inspector::workshop_lines(sim, tile.0, tile.1);
        let title = if lines.is_empty() {
            "Workshop".to_owned()
        } else {
            lines.remove(0)
        };
        for line in &mut lines {
            *line = shorten(line, CLIP_CHARS + 6);
        }
        let mut recipes = inspector::workshop_recipes(sim, tile.0, tile.1);
        for r in &mut recipes {
            r.label = shorten(&r.label, CLIP_CHARS + 6);
            r.detail = shorten(&r.detail, CLIP_CHARS + 6);
        }
        Some(Self {
            tile,
            title,
            lines,
            recipes,
        })
    }

    fn layout(&self, origin: (f32, f32)) -> ui::WorkshopLayout {
        let labels: Vec<(String, String)> = self
            .recipes
            .iter()
            .map(|r| (r.label.clone(), r.detail.clone()))
            .collect();
        ui::workshop_layout(origin, &self.title, &self.lines, &labels)
    }
}

/// Text of the stockpile filter panel.
struct FilterPanel {
    id: StockpileId,
    labels: Vec<String>,
    colors: Vec<Option<Color>>,
}

impl FilterPanel {
    fn layout(&self, origin: (f32, f32)) -> ui::GridLayout {
        ui::grid_layout(origin, "Stockpile", &self.labels, FILTER_WIDTH)
    }
}

/// The side panel near the tile panel.
enum Side {
    Filter(FilterPanel),
    Workshop(WorkshopPanel),
}

/// The text and data of every panel of one frame.
struct Content {
    clock: String,
    date: String,
    count: String,
    work_names: Vec<String>,
    build: Option<BuildPanel>,
    colonist: Vec<Row>,
    colonist_folds: Vec<Fold>,
    tile: Option<Vec<Row>>,
    tile_folds: Vec<Fold>,
    side: Option<Side>,
    materials: Vec<u16>,
}

impl Content {
    fn new(h: &HudInput) -> Self {
        let time = h.sim.time();
        let max_h = layout::panel_height(h.view, Tool::ALL.len() + PANEL_BUTTONS);
        let (colonist, colonist_folds) = match h.selected {
            Some(id) => {
                let (rows, folds) =
                    colonist_panel_rows(inspector::colonist_rows(h.sim, id), h.state);
                (fit_rows(rows, max_h), folds)
            }
            None => (Vec::new(), Vec::new()),
        };
        let materials = tools::haulable_materials(h.sim.rules());
        let (tile, tile_folds) = match h.selected_tile {
            Some((x, y)) => {
                let (rows, folds) =
                    tile_panel_rows(inspector::tile_rows(h.sim, x, y, &materials), h.state);
                (Some(fit_rows(rows, max_h)), folds)
            }
            None => (None, Vec::new()),
        };
        let side = h.selected_tile.and_then(|tile| {
            if let Some(panel) = WorkshopPanel::new(h.sim, tile) {
                return Some(Side::Workshop(panel));
            }
            let id = inspector::zone_at(h.sim, tile.0, tile.1)?;
            let mut labels = vec!["All".to_owned()];
            let mut colors = vec![None];
            for &m in &materials {
                labels.push(inspector::material_name(h.sim, m));
                colors.push(
                    h.sim
                        .rules()
                        .materials
                        .get(usize::from(m))
                        .map(|d| rgb_color(d.color, 255)),
                );
            }
            labels.push("Remove zone".to_owned());
            colors.push(None);
            Some(Side::Filter(FilterPanel { id, labels, colors }))
        });
        Self {
            clock: ui::clock_text(&time),
            date: ui::date_text(&time),
            count: h.sim.colonists().iter().count().to_string(),
            work_names: inspector::work_rows(h.sim)
                .iter()
                .map(|r| r.name.clone())
                .collect(),
            build: (h.tool == Tool::Build).then(|| BuildPanel::new(h)),
            colonist,
            colonist_folds,
            tile,
            tile_folds,
            side,
            materials,
        }
    }

    fn count_w(&self) -> f32 {
        ui::ICON_PX + 6.0 + ui::text_px(&self.count)
    }

    fn sizes(&self, h: &HudInput) -> Sizes {
        let size = |r: ui::Rect| Size::new(r.w, r.h);
        let origin = (0.0, 0.0);
        Sizes {
            clock_w: ui::text_px(&self.clock).max(ui::text_px(&self.date)),
            count_w: self.count_w(),
            toolbar_buttons: (Tool::ALL.len() + PANEL_BUTTONS, Tool::ALL.len()),
            work: h
                .show_work
                .then(|| size(ui::work_layout(origin, &self.work_names).panel)),
            build: self.build.as_ref().map(|b| size(b.layout(origin).panel)),
            colonist: (!self.colonist.is_empty())
                .then(|| size(ui::rows_layout(origin, &self.colonist, true).panel)),
            tile: self
                .tile
                .as_ref()
                .map(|rows| size(ui::rows_layout(origin, rows, true).panel)),
            side: self.side.as_ref().map(|s| match s {
                Side::Filter(f) => size(f.layout(origin).panel),
                Side::Workshop(w) => size(w.layout(origin).panel),
            }),
        }
    }
}

/// Where every panel goes for the frame, for tests and for `draw`.
fn plan(h: &HudInput) -> (Content, Placed) {
    let content = Content::new(h);
    let placed = layout::place(h.view, &content.sizes(h));
    (content, placed)
}

fn draw_top(ui: &mut Ui, placed: &Placed, content: &Content, h: &HudInput) -> Vec<UiAction> {
    ui.bar(placed.top);
    ui.label((placed.clock.0, placed.clock.1), &content.clock);
    ui.label_color(
        (placed.clock.0, placed.clock.1 + ui::LINE_H),
        &content.date,
        ui::dim(),
    );
    let mut actions = Vec::new();
    for (rect, (speed, icon, tip)) in placed.speeds.iter().zip(SPEED_BUTTONS) {
        if ui.icon_button(*rect, icon, h.speed == speed, tip) {
            actions.push(UiAction::SetSpeed(speed));
        }
    }
    let c = placed.count;
    ui.icon(
        cells::icons::PERSON,
        (c.x, c.y + (c.h - ui::ICON_PX) / 2.0),
        ui::ICON_PX,
        rgb_color(pal::KEY, 255),
    );
    ui.label(
        (c.x + ui::ICON_PX + 6.0, c.y + (c.h - 16.0) / 2.0),
        &content.count,
    );
    ui.tip(c, "Colonists");
    draw_temp(ui, placed, h);
    actions
}

/// The temperature readout of the top bar: a thermometer in the color of the temperature, the
/// value, and a drop if it rains today. The tooltip has the forecast.
fn draw_temp(ui: &mut Ui, placed: &Placed, h: &HudInput) {
    let r = placed.temp_slot;
    let outdoor = i32::from(h.sim.outdoor_temp());
    let icon_y = r.y + (r.h - ui::ICON_PX) / 2.0;
    ui.icon(
        cells::icons::THERMOMETER,
        (r.x, icon_y),
        ui::ICON_PX,
        rgb_color(weather::temp_color(outdoor), 255),
    );
    let text = weather::top_text(h.sim);
    let tx = r.x + ui::ICON_PX + 4.0;
    ui.label((tx, r.y + (r.h - 16.0) / 2.0), &text);
    if h.sim.weather_today().rain {
        ui.icon(
            cells::icons::RAIN,
            (tx + ui::text_px(&text) + 4.0, icon_y),
            ui::ICON_PX,
            rgb_color(RAIN_COLOR, 255),
        );
    }
    ui.tip(r, &weather::forecast_tip(h.sim));
    if h.show_temp {
        draw_scale(ui, placed.scale, outdoor);
    }
}

/// The scale of the temperature overlay: a gradient with the two end values and a tick at the
/// outdoor temperature.
fn draw_scale(ui: &Ui, r: ui::Rect, outdoor: i32) {
    let low = (weather::SCALE_MIN / 10).to_string();
    let high = (weather::SCALE_MAX / 10).to_string();
    let (lw, hw) = (ui::text_px(&low), ui::text_px(&high));
    let text_y = r.y + (r.h - 16.0) / 2.0;
    ui.label_color((r.x, text_y), &low, ui::dim());
    ui.label_color((r.right() - hw, text_y), &high, ui::dim());
    let bar = ui::Rect::new(
        r.x + lw + 8.0,
        (r.y + (r.h - 12.0) / 2.0).round(),
        r.w - lw - hw - 16.0,
        12.0,
    );
    let ink = rgb_color(pal::INK, 255);
    draw_rectangle(bar.x - 2.0, bar.y - 2.0, bar.w + 4.0, bar.h + 4.0, ink);
    let step: f32 = 2.0;
    let mut x = 0.0;
    while x < bar.w {
        let t = weather::SCALE_MIN
            + (weather::SCALE_MAX - weather::SCALE_MIN) * (x / bar.w * 1000.0) as i32 / 1000;
        let w = step.min(bar.w - x);
        draw_rectangle(
            bar.x + x,
            bar.y,
            w,
            bar.h,
            rgb_color(weather::temp_color(t), 255),
        );
        x += step;
    }
    let mark = (bar.x + bar.w * weather::scale_permille(outdoor) as f32 / 1000.0).round();
    draw_rectangle(mark - 2.0, bar.y - 4.0, 4.0, bar.h + 8.0, ink);
    draw_rectangle(
        mark - 1.0,
        bar.y - 3.0,
        2.0,
        bar.h + 6.0,
        rgb_color(pal::TEXT, 255),
    );
    ui.tip(r, "Outdoor temperature");
}

/// The alert strip under the top bar: one chip for each alert. A click jumps to the hearth.
fn draw_alerts(ui: &mut Ui, placed: &Placed, h: &HudInput) -> Vec<UiAction> {
    let chips = weather::alert_chips(h.sim);
    let gap = ui::GAP * 2.0;
    let widths: Vec<f32> = chips
        .iter()
        .map(|c| Ui::alert_chip_width(&c.text))
        .collect();
    let total = widths.iter().sum::<f32>() + gap * chips.len().saturating_sub(1) as f32;
    let slot = placed.alerts_slot;
    let mut x = (slot.x + (slot.w - total) / 2.0).round();
    let mut actions = Vec::new();
    for (chip, w) in chips.iter().zip(widths) {
        let r = ui::Rect::new(x, slot.y, w, slot.h);
        let tint = rgb_color(chip.tint, 255);
        if ui.alert_chip(r, chip.icon, tint, &chip.text)
            && let Some((tx, ty)) = chip.target
        {
            actions.push(UiAction::JumpTo(tx, ty));
        }
        x += w + gap;
    }
    actions
}

fn draw_toolbar(ui: &mut Ui, placed: &Placed, h: &HudInput) -> Vec<UiAction> {
    ui.panel(placed.toolbar);
    let mut actions = Vec::new();
    for (i, rect) in placed.toolbar_buttons.iter().enumerate() {
        match Tool::ALL.get(i) {
            Some(tool) => {
                if ui.icon_button(
                    *rect,
                    icons::cell_for(*tool),
                    h.tool == *tool,
                    &tool_label(*tool),
                ) {
                    actions.push(UiAction::SetTool(*tool));
                }
            }
            None if i == Tool::ALL.len() => {
                if ui.icon_button(*rect, cells::icons::WORK, h.show_work, WORK_LABEL) {
                    actions.push(UiAction::ToggleWork);
                }
            }
            None if i == Tool::ALL.len() + 1 => {
                if ui.icon_button(*rect, cells::icons::BRACKETS, h.show_rooms, ROOMS_LABEL) {
                    actions.push(UiAction::ToggleRooms);
                }
            }
            None => {
                if ui.icon_button(*rect, cells::icons::THERMOMETER, h.show_temp, TEMP_LABEL) {
                    actions.push(UiAction::ToggleTemp);
                }
            }
        }
    }
    actions
}

fn draw_toast(ui: &Ui, placed: &Placed, h: &HudInput) {
    let Some(r) = placed.toast else { return };
    if h.last_event.is_empty() {
        return;
    }
    let chars = ((r.w - 2.0 * ui::PAD) / (6.0 * ui::SCALE as f32)) as usize;
    let text = shorten(h.last_event, chars);
    ui.label_color((r.x + ui::PAD, r.y + (r.h - 16.0) / 2.0), &text, ui::dim());
}

fn draw_debug(ui: &Ui, placed: &Placed, h: &HudInput) {
    if !h.state.debug {
        return;
    }
    let hover = h
        .hover_tile
        .map_or_else(|| "-".to_owned(), |(x, y)| format!("({x}, {y})"));
    let lines = [
        format!("Tick {}  {}", h.sim.tick(), h.map_name),
        format!("Hash {:016x}", h.sim.state_hash()),
        format!("FPS {}  {:.1} px/tile  hover {hover}", h.fps, h.zoom),
    ];
    let mut y = placed.toolbar.y - ui::LINE_H * lines.len() as f32 - 4.0;
    if let Some(b) = placed.build {
        y = y.min(b.y - ui::LINE_H * lines.len() as f32 - 4.0);
    }
    for line in &lines {
        ui.label_color((layout::MARGIN, y), line, ui::dim());
        y += ui::LINE_H;
    }
}

fn draw_work(ui: &mut Ui, origin: (f32, f32), content: &Content, h: &HudInput) -> Vec<UiAction> {
    let rows = inspector::work_rows(h.sim);
    let layout = ui::work_layout(origin, &content.work_names);
    let mut actions = Vec::new();
    if ui.titled_panel(layout.panel, "Work priorities", Some(layout.close)) {
        actions.push(UiAction::ToggleWork);
    }
    for (kind, pos) in JobKind::ALL.iter().zip(&layout.headers) {
        ui.label_color(
            *pos,
            &kind.name().chars().take(3).collect::<String>(),
            ui::dim(),
        );
    }
    for (row_index, row) in rows.iter().enumerate() {
        if ui.button(
            layout.names[row_index],
            &row.name,
            h.selected == Some(row.id),
        ) {
            actions.push(UiAction::SelectColonist(row.id));
        }
        for kind in JobKind::ALL {
            let priority = row.priorities[kind.index()];
            let fill = PRIORITY_FILL[usize::from(priority.min(4))];
            let press = ui.tinted_button(
                layout.cells[row_index][kind.index()],
                tools::priority_text(priority),
                fill,
            );
            if press.left {
                actions.push(UiAction::Command(priority_command(
                    row.id, kind, priority, false,
                )));
            } else if press.right {
                actions.push(UiAction::Command(priority_command(
                    row.id, kind, priority, true,
                )));
            }
        }
    }
    actions
}

fn draw_build(ui: &mut Ui, origin: (f32, f32), panel: &BuildPanel) -> Vec<UiAction> {
    let layout = panel.layout(origin);
    let mut actions = Vec::new();
    if ui.titled_panel(layout.panel, &panel.title, Some(layout.close)) {
        actions.push(UiAction::SetTool(Tool::Select));
    }
    for (i, (rect, label)) in layout.buildings.iter().zip(&panel.buildings).enumerate() {
        if ui.chip(*rect, label, None, i == panel.active_building) {
            actions.push(UiAction::PickBuilding(i));
        }
    }
    ui.rule(
        layout.panel.x + ui::PAD,
        layout.rule_y,
        layout.panel.w - 2.0 * ui::PAD,
    );
    for (((rect, label), id), color) in layout
        .materials
        .iter()
        .zip(&panel.materials)
        .zip(&panel.material_ids)
        .zip(&panel.material_colors)
    {
        if ui.chip(
            *rect,
            label,
            Some(*color),
            panel.active_material == Some(*id),
        ) {
            actions.push(UiAction::PickMaterial(*id));
        }
    }
    for (i, (pos, line)) in layout.lines.iter().zip(&panel.lines).enumerate() {
        if i == 0 {
            ui.label(*pos, line);
        } else {
            ui.label_color(*pos, line, ui::dim());
        }
    }
    actions
}

fn draw_workshop(ui: &mut Ui, origin: (f32, f32), panel: &WorkshopPanel) -> Vec<UiAction> {
    let layout = panel.layout(origin);
    let mut actions = Vec::new();
    if ui.titled_panel(layout.panel, &panel.title, Some(layout.close)) {
        actions.push(UiAction::CloseTile);
    }
    for (pos, line) in layout.lines.iter().zip(&panel.lines) {
        ui.label_color(*pos, line, ui::dim());
    }
    for (row, recipe) in layout.recipes.iter().zip(&panel.recipes) {
        ui.rule(
            layout.panel.x + ui::PAD,
            row.rule_y,
            layout.panel.w - 2.0 * ui::PAD,
        );
        ui.label(row.label, &recipe.label);
        ui.label_color(row.detail, &recipe.detail, ui::dim());
        for (i, (rect, text)) in row.buttons.iter().zip(ui::RECIPE_BUTTONS).enumerate() {
            if ui.button(*rect, text, false) {
                actions.push(UiAction::Command(recipe_command(
                    panel.tile.0,
                    panel.tile.1,
                    recipe.recipe,
                    i,
                )));
            }
        }
    }
    actions
}

fn draw_filter(
    ui: &mut Ui,
    origin: (f32, f32),
    sim: &Sim,
    panel: &FilterPanel,
    materials: &[u16],
) -> Vec<UiAction> {
    let Some(zone) = sim.stockpiles().get(panel.id) else {
        return Vec::new();
    };
    let id = panel.id;
    let layout = panel.layout(origin);
    let mut actions = Vec::new();
    if ui.titled_panel(layout.panel, "Stockpile", Some(layout.close)) {
        actions.push(UiAction::CloseTile);
    }
    let last = panel.labels.len() - 1;
    for (i, (rect, label)) in layout.buttons.iter().zip(&panel.labels).enumerate() {
        let active = match i {
            0 => zone.filter == ItemFilter::All,
            n if n == last => false,
            n => materials
                .get(n - 1)
                .is_some_and(|&m| zone.filter.accepts(m)),
        };
        let swatch = panel.colors.get(i).copied().flatten();
        if ui.chip(*rect, label, swatch, active) {
            actions.push(UiAction::Command(if i == last {
                Command::RemoveStockpile { id }
            } else {
                Command::SetStockpileFilter {
                    id,
                    filter: filter_after_click(&zone.filter, materials, i),
                }
            }));
        }
    }
    actions
}

/// Draw an inspector panel and turn its clicks into actions. `folds` lists the fold of each
/// fold row, in order. `close` is the action of the close button.
fn draw_inspector(
    ui: &mut Ui,
    origin: (f32, f32),
    rows: &[Row],
    folds: &[Fold],
    close: UiAction,
) -> Vec<UiAction> {
    let (_, hit) = ui::rows_panel(ui, origin, rows, true);
    let mut actions = Vec::new();
    if let Some(fold) = hit.fold.and_then(|i| folds.get(i)) {
        actions.push(UiAction::ToggleFold(*fold));
    }
    if hit.closed {
        actions.push(close);
    }
    actions
}

/// Draw the whole display. Returns what the player asked for.
pub fn draw(ui: &mut Ui, h: &HudInput) -> Vec<UiAction> {
    let (content, placed) = plan(h);
    let mut actions = Vec::new();
    actions.extend(draw_top(ui, &placed, &content, h));
    actions.extend(draw_toolbar(ui, &placed, h));
    actions.extend(draw_alerts(ui, &placed, h));
    draw_toast(ui, &placed, h);
    draw_debug(ui, &placed, h);
    if let (Some(r), Some(panel)) = (placed.build, &content.build) {
        actions.extend(draw_build(ui, (r.x, r.y), panel));
    }
    if let Some(r) = placed.work {
        actions.extend(draw_work(ui, (r.x, r.y), &content, h));
    }
    if let Some(r) = placed.colonist {
        actions.extend(draw_inspector(
            ui,
            (r.x, r.y),
            &content.colonist,
            &content.colonist_folds,
            UiAction::CloseColonist,
        ));
    }
    if let (Some(r), Some(rows)) = (placed.tile, &content.tile) {
        actions.extend(draw_inspector(
            ui,
            (r.x, r.y),
            rows,
            &content.tile_folds,
            UiAction::CloseTile,
        ));
    }
    if let (Some(r), Some(side)) = (placed.side, &content.side) {
        match side {
            Side::Filter(f) => {
                actions.extend(draw_filter(ui, (r.x, r.y), h.sim, f, &content.materials));
            }
            Side::Workshop(w) => actions.extend(draw_workshop(ui, (r.x, r.y), w)),
        }
    }
    ui.finish(h.view);
    actions
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inspector::WorkRow;
    use crate::layout::MARGIN;
    use sim::scenario::sim_from_ascii;

    /// A work panel row as text, for tests: name and the priority of each job type.
    pub fn work_text(row: &WorkRow) -> String {
        format!(
            "{} {}",
            row.name,
            JobKind::ALL
                .iter()
                .map(|k| tools::priority_text(row.priorities[k.index()]))
                .collect::<Vec<_>>()
                .join(" ")
        )
    }

    #[test]
    fn tooltips_show_the_key_of_each_tool() {
        assert_eq!(tool_label(Tool::Cut), "Cut (T)");
        assert_eq!(tool_label(Tool::Stockpile), "Zone (Z)");
        assert_eq!(tool_label(Tool::Build), "Build (B)");
        assert_eq!(tool_label(Tool::Deconstruct), "Remove (X)");
        assert_eq!(WORK_LABEL, "Work (P)");
        assert_eq!(ROOMS_LABEL, "Rooms (O)");
        assert_eq!(TEMP_LABEL, "Temperature (Y)");
    }

    #[test]
    fn every_speed_has_a_button() {
        let speeds: Vec<Speed> = SPEED_BUTTONS.iter().map(|b| b.0).collect();
        assert_eq!(speeds, vec![Speed::Paused, Speed::X1, Speed::X3, Speed::X6]);
    }

    #[test]
    fn a_priority_click_sends_the_next_value_and_a_right_click_the_previous() {
        let sim = sim_from_ascii("c", 1).unwrap();
        let (id, _) = sim.colonists().iter().next().unwrap();
        assert_eq!(
            priority_command(id, JobKind::Mine, 3, false),
            Command::SetPriority {
                colonist: id,
                job: JobKind::Mine,
                priority: 4
            }
        );
        assert_eq!(
            priority_command(id, JobKind::Cut, 1, true),
            Command::SetPriority {
                colonist: id,
                job: JobKind::Cut,
                priority: 0
            }
        );
    }

    #[test]
    fn recipe_buttons_order_1_5_10_runs_or_clear() {
        let order = |count| Command::OrderCraft {
            x: 3,
            y: 4,
            recipe: 1,
            count,
        };
        assert_eq!(recipe_command(3, 4, 1, 0), order(1));
        assert_eq!(recipe_command(3, 4, 1, 1), order(5));
        assert_eq!(recipe_command(3, 4, 1, 2), order(10));
        assert_eq!(
            recipe_command(3, 4, 1, 3),
            Command::CancelCraft {
                x: 3,
                y: 4,
                recipe: 1
            }
        );
    }

    #[test]
    fn filter_buttons_toggle_one_material_and_all_resets() {
        let materials = [3, 5, 8];
        let off = filter_after_click(&ItemFilter::All, &materials, 2);
        assert_eq!(off, ItemFilter::Only(vec![3, 8]));
        assert_eq!(filter_after_click(&off, &materials, 0), ItemFilter::All);
        assert_eq!(filter_after_click(&off, &materials, 2), ItemFilter::All);
    }

    #[test]
    fn work_text_lists_the_priorities_in_job_order() {
        let sim = sim_from_ascii("c", 1).unwrap();
        let rows = inspector::work_rows(&sim);
        assert_eq!(work_text(&rows[0]), "Colonist 0 3 3 3 3 3 3");
    }

    #[test]
    fn shorten_cuts_long_text_and_keeps_short_text() {
        assert_eq!(shorten("abc", 5), "abc");
        assert_eq!(shorten("abcdef", 5), "abc..");
    }

    #[test]
    fn ui_state_toggles_a_fold() {
        let mut state = UiState::default();
        state.toggle(Fold::Skills);
        assert!(state.open.contains(&Fold::Skills));
        state.toggle(Fold::Skills);
        assert!(state.open.is_empty());
    }

    /// A colony with seven colonists, a roofed room, and a workshop.
    fn busy_sim() -> Sim {
        let mut sim = sim_from_ascii(
            "ccccccc.........\n................\n................\n................\n................\n................\n................",
            1,
        )
        .unwrap();
        let rules = sim.rules();
        let wall = rules.building_index("wall").unwrap();
        let roof = rules.building_index("roof").unwrap();
        let floor = rules.building_index("floor").unwrap();
        let workshop = rules.building_index("workshop").unwrap();
        let granite = rules.material_index("granite").unwrap();
        let log = rules.log;
        for y in 2..6u16 {
            for x in 4..8u16 {
                if x == 4 || y == 2 || x == 7 || y == 5 {
                    assert!(sim.debug_build(wall, x, y, granite));
                }
                assert!(sim.debug_build(roof, x, y, log));
            }
        }
        assert!(sim.debug_build(floor, 5, 3, log));
        assert!(sim.debug_build(workshop, 6, 4, granite));
        sim.step(&[]);
        sim.step(&[]);
        sim
    }

    fn input<'a>(
        sim: &'a Sim,
        build: &'a BuildState,
        state: &'a UiState,
        view: View,
        tool: Tool,
        tile: Option<(u16, u16)>,
    ) -> HudInput<'a> {
        HudInput {
            sim,
            view,
            tool,
            build,
            show_work: true,
            show_rooms: true,
            show_temp: false,
            selected: sim.colonists().iter().next().map(|(id, _)| id),
            selected_tile: tile,
            hover_tile: None,
            speed: Speed::X1,
            fps: 60,
            zoom: 16.0,
            map_name: "temperate forest",
            last_event: "Colonist 0 stopped Cut: no path",
            state,
        }
    }

    fn all_folds_open() -> UiState {
        UiState {
            open: [
                Fold::Skills,
                Fold::WorkSpeed,
                Fold::Details,
                Fold::TempChain,
            ]
            .into_iter()
            .collect(),
            debug: false,
        }
    }

    const VIEWS: [(f32, f32); 4] = [
        (1280.0, 720.0),
        (1920.0, 1080.0),
        (1024.0, 768.0),
        (800.0, 600.0),
    ];

    #[test]
    fn panels_do_not_overlap_and_stay_inside_the_window() {
        let sim = busy_sim();
        let build = BuildState::new(sim.rules());
        for state in [UiState::default(), all_folds_open()] {
            for view in VIEWS {
                for tool in [Tool::Select, Tool::Build, Tool::Deconstruct] {
                    // A room tile, a workshop tile, an empty tile, nothing.
                    for tile in [Some((5, 3)), Some((6, 4)), Some((0, 0)), None] {
                        let h = input(&sim, &build, &state, view, tool, tile);
                        let (_, placed) = plan(&h);
                        assert_eq!(
                            layout::first_overlap(&placed),
                            None,
                            "view {view:?}, tool {tool:?}, tile {tile:?}"
                        );
                        for (name, r) in placed.all() {
                            assert!(r.x >= 0.0 && r.y >= 0.0, "{name} starts outside");
                            assert!(
                                r.right() <= view.0 && r.bottom() <= view.1,
                                "{name} {r:?} ends outside {view:?}"
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn the_important_panels_show_at_1280_and_1920() {
        let sim = busy_sim();
        let build = BuildState::new(sim.rules());
        let state = UiState::default();
        for view in [(1280.0, 720.0), (1920.0, 1080.0)] {
            // The build tool shows the build panel and the colonist panel (work panel closed).
            let mut h = input(&sim, &build, &state, view, Tool::Build, None);
            h.show_work = false;
            let (_, placed) = plan(&h);
            assert!(placed.build.is_some(), "the build panel must show");
            assert!(placed.colonist.is_some(), "the colonist panel must show");
            // The select tool on a workshop shows the tile panel and the workshop panel.
            let mut h = input(&sim, &build, &state, view, Tool::Select, Some((6, 4)));
            h.show_work = false;
            let (_, placed) = plan(&h);
            assert!(placed.colonist.is_some());
            assert!(placed.tile.is_some());
            assert!(placed.side.is_some(), "the workshop panel must show");
        }
    }

    #[test]
    fn the_work_panel_shows_at_1280_without_the_right_panels() {
        let sim = busy_sim();
        let build = BuildState::new(sim.rules());
        let state = UiState::default();
        let mut h = input(&sim, &build, &state, (1280.0, 720.0), Tool::Select, None);
        h.selected = None;
        let (_, placed) = plan(&h);
        assert!(placed.work.is_some());
    }

    #[test]
    fn the_reserved_slots_stay_free() {
        let sim = busy_sim();
        let build = BuildState::new(sim.rules());
        let state = all_folds_open();
        for view in VIEWS {
            let h = input(&sim, &build, &state, view, Tool::Build, Some((5, 3)));
            let (_, placed) = plan(&h);
            for (name, r) in placed.all() {
                if name != "top" {
                    assert!(!r.intersects(&placed.alerts_slot), "{name} covers alerts");
                }
            }
            if view.0 >= 1024.0 {
                for s in &placed.speeds {
                    assert!(!s.intersects(&placed.temp_slot));
                }
                assert!(!placed.count.intersects(&placed.temp_slot));
            }
        }
    }

    #[test]
    fn the_toolbar_has_a_button_for_every_tool_and_the_two_panel_buttons() {
        let sim = busy_sim();
        let build = BuildState::new(sim.rules());
        let state = UiState::default();
        for view in VIEWS {
            let h = input(&sim, &build, &state, view, Tool::Select, None);
            let (_, placed) = plan(&h);
            assert_eq!(
                placed.toolbar_buttons.len(),
                Tool::ALL.len() + PANEL_BUTTONS
            );
            assert_eq!(placed.speeds.len(), 4);
            for b in &placed.toolbar_buttons {
                assert!(b.right() <= view.0 && b.bottom() <= view.1);
            }
        }
    }

    #[test]
    fn the_colonist_panel_hides_the_details_until_a_fold_opens() {
        let sim = busy_sim();
        let (id, _) = sim.colonists().iter().next().unwrap();
        let rows = inspector::colonist_rows(&sim, id);
        let closed = UiState::default();
        let (short, folds) = colonist_panel_rows(rows.clone(), &closed);
        assert_eq!(folds, vec![Fold::Skills, Fold::WorkSpeed]);
        assert!(
            !short
                .iter()
                .any(|r| matches!(r, Row::Pair(k, _) if k == "Position" || k == "Step"))
        );
        assert!(!short.iter().any(|r| matches!(r, Row::Line(_))));
        assert!(
            short
                .iter()
                .any(|r| matches!(r, Row::Pair(k, _) if k == "Job"))
        );
        let (long, _) = colonist_panel_rows(rows, &all_folds_open());
        assert!(long.len() > short.len() + 9);
        assert!(long.iter().any(|r| matches!(r, Row::Line(_))));
    }

    #[test]
    fn the_tile_panel_drops_empty_values_and_folds_the_room_details() {
        let sim = busy_sim();
        let materials = tools::haulable_materials(sim.rules());
        let rows = inspector::tile_rows(&sim, 5, 3, &materials);
        let (short, folds) = tile_panel_rows(rows.clone(), &UiState::default());
        assert!(
            !short
                .iter()
                .any(|r| matches!(r, Row::Pair(_, v) if v == "-"))
        );
        assert_eq!(folds, vec![Fold::TempChain, Fold::Details]);
        assert!(
            short
                .iter()
                .any(|r| matches!(r, Row::Pair(k, _) if k == "Temperature"))
        );
        assert!(
            !short
                .iter()
                .any(|r| matches!(r, Row::Line(l) if l.starts_with("  ")))
        );
        assert!(
            short
                .iter()
                .any(|r| matches!(r, Row::Pair(k, _) if k == "Insulation"))
        );
        let (long, _) = tile_panel_rows(rows, &all_folds_open());
        assert!(long.len() > short.len());
    }

    #[test]
    fn long_inspector_panels_are_cut_to_the_free_height() {
        let sim = busy_sim();
        let (id, _) = sim.colonists().iter().next().unwrap();
        let (rows, _) = colonist_panel_rows(inspector::colonist_rows(&sim, id), &all_folds_open());
        let fitted = fit_rows(rows, 300.0);
        assert!(ui::rows_layout((0.0, 0.0), &fitted, true).panel.h <= 300.0);
        assert_eq!(fitted.last(), Some(&Row::Line("...".to_owned())));
    }

    #[test]
    fn the_build_panel_has_no_hint_lines() {
        let sim = busy_sim();
        let build = BuildState::new(sim.rules());
        let state = UiState::default();
        let h = input(&sim, &build, &state, (1280.0, 720.0), Tool::Build, None);
        let panel = BuildPanel::new(&h);
        assert!(panel.lines.len() <= 2);
        assert!(panel.buildings.iter().all(|b| !b.contains('(')));
    }

    #[test]
    fn the_top_bar_keeps_its_margin_to_the_window_edge() {
        let sim = busy_sim();
        let build = BuildState::new(sim.rules());
        let state = UiState::default();
        let h = input(&sim, &build, &state, (1280.0, 720.0), Tool::Select, None);
        let (_, placed) = plan(&h);
        assert!(placed.count.right() <= 1280.0 - MARGIN);
        assert!(placed.clock.0 >= MARGIN);
    }
}
