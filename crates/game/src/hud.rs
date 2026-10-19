//! The heads-up display: status panel, work panel, toolbar, help, inspectors, and the
//! stockpile filter panel. It reads `&Sim` and returns `UiAction` values. It never changes
//! the sim (AGENTS.md code rules).

use crate::camera::View;
use crate::inspector;
use crate::tools::{self, Tool};
use crate::ui::{self, Rect, Ui, UiAction};
use macroquad::prelude::Color;
use sim::{ColonistId, Command, ItemFilter, JobKind, Sim, Speed, StockpileId};

/// Gap between panels, in screen pixels.
const MARGIN: f32 = 8.0;
/// Widest row of buttons in the filter panel, in pixels.
const FILTER_WIDTH: f32 = 340.0;
/// Title color.
const ACCENT: Color = Color::new(0.62, 0.80, 1.0, 1.0);
/// Hint color.
const DIM: Color = Color::new(0.58, 0.64, 0.74, 1.0);

/// Fill of a priority cell: off, then 1 (first) to 4.
const PRIORITY_FILL: [Color; 5] = [
    Color::new(0.13, 0.14, 0.17, 1.0),
    Color::new(0.16, 0.50, 0.32, 1.0),
    Color::new(0.16, 0.40, 0.32, 1.0),
    Color::new(0.17, 0.30, 0.30, 1.0),
    Color::new(0.20, 0.24, 0.28, 1.0),
];

/// What the display needs to know for one frame.
pub struct HudInput<'a> {
    /// The simulation (read only).
    pub sim: &'a Sim,
    /// The viewport size.
    pub view: View,
    /// The active tool.
    pub tool: Tool,
    /// True if the work panel is open.
    pub show_work: bool,
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

/// The lines of the help panel. Each line is short enough for a 1280 pixel window.
pub fn help_lines() -> Vec<String> {
    vec![
        "WASD/arrows, middle drag: pan   Wheel, +/-: zoom   G: grid   Esc: quit".to_owned(),
        "Tools: V select  T cut  M mine  H haul  Z zone  C cancel   P: work panel".to_owned(),
        "Drag a rectangle on the map. Right click: cancel the drag".to_owned(),
        "Select tool: left click picks, right click moves the colonist (debug)".to_owned(),
        "Space: pause   Q/E/R: 1x/3x/6x   F1-F3: map".to_owned(),
    ]
}

/// The label of a toolbar button, for example `Cut (T)`.
pub fn tool_label(tool: Tool) -> String {
    format!("{} ({})", tool.name(), tool.key_label())
}

/// The label of the work panel button.
pub const WORK_LABEL: &str = "Work (P)";

