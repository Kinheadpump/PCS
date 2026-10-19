//! Drawing and picking of colonists on the map.
//!
//! Each colonist has a `Look` (shirt, hair, skin, hair style). The actor sheet has one hair style
//! per block of cells. Hair and skin are painted in the colors of index 0 in `palette.rs`; the
//! game swaps them for the look of the colonist (palette swap). The shirt is gray and gets the
//! shirt color through the ramp. The sim is only read, never changed.

use crate::camera::{Camera, View};
use crate::cells::{TILE, actors as cell};
use crate::palette::{self as pal, Look, Rgb};
use crate::sprites::{Png, Sprites};
use macroquad::prelude::{
    Color, DrawTextureParams, FilterMode, Image, Rect, Texture2D, WHITE, draw_ellipse,
    draw_ellipse_lines, draw_rectangle, draw_texture_ex, vec2,
};
use sim::{Colonist, ColonistId, JobStep, Sim};
use std::cell::RefCell;
use std::collections::BTreeMap;

const SELECT_FILL: Color = Color::new(1.0, 0.95, 0.2, 0.30);
const SELECT: Color = Color::new(1.0, 0.95, 0.2, 1.0);
/// Smallest drawn sprite size in screen pixels, so colonists stay visible when zoomed out.
const MIN_SPRITE_PIXELS: f32 = 12.0;
/// Walk cycle: sprite step frame for each quarter of a cycle (0 stands, 1 and 2 step).
const WALK_CYCLE: [usize; 4] = [1, 0, 2, 0];
/// Distance walked (in 1/1000 tile) for one quarter of the walk cycle.
const WALK_QUARTER_MILLI: i32 = 250;
/// Work progress (tick-percent units) for one work frame. About 6 ticks at 100% speed.
const WORK_FRAME_PROGRESS: u32 = 600;
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

/// Draw the path of one colonist: a dot on each tile of the path and a marker on the goal.
fn draw_path(c: &Colonist, camera: &Camera, view: View) {
    let dot = (camera.zoom / 8.0).round().max(2.0);
    let color = Color::new(1.0, 0.95, 0.2, 0.85);
    for &(x, y) in c.path.iter() {
        let p = camera.tile_to_screen((f32::from(x) + 0.5, f32::from(y) + 0.5), view);
        draw_rectangle(
            (p.0 - dot / 2.0).round(),
            (p.1 - dot / 2.0).round(),
            dot,
            dot,
            color,
        );
    }
    if let Some((x, y)) = c.target {
        let a = camera.tile_to_screen((f32::from(x), f32::from(y)), view);
        let b = camera.tile_to_screen((f32::from(x) + 1.0, f32::from(y) + 1.0), view);
        let (x0, y0, w, h) = (
            a.0.round(),
            a.1.round(),
            b.0.round() - a.0.round(),
            b.1.round() - a.1.round(),
        );
        let t = (camera.zoom / 16.0).round().max(1.0) * 2.0;
        let arm = (w / 3.0).round();
        for (cx, cy, sx, sy) in [
            (x0, y0, 1.0, 1.0),
            (x0 + w, y0, -1.0, 1.0),
            (x0, y0 + h, 1.0, -1.0),
            (x0 + w, y0 + h, -1.0, -1.0),
        ] {
            let horizontal = (
                cx.min(cx + sx * arm),
                if sy > 0.0 { cy } else { cy - t },
                arm,
                t,
            );
            let vertical = (
                if sx > 0.0 { cx } else { cx - t },
                cy.min(cy + sy * arm),
                t,
                arm,
            );
            draw_rectangle(
                horizontal.0,
                horizontal.1,
                horizontal.2,
                horizontal.3,
                color,
            );
            draw_rectangle(vertical.0, vertical.1, vertical.2, vertical.3, color);
        }
    }
}

/// The frame of a colonist in its hair style block, and whether the sprite is mirrored.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pose {
    /// Frame number inside one style block (see `cells::actors`).
    pub frame: usize,
    /// True for west (the east frame, mirrored).
    pub flip: bool,
}

