//! Small immediate-mode interface: framed panels, icon buttons, chips, folds, tooltips, and the
//! layouts of the panels. It reads game state and returns `UiAction` values. It never changes
//! the `Sim` (AGENTS.md code rules). Layout is pure math, so tests run without a window.
//!
//! All sizes are multiples of `PX`, one font pixel (2 screen pixels), so that frames, text, and
//! icons sit on one pixel grid.

use crate::font;
use crate::icons;
use crate::palette::ui as pal;
use crate::render::color as rgb_color;
use crate::tools::Tool;
use macroquad::prelude::{Color, draw_rectangle};
use sim::time::{GameTime, Season};
use sim::{ColonistId, Command, JobKind, Speed};
use std::cell::RefCell;

/// Screen pixels per font pixel.
pub const SCALE: u32 = 2;
/// Padding inside a panel, in screen pixels.
pub const PAD: f32 = 10.0;
/// Gap between buttons, in screen pixels.
pub const GAP: f32 = 4.0;
/// Height of a text line.
pub const LINE_H: f32 = (font::GLYPH_H * SCALE) as f32 + 4.0;
/// Height of a text button and of a chip.
pub const BUTTON_H: f32 = (font::GLYPH_H * SCALE) as f32 + 12.0;
/// Horizontal padding inside a button.
const BUTTON_PAD: f32 = 10.0;
/// Height of a row in a key and value panel.
pub const ROW_H: f32 = (font::GLYPH_H * SCALE) as f32 + 4.0;
/// Height of a fold header row.
const FOLD_H: f32 = ROW_H + 4.0;
/// Extra space above a section heading and a fold.
const SECTION_GAP: f32 = 6.0;
/// Space between the key column and the value column.
const KEY_GAP: f32 = 24.0;
/// Width of a priority cell in the work panel.
pub const CELL_W: f32 = 48.0;
/// Height of a row in the work panel.
pub const WORK_ROW_H: f32 = BUTTON_H - 2.0;
/// Height of the title strip of a panel.
pub const TITLE_H: f32 = 36.0;
/// Side of the close button.
pub const CLOSE_W: f32 = 28.0;
/// Side of a square icon button.
pub const ICON_BUTTON: f32 = 44.0;
/// Size of an icon cell on the screen: 2 screen pixels for each icon pixel.
pub const ICON_PX: f32 = 32.0;
/// Width that a fold chevron takes left of the fold label.
const FOLD_INDENT: f32 = 20.0;

/// Size of one interface pixel in screen pixels: a font pixel. Frames use the same grid.
const PX: f32 = SCALE as f32;

fn text_color() -> Color {
    rgb_color(pal::TEXT, 255)
}

/// Title text color.
pub fn accent() -> Color {
    rgb_color(pal::ACCENT, 255)
}

/// Section heading and secondary text color.
pub fn dim() -> Color {
    rgb_color(pal::DIM, 255)
}

fn key_color() -> Color {
    rgb_color(pal::KEY, 255)
}

/// Draw a box with a hard outline, rounded corners (one pixel cut), and a bevel: light on
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

/// A part of a panel that the player can fold: it shows its rows or only its heading.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Fold {
    /// The skills of the colonist panel.
    Skills,
    /// The work speeds of the colonist panel (a why chain).
    WorkSpeed,
    /// The why chains of the room part of the tile panel.
    Details,
    /// The named parts of the temperature of the tile panel (a why chain).
    TempChain,
    /// The modifiers of the mood of the colonist panel (a why chain).
    Mood,
    /// The hourly health change of the colonist panel (a why chain).
    Health,
    /// The parts of the comfort band of the colonist panel (a why chain).
    Comfort,
    /// The parts of the food stock and use of the colony panel (a why chain).
    FoodParts,
    /// The parts of the fuel stock and use of the colony panel (a why chain).
    FuelParts,
    /// The abandon row of the colony panel. Open means that the next click abandons.
    Abandon,
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
    /// The player pressed the temperature overlay button.
    ToggleTemp,
    /// The player pressed the colony panel button, or clicked a ledger chip.
    ToggleColony,
    /// The player chose a crop in the farm panel (index in `farm.ron`).
    PickCrop(u16),
    /// The player pressed the button to start a new colony after the run ended.
    NewColony,
    /// The player closed the pause menu.
    Resume,
    /// The player chose to quit in the pause menu.
    Quit,
    /// The player clicked an alert: center the camera on this tile.
    JumpTo(u16, u16),
    /// The player clicked a colonist name in the work panel.
    SelectColonist(ColonistId),
    /// The player chose a building type in the build panel (index in `buildings.ron`).
    PickBuilding(usize),
    /// The player chose a material in the build panel (material index).
    PickMaterial(u16),
    /// The player opened or closed a fold.
    ToggleFold(Fold),
    /// The player closed the colonist panel.
    CloseColonist,
    /// The player closed the tile panel and the panels that belong to it.
    CloseTile,
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
    /// A heading that the player can click to show or hide the rows below it. The flag is
    /// true if the rows show.
    Fold(String, bool),
    /// A need as a small icon, a bar, and a value. With `fold` set, the player can click the
    /// row to show or hide the `Line` rows below it.
    Meter(Meter),
    /// A short label with an icon, for example a condition.
    Tag(usize, String, [u8; 3]),
}

