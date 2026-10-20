//! The heads-up display: status panel, work panel, toolbar, help, build panel, inspectors,
//! the stockpile filter panel, and the workshop panel. It reads `&Sim` and returns `UiAction`
//! values. It never changes the sim (AGENTS.md code rules).
//!
//! `Content::new` makes the text of every panel. `Content::sizes` gives the panel sizes to
//! `layout::place`, which gives the rectangles. A test checks that no two panels overlap.

use crate::build_ui::{self, BuildState};
use crate::camera::View;
use crate::cells;
use crate::clock::{SPEEDS, speed_label};
use crate::icons;
use crate::inspector::{self, RecipeInfo};
use crate::layout::{self, Placed, Size, Sizes};
use crate::tools::{self, Tool};
use crate::ui::{self, Row, Ui, UiAction};
use macroquad::prelude::Color;
use sim::{ColonistId, Command, ItemFilter, JobKind, Sim, Speed, StockpileId};

/// Widest row of buttons in the filter panel, in pixels.
const FILTER_WIDTH: f32 = 340.0;
/// Widest row of buttons in the build panel, in pixels.
const BUILD_WIDTH: f32 = 400.0;
/// Fill of a priority cell: off, then 1 (first) to 4.
const PRIORITY_FILL: [Color; 5] = [
    Color::new(0.13, 0.14, 0.17, 1.0),
    Color::new(0.16, 0.50, 0.32, 1.0),
    Color::new(0.16, 0.40, 0.32, 1.0),
    Color::new(0.17, 0.30, 0.30, 1.0),
    Color::new(0.20, 0.24, 0.28, 1.0),
];
/// Text of the hint line of the work panel.
const WORK_HINT: &str = "Click: next   Right click: back";

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
    /// The selected colonist.
    pub selected: Option<ColonistId>,
    /// The selected tile.
    pub selected_tile: Option<(u16, u16)>,
    /// The tile under the mouse, if the mouse is over the map.
    pub hover_tile: Option<(u16, u16)>,
    /// The game speed.
    pub speed: Speed,
    /// Frames per second.
    pub fps: i32,
    /// Zoom in screen pixels per tile.
    pub zoom: f32,
    /// Name of the map preset.
    pub map_name: &'a str,
    /// Text of the last event.
    pub last_event: &'a str,
}

/// Longest line of the status panel, in characters. It keeps the status panel narrow, so that
/// the tile panel fits beside it in a 1280 pixel window.
const STATUS_CHARS: usize = 35;

/// `text`, cut to `max` characters with ".." at the end if it is longer.
fn shorten(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_owned();
    }
    let head: String = text.chars().take(max.saturating_sub(2)).collect();
    format!("{head}..")
}

/// The lines of the help panel. The lines are short, so that the help panel stays left of the
/// colonist panel in a 1280 pixel window.
pub fn help_lines() -> Vec<String> {
    vec![
        "WASD/arrows, middle drag: pan   Wheel, +/-: zoom".to_owned(),
        "G: grid   Esc: quit   Space: pause".to_owned(),
        "Q/E/R: 1x/3x/6x speed   F1-F3: map".to_owned(),
        "Tools: V select  T cut  M mine  H haul".to_owned(),
        "Z zone  C cancel  B build  X remove".to_owned(),
        "P: work panel   O: rooms overlay".to_owned(),
        "1-7: choose the building (Build tool)".to_owned(),
        "Drag a rectangle on the map. Right click: cancel".to_owned(),
        "Select: click picks, right click moves (debug)".to_owned(),
    ]
}

/// The label of a toolbar button, for example `Cut (T)`.
pub fn tool_label(tool: Tool) -> String {
    format!("{} ({})", tool.name(), tool.key_label())
}

/// The label of the work panel button.
pub const WORK_LABEL: &str = "Work (P)";
/// The label of the rooms overlay button.
pub const ROOMS_LABEL: &str = "Rooms (O)";

/// The hint line of a tool.
pub fn tool_hint(tool: Tool) -> &'static str {
    match tool {
        Tool::Select => "Select: click a colonist or a tile",
        Tool::Cut => "Cut: drag over trees",
        Tool::Mine => "Mine: drag over rock",
        Tool::Haul => "Haul: drag over items on the ground",
        Tool::Stockpile => "Zone: drag to make a stockpile zone",
        Tool::Cancel => "Cancel: drag over orders",
        Tool::Build => "Build: pick, then drag on the map",
        Tool::Deconstruct => "Remove: drag over buildings",
    }
}

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

