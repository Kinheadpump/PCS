//! Small immediate-mode interface: panel, label, button, key and value panel, and the
//! layouts of the work panel. It reads game state and returns `UiAction` values. It never
//! changes the `Sim` (AGENTS.md code rules). Layout is pure math, so tests run without a window.

use crate::clock::{SPEEDS, speed_label};
use crate::font;
use crate::icons;
use crate::palette::ui as pal;
use crate::render::color as rgb_color;
use crate::tools::Tool;
use macroquad::prelude::{Color, draw_rectangle};
use sim::time::{GameTime, Season};
use sim::{ColonistId, Command, JobKind, Speed};

/// Screen pixels per font pixel.
pub const SCALE: u32 = 2;
/// Padding inside a panel, in screen pixels.
pub const PAD: f32 = 8.0;
/// Gap between buttons, in screen pixels.
pub const GAP: f32 = 4.0;
/// Height of a text line.
pub const LINE_H: f32 = (font::GLYPH_H * SCALE) as f32 + 4.0;
/// Height of a button.
pub const BUTTON_H: f32 = (font::GLYPH_H * SCALE) as f32 + 8.0;
/// Horizontal padding inside a button.
const BUTTON_PAD: f32 = 8.0;
/// Height of a row in a key and value panel.
pub const ROW_H: f32 = (font::GLYPH_H * SCALE) as f32 + 2.0;
/// Extra space above a section heading.
const SECTION_GAP: f32 = 6.0;
/// Space between the key column and the value column.
const KEY_GAP: f32 = 16.0;
/// Width of a priority cell in the work panel.
pub const CELL_W: f32 = 56.0;
/// Height of a row in the work panel.
pub const WORK_ROW_H: f32 = BUTTON_H - 2.0;

/// Size of one interface pixel in screen pixels: a font pixel. Frames use the same grid.
const PX: f32 = SCALE as f32;
/// Width of an icon in a button, in screen pixels.
pub const ICON_W: f32 = 16.0;

fn text_color() -> Color {
    rgb_color(pal::TEXT, 255)
}

/// Title text color of a key and value panel.
pub fn accent() -> Color {
    rgb_color(pal::ACCENT, 255)
}

/// Section heading and hint text color.
pub fn dim() -> Color {
    rgb_color(pal::DIM, 255)
}

fn key_color() -> Color {
    rgb_color(pal::KEY, 255)
}

/// Draw a box with a hard 1 pixel outline, rounded corners (one pixel cut), and a bevel: light on
/// the top and left, dark on the bottom and right. `sunk` swaps the bevel. Pixels are `PX` big.
pub fn draw_frame(r: Rect, fill: Color, light: Color, dark: Color, ink: Color, sunk: bool) {
    let (x, y, w, h) = (r.x.round(), r.y.round(), r.w.round(), r.h.round());
    // Outline with cut corners.
    draw_rectangle(x + PX, y, w - 2.0 * PX, h, ink);
    draw_rectangle(x, y + PX, w, h - 2.0 * PX, ink);
    // Fill.
    let (ix, iy, iw, ih) = (x + PX, y + PX, w - 2.0 * PX, h - 2.0 * PX);
    draw_rectangle(ix, iy, iw, ih, fill);
    let (top, bottom) = if sunk { (dark, light) } else { (light, dark) };
    // Bevel: one pixel on each side.
    draw_rectangle(ix + PX, iy, iw - PX, PX, top);
    draw_rectangle(ix, iy + PX, PX, ih - PX, top);
    draw_rectangle(ix, iy + ih - PX, iw, PX, bottom);
    draw_rectangle(ix + iw - PX, iy, PX, ih - PX, bottom);
}

/// Draw a small plate with an accent colored bevel, for tags on the map.
pub fn draw_tag(x: f32, y: f32, w: f32, h: f32, accent: Color) {
    let ink = rgb_color(pal::INK, 255);
    draw_frame(
        Rect::new(x, y, w, h),
        rgb_color(pal::PANEL, 240),
        accent,
        rgb_color(pal::PANEL_DARK, 255),
        ink,
        false,
    );
}

/// Draw text with a one pixel shadow, so it reads on any fill.
fn shadow_text(text: &str, x: f32, y: f32, color: Color) {
    font::draw_text(text, x + PX, y + PX, SCALE, rgb_color(pal::INK, 255));
    font::draw_text(text, x, y, SCALE, color);
}

fn lighten(c: Color, amount: f32) -> Color {
    Color::new(
        (c.r + amount).min(1.0),
        (c.g + amount).min(1.0),
        (c.b + amount).min(1.0),
        c.a,
    )
}

/// An axis-aligned rectangle in screen pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    /// Left edge.
    pub x: f32,
    /// Top edge.
    pub y: f32,
    /// Width.
    pub w: f32,
    /// Height.
    pub h: f32,
}