/// A bar row of a key and value panel.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Meter {
    /// Icon cell.
    pub icon: usize,
    /// Tooltip: the name of the value.
    pub tip: String,
    /// Fill of the bar, from 0 to 1000.
    pub permille: i32,
    /// Color of the bar.
    pub tint: [u8; 3],
    /// The value as text.
    pub text: String,
    /// `Some(open)` if the row folds the lines below it.
    pub fold: Option<bool>,
}

/// Size of a small icon in a row, in screen pixels (one icon pixel is one screen pixel).
pub const SMALL_ICON: f32 = 16.0;
/// Width of the bar of a meter row.
pub const BAR_W: f32 = 96.0;
/// Height of the bar of a meter row.
pub const BAR_H: f32 = 10.0;

impl Row {
    /// `Some(open)` if the row is a fold header that the player can click.
    pub fn fold_state(&self) -> Option<bool> {
        match self {
            Row::Fold(_, open) => Some(*open),
            Row::Meter(m) => m.fold,
            _ => None,
        }
    }
}

/// Width of a meter row.
fn meter_w(m: &Meter) -> f32 {
    FOLD_INDENT + SMALL_ICON + GAP + BAR_W + 8.0 + text_px(&m.text)
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

/// The tooltip of the frame: the rectangle of the widget and the text.
type Tip = Option<(Rect, String)>;

/// Immediate-mode UI for one frame.
pub struct Ui {
    mouse: (f32, f32),
    click: Option<(f32, f32)>,
    right_click: Option<(f32, f32)>,
    panels: Vec<Rect>,
    tip: RefCell<Tip>,
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
            tip: RefCell::new(None),
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

    /// Draw a panel background: a soft shadow and a frame with a bevel.
    pub fn panel(&mut self, r: Rect) {
        let ink = rgb_color(pal::INK, 255);
        draw_rectangle(
            r.x + 2.0 * PX,
            r.y + 2.0 * PX,
            r.w,
            r.h,
            Color::new(ink.r, ink.g, ink.b, 0.38),
        );
        draw_frame(
            r,
            rgb_color(pal::PANEL, 246),
            rgb_color(pal::PANEL_LIGHT, 255),
            rgb_color(pal::PANEL_DARK, 255),
            ink,
            false,
        );
        self.panels.push(r);
    }

    /// Draw a panel with a title strip and a close button. Returns true if the player
    /// clicked the close button in this frame.
    pub fn titled_panel(&mut self, r: Rect, title: &str, close: Option<Rect>) -> bool {
        self.panel(r);
        let bar = Rect::new(
            r.x + 2.0 * PX,
            r.y + 2.0 * PX,
            r.w - 4.0 * PX,
            TITLE_H - 2.0 * PX,
        );
        draw_rectangle(bar.x, bar.y, bar.w, bar.h, rgb_color(pal::HEADER, 255));
        draw_rectangle(
            bar.x,
            bar.bottom(),
            bar.w,
            PX,
            rgb_color(pal::PANEL_DARK, 255),
        );
        let ty = r.y + (TITLE_H - (font::GLYPH_H * SCALE) as f32) / 2.0;
        self.label_color((r.x + PAD, ty.round()), title, accent());
        close.is_some_and(|c| self.close_button(c))
    }

    /// Draw the bar at the top of the window. It has a shadow at the bottom edge.
    pub fn bar(&mut self, r: Rect) {
        let ink = rgb_color(pal::INK, 255);
        draw_rectangle(
            r.x,
            r.bottom(),
            r.w,
            2.0 * PX,
            Color::new(ink.r, ink.g, ink.b, 0.38),
        );
        draw_rectangle(r.x, r.y, r.w, r.h, rgb_color(pal::PANEL, 250));
        draw_rectangle(
            r.x,
            r.bottom() - 3.0 * PX,
            r.w,
            PX,
            rgb_color(pal::PANEL_LIGHT, 255),
        );
        draw_rectangle(r.x, r.bottom() - 2.0 * PX, r.w, 2.0 * PX, ink);
        self.panels.push(r);
    }

    /// Draw a thin dividing line.
    pub fn rule(&self, x: f32, y: f32, w: f32) {
        draw_rectangle(x, y, w, PX, rgb_color(pal::PANEL_DARK, 255));
        draw_rectangle(x, y + PX, w, PX, rgb_color(pal::PANEL_LIGHT, 120));
    }

    /// Draw a text label at a top left corner.
    pub fn label(&self, pos: (f32, f32), text: &str) {
        shadow_text(text, pos.0, pos.1, text_color());
    }

    /// Draw a text label in a color.
    pub fn label_color(&self, pos: (f32, f32), text: &str, color: Color) {
        shadow_text(text, pos.0, pos.1, color);
    }

    /// Draw an icon in a square at a position. `size` is a multiple of 16.
    pub fn icon(&self, cell: usize, pos: (f32, f32), size: f32, color: Color) {
        icons::draw(cell, pos.0, pos.1, size, color);
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

    fn clicked(&self, r: Rect) -> bool {
        self.click.is_some_and(|c| r.contains(c))
    }

    /// Remember a tooltip for a widget. The mouse must be over it.
    pub fn tip(&self, r: Rect, text: &str) {
        if r.contains(self.mouse) && !text.is_empty() {
            *self.tip.borrow_mut() = Some((r, text.to_owned()));
        }
    }

    /// Draw a button with a text. Returns true if the player clicked it in this frame.
    pub fn button(&self, r: Rect, text: &str, active: bool) -> bool {
        self.button_frame(r, self.state_color(r, active), active);
        let drop = f32::from(u8::from(active));
        let tx = r.x + (r.w - text_px(text)) / 2.0;
        let ty = r.y + (r.h - (font::GLYPH_H * SCALE) as f32) / 2.0 + drop;
        shadow_text(text, tx.round(), ty.round(), text_color());
        self.clicked(r)
    }

    /// Draw a button with an icon and no text. The tooltip shows when the mouse is over it.
    /// Returns true if the player clicked it in this frame.
    pub fn icon_button(&self, r: Rect, icon: usize, active: bool, tip: &str) -> bool {
        self.button_frame(r, self.state_color(r, active), active);
        let drop = f32::from(u8::from(active)) * PX;
        let color = if active || r.contains(self.mouse) {
            text_color()
        } else {
            key_color()
        };
        let pos = (
            r.x + (r.w - ICON_PX) / 2.0,
            r.y + (r.h - ICON_PX) / 2.0 + drop,
        );
        self.icon(icon, pos, ICON_PX, color);
        self.tip(r, tip);
        self.clicked(r)
    }

    /// Draw a chip: a button with a small color swatch and a text.
    pub fn chip(&self, r: Rect, text: &str, swatch: Option<Color>, active: bool) -> bool {
        self.button_frame(r, self.state_color(r, active), active);
        let drop = f32::from(u8::from(active));
        let mut tx = r.x + BUTTON_PAD;
        if let Some(c) = swatch {
            let s = 4.0 * PX;
            let sy = (r.y + (r.h - s) / 2.0 + drop).round();
            draw_rectangle(
                tx - PX,
                sy - PX,
                s + 2.0 * PX,
                s + 2.0 * PX,
                rgb_color(pal::INK, 255),
            );
            draw_rectangle(tx, sy, s, s, c);
            tx += s + GAP + PX;
        }
        let ty = r.y + (r.h - (font::GLYPH_H * SCALE) as f32) / 2.0 + drop;
        shadow_text(text, tx.round(), ty.round(), text_color());
        self.clicked(r)
    }

    /// Width of an alert chip with a text.
    pub fn alert_chip_width(text: &str) -> f32 {
        ICON_PX + 4.0 + text_px(text) + BUTTON_PAD
    }

    /// Draw an alert chip: a colored icon and a short text on a button. It counts as a panel,
    /// so a click on it does not reach the map. Returns true if clicked in this frame.
    pub fn alert_chip(&mut self, r: Rect, icon: usize, tint: Color, text: &str) -> bool {
        self.panels.push(r);
        self.button_frame(r, self.state_color(r, false), false);
        self.icon(
            icon,
            (r.x + 2.0, r.y + (r.h - ICON_PX) / 2.0),
            ICON_PX,
            tint,
        );
        let ty = r.y + (r.h - (font::GLYPH_H * SCALE) as f32) / 2.0;
        shadow_text(
            text,
            (r.x + ICON_PX + 4.0).round(),
            ty.round(),
            text_color(),
        );
        self.clicked(r)
    }

    /// Draw the close button of a panel. Returns true if clicked in this frame.
    pub fn close_button(&self, r: Rect) -> bool {
        let hover = r.contains(self.mouse);
        if hover {
            self.button_frame(r, rgb_color(pal::BUTTON_HOVER, 255), false);
        }
        let color = if hover { text_color() } else { dim() };
        let pos = (r.x + (r.w - ICON_PX) / 2.0, r.y + (r.h - ICON_PX) / 2.0);
        self.icon(crate::cells::icons::CLOSE, pos, ICON_PX, color);
        self.tip(r, "Close");
        self.clicked(r)
    }

    /// Draw a fold heading: a chevron and a label. Returns true if clicked in this frame.
    pub fn fold_header(&self, r: Rect, label: &str, open: bool) -> bool {
        let hover = r.contains(self.mouse);
        let cell = if open {
            crate::cells::icons::CHEVRON_DOWN
        } else {
            crate::cells::icons::CHEVRON_RIGHT
        };
        let color = if hover { text_color() } else { dim() };
        self.icon(
            cell,
            (r.x - 8.0, r.y + (r.h - ICON_PX) / 2.0),
            ICON_PX,
            color,
        );
        let ty = r.y + (r.h - (font::GLYPH_H * SCALE) as f32) / 2.0;
        self.label_color((r.x + FOLD_INDENT, ty.round()), label, color);
        self.clicked(r)
    }

    /// Draw the chevron of a fold meter row and report a click on the row. The tooltip of the
    /// row is set by `meter`.
    pub fn fold_marker(&self, r: Rect, open: bool) -> bool {
        let cell = if open {
            crate::cells::icons::CHEVRON_DOWN
        } else {
            crate::cells::icons::CHEVRON_RIGHT
        };
        let color = if r.contains(self.mouse) {
            text_color()
        } else {
            dim()
        };
        self.icon(
            cell,
            (r.x - 8.0, r.y + (r.h - ICON_PX) / 2.0),
            ICON_PX,
            color,
        );
        self.clicked(r)
    }

    /// Draw a meter row: a small icon, a bar, and the value.
    pub fn meter(&self, r: Rect, m: &Meter) {
        let ix = r.x + FOLD_INDENT;
        let tint = rgb_color(m.tint, 255);
        self.icon(
            m.icon,
            (ix, r.y + (r.h - SMALL_ICON) / 2.0),
            SMALL_ICON,
            tint,
        );
        let bx = (ix + SMALL_ICON + GAP).round();
        let by = (r.y + (r.h - BAR_H) / 2.0).round();
        let ink = rgb_color(pal::INK, 255);
        draw_rectangle(bx - PX, by - PX, BAR_W + 2.0 * PX, BAR_H + 2.0 * PX, ink);
        draw_rectangle(bx, by, BAR_W, BAR_H, rgb_color(pal::PANEL_DARK, 255));
        let fill = (BAR_W * m.permille.clamp(0, 1000) as f32 / 1000.0).round();
        if fill > 0.0 {
            draw_rectangle(bx, by, fill, BAR_H, tint);
            draw_rectangle(bx, by, fill, PX, lighten(tint, 0.3));
        }
        let ty = r.y + (r.h - (font::GLYPH_H * SCALE) as f32) / 2.0;
        shadow_text(
            &m.text,
            (bx + BAR_W + 8.0).round(),
            ty.round(),
            text_color(),
        );
        self.tip(Rect::new(r.x, r.y, meter_w(m), r.h), &m.tip);
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
            left: self.clicked(r),
            right: self.right_click.is_some_and(|c| r.contains(c)),
        }
    }

    /// Draw the tooltip of the frame, if any. Call it last, so that it lies over all panels.
    pub fn finish(&self, view: (f32, f32)) {
        let Some((r, text)) = self.tip.borrow_mut().take() else {
            return;
        };
        let tip = tooltip_rect(r, &text, view);
        draw_frame(
            tip,
            rgb_color(pal::INK, 250),
            rgb_color(pal::PANEL_LIGHT, 255),
            rgb_color(pal::PANEL_DARK, 255),
            rgb_color(pal::INK, 255),
            false,
        );
        let lines = text.lines().count().max(1);
        let ty = tip.y + (tip.h - lines as f32 * LINE_H + 4.0) / 2.0;
        for (i, line) in text.lines().enumerate() {
            let color = if i == 0 { text_color() } else { key_color() };
            shadow_text(
                line,
                (tip.x + 8.0).round(),
                (ty + i as f32 * LINE_H).round(),
                color,
            );
        }
    }
}

/// Where the tooltip of a widget goes: above it, or below it if there is no room above, and
/// always inside the window. A text with several lines (separated by a line break) makes a
/// taller tooltip.
pub fn tooltip_rect(target: Rect, text: &str, view: (f32, f32)) -> Rect {
    let w = text.lines().map(text_px).fold(0.0_f32, f32::max) + 16.0;
    let lines = text.lines().count().max(1);
    let h = if lines == 1 {
        28.0
    } else {
        lines as f32 * LINE_H + 8.0
    };
    let above = target.y - h - GAP;
    let y = if above >= 0.0 {
        above
    } else {
        target.bottom() + GAP
    };
    let x = (target.x + (target.w - w) / 2.0).clamp(0.0, (view.0 - w).max(0.0));
    Rect::new(x.round(), y.round(), w, h)
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
    /// The close button, if the panel has one.
    pub close: Option<Rect>,
    /// The clickable rectangle of each fold row, in row order.
    pub folds: Vec<Rect>,
}

/// Lay out a key and value panel. Keys line up in one column, values in the next. A `Title`
/// row goes in the title strip. A panel with `closable` has a close button in the strip.
pub fn rows_layout(origin: (f32, f32), rows: &[Row], closable: bool) -> RowsLayout {
    let key_w = rows
        .iter()
        .filter_map(|r| match r {
            Row::Pair(k, _) => Some(text_px(k)),
            _ => None,
        })
        .fold(0.0_f32, f32::max);
    let close_w = if closable { CLOSE_W + GAP } else { 0.0 };
    let inner_w = rows
        .iter()
        .map(|r| match r {
            Row::Title(t) => text_px(t) + close_w,
            Row::Section(t) | Row::Line(t) => text_px(t),
            Row::Fold(t, _) => FOLD_INDENT + text_px(t),
            Row::Meter(m) => meter_w(m),
            Row::Tag(_, t, _) => FOLD_INDENT + SMALL_ICON + GAP + text_px(t),
            Row::Pair(_, v) => key_w + KEY_GAP + text_px(v),
        })
        .fold(0.0_f32, f32::max);
    let inner_w = inner_w.max(if closable { 220.0 } else { 0.0 });
    let panel_w = inner_w + 2.0 * PAD;
    let x = origin.0 + PAD;
    let mut y = origin.1 + PAD;
    let mut out = Vec::new();
    let mut folds = Vec::new();
    let mut close = None;
    for (i, row) in rows.iter().enumerate() {
        if matches!(row, Row::Title(_)) {
            out.push(((x, origin.1), None));
            y = origin.1 + TITLE_H + PAD / 2.0;
            if closable {
                close = Some(close_rect(origin, panel_w));
            }
            continue;
        }
        if matches!(row, Row::Section(_) | Row::Fold(..)) && i > 0 {
            y += SECTION_GAP;
        }
        let value = matches!(row, Row::Pair(..)).then_some((x + key_w + KEY_GAP, y));
        out.push(((x, y), value));
        if matches!(row, Row::Fold(..)) {
            folds.push(Rect::new(x, y, inner_w, FOLD_H));
            y += FOLD_H;
        } else if matches!(row, Row::Meter(_)) {
            if row.fold_state().is_some() {
                folds.push(Rect::new(x, y, inner_w, ROW_H));
            }
            y += ROW_H;
        } else {
            y += ROW_H;
        }
    }
    RowsLayout {
        panel: Rect::new(origin.0, origin.1, panel_w, y - origin.1 + PAD - 2.0),
        rows: out,
        close,
        folds,
    }
}

/// What the player did in a key and value panel in this frame.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RowsHit {
    /// The index (among the fold rows) of the fold that the player clicked.
    pub fold: Option<usize>,
    /// True if the player clicked the close button.
    pub closed: bool,
}

