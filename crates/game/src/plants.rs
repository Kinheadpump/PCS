//! Drawing of farming: the farm zones, the crop plants, the wild plants, and the fertility
//! overlay. The plant sprites come from `objects.png` (full color). The pure parts (which cell,
//! which color) have tests.

use crate::cells::objects as oc;
use crate::overlay::{Frame, tool_color, with_alpha};
use crate::sprites::Sprites;
use crate::tools::Tool;
use macroquad::prelude::{
    Color, DrawTextureParams, FilterMode, Image, Rect, Texture2D, WHITE, draw_rectangle,
    draw_texture_ex, vec2,
};
use std::cell::RefCell;

/// Opacity of the tint of a farm zone tile.
const ZONE_ALPHA: f32 = 0.22;
/// Opacity of the fertility overlay.
const FERTILITY_ALPHA: f32 = 0.55;
/// Tint of a plant that does not grow now because of the temperature.
const DORMANT: Color = Color::new(0.62, 0.72, 1.0, 1.0);
/// Tint of a wild plant that is not ripe.
const UNRIPE: Color = Color::new(1.0, 1.0, 1.0, 0.8);

thread_local! {
    /// The object sheet as a texture, made on first use (needs the window).
    static SHEET: RefCell<Option<Texture2D>> = const { RefCell::new(None) };
}

/// The cell of a crop plant. A plant is a sprout in the first third of its growth, then
/// growing, then ripe (one variant for each crop).
pub fn crop_cell(crop: u16, hours: u32, hours_needed: u32) -> usize {
    if hours >= hours_needed {
        oc::CROP_RIPE + usize::from(crop) % oc::CROP_RIPE_COUNT
    } else if hours.saturating_mul(3) < hours_needed {
        oc::CROP_SPROUT
    } else {
        oc::CROP_GROWING
    }
}

/// The cell of a wild plant. Each type has a ripe cell, then a cell for the time after the
/// gathering.
pub fn wild_cell(kind: u16, ripe: bool) -> usize {
    oc::WILD + 2 * (usize::from(kind) % oc::WILD_COUNT) + usize::from(!ripe)
}

/// The color of the fertility overlay: brown below the middle, green above. `value` is
/// per-mille; `low` and `high` are the limits of the fertility.
pub fn fertility_color(value: i32, low: i32, high: i32) -> [u8; 3] {
    let span = (high - low).max(1);
    let t = ((value - low).clamp(0, span) * 255 / span) as u8;
    let t = i32::from(t);
    [
        (150 - t * 90 / 255) as u8,
        (96 + t * 120 / 255) as u8,
        (56 + t * 10 / 255) as u8,
    ]
}

fn draw_cell(sprites: &Sprites, cell: usize, rect: (f32, f32, f32, f32), tint: Color) {
    SHEET.with(|store| {
        let mut store = store.borrow_mut();
        if store.is_none() {
            let (w, h) = sprites.objects.size();
            let image = Image {
                bytes: sprites.objects.bytes().to_vec(),
                width: w as u16,
                height: h as u16,
            };
            let texture = Texture2D::from_image(&image);
            texture.set_filter(FilterMode::Nearest);
            *store = Some(texture);
        }
        let Some(texture) = store.as_ref() else {
            return;
        };
        let t = crate::cells::TILE as f32;
        let source = Rect::new((cell % 16) as f32 * t, (cell / 16) as f32 * t, t, t);
        draw_texture_ex(
            texture,
            rect.0,
            rect.1,
            tint,
            DrawTextureParams {
                dest_size: Some(vec2(rect.2, rect.3)),
                source: Some(source),
                ..Default::default()
            },
        );
    });
}

/// Draw the tint of the farm zones.
pub fn draw_zones(f: &Frame) {
    let Some(visible) = f.visible() else { return };
    let tint = with_alpha(tool_color(Tool::Farm), ZONE_ALPHA);
    for ((x, y), _) in f.sim.farm_zones() {
        if f.is_visible(&visible, x, y) {
            let r = f.rect(x, y);
            draw_rectangle(r.0, r.1, r.2, r.3, tint);
        }
    }
}

/// Draw the crop plants and the wild plants.
pub fn draw_plants(f: &Frame, sprites: &Sprites) {
    let Some(visible) = f.visible() else { return };
    for ((x, y), plant) in f.sim.plants() {
        if !f.is_visible(&visible, x, y) {
            continue;
        }
        let Some(report) = f.sim.plant_report(x, y) else {
            continue;
        };
        let cell = crop_cell(plant.crop, plant.hours, report.hours_needed);
        let tint = if report.growing || report.ripe {
            WHITE
        } else {
            DORMANT
        };
        draw_cell(sprites, cell, f.rect(x, y), tint);
    }
    for y in visible.min_y..=visible.max_y {
        for x in visible.min_x..=visible.max_x {
            let (Ok(x), Ok(y)) = (u16::try_from(x), u16::try_from(y)) else {
                continue;
            };
            if f.sim.plant_at(x, y).is_some() {
                continue;
            }
            if let Some(wild) = f.sim.wild_plant_at(x, y) {
                let tint = if wild.ripe { WHITE } else { UNRIPE };
                draw_cell(sprites, wild_cell(wild.kind, wild.ripe), f.rect(x, y), tint);
            }
        }
    }
}

/// Draw the fertility overlay: one tint for each tile that can be farmed.
pub fn draw_fertility(f: &Frame) {
    let Some(visible) = f.visible() else { return };
    let rules = &f.sim.rules().farm;
    for y in visible.min_y..=visible.max_y {
        for x in visible.min_x..=visible.max_x {
            let (Ok(x), Ok(y)) = (u16::try_from(x), u16::try_from(y)) else {
                continue;
            };
            let value = i32::from(f.sim.fertility(x, y));
            if value == 0 {
                continue;
            }
            let c = fertility_color(value, rules.fertility_min, rules.fertility_max);
            let r = f.rect(x, y);
            draw_rectangle(
                r.0,
                r.1,
                r.2,
                r.3,
                Color::from_rgba(c[0], c[1], c[2], (FERTILITY_ALPHA * 255.0) as u8),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_crop_is_a_sprout_then_growing_then_ripe() {
        assert_eq!(crop_cell(0, 0, 240), oc::CROP_SPROUT);
        assert_eq!(crop_cell(0, 100, 240), oc::CROP_GROWING);
        assert_eq!(crop_cell(0, 240, 240), oc::CROP_RIPE);
        assert_eq!(crop_cell(2, 300, 240), oc::CROP_RIPE + 2);
    }

    #[test]
    fn a_wild_plant_has_a_ripe_cell_and_a_picked_cell() {
        assert_eq!(wild_cell(0, true), oc::WILD);
        assert_eq!(wild_cell(0, false), oc::WILD + 1);
        assert_eq!(wild_cell(2, true), oc::WILD + 4);
    }

    #[test]
    fn fertility_goes_from_brown_to_green() {
        let low = fertility_color(500, 500, 1500);
        let high = fertility_color(1500, 500, 1500);
        assert!(low[0] > low[1], "{low:?}");
        assert!(high[1] > high[0], "{high:?}");
        assert_eq!(fertility_color(0, 500, 1500), low);
    }
}