impl Rect {
    /// The right edge.
    pub fn right(&self) -> f32 {
        self.x + self.w
    }

    /// The bottom edge.
    pub fn bottom(&self) -> f32 {
        self.y + self.h
    }

    /// True if the two rectangles share an area. Rectangles that only touch do not.
    pub fn intersects(&self, other: &Rect) -> bool {
        self.x < other.right()
            && other.x < self.right()
            && self.y < other.bottom()
            && other.y < self.bottom()
    }

    /// A rectangle.
    pub fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self { x, y, w, h }
    }

    /// True if the point is inside (left and top edges inclusive).
    pub fn contains(&self, p: (f32, f32)) -> bool {
        p.0 >= self.x && p.0 < self.x + self.w && p.1 >= self.y && p.1 < self.y + self.h
    }
}

/// What the interface asks the game to do.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UiAction {
    /// The player pressed a speed button.
    SetSpeed(Speed),
    /// The player chose a tool in the toolbar.
    SetTool(Tool),
    /// The player pressed the work panel button.
    ToggleWork,
    /// The player pressed the rooms overlay button.
    ToggleRooms,
    /// The player clicked a colonist name in the work panel.
    SelectColonist(ColonistId),
    /// The player chose a building type in the build panel (index in `buildings.ron`).
    PickBuilding(usize),
    /// The player chose a material in the build panel (material index).
    PickMaterial(u16),
    /// A command for the sim: a priority change, a stockpile change, or a craft order.
    Command(Command),
}

/// A line of a key and value panel.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Row {
    /// The panel title.
    Title(String),
    /// A heading for the rows below it.
    Section(String),
    /// A key and its value, in two columns.
    Pair(String, String),
    /// A plain line.
    Line(String),
}

/// Width of a text on screen, in pixels.
pub fn text_px(text: &str) -> f32 {
    (font::text_width(text) * SCALE) as f32
}

/// Positions of the parts of a panel.
#[derive(Clone, Debug, PartialEq)]
pub struct HudLayout {
    /// The panel rectangle.
    pub panel: Rect,
    /// Top left corner of each text line above the buttons.
    pub before: Vec<(f32, f32)>,
    /// One rectangle for each button, in one row.
    pub buttons: Vec<Rect>,
    /// Top left corner of each text line below the buttons.
    pub after: Vec<(f32, f32)>,
}

/// Lay out a panel: text lines, then one row of buttons, then more text lines.
pub fn hud_layout(
    origin: (f32, f32),
    before: &[String],
    button_labels: &[&str],
    after: &[String],
) -> HudLayout {
    let button_widths: Vec<f32> = button_labels
        .iter()
        .map(|l| text_px(l) + 2.0 * BUTTON_PAD)
        .collect();
    let row_w =
        button_widths.iter().sum::<f32>() + GAP * button_widths.len().saturating_sub(1) as f32;
    let text_w = before
        .iter()
        .chain(after)
        .map(|l| text_px(l))
        .fold(0.0_f32, f32::max);
    let inner_w = text_w.max(row_w);
    let mut y = origin.1 + PAD;
    let x = origin.0 + PAD;
    let place = |lines: &[String], y: &mut f32| -> Vec<(f32, f32)> {
        lines
            .iter()
            .map(|_| {
                let p = (x, *y);
                *y += LINE_H;
                p
            })
            .collect()
    };
    let before_pos = place(before, &mut y);
    let mut buttons = Vec::new();
    if !button_labels.is_empty() {
        let mut bx = x;
        for w in &button_widths {
            buttons.push(Rect::new(bx, y, *w, BUTTON_H));
            bx += w + GAP;
        }
        y += BUTTON_H + GAP;
    }
    let after_pos = place(after, &mut y);
    HudLayout {
        panel: Rect::new(
            origin.0,
            origin.1,
            inner_w + 2.0 * PAD,
            y - origin.1 + PAD - 2.0,
        ),
        before: before_pos,
        buttons,
        after: after_pos,
    }
}

/// Immediate-mode UI for one frame.
pub struct Ui {
    mouse: (f32, f32),
    click: Option<(f32, f32)>,
    right_click: Option<(f32, f32)>,
    panels: Vec<Rect>,
}

/// Which mouse buttons clicked a widget in this frame.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Press {
    /// The left button.
    pub left: bool,
    /// The right button.
    pub right: bool,
}

impl Ui {
    /// A UI for one frame. `click` is the position of a left click in this frame.
    pub fn new(mouse: (f32, f32), click: Option<(f32, f32)>) -> Self {
        Self {
            mouse,
            click,
            right_click: None,
            panels: Vec::new(),
        }
    }

    /// The same UI with the position of a right click in this frame.
    pub fn with_right_click(mut self, right_click: Option<(f32, f32)>) -> Self {
        self.right_click = right_click;
        self
    }

