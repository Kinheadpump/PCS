//! Where the panels go on the screen. Pure math: the sizes of the panels go in, the
//! rectangles come out, and a test checks that no two panels overlap.
//!
//! The top bar spans the window. The toolbar sits at the bottom, in the center. The build panel
//! sits above the toolbar, the work panel at the top left, and the colonist panel, the tile
//! panel, and a side panel (stockpile or workshop) flow from the right edge.
//!
//! Two places belong to the climate display: the temperature slot in the top bar (with the scale
//! of the temperature overlay at its right) and the alerts strip under the top bar. Nothing else
//! uses them.

use crate::ui::{self, Rect};

/// Gap between panels, in screen pixels.
pub const MARGIN: f32 = 8.0;
/// Height of the top bar.
pub const TOP_H: f32 = 56.0;
/// Padding inside the toolbar.
pub const TOOLBAR_PAD: f32 = 8.0;
/// Extra space between the tool buttons and the panel buttons of the toolbar.
pub const TOOLBAR_SPLIT: f32 = 12.0;
/// Width of a speed button in the top bar.
pub const SPEED_W: f32 = 44.0;
/// Height of a button in the top bar.
pub const TOP_BUTTON_H: f32 = 40.0;
/// Width of the temperature slot.
pub const TEMP_W: f32 = 168.0;
/// Size of the scale of the temperature overlay.
pub const SCALE_SIZE: (f32, f32) = (176.0, 28.0);
/// Size of the alerts strip.
pub const ALERTS_SIZE: (f32, f32) = (540.0, 36.0);
/// Space left of the clock in the top bar.
const TOP_EDGE: f32 = 16.0;
/// Space between the groups of the top bar.
const TOP_GAP: f32 = 20.0;
/// Smallest width of the event toast.
const TOAST_MIN_W: f32 = 120.0;

/// The size of a panel.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Size {
    /// Width.
    pub w: f32,
    /// Height.
    pub h: f32,
}

impl Size {
    /// A size.
    pub const fn new(w: f32, h: f32) -> Self {
        Self { w, h }
    }
}

/// The sizes of the panels that the display wants to show now.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Sizes {
    /// Width of the clock and date text at the left of the top bar.
    pub clock_w: f32,
    /// Width of the colonist count at the right of the top bar.
    pub count_w: f32,
    /// The number of buttons of the toolbar, and the number of them that are tools.
    pub toolbar_buttons: (usize, usize),
    /// The work panel, if open.
    pub work: Option<Size>,
    /// The build panel, if the build tool is active.
    pub build: Option<Size>,
    /// The colonist panel, if a colonist is selected.
    pub colonist: Option<Size>,
    /// The tile panel, if a tile is selected.
    pub tile: Option<Size>,
    /// The side panel (stockpile filter or workshop) near the tile panel.
    pub side: Option<Size>,
}

/// The rectangles of the panels. `None` means the panel does not show.
#[derive(Clone, Debug, PartialEq)]
pub struct Placed {
    /// The top bar.
    pub top: Rect,
    /// The speed buttons: pause, 1x, 3x, 6x.
    pub speeds: [Rect; 4],
    /// Top left corner of the clock text.
    pub clock: (f32, f32),
    /// The colonist count at the right of the top bar.
    pub count: Rect,
    /// The temperature readout. The forecast shows as a tooltip.
    pub temp_slot: Rect,
    /// The scale of the temperature overlay, right of the temperature slot.
    pub scale: Rect,
    /// The strip of alerts under the top bar.
    pub alerts_slot: Rect,
    /// The toolbar.
    pub toolbar: Rect,
    /// One rectangle for each toolbar button.
    pub toolbar_buttons: Vec<Rect>,
    /// The place of the event toast, left of the toolbar. `None` if there is no room.
    pub toast: Option<Rect>,
    /// The work panel.
    pub work: Option<Rect>,
    /// The build panel.
    pub build: Option<Rect>,
    /// The colonist panel.
    pub colonist: Option<Rect>,
    /// The tile panel.
    pub tile: Option<Rect>,
    /// The side panel.
    pub side: Option<Rect>,
}

#[cfg(test)]
impl Placed {
    /// All panels that draw, with their names. The reserved slots are not in the list.
    pub fn all(&self) -> Vec<(&'static str, Rect)> {
        let mut out = vec![("top", self.top), ("toolbar", self.toolbar)];
        for (name, rect) in [
            ("toast", self.toast),
            ("work", self.work),
            ("build", self.build),
            ("colonist", self.colonist),
            ("tile", self.tile),
            ("side", self.side),
        ] {
            if let Some(r) = rect {
                out.push((name, r));
            }
        }
        out
    }
}

fn rect(x: f32, y: f32, s: Size) -> Rect {
    Rect::new(x.round(), y.round(), s.w.round(), s.h.round())
}

/// The size of the toolbar for `n` buttons.
pub fn toolbar_size(n: usize) -> Size {
    let n = n as f32;
    Size::new(
        n * ui::ICON_BUTTON
            + (n - 1.0).max(0.0) * ui::GAP
            + TOOLBAR_SPLIT
            + 2.0 * TOOLBAR_PAD
            + 4.0,
        ui::ICON_BUTTON + 2.0 * TOOLBAR_PAD + 4.0,
    )
}