/// The hint line of a tool.
pub fn tool_hint(tool: Tool) -> &'static str {
    match tool {
        Tool::Select => "Select: click a colonist or a tile",
        Tool::Cut => "Cut: drag over trees",
        Tool::Mine => "Mine: drag over rock",
        Tool::Haul => "Haul: drag over items on the ground",
        Tool::Stockpile => "Zone: drag to make a stockpile zone",
        Tool::Cancel => "Cancel: drag over orders",
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

fn draw_status(ui: &mut Ui, h: &HudInput) -> (Rect, Vec<UiAction>) {
    let before = [
        ui::format_time(&h.sim.time()),
        format!(
            "Tick {}   state hash {:016x}",
            h.sim.tick(),
            h.sim.state_hash()
        ),
    ];
    let mut after = vec![
        format!("FPS {}  {:.1} px/tile  {}", h.fps, h.zoom, h.map_name),
        tool_hint(h.tool).to_owned(),
    ];
    if !h.last_event.is_empty() {
        after.push(h.last_event.to_owned());
    }
    let actions = ui::status_panel(ui, (MARGIN, MARGIN), &before, h.speed, &after);
    let panel = ui
        .panels()
        .last()
        .copied()
        .unwrap_or(Rect::new(0.0, 0.0, 0.0, 0.0));
    (panel, actions)
}

fn draw_work(ui: &mut Ui, origin: (f32, f32), h: &HudInput) -> Vec<UiAction> {
    let rows = inspector::work_rows(h.sim);
    let names: Vec<String> = rows.iter().map(|r| r.name.clone()).collect();
    let hint = "Click: next   Right click: back";
    let layout = ui::work_layout(origin, &names, hint);
    ui.panel(layout.panel);
    ui.label_color(layout.title, "Work priorities", ACCENT);
    ui.label_color(layout.hint, hint, DIM);
    for (kind, pos) in JobKind::ALL.iter().zip(&layout.headers) {
        ui.label(*pos, kind.name());
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

fn draw_toolbar(ui: &mut Ui, origin: (f32, f32), h: &HudInput) -> Vec<UiAction> {
    let mut labels: Vec<String> = Tool::ALL.iter().map(|t| tool_label(*t)).collect();
    labels.push(WORK_LABEL.to_owned());
    let refs: Vec<&str> = labels.iter().map(String::as_str).collect();
    let layout = ui::hud_layout(origin, &[], &refs, &[]);
    ui.panel(layout.panel);
    let mut actions = Vec::new();
    for (i, rect) in layout.buttons.iter().enumerate() {
        match Tool::ALL.get(i) {
            Some(tool) => {
                if ui.button(*rect, &labels[i], h.tool == *tool) {
                    actions.push(UiAction::SetTool(*tool));
                }
            }
            None => {
                if ui.button(*rect, &labels[i], h.show_work) {
                    actions.push(UiAction::ToggleWork);
                }
            }
        }
    }
    actions
}

fn draw_filter(
    ui: &mut Ui,
    origin: (f32, f32),
    sim: &Sim,
    id: StockpileId,
    materials: &[u16],
) -> Vec<UiAction> {
    let Some(zone) = sim.stockpiles().get(id) else {
        return Vec::new();
    };
    let mut labels = vec!["All".to_owned()];
    labels.extend(materials.iter().map(|&m| inspector::material_name(sim, m)));
    labels.push("Remove zone".to_owned());
    let layout = ui::grid_layout(origin, "Stockpile accepts", &labels, FILTER_WIDTH);
    ui.panel(layout.panel);
    ui.label_color(layout.header, "Stockpile accepts", ACCENT);
    let mut actions = Vec::new();
    let last = labels.len() - 1;
    for (i, (rect, label)) in layout.buttons.iter().zip(&labels).enumerate() {
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
    let mut actions = Vec::new();
    let (status, a) = draw_status(ui, h);
    actions.extend(a);
    if h.show_work {
        actions.extend(draw_work(ui, (MARGIN, status.y + status.h + MARGIN), h));
    }

    // Bottom left: toolbar above the help.
    let help = help_lines();
    let help_h = ui::hud_layout((0.0, 0.0), &help, &[], &[]).panel.h;
    let mut labels: Vec<String> = Tool::ALL.iter().map(|t| tool_label(*t)).collect();
    labels.push(WORK_LABEL.to_owned());
    let refs: Vec<&str> = labels.iter().map(String::as_str).collect();
    let toolbar_h = ui::hud_layout((0.0, 0.0), &[], &refs, &[]).panel.h;
    let help_y = h.view.1 - help_h - MARGIN;
    actions.extend(draw_toolbar(ui, (MARGIN, help_y - toolbar_h - 4.0), h));
    ui::text_panel(ui, (MARGIN, help_y), &help);

    // Right: the colonist panel, the tile panel at its left, the filter panel below the tile panel.
    let mut right_edge = h.view.0 - MARGIN;
    if let Some(id) = h.selected {
        let rows = inspector::colonist_rows(h.sim, id);
        if !rows.is_empty() {
            let w = ui::rows_layout((0.0, 0.0), &rows).panel.w;
            let r = ui::rows_panel(ui, (right_edge - w, MARGIN), &rows);
            right_edge = r.x - MARGIN;
        }
    }
    let materials = tools::haulable_materials(h.sim.rules());
    if let Some((x, y)) = h.selected_tile.or(h.hover_tile) {
        let rows = inspector::tile_rows(h.sim, x, y, &materials);
        let w = ui::rows_layout((0.0, 0.0), &rows).panel.w;
        let r = ui::rows_panel(ui, (right_edge - w, MARGIN), &rows);
        if let Some(zone) = h
            .selected_tile
            .and_then(|(sx, sy)| inspector::zone_at(h.sim, sx, sy))
        {
            let filter_origin = (
                right_edge - FILTER_WIDTH - 2.0 * ui::PAD,
                r.y + r.h + MARGIN,
            );
            actions.extend(draw_filter(ui, filter_origin, h.sim, zone, &materials));
        }
    }
    actions
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inspector::WorkRow;

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
        assert_eq!(WORK_LABEL, "Work (P)");
    }

    #[test]
    fn the_toolbar_fits_into_a_small_window() {
        let mut labels: Vec<String> = Tool::ALL.iter().map(|t| tool_label(*t)).collect();
        labels.push(WORK_LABEL.to_owned());
        let refs: Vec<&str> = labels.iter().map(String::as_str).collect();
        let l = ui::hud_layout((MARGIN, 0.0), &[], &refs, &[]);
        assert_eq!(l.buttons.len(), 7);
        assert!(l.panel.x + l.panel.w <= 1000.0, "width {}", l.panel.w);
    }

    #[test]
    fn a_priority_click_sends_the_next_value_and_a_right_click_the_previous() {
        let sim = sim::scenario::sim_from_ascii("c", 1).unwrap();
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
    fn filter_buttons_toggle_one_material_and_all_resets() {
        let materials = [3, 5, 8];
        let off = filter_after_click(&ItemFilter::All, &materials, 2);
        assert_eq!(off, ItemFilter::Only(vec![3, 8]));
        assert_eq!(filter_after_click(&off, &materials, 0), ItemFilter::All);
        assert_eq!(filter_after_click(&off, &materials, 2), ItemFilter::All);
    }

    #[test]
    fn work_text_lists_the_priorities_in_job_order() {
        let sim = sim::scenario::sim_from_ascii("c", 1).unwrap();
        let rows = inspector::work_rows(&sim);
        assert_eq!(work_text(&rows[0]), "Colonist 0 3 3 3");
    }

    #[test]
    fn every_tool_has_a_hint() {
        for tool in Tool::ALL {
            assert!(tool_hint(tool).starts_with(tool.name()));
        }
    }
}
