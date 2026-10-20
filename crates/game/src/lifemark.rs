//! Small marks on the map for life and death (R-011 to R-016): a mark over a colonist in need,
//! a health bar for a hurt colonist, and a grave where a colonist died. Cheap and sparse: each
//! colonist has at most one mark, and a colonist that is fine has none. Everything reads `&Sim`.

use crate::actors::position_tiles;
use crate::camera::{Camera, View};
use crate::cells::icons as ic;
use crate::font;
use crate::icons;
use crate::overlay::Frame;
use crate::palette::Rgb;
use crate::render::color as rgb_color;
use macroquad::prelude::{Color, draw_rectangle};
use sim::{Colonist, NeedAct, Sim};

/// Color of the mark of a danger.
const DANGER: Rgb = [244, 96, 88];
/// Color of the mark of a warning.
const WARNING: Rgb = [250, 190, 80];
/// Color of the mark of cold.
const COLD: Rgb = [140, 200, 255];
/// Color of the mark of sleep.
const SLEEP: Rgb = [170, 200, 255];
/// Color of the outline of a mark.
const INK: Rgb = [20, 16, 30];
/// Smallest sprite size in screen pixels at which a mark shows as an icon. Below it, a dot.
const ICON_FROM: f32 = 24.0;
/// A colonist with health below this share of the maximum (percent) gets a health bar.
const HURT_PCT: i32 = 60;

/// What a colonist needs, as one mark.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mark {
    /// Hunger is 0.
    Starving,
    /// Hypothermia.
    Freezing,
    /// Heatstroke.
    Overheated,
    /// A breakdown: the colonist does not work (R-015).
    Break,
    /// Rest is 0.
    Exhausted,
    /// A wound.
    Wounded,
    /// The colonist sleeps.
    Sleeping,
}

impl Mark {
    /// The icon cell, or `None` for the letter `z`.
    pub fn icon(self) -> Option<usize> {
        match self {
            Mark::Starving => Some(ic::FOOD),
            Mark::Freezing => Some(ic::THERMOMETER),
            Mark::Overheated => Some(ic::SUN),
            Mark::Break => Some(ic::MOOD),
            Mark::Exhausted => Some(ic::MOON),
            Mark::Wounded => Some(ic::HEART),
            Mark::Sleeping => None,
        }
    }

    /// The color of the mark.
    pub fn tint(self) -> Rgb {
        match self {
            Mark::Starving | Mark::Overheated | Mark::Exhausted | Mark::Wounded => DANGER,
            Mark::Freezing => COLD,
            Mark::Break => WARNING,
            Mark::Sleeping => SLEEP,
        }
    }
}

/// The mark of a colonist, if it has one. The most urgent need wins.
pub fn mark_of(sim: &Sim, c: &Colonist) -> Option<Mark> {
    if !sim.rules().needs.enabled {
        return None;
    }
    if c.hunger <= 0 {
        Some(Mark::Starving)
    } else if c.hypothermia > 0 {
        Some(Mark::Freezing)
    } else if c.heatstroke > 0 {
        Some(Mark::Overheated)
    } else if c.break_until > sim.tick() {
        Some(Mark::Break)
    } else if c.rest <= 0 {
        Some(Mark::Exhausted)
    } else if c.wounded {
        Some(Mark::Wounded)
    } else if matches!(c.act, Some(NeedAct::Sleep { asleep: true, .. })) {
        Some(Mark::Sleeping)
    } else {
        None
    }
}

/// True if the colonist is hurt enough to get a health bar.
pub fn is_hurt(sim: &Sim, c: &Colonist) -> bool {
    let max = i32::try_from(sim.rules().needs.health_max).unwrap_or(i32::MAX);
    sim.rules().needs.enabled && c.health * 100 < max * HURT_PCT
}

fn color(c: Rgb, alpha: u8) -> Color {
    rgb_color(c, alpha)
}

/// Draw one mark with its middle at `at`.
fn draw_mark(mark: Mark, at: (f32, f32), size: f32) {
    let tint = mark.tint();
    if size < ICON_FROM {
        // A dot with a dark rim.
        let d = 6.0;
        let (x, y) = ((at.0 - d / 2.0).round(), (at.1 - d / 2.0).round());
        draw_rectangle(x - 1.0, y - 1.0, d + 2.0, d + 2.0, color(INK, 230));
        draw_rectangle(x, y, d, d, color(tint, 255));
        return;
    }
    let plate = 20.0;
    let (x, y) = ((at.0 - plate / 2.0).round(), (at.1 - plate / 2.0).round());
    draw_rectangle(x, y, plate, plate, color(INK, 170));
    match mark.icon() {
        Some(cell) => icons::draw(cell, x + 2.0, y + 2.0, 16.0, color(tint, 255)),
        None => {
            let w = font::text_width("z") as f32;
            font::draw_text(
                "z",
                (at.0 - w / 2.0).round(),
                (at.1 - 7.0).round(),
                2,
                color(tint, 255),
            );
        }
    }
}