/// The height that the panels have between the top bar and the toolbar.
pub fn panel_height(view: (f32, f32), toolbar_buttons: usize) -> f32 {
    view.1 - MARGIN - toolbar_size(toolbar_buttons).h - MARGIN - (TOP_H + MARGIN)
}

/// Place the panels on a screen of `view` pixels.
pub fn place(view: (f32, f32), s: &Sizes) -> Placed {
    let m = MARGIN;
    let top = Rect::new(0.0, 0.0, view.0, TOP_H);
    let by = 5.0;
    let mut x = TOP_EDGE + s.clock_w + TOP_GAP;
    let mut speeds = [Rect::new(0.0, 0.0, 0.0, 0.0); 4];
    for r in &mut speeds {
        *r = Rect::new(x, by, SPEED_W, TOP_BUTTON_H);
        x += SPEED_W + ui::GAP;
    }
    let temp_slot = Rect::new(x + TOP_GAP - ui::GAP, by, TEMP_W, TOP_BUTTON_H);
    let scale = Rect::new(
        temp_slot.right() + TOP_GAP,
        by + (TOP_BUTTON_H - SCALE_SIZE.1) / 2.0,
        SCALE_SIZE.0,
        SCALE_SIZE.1,
    );
    let count = Rect::new(
        (view.0 - TOP_EDGE - s.count_w).round(),
        by,
        s.count_w,
        TOP_BUTTON_H,
    );
    let alerts_slot = Rect::new(
        ((view.0 - ALERTS_SIZE.0) / 2.0).round(),
        TOP_H + m,
        ALERTS_SIZE.0,
        ALERTS_SIZE.1,
    );

    let tb_size = toolbar_size(s.toolbar_buttons.0);
    let toolbar = rect((view.0 - tb_size.w) / 2.0, view.1 - m - tb_size.h, tb_size);
    let mut toolbar_buttons = Vec::new();
    let mut bx = toolbar.x + TOOLBAR_PAD + 2.0;
    for i in 0..s.toolbar_buttons.0 {
        if i == s.toolbar_buttons.1 {
            bx += TOOLBAR_SPLIT;
        }
        toolbar_buttons.push(Rect::new(
            bx,
            toolbar.y + TOOLBAR_PAD + 2.0,
            ui::ICON_BUTTON,
            ui::ICON_BUTTON,
        ));
        bx += ui::ICON_BUTTON + ui::GAP;
    }
    let toast_w = toolbar.x - 2.0 * m;
    let toast = (toast_w >= TOAST_MIN_W).then(|| Rect::new(m, toolbar.y, toast_w, toolbar.h));

    let top_y = TOP_H + m;
    let limit_bottom = toolbar.y - m;
    // A panel at the top moves down if it would cover the alerts strip.
    let avoid = |r: Rect| {
        if r.intersects(&alerts_slot) {
            Rect::new(r.x, alerts_slot.bottom() + m, r.w, r.h)
        } else {
            r
        }
    };
    let mut taken: Vec<Rect> = Vec::new();
    let try_place = |taken: &mut Vec<Rect>, candidates: &[Rect]| -> Option<Rect> {
        let found = candidates.iter().copied().find(|c| {
            c.x >= m
                && c.right() <= view.0 - m
                && c.bottom() <= limit_bottom
                && !taken.iter().any(|o| o.intersects(c))
        })?;
        taken.push(found);
        Some(found)
    };

    // The build panel sits above the toolbar, at its left edge, or at the window edge.
    let build = s
        .build
        .filter(|b| limit_bottom - b.h >= top_y)
        .and_then(|b| {
            let y = limit_bottom - b.h;
            let at = |x: f32| rect(x, y, b);
            try_place(&mut taken, &[at(toolbar.x), at(m)])
        });
    // The work panel sits at the top left.
    let work = s.work.and_then(|w| {
        let r = avoid(rect(m, top_y, w));
        try_place(&mut taken, &[r])
    });

    // The right column. The colonist panel sits at the right edge.
    let colonist = s.colonist.and_then(|c| {
        let r = avoid(rect(view.0 - m - c.w, top_y, c));
        try_place(&mut taken, &[r])
    });
    let edge = colonist.map_or(view.0 - m, |r| r.x - m);
    let tile = s.tile.and_then(|t| {
        let r = avoid(rect(edge - t.w, top_y, t));
        try_place(&mut taken, &[r])
    });
    let side = match (s.side, tile) {
        (Some(sd), Some(t)) => {
            let below = rect(t.x, t.bottom() + m, sd);
            let beside = avoid(rect(t.x - m - sd.w, top_y, sd));
            try_place(&mut taken, &[below, beside])
        }
        (Some(sd), None) => {
            let r = avoid(rect(edge - sd.w, top_y, sd));
            try_place(&mut taken, &[r])
        }
        _ => None,
    };

    Placed {
        top,
        speeds,
        clock: (TOP_EDGE, 8.0),
        count,
        temp_slot,
        scale,
        alerts_slot,
        toolbar,
        toolbar_buttons,
        toast,
        work,
        build,
        colonist,
        tile,
        side,
    }
}