/// Draw a key and value panel. Returns its rectangle and the clicks of this frame.
pub fn rows_panel(
    ui: &mut Ui,
    origin: (f32, f32),
    rows: &[Row],
    closable: bool,
) -> (Rect, RowsHit) {
    let layout = rows_layout(origin, rows, closable);
    let title = rows.iter().find_map(|r| match r {
        Row::Title(t) => Some(t.as_str()),
        _ => None,
    });
    let mut hit = RowsHit::default();
    match title {
        Some(t) => hit.closed = ui.titled_panel(layout.panel, t, layout.close),
        None => ui.panel(layout.panel),
    }
    let mut fold_index = 0;
    for (row, (pos, value)) in rows.iter().zip(&layout.rows) {
        match row {
            Row::Title(_) => {}
            Row::Section(t) => ui.label_color(*pos, t, dim()),
            Row::Line(t) => ui.label_color(*pos, t, key_color()),
            Row::Pair(k, v) => {
                ui.label_color(*pos, k, key_color());
                if let Some(vp) = value {
                    ui.label(*vp, v);
                }
            }
            Row::Fold(t, open) => {
                if let Some(r) = layout.folds.get(fold_index)
                    && ui.fold_header(*r, t, *open)
                {
                    hit.fold = Some(fold_index);
                }
                fold_index += 1;
            }
            Row::Meter(m) => {
                let r = Rect::new(pos.0, pos.1, layout.panel.w - 2.0 * PAD, ROW_H);
                ui.meter(r, m);
                if let Some(open) = m.fold {
                    if let Some(f) = layout.folds.get(fold_index)
                        && ui.fold_marker(*f, open)
                    {
                        hit.fold = Some(fold_index);
                    }
                    fold_index += 1;
                }
            }
            Row::Tag(icon, t, tint) => {
                ui.icon(
                    *icon,
                    (pos.0 + FOLD_INDENT, pos.1 + (ROW_H - SMALL_ICON) / 2.0),
                    SMALL_ICON,
                    rgb_color(*tint, 255),
                );
                ui.label_color(
                    (pos.0 + FOLD_INDENT + SMALL_ICON + GAP, pos.1),
                    t,
                    key_color(),
                );
            }
        }
    }
    (layout.panel, hit)
}