    /// The panels drawn in this frame.
    pub fn panels(&self) -> &[Rect] {
        &self.panels
    }

    /// True if the point is over a panel drawn in this frame.
    pub fn captures(&self, p: (f32, f32)) -> bool {
        self.panels.iter().any(|r| r.contains(p))
    }

    /// Draw a panel background.
    pub fn panel(&mut self, r: Rect) {
        draw_frame(
            r,
            rgb_color(pal::PANEL, 244),
            rgb_color(pal::PANEL_LIGHT, 255),
            rgb_color(pal::PANEL_DARK, 255),
            rgb_color(pal::INK, 255),
            false,
        );
        self.panels.push(r);
    }

    /// Draw a text label at a top left corner.
    pub fn label(&self, pos: (f32, f32), text: &str) {
        shadow_text(text, pos.0, pos.1, text_color());
    }

    /// Draw a text label in a color.
    pub fn label_color(&self, pos: (f32, f32), text: &str, color: Color) {
        shadow_text(text, pos.0, pos.1, color);
    }

    fn button_frame(&self, r: Rect, fill: Color, pressed: bool) {
        let light = lighten(fill, 0.16);
        let dark = Color::new(fill.r * 0.55, fill.g * 0.55, fill.b * 0.62, fill.a);
        draw_frame(r, fill, light, dark, rgb_color(pal::INK, 255), pressed);
    }

    fn state_color(&self, r: Rect, active: bool) -> Color {
        if active {
            rgb_color(pal::BUTTON_ACTIVE, 255)
        } else if r.contains(self.mouse) {
            rgb_color(pal::BUTTON_HOVER, 255)
        } else {
            rgb_color(pal::BUTTON, 255)
        }
    }

    /// Draw a button. Returns true if the player clicked it in this frame.
    pub fn button(&self, r: Rect, text: &str, active: bool) -> bool {
        self.button_frame(r, self.state_color(r, active), active);
        let drop = f32::from(u8::from(active));
        let tx = r.x + (r.w - text_px(text)) / 2.0;
        let ty = r.y + (r.h - (font::GLYPH_H * SCALE) as f32) / 2.0 + drop;
        shadow_text(text, tx.round(), ty.round(), text_color());
        self.click.is_some_and(|c| r.contains(c))
    }

    /// Draw a button with an icon at the left and a text. Returns true if clicked in this frame.
    pub fn icon_button(&self, r: Rect, icon: usize, text: &str, active: bool) -> bool {
        self.button_frame(r, self.state_color(r, active), active);
        let drop = f32::from(u8::from(active));
        let iy = r.y + (r.h - ICON_W) / 2.0 + drop;
        icons::draw(icon, r.x + BUTTON_PAD - 2.0, iy, ICON_W, text_color());
        let tx = r.x + BUTTON_PAD - 2.0 + ICON_W + GAP;
        let ty = r.y + (r.h - (font::GLYPH_H * SCALE) as f32) / 2.0 + drop;
        shadow_text(text, tx.round(), ty.round(), text_color());
        self.click.is_some_and(|c| r.contains(c))
    }

    /// Draw a button with a given fill. Hover makes it lighter. Returns which mouse
    /// buttons clicked it in this frame.
    pub fn tinted_button(&self, r: Rect, text: &str, fill: Color) -> Press {
        let lift = if r.contains(self.mouse) { 0.10 } else { 0.0 };
        let color = Color::new(fill.r + lift, fill.g + lift, fill.b + lift, fill.a);
        self.button_frame(r, color, false);
        let tx = r.x + (r.w - text_px(text)) / 2.0;
        let ty = r.y + (r.h - (font::GLYPH_H * SCALE) as f32) / 2.0;
        shadow_text(text, tx.round(), ty.round(), text_color());
        Press {
            left: self.click.is_some_and(|c| r.contains(c)),
            right: self.right_click.is_some_and(|c| r.contains(c)),
        }
    }
}

/// Draw a panel with text lines only.
pub fn text_panel(ui: &mut Ui, origin: (f32, f32), lines: &[String]) {
    let layout = hud_layout(origin, lines, &[], &[]);
    ui.panel(layout.panel);
    for (pos, line) in layout.before.iter().zip(lines) {
        ui.label(*pos, line);
    }
}

/// Position of a row: its text, and its value (pairs only).
pub type RowPos = ((f32, f32), Option<(f32, f32)>);

/// Positions of the rows of a key and value panel.
#[derive(Clone, Debug, PartialEq)]
pub struct RowsLayout {
    /// The panel rectangle.
    pub panel: Rect,
    /// For each row: the position of its text, and the position of its value (pairs only).
    pub rows: Vec<RowPos>,
}