/// Draw the marks over the colonists and a health bar under a hurt colonist.
pub fn draw_marks(sim: &Sim, camera: &Camera, view: View) {
    let size = camera.zoom.max(12.0);
    for (_, c) in sim.colonists().iter() {
        let p = camera.tile_to_screen(position_tiles(c), view);
        if p.0 < -size || p.1 < -size || p.0 > view.0 + size || p.1 > view.1 + size {
            continue;
        }
        if let Some(mark) = mark_of(sim, c) {
            draw_mark(mark, (p.0 + size * 0.34, p.1 - size * 0.58), size);
        }
        if is_hurt(sim, c) {
            let max = sim.rules().needs.health_max.max(1) as f32;
            let share = (c.health.max(0) as f32 / max).clamp(0.0, 1.0);
            let (w, h) = ((size * 0.7).round(), 3.0);
            let (x, y) = ((p.0 - w / 2.0).round(), (p.1 + size * 0.46).round());
            draw_rectangle(x - 1.0, y - 1.0, w + 2.0, h + 2.0, color(INK, 220));
            let tint = if share < 0.3 { DANGER } else { WARNING };
            draw_rectangle(x, y, (w * share).round(), h, color(tint, 255));
        }
    }
}

/// Draw a small gravestone on each tile where a colonist died.
pub fn draw_graves(f: &Frame) {
    let Some(v) = f.visible() else { return };
    for grave in f.sim.graveyard() {
        if !f.is_visible(&v, grave.x, grave.y) {
            continue;
        }
        let (x, y, w, _) = f.rect(grave.x, grave.y);
        let u = (w / 16.0).max(1.0);
        let at = |px: f32, py: f32, pw: f32, ph: f32, c: Color| {
            draw_rectangle(
                (x + px * u).round(),
                (y + py * u).round(),
                pw * u,
                ph * u,
                c,
            );
        };
        at(3.0, 12.0, 10.0, 2.0, color([0, 0, 0], 70));
        at(4.0, 2.0, 8.0, 11.0, color(INK, 255));
        at(5.0, 3.0, 6.0, 9.0, color([156, 158, 170], 255));
        at(5.0, 3.0, 6.0, 1.0, color([196, 198, 208], 255));
        at(7.0, 4.0, 2.0, 6.0, color([88, 90, 104], 255));
        at(6.0, 6.0, 4.0, 2.0, color([88, 90, 104], 255));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sim::scenario::sim_from_ascii;

    fn first(sim: &Sim) -> sim::ColonistId {
        sim.colonists().iter().next().map(|(id, _)| id).unwrap()
    }

    #[test]
    fn a_colonist_that_is_fine_has_no_mark() {
        let sim = sim_from_ascii("c..", 1).unwrap();
        let c = sim.colonist(first(&sim)).unwrap();
        assert_eq!(mark_of(&sim, c), None);
        assert!(!is_hurt(&sim, c));
    }

    #[test]
    fn starving_beats_the_other_marks_and_hurt_colonists_get_a_bar() {
        let mut sim = sim_from_ascii("c..", 1).unwrap();
        let id = first(&sim);
        sim.debug_set_rest(id, 0);
        assert_eq!(
            mark_of(&sim, sim.colonist(id).unwrap()),
            Some(Mark::Exhausted)
        );
        sim.debug_set_hunger(id, 0);
        assert_eq!(
            mark_of(&sim, sim.colonist(id).unwrap()),
            Some(Mark::Starving)
        );
        sim.debug_set_health(id, 20);
        assert!(is_hurt(&sim, sim.colonist(id).unwrap()));
    }

    #[test]
    fn each_mark_has_a_color_and_only_sleep_has_no_icon() {
        let all = [
            Mark::Starving,
            Mark::Freezing,
            Mark::Overheated,
            Mark::Break,
            Mark::Exhausted,
            Mark::Wounded,
            Mark::Sleeping,
        ];
        assert_eq!(all.iter().filter(|m| m.icon().is_none()).count(), 1);
        assert_ne!(Mark::Freezing.tint(), Mark::Starving.tint());
    }
}