/// Text of the build panel.
struct BuildPanel {
    title: String,
    buildings: Vec<String>,
    active_building: usize,
    materials: Vec<String>,
    material_ids: Vec<u16>,
    active_material: Option<u16>,
    lines: Vec<String>,
}

impl BuildPanel {
    fn new(h: &HudInput) -> Self {
        let rules = h.sim.rules();
        let building = h.build.building();
        let name = |b: u16| rules.building(b).map_or("?", |d| d.name.as_str());
        let buildings = (0..rules.buildings.len())
            .map(|i| format!("{} ({})", name(u16::try_from(i).unwrap_or(0)), i + 1))
            .collect();
        let material_ids = build_ui::fitting_materials(rules, building);
        let materials = material_ids
            .iter()
            .map(|&m| inspector::material_name(h.sim, m))
            .collect();
        let active_material = h.build.material(h.sim);
        let mut lines = Vec::new();
        if let Some(def) = rules.building(building) {
            let hint = build_ui::shape_hint(build_ui::shape_of(def.kind));
            match active_material {
                Some(m) => {
                    lines.push(format!(
                        "{hint}: {}",
                        build_ui::cost_text(rules, building, m)
                    ));
                    lines.push(format!(
                        "In stock: {} {}",
                        h.sim.items().total_units(m),
                        inspector::material_name(h.sim, m)
                    ));
                }
                None => lines.push(format!("{hint}. No material fits")),
            }
            if def.kind == base::content::BuildingKind::Roof {
                lines.push("A roof needs a wall within 6 tiles".to_owned());
            }
        }
        Self {
            title: format!("Build: {}", name(building)),
            buildings,
            active_building: usize::from(building),
            materials,
            material_ids,
            active_material,
            lines,
        }
    }