/// Lay out a key and value panel. Keys line up in one column, values in the next.
pub fn rows_layout(origin: (f32, f32), rows: &[Row]) -> RowsLayout {
    let key_w = rows
        .iter()
        .filter_map(|r| match r {
            Row::Pair(k, _) => Some(text_px(k)),
            _ => None,
        })
        .fold(0.0_f32, f32::max);
    let inner_w = rows
        .iter()
        .map(|r| match r {
            Row::Title(t) | Row::Section(t) | Row::Line(t) => text_px(t),
            Row::Pair(_, v) => key_w + KEY_GAP + text_px(v),
        })
        .fold(0.0_f32, f32::max);
    let x = origin.0 + PAD;
    let mut y = origin.1 + PAD;
    let mut out = Vec::new();
    for (i, row) in rows.iter().enumerate() {
        if matches!(row, Row::Section(_)) && i > 0 {
            y += SECTION_GAP;
        }
        let value = matches!(row, Row::Pair(..)).then_some((x + key_w + KEY_GAP, y));
        out.push(((x, y), value));
        y += ROW_H;
    }
    RowsLayout {
        panel: Rect::new(
            origin.0,
            origin.1,
            inner_w + 2.0 * PAD,
            y - origin.1 + PAD - 2.0,
        ),
        rows: out,
    }
}

/// Draw a key and value panel. Returns its rectangle.
pub fn rows_panel(ui: &mut Ui, origin: (f32, f32), rows: &[Row]) -> Rect {
    let layout = rows_layout(origin, rows);
    ui.panel(layout.panel);
    for (row, (pos, value)) in rows.iter().zip(&layout.rows) {
        match row {
            Row::Title(t) => ui.label_color(*pos, t, accent()),
            Row::Section(t) => ui.label_color(*pos, t, dim()),
            Row::Line(t) => ui.label(*pos, t),
            Row::Pair(k, v) => {
                ui.label_color(*pos, k, key_color());
                if let Some(vp) = value {
                    ui.label(*vp, v);
                }
            }
        }
    }
    layout.panel
}

/// Positions of a panel with a header line and wrapped buttons.
#[derive(Clone, Debug, PartialEq)]
pub struct GridLayout {
    /// The panel rectangle.
    pub panel: Rect,
    /// Top left corner of the header text.
    pub header: (f32, f32),
    /// One rectangle for each button.
    pub buttons: Vec<Rect>,
}

/// Lay out a header line and buttons that wrap at `max_inner_w` pixels.
pub fn grid_layout(
    origin: (f32, f32),
    header: &str,
    labels: &[String],
    max_inner_w: f32,
) -> GridLayout {
    let x0 = origin.0 + PAD;
    let header_pos = (x0, origin.1 + PAD);
    let mut y = origin.1 + PAD + ROW_H + GAP;
    let mut x = x0;
    let mut used_w = text_px(header);
    let mut buttons = Vec::new();
    for label in labels {
        let w = text_px(label) + 2.0 * BUTTON_PAD;
        if x > x0 && x + w > x0 + max_inner_w {
            x = x0;
            y += BUTTON_H + GAP;
        }
        buttons.push(Rect::new(x, y, w, BUTTON_H));
        x += w + GAP;
        used_w = used_w.max(x - GAP - x0);
    }
    if !labels.is_empty() {
        y += BUTTON_H;
    }
    GridLayout {
        panel: Rect::new(origin.0, origin.1, used_w + 2.0 * PAD, y - origin.1 + PAD),
        header: header_pos,
        buttons,
    }
}

/// Positions of the work panel: a table of colonists and job types.
#[derive(Clone, Debug, PartialEq)]
pub struct WorkLayout {
    /// The panel rectangle.
    pub panel: Rect,
    /// Top left corner of the title.
    pub title: (f32, f32),
    /// Top left corner of the hint line.
    pub hint: (f32, f32),
    /// Top left corner of the label of each job type column.
    pub headers: Vec<(f32, f32)>,
    /// The colonist name button of each row.
    pub names: Vec<Rect>,
    /// The priority cells of each row, one for each job type.
    pub cells: Vec<Vec<Rect>>,
}

