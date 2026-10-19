//! Small immediate-mode interface: panel, label, button. It reads game state and
//! returns `UiAction` values. It never changes the `Sim` (AGENTS.md code rules).
//! Layout is pure math, so tests run without a window.

use crate::clock::{SPEEDS, speed_label};
use crate::font;
use macroquad::prelude::{Color, draw_rectangle, draw_rectangle_lines};
use sim::Speed;
use sim::time::{GameTime, Season};

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

const PANEL_BG: Color = Color::new(0.04, 0.05, 0.08, 0.85);
const PANEL_EDGE: Color = Color::new(1.0, 1.0, 1.0, 0.25);
const TEXT: Color = Color::new(0.95, 0.95, 0.95, 1.0);
const BUTTON_BG: Color = Color::new(0.16, 0.18, 0.24, 1.0);
const BUTTON_HOVER: Color = Color::new(0.26, 0.30, 0.40, 1.0);
const BUTTON_ACTIVE: Color = Color::new(0.20, 0.45, 0.30, 1.0);

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
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UiAction {
    /// The player pressed a speed button.
    SetSpeed(Speed),
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
    panels: Vec<Rect>,
}

impl Ui {
    /// A UI for one frame. `click` is the position of a left click in this frame.
    pub fn new(mouse: (f32, f32), click: Option<(f32, f32)>) -> Self {
        Self {
            mouse,
            click,
            panels: Vec::new(),
        }
    }

    /// True if the point is over a panel drawn in this frame.
    pub fn captures(&self, p: (f32, f32)) -> bool {
        self.panels.iter().any(|r| r.contains(p))
    }

    /// Draw a panel background.
    pub fn panel(&mut self, r: Rect) {
        draw_rectangle(r.x, r.y, r.w, r.h, PANEL_BG);
        draw_rectangle_lines(r.x, r.y, r.w, r.h, 1.0, PANEL_EDGE);
        self.panels.push(r);
    }

    /// Draw a text label at a top left corner.
    pub fn label(&self, pos: (f32, f32), text: &str) {
        font::draw_text(text, pos.0, pos.1, SCALE, TEXT);
    }

    /// Draw a button. Returns true if the player clicked it in this frame.
    pub fn button(&self, r: Rect, text: &str, active: bool) -> bool {
        let color = if active {
            BUTTON_ACTIVE
        } else if r.contains(self.mouse) {
            BUTTON_HOVER
        } else {
            BUTTON_BG
        };
        draw_rectangle(r.x, r.y, r.w, r.h, color);
        draw_rectangle_lines(r.x, r.y, r.w, r.h, 1.0, PANEL_EDGE);
        let tx = r.x + (r.w - text_px(text)) / 2.0;
        let ty = r.y + (r.h - (font::GLYPH_H * SCALE) as f32) / 2.0;
        font::draw_text(text, tx.round(), ty.round(), SCALE, TEXT);
        self.click.is_some_and(|c| r.contains(c))
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