/// The names of two panels that overlap, if any do. The reserved slots count too.
#[cfg(test)]
pub fn first_overlap(placed: &Placed) -> Option<(&'static str, &'static str)> {
    let mut all = placed.all();
    all.push(("alerts_slot", placed.alerts_slot));
    for (i, (na, a)) in all.iter().enumerate() {
        for (nb, b) in &all[i + 1..] {
            if a.intersects(b) {
                return Some((na, nb));
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sizes() -> Sizes {
        Sizes {
            clock_w: 100.0,
            count_w: 70.0,
            toolbar_buttons: (10, 8),
            ..Sizes::default()
        }
    }

    #[test]
    fn the_top_bar_spans_the_window_and_holds_four_speed_buttons() {
        let p = place((1280.0, 720.0), &sizes());
        assert_eq!((p.top.x, p.top.w), (0.0, 1280.0));
        for pair in p.speeds.windows(2) {
            assert!(pair[1].x > pair[0].right());
        }
        for r in p.speeds.iter().chain([&p.count, &p.temp_slot, &p.scale]) {
            assert!(r.bottom() <= p.top.bottom(), "{r:?}");
        }
        assert!(p.temp_slot.x > p.speeds[3].right(), "the slot is free");
        assert!(p.scale.x >= p.temp_slot.right());
        assert!(
            p.count.x > p.scale.right(),
            "the scale fits left of the count"
        );
    }

    #[test]
    fn the_toolbar_is_centered_at_the_bottom_and_holds_every_button() {
        let p = place((1280.0, 720.0), &sizes());
        assert_eq!(p.toolbar.bottom(), 720.0 - MARGIN);
        let mid = p.toolbar.x + p.toolbar.w / 2.0;
        assert!((mid - 640.0).abs() <= 1.0);
        assert_eq!(p.toolbar_buttons.len(), 10);
        for b in &p.toolbar_buttons {
            assert!(b.x >= p.toolbar.x && b.right() <= p.toolbar.right());
            assert!(b.y >= p.toolbar.y && b.bottom() <= p.toolbar.bottom());
        }
        // The panel buttons are a little apart from the tools.
        let gap = |i: usize| p.toolbar_buttons[i + 1].x - p.toolbar_buttons[i].right();
        assert!(gap(7) > gap(6));
    }

    #[test]
    fn the_alerts_strip_is_reserved_under_the_top_bar_and_stays_free() {
        let mut s = sizes();
        s.work = Some(Size::new(500.0, 300.0));
        s.colonist = Some(Size::new(330.0, 400.0));
        let p = place((1280.0, 720.0), &s);
        assert!(p.alerts_slot.y >= p.top.bottom());
        assert_eq!(first_overlap(&p), None);
        let w = p.work.unwrap();
        assert!(w.y >= p.alerts_slot.bottom(), "the work panel moved down");
    }

    #[test]
    fn the_build_panel_sits_above_the_toolbar() {
        let mut s = sizes();
        s.build = Some(Size::new(420.0, 260.0));
        let p = place((1280.0, 720.0), &s);
        let b = p.build.unwrap();
        assert_eq!(b.bottom(), p.toolbar.y - MARGIN);
        assert_eq!(first_overlap(&p), None);
    }

    #[test]
    fn a_panel_that_does_not_fit_is_left_out_instead_of_overlapping() {
        let mut s = sizes();
        s.work = Some(Size::new(700.0, 300.0));
        s.colonist = Some(Size::new(430.0, 400.0));
        let p = place((1000.0, 720.0), &s);
        assert!(p.work.is_some() && p.colonist.is_none());
        assert_eq!(first_overlap(&p), None);
    }

    #[test]
    fn the_side_panel_goes_under_the_tile_panel_or_beside_it() {
        let mut s = sizes();
        s.tile = Some(Size::new(300.0, 200.0));
        s.side = Some(Size::new(300.0, 200.0));
        let p = place((1280.0, 720.0), &s);
        assert_eq!(p.side.unwrap().y, p.tile.unwrap().bottom() + MARGIN);
        s.tile = Some(Size::new(300.0, 420.0));
        let p = place((1280.0, 720.0), &s);
        assert!(p.side.unwrap().right() <= p.tile.unwrap().x);
        assert_eq!(first_overlap(&p), None);
    }

    #[test]
    fn the_toast_fills_the_space_left_of_the_toolbar() {
        let p = place((1280.0, 720.0), &sizes());
        let t = p.toast.unwrap();
        assert!(t.right() < p.toolbar.x && t.x >= MARGIN);
        let narrow = place((700.0, 600.0), &sizes());
        assert!(narrow.toast.is_none() || narrow.toast.unwrap().right() < narrow.toolbar.x);
    }

    #[test]
    fn panel_height_leaves_room_for_the_bars() {
        let h = panel_height((1280.0, 720.0), 10);
        let p = place((1280.0, 720.0), &sizes());
        assert!(TOP_H + MARGIN + h <= p.toolbar.y - MARGIN + 0.5);
    }
}