/// Lay out the work panel for colonists with the given names.
pub fn work_layout(origin: (f32, f32), names: &[String], hint: &str) -> WorkLayout {
    let name_w = names
        .iter()
        .map(|n| text_px(n))
        .fold(text_px("Colonist"), f32::max)
        + 2.0 * BUTTON_PAD;
    let x0 = origin.0 + PAD;
    let table_w = name_w + (CELL_W + GAP) * JobKind::COUNT as f32;
    let inner_w = table_w.max(text_px(hint)).max(text_px("Work priorities"));
    let title = (x0, origin.1 + PAD);
    let hint_pos = (x0, title.1 + ROW_H);
    let header_y = hint_pos.1 + ROW_H + GAP;
    let cell_x = |k: usize| x0 + name_w + GAP + k as f32 * (CELL_W + GAP);
    let headers = JobKind::ALL
        .iter()
        .enumerate()
        .map(|(k, kind)| {
            let w = text_px(kind.name());
            (cell_x(k) + (CELL_W - w) / 2.0, header_y)
        })
        .collect();
    let mut y = header_y + ROW_H + 2.0;
    let mut name_rects = Vec::new();
    let mut cells = Vec::new();
    for _ in names {
        name_rects.push(Rect::new(x0, y, name_w, WORK_ROW_H));
        cells.push(
            (0..JobKind::COUNT)
                .map(|k| Rect::new(cell_x(k), y, CELL_W, WORK_ROW_H))
                .collect(),
        );
        y += WORK_ROW_H + 2.0;
    }
    WorkLayout {
        panel: Rect::new(
            origin.0,
            origin.1,
            inner_w + 2.0 * PAD,
            y - origin.1 + PAD - 2.0,
        ),
        title,
        hint: hint_pos,
        headers,
        names: name_rects,
        cells,
    }
}

/// Lay out a toolbar as one column of buttons of equal width, each with an icon and a label.
pub fn column_layout(origin: (f32, f32), labels: &[&str]) -> HudLayout {
    let w =
        labels.iter().map(|l| text_px(l)).fold(0.0_f32, f32::max) + 2.0 * BUTTON_PAD + ICON_W + GAP
            - 4.0;
    let mut buttons = Vec::new();
    let (x, mut y) = (origin.0 + PAD, origin.1 + PAD);
    for _ in labels {
        buttons.push(Rect::new(x, y, w, BUTTON_H));
        y += BUTTON_H + GAP;
    }
    let h = if labels.is_empty() {
        0.0
    } else {
        y - GAP - origin.1 + PAD
    };
    HudLayout {
        panel: Rect::new(origin.0, origin.1, w + 2.0 * PAD, h),
        before: Vec::new(),
        buttons,
        after: Vec::new(),
    }
}

/// Place buttons in rows that wrap at `max_w`. Returns the rectangles, the width that the
/// rows use, and the y below the last row.
fn flow_buttons(x0: f32, y0: f32, labels: &[String], max_w: f32) -> (Vec<Rect>, f32, f32) {
    let (mut x, mut y) = (x0, y0);
    let mut used = 0.0_f32;
    let mut out = Vec::new();
    for label in labels {
        let w = text_px(label) + 2.0 * BUTTON_PAD;
        if x > x0 && x + w > x0 + max_w {
            x = x0;
            y += BUTTON_H + GAP;
        }
        out.push(Rect::new(x, y, w, BUTTON_H));
        x += w + GAP;
        used = used.max(x - GAP - x0);
    }
    if !labels.is_empty() {
        y += BUTTON_H;
    }
    (out, used, y)
}

/// Positions of the build panel.
#[derive(Clone, Debug, PartialEq)]
pub struct BuildLayout {
    /// The panel rectangle.
    pub panel: Rect,
    /// Top left corner of the title.
    pub title: (f32, f32),
    /// One button for each building type.
    pub buildings: Vec<Rect>,
    /// Top left corner of the material header.
    pub material_header: (f32, f32),
    /// One button for each material.
    pub materials: Vec<Rect>,
    /// Top left corner of each text line at the bottom.
    pub lines: Vec<(f32, f32)>,
}

/// Lay out the build panel: title, building buttons, material header, material buttons, and
/// text lines. The buttons wrap at `max_inner_w`.
pub fn build_layout(
    origin: (f32, f32),
    title: &str,
    buildings: &[String],
    material_header: &str,
    materials: &[String],
    lines: &[String],
    max_inner_w: f32,
) -> BuildLayout {
    let x0 = origin.0 + PAD;
    let title_pos = (x0, origin.1 + PAD);
    let (b_rects, b_w, y) = flow_buttons(x0, title_pos.1 + ROW_H + GAP, buildings, max_inner_w);
    let header = (x0, y + GAP + 2.0);
    let (m_rects, m_w, mut y) = flow_buttons(x0, header.1 + ROW_H + GAP, materials, max_inner_w);
    if materials.is_empty() {
        y = header.1 + ROW_H;
    } else {
        y += GAP;
    }
    let mut line_pos = Vec::new();
    for _ in lines {
        line_pos.push((x0, y));
        y += ROW_H;
    }
    let text_w = lines
        .iter()
        .map(|l| text_px(l))
        .chain([text_px(title), text_px(material_header)])
        .fold(0.0_f32, f32::max);
    let inner_w = b_w.max(m_w).max(text_w);
    BuildLayout {
        panel: Rect::new(
            origin.0,
            origin.1,
            inner_w + 2.0 * PAD,
            y - origin.1 + PAD - 2.0,
        ),
        title: title_pos,
        buildings: b_rects,
        material_header: header,
        materials: m_rects,
        lines: line_pos,
    }
}