/// Facing from a step in tiles. East and west win over north and south on diagonals, as the
/// side view reads best. Returns the first frame of the facing (walk or work) and the flip.
fn facing(dx: i32, dy: i32, walk: bool) -> (usize, bool) {
    let (south, north, east) = if walk {
        (cell::SOUTH, cell::NORTH, cell::EAST)
    } else {
        (cell::WORK_SOUTH, cell::WORK_NORTH, cell::WORK_EAST)
    };
    if dx != 0 {
        (east, dx < 0)
    } else if dy < 0 {
        (north, false)
    } else {
        (south, false)
    }
}

/// The pose of a colonist. A walker faces its next tile and steps with the distance walked. A
/// worker faces the tile of its job and swings between two work frames. A colonist that stands
/// faces south. `job` is the step of the job of the colonist and the tile of the job, if known.
pub fn pose_for(c: &Colonist, job: Option<(JobStep, Option<(u16, u16)>)>) -> Pose {
    if let Some((nx, ny)) = c.next_tile() {
        let (dx, dy) = (
            i32::from(nx) - i32::from(c.x),
            i32::from(ny) - i32::from(c.y),
        );
        let (mx, my) = c.position_milli();
        let phase = ((mx + my).rem_euclid(4 * WALK_QUARTER_MILLI) / WALK_QUARTER_MILLI) as usize;
        let (first, flip) = facing(dx, dy, true);
        return Pose {
            frame: first + WALK_CYCLE[phase % WALK_CYCLE.len()],
            flip,
        };
    }
    if let Some((JobStep::Work { progress }, target)) = job {
        let (tx, ty) = target.unwrap_or((c.x, c.y));
        let (dx, dy) = (
            i32::from(tx) - i32::from(c.x),
            i32::from(ty) - i32::from(c.y),
        );
        let (first, flip) = facing(dx, dy, false);
        return Pose {
            frame: first + ((progress / WORK_FRAME_PROGRESS) % 2) as usize,
            flip,
        };
    }
    Pose {
        frame: cell::SOUTH,
        flip: false,
    }
}

/// The pose of a colonist in a sim (reads the target of its job from the job board).
pub fn pose_in(sim: &Sim, c: &Colonist) -> Pose {
    let job = c.job.map(|j| {
        let target = sim.jobs().get(j.job).map(|job| (job.x, job.y));
        (j.step, target)
    });
    pose_for(c, job)
}

/// The sheet bytes (RGBA) painted for one look: hair and skin swapped, the shirt colored.
pub fn recolor(sheet: &Png, look: Look) -> Vec<u8> {
    let mut out = sheet.bytes().to_vec();
    let swap = |px: &mut [u8], from: Rgb, to: Rgb| -> bool {
        if px[..3] == from {
            px[..3].copy_from_slice(&to);
            true
        } else {
            false
        }
    };
    for px in out.chunks_mut(4) {
        if px[3] != 255 {
            continue;
        }
        if px[0] == px[1] && px[1] == px[2] {
            let c = pal::ramp(
                pal::SHIRTS[look.shirt % pal::SHIRTS.len()],
                i32::from(px[0]),
            );
            px[..3].copy_from_slice(&c);
            continue;
        }
        let hair = pal::HAIR[look.hair % pal::HAIR.len()];
        let skin = pal::SKIN[look.skin % pal::SKIN.len()];
        let _ = swap(px, pal::HAIR[0][0], hair[0])
            || swap(px, pal::HAIR[0][1], hair[1])
            || swap(px, pal::SKIN[0][0], skin[0])
            || swap(px, pal::SKIN[0][1], skin[1]);
    }
    out
}

/// The loaded sheet and one texture for each (style, shirt, skin and hair) look.
type Cache = (Option<Sprites>, BTreeMap<(usize, usize, usize), Texture2D>);

thread_local! {
    /// The actor sheet and one texture for each look, made on first use (needs the window).
    static TEXTURES: RefCell<Cache> =
        const { RefCell::new((None, BTreeMap::new())) };
}