/// Lay out chips that wrap at `max_w` pixels. Returns the rectangles, the width that the
/// rows use, and the y below the last row.
pub fn flow_chips(
    x0: f32,
    y0: f32,
    labels: &[String],
    extra_w: f32,
    max_w: f32,
) -> (Vec<Rect>, f32, f32) {
    let (mut x, mut y) = (x0, y0);
    let mut used = 0.0_f32;
    let mut out = Vec::new();
    for label in labels {
        let w = text_px(label) + 2.0 * BUTTON_PAD + extra_w;
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

/// Extra width of a chip with a color swatch.
pub const SWATCH_W: f32 = 4.0 * PX + GAP + PX;

/// Positions of a panel with a title and wrapped chips (the stockpile panel).
#[derive(Clone, Debug, PartialEq)]
pub struct GridLayout {
    /// The panel rectangle.
    pub panel: Rect,
    /// The close button.
    pub close: Rect,
    /// One rectangle for each chip.
    pub buttons: Vec<Rect>,
}

/// Lay out a title strip and chips that wrap at `max_inner_w` pixels.
pub fn grid_layout(
    origin: (f32, f32),
    title: &str,
    labels: &[String],
    max_inner_w: f32,
) -> GridLayout {
    let x0 = origin.0 + PAD;
    let (buttons, used, y) =
        flow_chips(x0, origin.1 + TITLE_H + PAD, labels, SWATCH_W, max_inner_w);
    let inner_w = used.max(text_px(title) + CLOSE_W + 2.0 * GAP);
    let w = inner_w + 2.0 * PAD;
    GridLayout {
        panel: Rect::new(origin.0, origin.1, w, y - origin.1 + PAD),
        close: close_rect(origin, w),
        buttons,
    }
}

/// The close button of a panel with the given origin and width.
pub fn close_rect(origin: (f32, f32), w: f32) -> Rect {
    Rect::new(
        origin.0 + w - CLOSE_W - 6.0,
        origin.1 + (TITLE_H - CLOSE_W) / 2.0,
        CLOSE_W,
        CLOSE_W,
    )
}

/// Positions of the work panel: a table of colonists and job types.
#[derive(Clone, Debug, PartialEq)]
pub struct WorkLayout {
    /// The panel rectangle.
    pub panel: Rect,
    /// The close button.
    pub close: Rect,
    /// Top left corner of the label of each job type column.
    pub headers: Vec<(f32, f32)>,
    /// The colonist name button of each row.
    pub names: Vec<Rect>,
    /// The priority cells of each row, one for each job type.
    pub cells: Vec<Vec<Rect>>,
}

/// Lay out the work panel for colonists with the given names.
pub fn work_layout(origin: (f32, f32), names: &[String]) -> WorkLayout {
    let name_w = names
        .iter()
        .map(|n| text_px(n))
        .fold(text_px("Colonist"), f32::max)
        + 2.0 * BUTTON_PAD;
    let x0 = origin.0 + PAD;
    let table_w = name_w + (CELL_W + GAP) * JobKind::COUNT as f32;
    let inner_w = table_w.max(text_px("Work priorities") + CLOSE_W + 2.0 * GAP);
    let header_y = origin.1 + TITLE_H + PAD;
    let cell_x = |k: usize| x0 + name_w + GAP + k as f32 * (CELL_W + GAP);
    let headers = JobKind::ALL
        .iter()
        .enumerate()
        .map(|(k, kind)| {
            let label: String = kind.name().chars().take(3).collect();
            let w = text_px(&label);
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
    let w = inner_w + 2.0 * PAD;
    WorkLayout {
        panel: Rect::new(origin.0, origin.1, w, y - origin.1 + PAD - 2.0),
        close: close_rect(origin, w),
        headers,
        names: name_rects,
        cells,
    }
}

/// Positions of the build panel.
#[derive(Clone, Debug, PartialEq)]
pub struct BuildLayout {
    /// The panel rectangle.
    pub panel: Rect,
    /// The close button.
    pub close: Rect,
    /// One chip for each building type.
    pub buildings: Vec<Rect>,
    /// Y of the line between the building chips and the material chips.
    pub rule_y: f32,
    /// One chip for each material.
    pub materials: Vec<Rect>,
    /// Top left corner of each text line at the bottom.
    pub lines: Vec<(f32, f32)>,
}

/// Lay out the build panel: a title strip, building chips, a line, material chips, and text
/// lines. The chips wrap at `max_inner_w`.
pub fn build_layout(
    origin: (f32, f32),
    title: &str,
    buildings: &[String],
    materials: &[String],
    lines: &[String],
    max_inner_w: f32,
) -> BuildLayout {
    let x0 = origin.0 + PAD;
    let (b_rects, b_w, y) = flow_chips(x0, origin.1 + TITLE_H + PAD, buildings, 0.0, max_inner_w);
    let rule_y = y + GAP + 2.0;
    let mut y = rule_y + PX * 2.0 + GAP + 2.0;
    let (m_rects, m_w, my) = flow_chips(x0, y, materials, SWATCH_W, max_inner_w);
    if !materials.is_empty() {
        y = my + GAP + 2.0;
    }
    let mut line_pos = Vec::new();
    for _ in lines {
        line_pos.push((x0, y));
        y += ROW_H;
    }
    let text_w = lines
        .iter()
        .map(|l| text_px(l))
        .chain([text_px(title) + CLOSE_W + 2.0 * GAP])
        .fold(0.0_f32, f32::max);
    let inner_w = b_w.max(m_w).max(text_w);
    let w = inner_w + 2.0 * PAD;
    BuildLayout {
        panel: Rect::new(origin.0, origin.1, w, y - origin.1 + PAD - 2.0),
        close: close_rect(origin, w),
        buildings: b_rects,
        rule_y,
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
    /// Buttons: add 1, add 5, add 10, clear, next keep target.
    pub buttons: [Rect; 5],
    /// Y of the line above the recipe.
    pub rule_y: f32,
}

/// Positions of the workshop panel.
#[derive(Clone, Debug, PartialEq)]
pub struct WorkshopLayout {
    /// The panel rectangle.
    pub panel: Rect,
    /// The close button.
    pub close: Rect,
    /// Top left corner of each text line under the title.
    pub lines: Vec<(f32, f32)>,
    /// One row for each recipe.
    pub recipes: Vec<RecipeRow>,
}

/// Labels of the five buttons of a recipe row.
pub const RECIPE_BUTTONS: [&str; 5] = ["+1", "+5", "+10", "Clear", "Keep"];

/// Lay out the workshop panel: a title strip, text lines, and for each recipe a name, a detail
/// line, and a row of buttons.
pub fn workshop_layout(
    origin: (f32, f32),
    title: &str,
    lines: &[String],
    recipe_labels: &[(String, String)],
) -> WorkshopLayout {
    let x0 = origin.0 + PAD;
    let mut y = origin.1 + TITLE_H + PAD / 2.0;
    let mut line_pos = Vec::new();
    for _ in lines {
        line_pos.push((x0, y));
        y += ROW_H;
    }
    let widths: Vec<f32> = RECIPE_BUTTONS
        .iter()
        .map(|l| text_px(l) + 2.0 * BUTTON_PAD)
        .collect();
    let button_row_w = widths.iter().sum::<f32>() + GAP * 4.0;
    let mut recipes = Vec::new();
    for _ in recipe_labels {
        let rule_y = y + GAP;
        y += GAP * 2.0 + PX * 2.0;
        let label_pos = (x0, y);
        y += ROW_H;
        let detail_pos = (x0, y);
        y += ROW_H + GAP;
        let mut bx = x0;
        let mut rects = [Rect::new(0.0, 0.0, 0.0, 0.0); 5];
        for (rect, w) in rects.iter_mut().zip(&widths) {
            *rect = Rect::new(bx, y, *w, BUTTON_H);
            bx += w + GAP;
        }
        y += BUTTON_H;
        recipes.push(RecipeRow {
            label: label_pos,
            detail: detail_pos,
            buttons: rects,
            rule_y,
        });
    }
    let text_w = lines
        .iter()
        .chain(recipe_labels.iter().flat_map(|(a, b)| [a, b]))
        .map(|l| text_px(l))
        .chain([text_px(title) + CLOSE_W + 2.0 * GAP])
        .fold(0.0_f32, f32::max);
    let w = text_w.max(button_row_w) + 2.0 * PAD;
    WorkshopLayout {
        panel: Rect::new(origin.0, origin.1, w, y - origin.1 + PAD),
        close: close_rect(origin, w),
        lines: line_pos,
        recipes,
    }
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
#[cfg(test)]
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

/// The clock text of the top bar, for example `07:30`.
pub fn clock_text(t: &GameTime) -> String {
    format!("{:02}:{:02}", t.hour, t.tick_of_hour * 60 / 100)
}

/// The date text of the top bar, for example `Spring 3, Y1`.
pub fn date_text(t: &GameTime) -> String {
    format!("{} {}, Y{}", season_name(t.season), t.day_of_season, t.year)
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
        let l = rows_layout((10.0, 20.0), &rows, true);
        assert_eq!(l.rows.len(), 5);
        let (_, v1) = l.rows[1];
        let (_, v2) = l.rows[2];
        assert_eq!(v1.map(|p| p.0), v2.map(|p| p.0));
        assert!(l.rows[0].1.is_none() && l.rows[4].1.is_none());
        assert_eq!(l.rows[2].0.1, l.rows[1].0.1 + ROW_H);
        assert_eq!(l.rows[3].0.1, l.rows[2].0.1 + ROW_H + SECTION_GAP);
        let value_end = v2.map_or(0.0, |p| p.0) + text_px("nothing");
        assert!(l.panel.x + l.panel.w >= value_end);
        let last_bottom = l.rows[4].0.1 + ROW_H;
        assert!(l.panel.y + l.panel.h >= last_bottom);
        let close = l.close.unwrap();
        assert!(close.right() <= l.panel.right() && close.bottom() <= l.rows[1].0.1);
    }

    #[test]
    fn a_fold_row_has_a_click_rectangle_inside_the_panel() {
        let rows = vec![
            Row::Title("T".to_owned()),
            Row::Fold("Skills".to_owned(), false),
            Row::Fold("Speed".to_owned(), true),
            Row::Line("part".to_owned()),
        ];
        let l = rows_layout((0.0, 0.0), &rows, true);
        assert_eq!(l.folds.len(), 2);
        assert!(l.folds[1].y >= l.folds[0].bottom());
        for f in &l.folds {
            assert!(f.x >= l.panel.x && f.right() <= l.panel.right());
            assert!(f.bottom() <= l.panel.bottom());
        }
    }

    #[test]
    fn grid_layout_wraps_chips_and_keeps_them_inside_the_panel() {
        let labels: Vec<String> = ["All", "Log", "Granite", "Sandstone", "Iron ore"]
            .iter()
            .map(|s| (*s).to_owned())
            .collect();
        let l = grid_layout((0.0, 0.0), "Stockpile", &labels, 200.0);
        assert_eq!(l.buttons.len(), 5);
        assert!(l.buttons[4].y > l.buttons[0].y, "the row wrapped");
        for r in &l.buttons {
            assert!(r.x >= l.panel.x && r.x + r.w <= l.panel.x + l.panel.w);
            assert!(r.y + r.h <= l.panel.y + l.panel.h);
        }
        assert!(l.close.bottom() <= l.buttons[0].y);
        let empty = grid_layout((0.0, 0.0), "Stockpile", &[], 200.0);
        assert!(empty.buttons.is_empty());
    }

    #[test]
    fn work_layout_has_one_row_and_one_cell_for_each_colonist_and_job_type() {
        let names: Vec<String> = (0..3).map(|i| format!("Colonist {i}")).collect();
        let l = work_layout((8.0, 100.0), &names);
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
        assert!(l.close.right() <= l.panel.right());
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
    fn build_layout_wraps_both_chip_groups_inside_the_panel() {
        let to_strings =
            |list: &[&str]| -> Vec<String> { list.iter().map(|s| (*s).to_owned()).collect() };
        let buildings = to_strings(&["Wall", "Door", "Floor", "Roof", "Bed", "Hearth", "Workshop"]);
        let materials = to_strings(&["Granite", "Limestone", "Log"]);
        let lines = to_strings(&["Drag a line", "4 x Log, 3.0 h"]);
        let l = build_layout((10.0, 10.0), "Build", &buildings, &materials, &lines, 300.0);
        assert_eq!(l.buildings.len(), 7);
        assert_eq!(l.materials.len(), 3);
        assert!(l.buildings[6].y > l.buildings[0].y, "the buildings wrap");
        assert!(l.rule_y > l.buildings[6].bottom());
        assert!(l.materials[0].y > l.rule_y);
        assert!(l.lines[0].1 >= l.materials[0].bottom());
        for r in l.buildings.iter().chain(&l.materials) {
            assert!(r.x >= l.panel.x && r.right() <= l.panel.right());
            assert!(r.bottom() <= l.panel.bottom());
        }
        assert!(l.lines[1].1 + ROW_H <= l.panel.bottom() + 2.0);
    }

    #[test]
    fn build_layout_without_materials_keeps_the_lines_below_the_rule() {
        let l = build_layout(
            (0.0, 0.0),
            "Build",
            &["Wall".to_owned()],
            &[],
            &["x".to_owned()],
            300.0,
        );
        assert!(l.materials.is_empty());
        assert!(l.lines[0].1 >= l.rule_y);
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
    fn a_tooltip_stays_inside_the_window() {
        let view = (400.0, 300.0);
        let near_top = tooltip_rect(Rect::new(10.0, 4.0, 40.0, 40.0), "Pause (Space)", view);
        assert!(near_top.y >= 0.0 && near_top.x >= 0.0);
        let near_right = tooltip_rect(Rect::new(380.0, 200.0, 40.0, 40.0), "Remove (X)", view);
        assert!(near_right.right() <= view.0);
        let target = Rect::new(100.0, 200.0, 40.0, 40.0);
        assert!(!tooltip_rect(target, "Cut (T)", view).intersects(&target));
    }

    #[test]
    fn a_tooltip_with_several_lines_is_taller_and_as_wide_as_its_longest_line() {
        let view = (800.0, 600.0);
        let target = Rect::new(300.0, 300.0, 40.0, 40.0);
        let one = tooltip_rect(target, "Now", view);
        let three = tooltip_rect(
            target,
            "Now
Tomorrow 3 degrees, rain
In 2 days",
            view,
        );
        assert!(three.h > one.h);
        assert!(three.w > one.w);
        assert_eq!(three.w, text_px("Tomorrow 3 degrees, rain") + 16.0);
        assert!(!three.intersects(&target));
    }

    #[test]
    fn time_text_shows_year_season_day_and_clock() {
        assert_eq!(
            format_time(&GameTime::from_tick(0)),
            "Year 1, Spring, day 1, 00:00"
        );
        assert_eq!(
            format_time(&GameTime::from_tick(2_400 + 750)),
            "Year 1, Spring, day 2, 07:30"
        );
        assert_eq!(
            format_time(&GameTime::from_tick(96_000 + 10 * 2_400)),
            "Year 2, Summer, day 1, 00:00"
        );
    }

    #[test]
    fn the_top_bar_shows_the_clock_and_the_date_apart() {
        let t = GameTime::from_tick(2_400 + 750);
        assert_eq!(clock_text(&t), "07:30");
        assert_eq!(date_text(&t), "Spring 2, Y1");
    }
}
