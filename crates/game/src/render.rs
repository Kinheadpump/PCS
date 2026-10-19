//! Map rendering. The map is baked into one texture per chunk (32 x 32 tiles, 16 pixels per tile)
//! by `bake.rs`, and only the visible chunks are drawn.

use crate::bake::{CHUNK, PIXELS_PER_TILE, Palette, bake_chunk, chunk_counts};
use crate::camera::{Camera, View};
use crate::palette::{self as pal, Rgb};
use crate::sprites::Sprites;
use crate::ui;
use macroquad::prelude::{
    Color, DrawTextureParams, FilterMode, Image, Texture2D, WHITE, clear_background, draw_line,
    draw_texture_ex, get_fps, screen_height, screen_width, vec2,
};
use std::collections::BTreeSet;
use worldgen::LocalMapData;

/// Below this zoom (screen pixels per tile) the textures shrink, so they use linear filtering.
const NEAREST_MIN_ZOOM: f32 = 16.0;

/// A color from a palette entry.
pub fn color(c: Rgb, alpha: u8) -> Color {
    Color::from_rgba(c[0], c[1], c[2], alpha)
}

/// The baked chunk textures of one map.
pub struct MapView {
    chunks: Vec<Texture2D>,
    chunks_x: u32,
    chunks_y: u32,
}

impl MapView {
    /// Bake every chunk into a texture. Needs a running window.
    pub fn new(map: &LocalMapData, palette: &Palette, sprites: &Sprites) -> Self {
        let (chunks_x, chunks_y) = chunk_counts(map);
        let side = (CHUNK * PIXELS_PER_TILE) as u16;
        let mut chunks = Vec::new();
        for cy in 0..chunks_y {
            for cx in 0..chunks_x {
                let image = Image {
                    bytes: bake_chunk(map, palette, sprites, cx, cy),
                    width: side,
                    height: side,
                };
                let texture = Texture2D::from_image(&image);
                texture.set_filter(FilterMode::Nearest);
                chunks.push(texture);
            }
        }
        Self {
            chunks,
            chunks_x,
            chunks_y,
        }
    }

    /// Bake the given chunks again, for example after a tile changed. Needs a running window.
    pub fn rebake_chunks(
        &self,
        map: &LocalMapData,
        palette: &Palette,
        sprites: &Sprites,
        chunks: &BTreeSet<(u32, u32)>,
    ) {
        let side = (CHUNK * PIXELS_PER_TILE) as u16;
        for &(cx, cy) in chunks {
            if cx >= self.chunks_x || cy >= self.chunks_y {
                continue;
            }
            let Some(texture) = self.chunks.get((cy * self.chunks_x + cx) as usize) else {
                continue;
            };
            let image = Image {
                bytes: bake_chunk(map, palette, sprites, cx, cy),
                width: side,
                height: side,
            };
            texture.update(&image);
        }
    }

    /// Draw the chunks that touch the viewport.
    pub fn draw(&self, camera: &Camera, map_size: (u32, u32)) {
        let view = view_size();
        let Some(rect) = camera.visible_tiles(view, map_size) else {
            return;
        };
        let filter = if camera.zoom < NEAREST_MIN_ZOOM {
            FilterMode::Linear
        } else {
            FilterMode::Nearest
        };
        let (c0x, c0y) = (rect.min_x as u32 / CHUNK, rect.min_y as u32 / CHUNK);
        let (c1x, c1y) = (rect.max_x as u32 / CHUNK, rect.max_y as u32 / CHUNK);
        // Round the chunk edges to whole pixels so neighbor chunks leave no gap.
        let edge_x = |t: u32| {
            camera
                .tile_to_screen(((t * CHUNK) as f32, 0.0), view)
                .0
                .round()
        };
        let edge_y = |t: u32| {
            camera
                .tile_to_screen((0.0, (t * CHUNK) as f32), view)
                .1
                .round()
        };
        for cy in c0y..=c1y.min(self.chunks_y.saturating_sub(1)) {
            for cx in c0x..=c1x.min(self.chunks_x.saturating_sub(1)) {
                let Some(texture) = self.chunks.get((cy * self.chunks_x + cx) as usize) else {
                    continue;
                };
                texture.set_filter(filter);
                let (x0, x1) = (edge_x(cx), edge_x(cx + 1));
                let (y0, y1) = (edge_y(cy), edge_y(cy + 1));
                draw_texture_ex(
                    texture,
                    x0,
                    y0,
                    WHITE,
                    DrawTextureParams {
                        dest_size: Some(vec2(x1 - x0, y1 - y0)),
                        ..Default::default()
                    },
                );
            }
        }
    }

    /// Draw thin lines between tiles (off by default; key G).
    pub fn draw_grid(camera: &Camera, map_size: (u32, u32)) {
        if camera.zoom < 8.0 {
            return;
        }
        let view = view_size();
        let Some(rect) = camera.visible_tiles(view, map_size) else {
            return;
        };
        let line = Color::from_rgba(0, 0, 0, 40);
        for x in rect.min_x..=rect.max_x + 1 {
            let sx = camera.tile_to_screen((x as f32, 0.0), view).0.round();
            draw_line(sx, 0.0, sx, view.1, 1.0, line);
        }
        for y in rect.min_y..=rect.max_y + 1 {
            let sy = camera.tile_to_screen((0.0, y as f32), view).1.round();
            draw_line(0.0, sy, view.0, sy, 1.0, line);
        }
    }
}

/// Current viewport size in pixels.
pub fn view_size() -> View {
    (screen_width(), screen_height())
}

/// Clear the screen.
pub fn clear() {
    clear_background(color(pal::ui::INK, 255));
}

/// Draw lines of text in a panel at the top left. For error messages.
pub fn draw_overlay(ui: &mut ui::Ui, lines: &[String]) {
    ui::text_panel(ui, (8.0, 8.0), lines);
}

/// Frames per second.
pub fn fps() -> i32 {
    get_fps()
}
