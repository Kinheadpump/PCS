//! Drawing and picking of colonists on the map.

use crate::camera::{Camera, View};
use macroquad::prelude::{Color, draw_circle, draw_circle_lines, draw_line};
use sim::{Colonist, ColonistId, Sim};

/// Fill colors of the colonists, by slot order.
const COLORS: [Color; 7] = [
    Color::new(0.90, 0.35, 0.30, 1.0),
    Color::new(0.95, 0.70, 0.25, 1.0),
    Color::new(0.45, 0.80, 0.35, 1.0),
    Color::new(0.30, 0.75, 0.85, 1.0),
    Color::new(0.45, 0.50, 0.95, 1.0),
    Color::new(0.80, 0.45, 0.90, 1.0),
    Color::new(0.95, 0.95, 0.95, 1.0),
];
const OUTLINE: Color = Color::new(0.05, 0.05, 0.08, 1.0);
const SELECT: Color = Color::new(1.0, 0.95, 0.2, 1.0);
const PATH: Color = Color::new(1.0, 0.95, 0.2, 0.8);
/// Marker radius in tiles.
const RADIUS: f32 = 0.32;
/// Largest distance (in tiles) from a click to a colonist that still picks it.
pub const PICK_RADIUS: f32 = 0.6;

/// Map position of a colonist in tiles, at the center of its sub-tile position.
pub fn position_tiles(c: &Colonist) -> (f32, f32) {
    let (mx, my) = c.position_milli();
    (mx as f32 / 1000.0 + 0.5, my as f32 / 1000.0 + 0.5)
}

/// The candidate nearest to `point` within `radius`. Ties go to the first candidate.
pub fn pick_nearest<T: Copy>(
    candidates: impl IntoIterator<Item = (T, (f32, f32))>,
    point: (f32, f32),
    radius: f32,
) -> Option<T> {
    let mut best: Option<(T, f32)> = None;
    for (item, pos) in candidates {
        let d = ((pos.0 - point.0).powi(2) + (pos.1 - point.1).powi(2)).sqrt();
        if d <= radius && best.is_none_or(|(_, bd)| d < bd) {
            best = Some((item, d));
        }
    }
    best.map(|(item, _)| item)
}

/// The colonist at a map position (tiles), if any.
pub fn pick_colonist(sim: &Sim, point: (f32, f32)) -> Option<ColonistId> {
    pick_nearest(
        sim.colonists()
            .iter()
            .map(|(id, c)| (id, position_tiles(c))),
        point,
        PICK_RADIUS,
    )
}

/// Draw the path of one colonist: from its position through the path tiles to the goal.
fn draw_path(c: &Colonist, camera: &Camera, view: View) {
    let mut from = camera.tile_to_screen(position_tiles(c), view);
    for &(x, y) in c.path.iter().rev() {
        let to = camera.tile_to_screen((f32::from(x) + 0.5, f32::from(y) + 0.5), view);
        draw_line(from.0, from.1, to.0, to.1, 2.0, PATH);
        from = to;
    }
    if let Some((x, y)) = c.target {
        let g = camera.tile_to_screen((f32::from(x) + 0.5, f32::from(y) + 0.5), view);
        draw_circle_lines(g.0, g.1, camera.zoom * 0.25, 2.0, SELECT);
    }
}

/// Draw all colonists and the path of the selected one.
pub fn draw(sim: &Sim, camera: &Camera, view: View, selected: Option<ColonistId>) {
    if let Some(c) = selected.and_then(|id| sim.colonist(id)) {
        draw_path(c, camera, view);
    }
    for (i, (id, c)) in sim.colonists().iter().enumerate() {
        let p = camera.tile_to_screen(position_tiles(c), view);
        let r = (camera.zoom * RADIUS).max(3.0);
        if selected == Some(id) {
            draw_circle(p.0, p.1, r + 5.0, SELECT);
        }
        draw_circle(p.0, p.1, r + 1.5, OUTLINE);
        draw_circle(p.0, p.1, r, COLORS[i % COLORS.len()]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pick_takes_the_nearest_candidate_within_the_radius() {
        let c = [(1, (5.0, 5.0)), (2, (5.4, 5.0)), (3, (9.0, 9.0))];
        assert_eq!(pick_nearest(c, (5.35, 5.0), 0.6), Some(2));
        assert_eq!(pick_nearest(c, (7.0, 7.0), 0.6), None);
    }

    #[test]
    fn pick_breaks_ties_by_order() {
        let c = [(1, (4.0, 5.0)), (2, (6.0, 5.0))];
        assert_eq!(pick_nearest(c, (5.0, 5.0), 1.5), Some(1));
    }

    #[test]
    fn a_standing_colonist_is_drawn_at_its_tile_center() {
        let c = Colonist::new(3, 4);
        assert_eq!(position_tiles(&c), (3.5, 4.5));
    }
}
