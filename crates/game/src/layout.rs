//! Where the panels go on the screen. Pure math: the sizes of the panels go in, the
//! rectangles come out, and a test checks that no two panels overlap.
//!
//! The left column holds the status panel, the toolbar under it, and the help at the bottom.
//! The build panel and the work panel stack in the free area to the right of the toolbar.
//! The right side holds the colonist panel at the right edge, the tile panel at its left,
//! and a side panel (stockpile filter or workshop) under the tile panel. The tile panel of
//! the hover is optional: it only shows if there is room for it.

use crate::ui::Rect;

/// Gap between panels, in screen pixels.
pub const MARGIN: f32 = 8.0;

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
    /// The status panel.
    pub status: Size,
    /// The toolbar.
    pub toolbar: Size,
    /// The help panel.
    pub help: Size,
    /// The work panel, if open.
    pub work: Option<Size>,
    /// The build panel, if the build tool is active.
    pub build: Option<Size>,
    /// The colonist panel, if a colonist is selected.
    pub colonist: Option<Size>,
    /// The tile panel, if there is a tile to show.
    pub tile: Option<Size>,
    /// True if the tile panel only shows the tile under the mouse. Then it is optional.
    pub tile_is_hover: bool,
    /// The side panel (stockpile filter or workshop) under the tile panel.
    pub side: Option<Size>,
}