/// Positions of one recipe in the workshop panel.
#[derive(Clone, Debug, PartialEq)]
pub struct RecipeRow {
    /// Top left corner of the recipe name.
    pub label: (f32, f32),
    /// Top left corner of the line under the name (inputs, outputs, time).
    pub detail: (f32, f32),
    /// Buttons: add 1, add 5, add 10, clear.
    pub buttons: [Rect; 4],
}

/// Positions of the workshop panel.
#[derive(Clone, Debug, PartialEq)]
pub struct WorkshopLayout {
    /// The panel rectangle.
    pub panel: Rect,
    /// Top left corner of the title.
    pub title: (f32, f32),
    /// Top left corner of each text line under the title.
    pub lines: Vec<(f32, f32)>,
    /// One row for each recipe.
    pub recipes: Vec<RecipeRow>,
}

/// Labels of the four buttons of a recipe row.
pub const RECIPE_BUTTONS: [&str; 4] = ["+1", "+5", "+10", "Clear"];

/// Lay out the workshop panel: a title, text lines, and for each recipe a name, a detail line,
/// and a row of buttons.
pub fn workshop_layout(
    origin: (f32, f32),
    title: &str,
    lines: &[String],
    recipe_labels: &[(String, String)],
) -> WorkshopLayout {
    let x0 = origin.0 + PAD;
    let title_pos = (x0, origin.1 + PAD);
    let mut y = title_pos.1 + ROW_H;
    let mut line_pos = Vec::new();
    for _ in lines {
        line_pos.push((x0, y));
        y += ROW_H;
    }
    let widths: Vec<f32> = RECIPE_BUTTONS
        .iter()
        .map(|l| text_px(l) + 2.0 * BUTTON_PAD)
        .collect();
    let button_row_w = widths.iter().sum::<f32>() + GAP * 3.0;
    let mut recipes = Vec::new();
    for _ in recipe_labels {
        y += GAP + 2.0;
        let label_pos = (x0, y);
        y += ROW_H;
        let detail_pos = (x0, y);
        y += ROW_H;
        let mut bx = x0;
        let mut rects = [Rect::new(0.0, 0.0, 0.0, 0.0); 4];
        for (rect, w) in rects.iter_mut().zip(&widths) {
            *rect = Rect::new(bx, y, *w, BUTTON_H);
            bx += w + GAP;
        }
        y += BUTTON_H;
        recipes.push(RecipeRow {
            label: label_pos,
            detail: detail_pos,
            buttons: rects,
        });
    }
    let text_w = lines
        .iter()
        .chain(recipe_labels.iter().flat_map(|(a, b)| [a, b]))
        .map(|l| text_px(l))
        .chain([text_px(title)])
        .fold(0.0_f32, f32::max);
    WorkshopLayout {
        panel: Rect::new(
            origin.0,
            origin.1,
            text_w.max(button_row_w) + 2.0 * PAD,
            y - origin.1 + PAD,
        ),
        title: title_pos,
        lines: line_pos,
        recipes,
    }
}

/// Draw the status panel: text, speed buttons, more text.
pub fn status_panel(
    ui: &mut Ui,
    origin: (f32, f32),
    before: &[String],
    speed: Speed,
    after: &[String],
) -> Vec<UiAction> {
    let labels: Vec<&str> = SPEEDS.iter().map(|s| speed_label(*s)).collect();
    let layout = hud_layout(origin, before, &labels, after);
    ui.panel(layout.panel);
    for (pos, line) in layout.before.iter().zip(before) {
        ui.label(*pos, line);
    }
    let mut actions = Vec::new();
    for ((rect, s), label) in layout.buttons.iter().zip(SPEEDS).zip(&labels) {
        if ui.button(*rect, label, s == speed) {
            actions.push(UiAction::SetSpeed(s));
        }
    }
    for (pos, line) in layout.after.iter().zip(after) {
        ui.label(*pos, line);
    }
    actions
}

/// Name of a season.
pub fn season_name(season: Season) -> &'static str {
    match season {
        Season::Spring => "Spring",
        Season::Summer => "Summer",
        Season::Autumn => "Autumn",
        Season::Winter => "Winter",
    }
}

