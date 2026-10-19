//! Drawing and picking of colonists on the map.

use crate::camera::{Camera, View};
use crate::sprites::{ActorSheet, TILE, actor_cell};
use macroquad::prelude::{
    Color, DrawTextureParams, FilterMode, Image, Rect, Texture2D, WHITE, draw_circle_lines,
    draw_ellipse, draw_ellipse_lines, draw_line, draw_texture_ex, vec2,
};
use sim::{Colonist, ColonistId, Sim};
use std::cell::RefCell;

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
const SELECT_FILL: Color = Color::new(1.0, 0.95, 0.2, 0.35);
const SELECT: Color = Color::new(1.0, 0.95, 0.2, 1.0);
const PATH: Color = Color::new(1.0, 0.95, 0.2, 0.8);
/// Smallest drawn sprite size in screen pixels, so colonists stay visible when zoomed out.
const MIN_SPRITE_PIXELS: f32 = 12.0;
/// Walk cycle: sprite step frame for each quarter of a cycle (0 stands, 1 and 2 step).
const WALK_CYCLE: [usize; 4] = [1, 0, 2, 0];
/// Distance walked (in 1/1000 tile) for one quarter of the walk cycle.
const WALK_QUARTER_MILLI: i32 = 250;
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

/// Sprite cell and flip flag for a colonist: facing from the next path tile, step from
/// the distance walked. A colonist that stands faces south.
pub fn sprite_for(c: &Colonist) -> (usize, bool) {
    let Some((nx, ny)) = c.next_tile() else {
        return (actor_cell::SOUTH, false);
    };
    let (dx, dy) = (
        i32::from(nx) - i32::from(c.x),
        i32::from(ny) - i32::from(c.y),
    );
    let (mx, my) = c.position_milli();
    let phase = ((mx + my).rem_euclid(4 * WALK_QUARTER_MILLI) / WALK_QUARTER_MILLI) as usize;
    let step = WALK_CYCLE[phase % WALK_CYCLE.len()];
    // East and west win over north and south on diagonals, as the side view reads best.
    if dx != 0 {
        (actor_cell::EAST + step, dx < 0)
    } else if dy < 0 {
        (actor_cell::NORTH + step, false)
    } else {
        (actor_cell::SOUTH + step, false)
    }
}

thread_local! {
    /// One texture per shirt color, made on first use (needs the window).
    static TEXTURES: RefCell<Vec<Texture2D>> = const { RefCell::new(Vec::new()) };
}

fn color_bytes(c: Color) -> [u8; 3] {
    [
        (c.r * 255.0) as u8,
        (c.g * 255.0) as u8,
        (c.b * 255.0) as u8,
    ]
}

fn build_textures() -> Vec<Texture2D> {
    let Ok(sheet) = ActorSheet::shipped() else {
        return Vec::new();
    };
    let (w, h) = sheet.size();
    COLORS
        .iter()
        .map(|&color| {
            let image = Image {
                bytes: sheet.tinted(color_bytes(color)),
                width: w as u16,
                height: h as u16,
            };
            let texture = Texture2D::from_image(&image);
            texture.set_filter(FilterMode::Nearest);
            texture
        })
        .collect()
}

/// Draw one colonist sprite with its feet shadow. `p` is the screen position of the tile center.
fn draw_sprite(texture: &Texture2D, cell: usize, flip: bool, p: (f32, f32), size: f32) {
    let (x, y) = ((p.0 - size / 2.0).round(), (p.1 - size / 2.0).round());
    let source = Rect::new((cell * TILE) as f32, 0.0, TILE as f32, TILE as f32);
    draw_texture_ex(
        texture,
        x,
        y,
        WHITE,
        DrawTextureParams {
            dest_size: Some(vec2(size.round(), size.round())),
            source: Some(source),
            flip_x: flip,
            ..Default::default()
        },
    );
}

/// Draw all colonists and the path of the selected one.
pub fn draw(sim: &Sim, camera: &Camera, view: View, selected: Option<ColonistId>) {
    if let Some(c) = selected.and_then(|id| sim.colonist(id)) {
        draw_path(c, camera, view);
    }
    TEXTURES.with(|cell| {
        let mut textures = cell.borrow_mut();
        if textures.is_empty() {
            *textures = build_textures();
        }
        let size = camera.zoom.max(MIN_SPRITE_PIXELS);
        for (i, (id, c)) in sim.colonists().iter().enumerate() {
            let p = camera.tile_to_screen(position_tiles(c), view);
            if selected == Some(id) {
                // A ring around the feet. The sprite feet are 5.5 / 16 of a sprite below the center.
                let feet = (p.0, p.1 + size * 0.34);
                draw_ellipse(feet.0, feet.1, size * 0.5, size * 0.22, 0.0, SELECT_FILL);
                draw_ellipse_lines(feet.0, feet.1, size * 0.5, size * 0.22, 0.0, 2.0, SELECT);
            }
            let (cell, flip) = sprite_for(c);
            if let Some(texture) = textures.get(i % COLORS.len()) {
                draw_sprite(texture, cell, flip, p, size);
            }
        }
    });
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
    fn a_standing_colonist_faces_south_and_a_walker_faces_its_next_tile() {
        let mut c = Colonist::new(5, 5);
        assert_eq!(sprite_for(&c), (actor_cell::SOUTH, false));
        c.path = vec![(4, 5)];
        let (cell, flip) = sprite_for(&c);
        assert!((actor_cell::EAST..actor_cell::EAST + 3).contains(&cell) && flip);
        c.path = vec![(5, 4)];
        assert!((actor_cell::NORTH..actor_cell::NORTH + 3).contains(&sprite_for(&c).0));
    }

    #[test]
    fn a_standing_colonist_is_drawn_at_its_tile_center() {
        let c = Colonist::new(3, 4);
        assert_eq!(position_tiles(&c), (3.5, 4.5));
    }
}