/// The rectangles of the panels. `None` means the panel does not show.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placed {
    /// The status panel.
    pub status: Rect,
    /// The toolbar.
    pub toolbar: Rect,
    /// The help panel. It is the first panel to go when a workshop or filter panel has no
    /// other room.
    pub help: Option<Rect>,
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
    /// All placed panels with their names.
    pub fn all(&self) -> Vec<(&'static str, Rect)> {
        let mut out = vec![("status", self.status), ("toolbar", self.toolbar)];
        for (name, rect) in [
            ("help", self.help),
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

/// Place the panels on a screen of `view` pixels.
pub fn place(view: (f32, f32), s: &Sizes) -> Placed {
    let m = MARGIN;
    let status = rect(m, m, s.status);
    let toolbar = rect(m, status.bottom() + m, s.toolbar);
    let help = rect(m, view.1 - s.help.h - m, s.help);

    // Right side. The colonist panel sits at the right edge.
    let colonist = s.colonist.map(|c| rect(view.0 - m - c.w, m, c));
    let mut edge = colonist.map_or(view.0 - m, |r| r.x - m);
    let mut right_side: Vec<Rect> = colonist.into_iter().collect();

    let mut tile = None;
    let mut side = None;
    let mut help_shown = true;
    if let Some(t) = s.tile.filter(|_| !s.tile_is_hover) {
        let r = rect(edge - t.w, m, t);
        edge = r.x;
        tile = Some(r);
        right_side.push(r);
        if let Some(sd) = s.side {
            // Under the tile panel, else at its left. The help gives way if the side panel
            // fits nowhere else. A side panel that fits nowhere does not show.
            let below = rect(r.x, r.bottom() + m, sd);
            let beside = rect(r.x - m - sd.w, m, sd);
            let free = |c: &Rect, with_help: bool| {
                c.x >= 0.0
                    && c.bottom() <= view.1 - m
                    && ![status, toolbar].iter().any(|o| o.intersects(c))
                    && !(with_help && help.intersects(c))
                    && !right_side.iter().any(|o| o.intersects(c))
            };
            let found = [below, beside]
                .into_iter()
                .find(|c| free(c, true))
                .or_else(|| {
                    let c = [below, beside].into_iter().find(|c| free(c, false))?;
                    help_shown = false;
                    Some(c)
                });
            if let Some(r2) = found {
                side = Some(r2);
                right_side.push(r2);
            }
        }
    } else if let Some(sd) = s.side {
        let r = rect(edge - sd.w, m, sd);
        side = Some(r);
        right_side.push(r);
    }

    // The free area to the right of the toolbar.
    let x0 = toolbar.right() + m;
    let y0 = status.bottom() + m;
    let limit_right = right_side
        .iter()
        .map(|r| r.x - m)
        .fold(view.0 - m, f32::min);
    let help_rect = help_shown.then_some(help);
    let limit_bottom = help_rect.map_or(view.1 - m, |h| h.y - m);
    let wanted = [s.build, s.work];
    let mut first: Option<Rect> = None;
    let mut slots: [Option<Rect>; 2] = [None, None];
    for (slot, size) in slots.iter_mut().zip(wanted) {
        let Some(size) = size else { continue };
        let candidate = match first {
            None => rect(x0, y0, size),
            Some(f) => {
                let below = rect(x0, f.bottom() + m, size);
                if below.bottom() <= limit_bottom {
                    below
                } else {
                    rect(f.right() + m, y0, size)
                }
            }
        };
        let fits = candidate.right() <= limit_right
            && candidate.bottom() <= view.1 - m
            && !right_side.iter().any(|r| r.intersects(&candidate))
            && !help_rect.is_some_and(|h| h.intersects(&candidate));
        if fits {
            if first.is_none() {
                first = Some(candidate);
            }
            *slot = Some(candidate);
        }
    }
    let [build, work] = slots;

    // The tile panel of the hover shows only if it overlaps no other panel.
    if let Some(t) = s.tile.filter(|_| s.tile_is_hover) {
        let r = rect(edge - t.w, m, t);
        let others = [Some(status), Some(toolbar), help_rect, build, work, side]
            .into_iter()
            .flatten()
            .chain(right_side.iter().copied());
        let mut clash = r.x < 0.0 || r.bottom() > view.1;
        for o in others {
            clash |= o.intersects(&r);
        }
        if !clash {
            tile = Some(r);
        }
    }

    Placed {
        status,
        toolbar,
        help: help_rect,
        work,
        build,
        colonist,
        tile,
        side,
    }
}

/// The names of two panels that overlap, if any do.
#[cfg(test)]
pub fn first_overlap(placed: &Placed) -> Option<(&'static str, &'static str)> {
    let all = placed.all();
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
            status: Size::new(430.0, 130.0),
            toolbar: Size::new(172.0, 246.0),
            help: Size::new(800.0, 104.0),
            ..Sizes::default()
        }
    }

    #[test]
    fn the_left_column_stacks_status_toolbar_and_help() {
        let p = place((1280.0, 720.0), &sizes());
        assert_eq!((p.status.x, p.status.y), (MARGIN, MARGIN));
        assert_eq!(p.toolbar.y, p.status.bottom() + MARGIN);
        assert_eq!(p.help.unwrap().bottom(), 720.0 - MARGIN);
        assert_eq!(first_overlap(&p), None);
    }

    #[test]
    fn a_colonist_panel_does_not_cover_the_toolbar_or_the_help() {
        let mut s = sizes();
        s.colonist = Some(Size::new(430.0, 640.0));
        let p = place((1280.0, 720.0), &s);
        let c = p.colonist.unwrap();
        assert_eq!(c.right(), 1280.0 - MARGIN);
        assert!(!c.intersects(&p.toolbar) && !c.intersects(&p.help.unwrap()));
        assert_eq!(first_overlap(&p), None);
    }

    #[test]
    fn build_and_work_panels_stack_to_the_right_of_the_toolbar() {
        let mut s = sizes();
        s.build = Some(Size::new(340.0, 210.0));
        s.work = Some(Size::new(512.0, 200.0));
        let p = place((1280.0, 720.0), &s);
        let (b, w) = (p.build.unwrap(), p.work.unwrap());
        assert_eq!(b.x, p.toolbar.right() + MARGIN);
        assert_eq!(w.x, b.x);
        assert!(w.y >= b.bottom() + MARGIN);
        assert_eq!(first_overlap(&p), None);
    }

    #[test]
    fn a_panel_that_does_not_fit_is_left_out_instead_of_overlapping() {
        let mut s = sizes();
        s.colonist = Some(Size::new(430.0, 640.0));
        s.tile = Some(Size::new(300.0, 300.0));
        s.build = Some(Size::new(600.0, 210.0));
        let p = place((1280.0, 720.0), &s);
        assert!(p.build.is_none());
        assert_eq!(first_overlap(&p), None);
    }

    #[test]
    fn a_hover_tile_panel_is_dropped_when_it_would_overlap() {
        let mut s = sizes();
        s.build = Some(Size::new(400.0, 210.0));
        s.tile = Some(Size::new(300.0, 300.0));
        s.tile_is_hover = true;
        let p = place((1280.0, 720.0), &s);
        assert!(p.tile.is_some(), "there is room in a wide window");
        s.tile = Some(Size::new(1000.0, 300.0));
        let p = place((1280.0, 720.0), &s);
        assert!(p.tile.is_none());
        assert_eq!(first_overlap(&p), None);
    }

    #[test]
    fn the_help_gives_way_to_a_side_panel_that_has_no_other_room() {
        let mut s = sizes();
        s.help = Size::new(590.0, 194.0);
        s.colonist = Some(Size::new(436.0, 656.0));
        s.tile = Some(Size::new(376.0, 380.0));
        s.side = Some(Size::new(326.0, 184.0));
        let p = place((1280.0, 720.0), &s);
        assert!(p.side.is_some() && p.help.is_none());
        assert_eq!(first_overlap(&p), None);
    }

    #[test]
    fn the_side_panel_goes_under_the_tile_panel_or_beside_it() {
        let mut s = sizes();
        s.tile = Some(Size::new(300.0, 200.0));
        s.side = Some(Size::new(300.0, 200.0));
        let p = place((1280.0, 720.0), &s);
        assert_eq!(p.side.unwrap().y, p.tile.unwrap().bottom() + MARGIN);
        s.tile = Some(Size::new(300.0, 520.0));
        let p = place((1280.0, 720.0), &s);
        assert!(p.side.unwrap().right() <= p.tile.unwrap().x);
        assert_eq!(first_overlap(&p), None);
    }
}