/// Calendar text, for example `Year 1, Spring, day 3, 07:00`.
pub fn format_time(t: &GameTime) -> String {
    format!(
        "Year {}, {}, day {}, {:02}:{:02}",
        t.year,
        season_name(t.season),
        t.day_of_season,
        t.hour,
        t.tick_of_hour * 60 / 100
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rect_contains_includes_left_top_and_excludes_right_bottom() {
        let r = Rect::new(10.0, 20.0, 30.0, 40.0);
        assert!(r.contains((10.0, 20.0)));
        assert!(r.contains((39.9, 59.9)));
        assert!(!r.contains((40.0, 30.0)));
        assert!(!r.contains((20.0, 60.0)));
        assert!(!r.contains((9.9, 30.0)));
    }

    #[test]
    fn layout_stacks_text_buttons_text_inside_the_panel() {
        let before = vec!["one".to_owned(), "two".to_owned()];
        let after = vec!["three".to_owned()];
        let l = hud_layout((8.0, 8.0), &before, &["Pause", "1x"], &after);
        assert_eq!(l.before[0], (8.0 + PAD, 8.0 + PAD));
        assert_eq!(l.before[1].1, l.before[0].1 + LINE_H);
        assert_eq!(l.buttons.len(), 2);
        assert!(l.buttons[0].y >= l.before[1].1 + LINE_H);
        assert_eq!(l.buttons[1].x, l.buttons[0].x + l.buttons[0].w + GAP);
        assert!(l.after[0].1 >= l.buttons[0].y + BUTTON_H);
        for r in &l.buttons {
            assert!(r.x >= l.panel.x && r.x + r.w <= l.panel.x + l.panel.w);
            assert!(r.y + r.h <= l.panel.y + l.panel.h);
        }
        assert!(l.after[0].1 + LINE_H <= l.panel.y + l.panel.h + 4.0);
    }

    #[test]
    fn panel_is_wide_enough_for_the_longest_line() {
        let lines = vec!["short".to_owned(), "a much longer line of text".to_owned()];
        let l = hud_layout((0.0, 0.0), &lines, &[], &[]);
        assert!(l.panel.w >= text_px(&lines[1]) + 2.0 * PAD);
        assert!(l.buttons.is_empty());
    }

    #[test]
    fn captures_only_points_over_registered_panels() {
        // Panels are registered by `panel`, which draws. Test the hit rule directly.
        let mut ui = Ui::new((0.0, 0.0), None);
        ui.panels.push(Rect::new(0.0, 0.0, 10.0, 10.0));
        assert!(ui.captures((5.0, 5.0)));
        assert!(!ui.captures((15.0, 5.0)));
    }

    #[test]
    fn rows_layout_aligns_values_and_adds_a_gap_before_sections() {
        let rows = vec![
            Row::Title("T".to_owned()),
            Row::Pair("Pos".to_owned(), "(1, 2)".to_owned()),
            Row::Pair("Carrying".to_owned(), "nothing".to_owned()),
            Row::Section("Skills".to_owned()),
            Row::Line("a line".to_owned()),
        ];
        let l = rows_layout((10.0, 20.0), &rows);
        assert_eq!(l.rows.len(), 5);
        let (_, v1) = l.rows[1];
        let (_, v2) = l.rows[2];
        assert_eq!(v1.map(|p| p.0), v2.map(|p| p.0));
        assert!(l.rows[0].1.is_none() && l.rows[4].1.is_none());
        assert_eq!(l.rows[1].0.1, l.rows[0].0.1 + ROW_H);
        assert_eq!(l.rows[3].0.1, l.rows[2].0.1 + ROW_H + SECTION_GAP);
        let value_end = v2.map_or(0.0, |p| p.0) + text_px("nothing");
        assert!(l.panel.x + l.panel.w >= value_end);
        let last_bottom = l.rows[4].0.1 + ROW_H;
        assert!(l.panel.y + l.panel.h >= last_bottom);
    }

    #[test]
    fn grid_layout_wraps_buttons_and_keeps_them_inside_the_panel() {
        let labels: Vec<String> = ["All", "Log", "Granite", "Sandstone", "Iron ore"]
            .iter()
            .map(|s| (*s).to_owned())
            .collect();
        let l = grid_layout((0.0, 0.0), "Accepts", &labels, 200.0);
        assert_eq!(l.buttons.len(), 5);
        assert!(l.buttons[4].y > l.buttons[0].y, "the row wrapped");
        for r in &l.buttons {
            assert!(r.x >= l.panel.x && r.x + r.w <= l.panel.x + l.panel.w);
            assert!(r.y + r.h <= l.panel.y + l.panel.h);
        }
        let empty = grid_layout((0.0, 0.0), "Accepts", &[], 200.0);
        assert!(empty.buttons.is_empty());
    }

    #[test]
    fn work_layout_has_one_row_and_one_cell_for_each_colonist_and_job_type() {
        let names: Vec<String> = (0..3).map(|i| format!("Colonist {i}")).collect();
        let l = work_layout((8.0, 100.0), &names, "hint");
        assert_eq!(l.names.len(), 3);
        assert_eq!(l.headers.len(), JobKind::COUNT);
        for (row, cells) in l.cells.iter().enumerate() {
            assert_eq!(cells.len(), JobKind::COUNT);
            for c in cells {
                assert_eq!(c.y, l.names[row].y);
                assert!(c.x + c.w <= l.panel.x + l.panel.w);
                assert!(c.y + c.h <= l.panel.y + l.panel.h);
            }
        }
        assert!(l.names[1].y > l.names[0].y);
        assert_eq!(l.cells[0][1].x, l.cells[0][0].x + CELL_W + GAP);
        assert!(l.names[0].x + l.names[0].w < l.cells[0][0].x);
    }

    #[test]
    fn a_column_toolbar_stacks_equal_buttons_inside_its_panel() {
        let l = column_layout((8.0, 100.0), &["Select (V)", "Remove (X)", "Work (P)"]);
        assert_eq!(l.buttons.len(), 3);
        assert!(l.buttons.iter().all(|b| b.w == l.buttons[0].w));
        assert_eq!(l.buttons[1].y, l.buttons[0].y + BUTTON_H + GAP);
        for b in &l.buttons {
            assert!(b.x >= l.panel.x && b.right() <= l.panel.right());
            assert!(b.bottom() <= l.panel.bottom());
        }
        assert!(column_layout((0.0, 0.0), &[]).panel.h == 0.0);
    }

    #[test]
    fn rects_that_only_touch_do_not_intersect() {
        let a = Rect::new(0.0, 0.0, 10.0, 10.0);
        assert!(!a.intersects(&Rect::new(10.0, 0.0, 5.0, 5.0)));
        assert!(!a.intersects(&Rect::new(0.0, 10.0, 5.0, 5.0)));
        assert!(a.intersects(&Rect::new(9.0, 9.0, 5.0, 5.0)));
        assert!(a.intersects(&Rect::new(2.0, 2.0, 1.0, 1.0)));
    }

    #[test]
    fn build_layout_wraps_both_button_groups_inside_the_panel() {
        let to_strings =
            |list: &[&str]| -> Vec<String> { list.iter().map(|s| (*s).to_owned()).collect() };
        let buildings = to_strings(&["Wall", "Door", "Floor", "Roof", "Bed", "Hearth", "Workshop"]);
        let materials = to_strings(&["Granite 3", "Limestone 0", "Log 12"]);
        let lines = to_strings(&["Drag a line", "4 x Log, 3.0 h"]);
        let l = build_layout(
            (10.0, 10.0),
            "Build",
            &buildings,
            "Material",
            &materials,
            &lines,
            300.0,
        );
        assert_eq!(l.buildings.len(), 7);
        assert_eq!(l.materials.len(), 3);
        assert!(l.buildings[6].y > l.buildings[0].y, "the buildings wrap");
        assert!(l.material_header.1 > l.buildings[6].bottom());
        assert!(l.materials[0].y > l.material_header.1);
        assert!(l.lines[0].1 >= l.materials[0].bottom());
        for r in l.buildings.iter().chain(&l.materials) {
            assert!(r.x >= l.panel.x && r.right() <= l.panel.right());
            assert!(r.bottom() <= l.panel.bottom());
        }
        assert!(l.lines[1].1 + ROW_H <= l.panel.bottom() + 2.0);
    }

    #[test]
    fn build_layout_without_materials_keeps_the_lines_below_the_header() {
        let l = build_layout(
            (0.0, 0.0),
            "Build",
            &["Wall".to_owned()],
            "Material",
            &[],
            &["x".to_owned()],
            300.0,
        );
        assert!(l.materials.is_empty());
        assert!(l.lines[0].1 >= l.material_header.1 + ROW_H);
    }

    #[test]
    fn workshop_layout_gives_each_recipe_a_text_and_four_buttons() {
        let recipes = vec![
            ("Make planks".to_owned(), "1 Log to 2 Plank".to_owned()),
            ("Make firewood (ordered 3)".to_owned(), "1 Log".to_owned()),
        ];
        let l = workshop_layout((0.0, 0.0), "Workshop", &["Bin: empty".to_owned()], &recipes);
        assert_eq!(l.recipes.len(), 2);
        assert!(l.recipes[1].label.1 > l.recipes[0].buttons[0].bottom());
        let row = &l.recipes[0];
        assert!(row.detail.1 > row.label.1 && row.buttons[0].y > row.detail.1);
        for pair in row.buttons.windows(2) {
            assert!(pair[1].x > pair[0].right());
        }
        for r in l.recipes.iter().flat_map(|r| r.buttons.iter()) {
            assert!(r.right() <= l.panel.right() && r.bottom() <= l.panel.bottom());
        }
    }

    #[test]
    fn time_text_shows_year_season_day_and_clock() {
        assert_eq!(
            format_time(&GameTime::from_tick(0)),
            "Year 1, Spring, day 1, 00:00"
        );
        // Day 2 (tick 2400), hour 7, half an hour in.
        assert_eq!(
            format_time(&GameTime::from_tick(2_400 + 750)),
            "Year 1, Spring, day 2, 07:30"
        );
        assert_eq!(
            format_time(&GameTime::from_tick(96_000 + 10 * 2_400)),
            "Year 2, Summer, day 1, 00:00"
        );
    }
}
