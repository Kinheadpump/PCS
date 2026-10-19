//! Icons from `icons.png`: tool icons, speed symbols, designation badges, and corner brackets.
//! The sheet is gray. The game multiplies it with a color when it draws.

use crate::cells::{TILE, icons as ic};
use crate::sprites::Sprites;
use crate::tools::Tool;
use macroquad::prelude::{
    Color, DrawTextureParams, FilterMode, Image, Rect, Texture2D, draw_texture_ex, vec2,
};
use std::cell::RefCell;

thread_local! {
    /// The icon sheet as a texture, made on first use (needs the window).
    static SHEET: RefCell<Option<Texture2D>> = const { RefCell::new(None) };
}

/// The icon cell of a tool.
pub fn cell_for(tool: Tool) -> usize {
    match tool {
        Tool::Select => ic::SELECT,
        Tool::Cut => ic::CUT,
        Tool::Mine => ic::MINE,
        Tool::Haul => ic::HAUL,
        Tool::Stockpile => ic::ZONE,
        Tool::Cancel => ic::CANCEL,
    }
}

/// The badge cell of a designation tool, if it has a badge.
pub fn badge_for(tool: Tool) -> Option<usize> {
    match tool {
        Tool::Cut => Some(ic::BADGE_CUT),
        Tool::Mine => Some(ic::BADGE_MINE),
        Tool::Haul => Some(ic::BADGE_HAUL),
        _ => None,
    }
}

/// Draw one icon cell in a square of `size` screen pixels with its top left corner at `(x, y)`.
/// `tint` multiplies the gray of the icon.
pub fn draw(cell: usize, x: f32, y: f32, size: f32, tint: Color) {
    SHEET.with(|store| {
        let mut store = store.borrow_mut();
        if store.is_none() {
            let Ok(sprites) = Sprites::shipped() else {
                return;
            };
            let (w, h) = sprites.icons.size();
            let image = Image {
                bytes: sprites.icons.bytes().to_vec(),
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
        let t = TILE as f32;
        let source = Rect::new((cell % 16) as f32 * t, (cell / 16) as f32 * t, t, t);
        draw_texture_ex(
            texture,
            x.round(),
            y.round(),
            tint,
            DrawTextureParams {
                dest_size: Some(vec2(size.round(), size.round())),
                source: Some(source),
                ..Default::default()
            },
        );
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_tool_has_its_own_icon_and_the_icons_have_pixels() {
        let mut cells: Vec<usize> = Tool::ALL.iter().map(|t| cell_for(*t)).collect();
        cells.sort_unstable();
        cells.dedup();
        assert_eq!(cells.len(), Tool::ALL.len());
        let Ok(s) = Sprites::shipped() else { return };
        for cell in cells {
            let solid = (0..TILE)
                .flat_map(|y| (0..TILE).map(move |x| (x, y)))
                .filter(|&(x, y)| s.icons.cell_px(cell, x, y)[3] == 255)
                .count();
            assert!(solid >= 20, "icon {cell} has {solid} pixels");
        }
    }

    #[test]
    fn the_three_designation_tools_have_badges() {
        assert!(badge_for(Tool::Cut).is_some());
        assert!(badge_for(Tool::Mine).is_some());
        assert!(badge_for(Tool::Haul).is_some());
        assert!(badge_for(Tool::Select).is_none());
    }
}