    fn layout(&self, origin: (f32, f32)) -> ui::BuildLayout {
        ui::build_layout(
            origin,
            &self.title,
            &self.buildings,
            "Material",
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
        Some(Self {
            tile,
            title,
            lines,
            recipes: inspector::workshop_recipes(sim, tile.0, tile.1),
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
}

impl FilterPanel {
    fn layout(&self, origin: (f32, f32)) -> ui::GridLayout {
        ui::grid_layout(origin, "Stockpile accepts", &self.labels, FILTER_WIDTH)
    }
}

/// The side panel under the tile panel.
enum Side {
    Filter(FilterPanel),
    Workshop(WorkshopPanel),
}

/// The text and data of every panel of one frame.
struct Content {
    status_before: Vec<String>,
    status_after: Vec<String>,
    help: Vec<String>,
    toolbar_labels: Vec<String>,
    work_names: Vec<String>,
    build: Option<BuildPanel>,
    colonist: Vec<Row>,
    tile: Option<Vec<Row>>,
    side: Option<Side>,
    materials: Vec<u16>,
}

impl Content {
    fn new(h: &HudInput) -> Self {
        let status_before = vec![
            ui::format_time(&h.sim.time()),
            format!("Tick {}   {}", h.sim.tick(), h.map_name),
            format!("State hash {:016x}", h.sim.state_hash()),
        ];
        let mut status_after = vec![
            format!("FPS {}  {:.1} px/tile", h.fps, h.zoom),
            tool_hint(h.tool).to_owned(),
        ];
        if !h.last_event.is_empty() {
            status_after.push(shorten(h.last_event, STATUS_CHARS));
        }
        let mut toolbar_labels: Vec<String> = Tool::ALL.iter().map(|t| tool_label(*t)).collect();
        toolbar_labels.push(WORK_LABEL.to_owned());
        toolbar_labels.push(ROOMS_LABEL.to_owned());
        let colonist = h
            .selected
            .map(|id| inspector::colonist_rows(h.sim, id))
            .unwrap_or_default();
        let materials = tools::haulable_materials(h.sim.rules());
        let tile = h
            .selected_tile
            .or(h.hover_tile)
            .map(|(x, y)| inspector::tile_rows(h.sim, x, y, &materials));
        let side = h.selected_tile.and_then(|tile| {
            if let Some(panel) = WorkshopPanel::new(h.sim, tile) {
                return Some(Side::Workshop(panel));
            }
            let id = inspector::zone_at(h.sim, tile.0, tile.1)?;
            let mut labels = vec!["All".to_owned()];
            labels.extend(
                materials
                    .iter()
                    .map(|&m| inspector::material_name(h.sim, m)),
            );
            labels.push("Remove zone".to_owned());
            Some(Side::Filter(FilterPanel { id, labels }))
        });
        Self {
            status_before,
            status_after,
            help: help_lines(),
            toolbar_labels,
            work_names: inspector::work_rows(h.sim)
                .iter()
                .map(|r| r.name.clone())
                .collect(),
            build: (h.tool == Tool::Build).then(|| BuildPanel::new(h)),
            colonist,
            tile,
            side,
            materials,
        }
    }

    fn toolbar_refs(&self) -> Vec<&str> {
        self.toolbar_labels.iter().map(String::as_str).collect()
    }

    fn sizes(&self, h: &HudInput) -> Sizes {
        let size = |r: ui::Rect| Size::new(r.w, r.h);
        let origin = (0.0, 0.0);
        let labels: Vec<&str> = SPEEDS.iter().map(|s| speed_label(*s)).collect();
        Sizes {
            status: size(
                ui::hud_layout(origin, &self.status_before, &labels, &self.status_after).panel,
            ),
            toolbar: size(ui::column_layout(origin, &self.toolbar_refs()).panel),
            help: size(ui::hud_layout(origin, &self.help, &[], &[]).panel),
            work: h
                .show_work
                .then(|| size(ui::work_layout(origin, &self.work_names, WORK_HINT).panel)),
            build: self.build.as_ref().map(|b| size(b.layout(origin).panel)),
            colonist: (!self.colonist.is_empty())
                .then(|| size(ui::rows_layout(origin, &self.colonist).panel)),
            tile: self
                .tile
                .as_ref()
                .map(|rows| size(ui::rows_layout(origin, rows).panel)),
            tile_is_hover: h.selected_tile.is_none(),
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

fn draw_work(ui: &mut Ui, origin: (f32, f32), content: &Content, h: &HudInput) -> Vec<UiAction> {
    let rows = inspector::work_rows(h.sim);
    let layout = ui::work_layout(origin, &content.work_names, WORK_HINT);
    ui.panel(layout.panel);
    ui.label_color(layout.title, "Work priorities", ui::accent());
    ui.label_color(layout.hint, WORK_HINT, ui::dim());
    for (kind, pos) in JobKind::ALL.iter().zip(&layout.headers) {
        ui.label(*pos, &kind.name().chars().take(3).collect::<String>());
    }
    let mut actions = Vec::new();
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

fn draw_toolbar(ui: &mut Ui, origin: (f32, f32), content: &Content, h: &HudInput) -> Vec<UiAction> {
    let layout = ui::column_layout(origin, &content.toolbar_refs());
    ui.panel(layout.panel);
    let mut actions = Vec::new();
    for (i, rect) in layout.buttons.iter().enumerate() {
        let label = &content.toolbar_labels[i];
        match Tool::ALL.get(i) {
            Some(tool) => {
                if ui.icon_button(*rect, icons::cell_for(*tool), label, h.tool == *tool) {
                    actions.push(UiAction::SetTool(*tool));
                }
            }
            None if i == Tool::ALL.len() => {
                if ui.icon_button(*rect, cells::icons::WORK, label, h.show_work) {
                    actions.push(UiAction::ToggleWork);
                }
            }
            None => {
                if ui.icon_button(*rect, cells::icons::BRACKETS, label, h.show_rooms) {
                    actions.push(UiAction::ToggleRooms);
                }
            }
        }
    }
    actions
}

fn draw_build(ui: &mut Ui, origin: (f32, f32), panel: &BuildPanel) -> Vec<UiAction> {
    let layout = panel.layout(origin);
    ui.panel(layout.panel);
    ui.label_color(layout.title, &panel.title, ui::accent());
    let mut actions = Vec::new();
    for (i, (rect, label)) in layout.buildings.iter().zip(&panel.buildings).enumerate() {
        if ui.button(*rect, label, i == panel.active_building) {
            actions.push(UiAction::PickBuilding(i));
        }
    }
    ui.label_color(layout.material_header, "Material", ui::dim());
    for ((rect, label), id) in layout
        .materials
        .iter()
        .zip(&panel.materials)
        .zip(&panel.material_ids)
    {
        if ui.button(*rect, label, panel.active_material == Some(*id)) {
            actions.push(UiAction::PickMaterial(*id));
        }
    }
    for (pos, line) in layout.lines.iter().zip(&panel.lines) {
        ui.label(*pos, line);
    }
    actions
}

fn draw_workshop(ui: &mut Ui, origin: (f32, f32), panel: &WorkshopPanel) -> Vec<UiAction> {
    let layout = panel.layout(origin);
    ui.panel(layout.panel);
    ui.label_color(layout.title, &panel.title, ui::accent());
    for (pos, line) in layout.lines.iter().zip(&panel.lines) {
        ui.label(*pos, line);
    }
    let mut actions = Vec::new();
    for (row, recipe) in layout.recipes.iter().zip(&panel.recipes) {
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
    ui.panel(layout.panel);
    ui.label_color(layout.header, "Stockpile accepts", ui::accent());
    let mut actions = Vec::new();
    let last = panel.labels.len() - 1;
    for (i, (rect, label)) in layout.buttons.iter().zip(&panel.labels).enumerate() {
        let active = match i {
            0 => zone.filter == ItemFilter::All,
            n if n == last => false,
            n => materials
                .get(n - 1)
                .is_some_and(|&m| zone.filter.accepts(m)),
        };
        if ui.button(*rect, label, active) {
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

/// Draw the whole display. Returns what the player asked for.
pub fn draw(ui: &mut Ui, h: &HudInput) -> Vec<UiAction> {
    let (content, placed) = plan(h);
    let mut actions = Vec::new();
    actions.extend(ui::status_panel(
        ui,
        (placed.status.x, placed.status.y),
        &content.status_before,
        h.speed,
        &content.status_after,
    ));
    actions.extend(draw_toolbar(
        ui,
        (placed.toolbar.x, placed.toolbar.y),
        &content,
        h,
    ));
    if let Some(help) = placed.help {
        ui::text_panel(ui, (help.x, help.y), &content.help);
    }
    if let (Some(r), Some(panel)) = (placed.build, &content.build) {
        actions.extend(draw_build(ui, (r.x, r.y), panel));
    }
    if let Some(r) = placed.work {
        actions.extend(draw_work(ui, (r.x, r.y), &content, h));
    }
    if let Some(r) = placed.colonist {
        ui::rows_panel(ui, (r.x, r.y), &content.colonist);
    }
    if let (Some(r), Some(rows)) = (placed.tile, &content.tile) {
        ui::rows_panel(ui, (r.x, r.y), rows);
    }
    if let (Some(r), Some(side)) = (placed.side, &content.side) {
        match side {
            Side::Filter(f) => {
                actions.extend(draw_filter(ui, (r.x, r.y), h.sim, f, &content.materials));
            }
            Side::Workshop(w) => actions.extend(draw_workshop(ui, (r.x, r.y), w)),
        }
    }
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
    fn help_names_every_tool_key_and_fits_the_window() {
        let help = help_lines().join("\n");
        for tool in Tool::ALL {
            let key = tool.key_label();
            assert!(
                help.contains(&format!("{key} ")),
                "help does not mention key {key}"
            );
        }
        assert!(help.contains("P: work panel"));
        assert!(help.contains("O: rooms overlay"));
        assert!(help.contains("1-7"));
        for line in help_lines() {
            assert!(
                ui::text_px(&line) + 2.0 * ui::PAD <= 1280.0 - 2.0 * MARGIN,
                "{line}"
            );
        }
    }

    #[test]
    fn toolbar_labels_show_the_key_of_each_tool() {
        assert_eq!(tool_label(Tool::Cut), "Cut (T)");
        assert_eq!(tool_label(Tool::Stockpile), "Zone (Z)");
        assert_eq!(tool_label(Tool::Build), "Build (B)");
        assert_eq!(tool_label(Tool::Deconstruct), "Remove (X)");
        assert_eq!(WORK_LABEL, "Work (P)");
        assert_eq!(ROOMS_LABEL, "Rooms (O)");
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
    fn every_tool_has_a_hint() {
        for tool in Tool::ALL {
            assert!(tool_hint(tool).starts_with(tool.name()));
            assert!(tool_hint(tool).chars().count() <= STATUS_CHARS);
        }
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
        view: View,
        tool: Tool,
        tile: Option<(u16, u16)>,
        hover: Option<(u16, u16)>,
    ) -> HudInput<'a> {
        HudInput {
            sim,
            view,
            tool,
            build,
            show_work: true,
            show_rooms: true,
            selected: sim.colonists().iter().next().map(|(id, _)| id),
            selected_tile: tile,
            hover_tile: hover,
            speed: Speed::X1,
            fps: 60,
            zoom: 16.0,
            map_name: "temperate forest",
            last_event: "Colonist 0 stopped Cut: no path",
        }
    }

    #[test]
    fn panels_do_not_overlap_at_1280_and_1920() {
        let sim = busy_sim();
        let build = BuildState::new(sim.rules());
        for view in [(1280.0, 720.0), (1920.0, 1080.0)] {
            for tool in [Tool::Select, Tool::Build, Tool::Deconstruct] {
                // A room tile, a workshop tile, an empty tile, a hover tile, nothing.
                for (tile, hover) in [
                    (Some((5, 3)), None),
                    (Some((6, 4)), None),
                    (Some((0, 0)), None),
                    (None, Some((5, 3))),
                    (None, None),
                ] {
                    let h = input(&sim, &build, view, tool, tile, hover);
                    let (_, placed) = plan(&h);
                    assert_eq!(
                        layout::first_overlap(&placed),
                        None,
                        "view {view:?}, tool {tool:?}, tile {tile:?}, hover {hover:?}"
                    );
                    for (name, r) in placed.all() {
                        assert!(r.x >= 0.0 && r.y >= 0.0, "{name} starts outside the window");
                    }
                }
            }
        }
    }

    #[test]
    fn the_colonist_panel_does_not_cover_the_toolbar_at_1280() {
        let sim = busy_sim();
        let build = BuildState::new(sim.rules());
        let h = input(
            &sim,
            &build,
            (1280.0, 720.0),
            Tool::Select,
            Some((5, 3)),
            None,
        );
        let (_, placed) = plan(&h);
        let colonist = placed.colonist.unwrap();
        assert!(!colonist.intersects(&placed.toolbar));
        let help = placed.help.unwrap();
        assert!(!colonist.intersects(&help));
        assert!(placed.toolbar.bottom() <= help.y);
    }

    #[test]
    fn the_important_panels_stay_inside_the_window() {
        let sim = busy_sim();
        let build = BuildState::new(sim.rules());
        let view = (1280.0, 720.0);
        // The build tool keeps no selected tile, so the build panel has room.
        let h = input(&sim, &build, view, Tool::Build, None, None);
        let (_, placed) = plan(&h);
        assert!(placed.build.is_some(), "the build panel must show");
        assert!(placed.colonist.is_some());
        for (name, r) in placed.all() {
            assert!(r.right() <= view.0 && r.bottom() <= view.1, "{name} {r:?}");
        }
        // The select tool on a workshop shows the tile and the workshop panel.
        let h = input(&sim, &build, view, Tool::Select, Some((6, 4)), None);
        let (_, placed) = plan(&h);
        assert!(placed.colonist.is_some());
        assert!(placed.tile.is_some());
        assert!(placed.side.is_some(), "the workshop panel must show");
        for (name, r) in placed.all() {
            assert!(r.right() <= view.0 && r.bottom() <= view.1, "{name} {r:?}");
        }
    }

    #[test]
    fn the_toolbar_is_one_column_of_all_tools_the_work_button_and_the_rooms_button() {
        let sim = busy_sim();
        let build = BuildState::new(sim.rules());
        let h = input(&sim, &build, (1280.0, 720.0), Tool::Select, None, None);
        let content = Content::new(&h);
        let l = ui::column_layout((MARGIN, 0.0), &content.toolbar_refs());
        assert_eq!(l.buttons.len(), Tool::ALL.len() + 2);
        let x = l.buttons[0].x;
        assert!(l.buttons.iter().all(|b| b.x == x));
    }
}