/// Draw one colonist sprite. `p` is the screen position of the tile center.
fn draw_sprite(
    texture: &Texture2D,
    frame: usize,
    style: usize,
    flip: bool,
    p: (f32, f32),
    size: f32,
) {
    let (x, y) = ((p.0 - size / 2.0).round(), (p.1 - size / 2.0).round());
    let index = style * cell::STYLE_CELLS + frame;
    let t = TILE as f32;
    let source = Rect::new((index % 16) as f32 * t, (index / 16) as f32 * t, t, t);
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
    TEXTURES.with(|store| {
        let mut store = store.borrow_mut();
        let (sheet, textures) = &mut *store;
        if sheet.is_none() {
            *sheet = Sprites::shipped().ok();
        }
        let Some(sheet) = sheet.as_ref() else { return };
        let size = camera.zoom.max(MIN_SPRITE_PIXELS);
        // Draw from north to south, so a colonist in front covers a colonist behind.
        let mut order: Vec<(usize, ColonistId, &Colonist)> = sim
            .colonists()
            .iter()
            .enumerate()
            .map(|(i, (id, c))| (i, id, c))
            .collect();
        order.sort_by_key(|&(i, _, c)| (c.position_milli().1, i));
        for (slot, id, c) in order {
            let look = pal::look_for(slot);
            let p = camera.tile_to_screen(position_tiles(c), view);
            if selected == Some(id) {
                // A ring around the feet. The sprite feet are 5.5 / 16 of a sprite below the center.
                let feet = (p.0, p.1 + size * 0.34);
                draw_ellipse(feet.0, feet.1, size * 0.5, size * 0.22, 0.0, SELECT_FILL);
                draw_ellipse_lines(feet.0, feet.1, size * 0.5, size * 0.22, 0.0, 2.0, SELECT);
            }
            let pose = pose_in(sim, c);
            let texture = textures
                .entry((look.shirt, look.hair, look.skin))
                .or_insert_with(|| {
                    let (w, h) = sheet.actors.size();
                    let image = Image {
                        bytes: recolor(&sheet.actors, look),
                        width: w as u16,
                        height: h as u16,
                    };
                    let texture = Texture2D::from_image(&image);
                    texture.set_filter(FilterMode::Nearest);
                    texture
                });
            draw_sprite(texture, pose.frame, look.style, pose.flip, p, size);
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
        assert_eq!(
            pose_for(&c, None),
            Pose {
                frame: cell::SOUTH,
                flip: false
            }
        );
        c.path = vec![(4, 5)];
        let p = pose_for(&c, None);
        assert!((cell::EAST..cell::EAST + 3).contains(&p.frame) && p.flip);
        c.path = vec![(5, 4)];
        assert!((cell::NORTH..cell::NORTH + 3).contains(&pose_for(&c, None).frame));
    }

    #[test]
    fn a_worker_faces_its_job_and_swings_between_two_work_frames() {
        let c = Colonist::new(5, 5);
        let work = |progress, target| Some((JobStep::Work { progress }, Some(target)));
        let a = pose_for(&c, work(0, (6, 5)));
        let b = pose_for(&c, work(WORK_FRAME_PROGRESS, (6, 5)));
        assert_eq!(a.frame, cell::WORK_EAST);
        assert_eq!(b.frame, cell::WORK_EAST + 1);
        assert!(!a.flip);
        assert!(pose_for(&c, work(0, (4, 5))).flip);
        assert_eq!(
            pose_for(&c, work(WORK_FRAME_PROGRESS, (5, 4))).frame,
            cell::WORK_NORTH + 1
        );
    }

    #[test]
    fn a_standing_colonist_is_drawn_at_its_tile_center() {
        let c = Colonist::new(3, 4);
        assert_eq!(position_tiles(&c), (3.5, 4.5));
    }

    #[test]
    fn recolor_swaps_hair_and_skin_and_tints_the_shirt() {
        let Ok(sprites) = Sprites::shipped() else {
            return;
        };
        let plain = recolor(
            &sprites.actors,
            Look {
                shirt: 0,
                hair: 0,
                skin: 0,
                style: 0,
            },
        );
        let other = recolor(
            &sprites.actors,
            Look {
                shirt: 3,
                hair: 2,
                skin: 3,
                style: 0,
            },
        );
        assert_ne!(plain, other);
        // The outline keeps its color in every look.
        let has = |bytes: &[u8], c: Rgb| bytes.chunks(4).any(|p| p[3] == 255 && p[..3] == c);
        assert!(has(&plain, pal::OUTLINE) && has(&other, pal::OUTLINE));
        assert!(has(&other, pal::HAIR[2][0]) && has(&other, pal::SKIN[3][0]));
        assert!(!has(&other, pal::HAIR[0][0]));
    }
}
