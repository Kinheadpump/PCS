//! Map rendering. Tiles are baked into one texture per chunk (32 x 32 tiles, 8 pixels per tile)
//! and only the visible chunks are drawn. The baking code is pure and has no `macroquad` calls.

use crate::camera::{Camera, View};
use base::content::MaterialDef;
use macroquad::prelude::{
    Color, DrawTextureParams, FilterMode, Image, Texture2D, WHITE, clear_background,
    draw_rectangle, draw_text, draw_texture_ex, get_fps, screen_height, screen_width, vec2,
};
use worldgen::{Ground, LocalMapData, Ore, RockKind};

/// Tiles per chunk side.
pub const CHUNK: u32 = 32;
/// Texture pixels per tile in a chunk texture.
pub const PIXELS_PER_TILE: u32 = 8;

type Rgb = [u8; 3];

/// Tile colors, taken from `materials.ron`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Palette {
    soil: Rgb,
    sand: Rgb,
    clay: Rgb,
    water: Rgb,
    sandstone: Rgb,
    limestone: Rgb,
    granite: Rgb,
    slate: Rgb,
    coal: Rgb,
    copper_ore: Rgb,
    iron_ore: Rgb,
    tree: Rgb,
}

fn color_of(materials: &[MaterialDef], key: &str) -> Result<Rgb, String> {
    materials
        .iter()
        .find(|m| m.key == key)
        .map(|m| m.color)
        .ok_or_else(|| format!("materials.ron has no entry \"{key}\""))
}

impl Palette {
    /// Build the palette. Fails with a message if a needed material is missing.
    pub fn from_materials(materials: &[MaterialDef]) -> Result<Self, String> {
        let c = |key: &str| color_of(materials, key);
        let grass = c("grass")?;
        // The tree marker is a darker green than the grass.
        let tree = [grass[0] / 3, grass[1] / 2, grass[2] / 3];
        Ok(Self {
            soil: c("soil")?,
            sand: c("sand")?,
            clay: c("clay")?,
            water: c("water")?,
            sandstone: c("sandstone")?,
            limestone: c("limestone")?,
            granite: c("granite")?,
            slate: c("slate")?,
            coal: c("coal")?,
            copper_ore: c("copper_ore")?,
            iron_ore: c("iron_ore")?,
            tree,
        })
    }

    /// Tint of a tile: ore, else rock wall, else ground.
    pub fn tile_color(&self, ground: Ground, rock: RockKind, ore: Ore) -> Rgb {
        match ore {
            Ore::Coal => return self.coal,
            Ore::Copper => return self.copper_ore,
            Ore::Iron => return self.iron_ore,
            Ore::None => {}
        }
        match rock {
            RockKind::Sandstone => return self.sandstone,
            RockKind::Limestone => return self.limestone,
            RockKind::Granite => return self.granite,
            RockKind::None => {}
        }
        match ground {
            Ground::Soil => self.soil,
            Ground::Sand => self.sand,
            Ground::Clay => self.clay,
            Ground::Water => self.water,
            Ground::Stone => self.slate,
        }
    }

    /// Color of the tree marker.
    pub fn tree_color(&self) -> Rgb {
        self.tree
    }
}

/// Number of chunks on each axis.
pub fn chunk_counts(map: &LocalMapData) -> (u32, u32) {
    (
        map.ground.width().div_ceil(CHUNK),
        map.ground.height().div_ceil(CHUNK),
    )
}

/// RGBA pixels of one chunk (row-major). Tiles outside the map are black.
pub fn bake_chunk(map: &LocalMapData, palette: &Palette, chunk_x: u32, chunk_y: u32) -> Vec<u8> {
    let side = (CHUNK * PIXELS_PER_TILE) as usize;
    let mut bytes = vec![0u8; side * side * 4];
    let ppt = PIXELS_PER_TILE as usize;
    for ty in 0..CHUNK as usize {
        for tx in 0..CHUNK as usize {
            let mx = ((chunk_x * CHUNK) as usize + tx) as i32;
            let my = ((chunk_y * CHUNK) as usize + ty) as i32;
            let (base, has_tree) = match (
                map.ground.get(mx, my),
                map.rock.get(mx, my),
                map.ore.get(mx, my),
            ) {
                (Some(g), Some(r), Some(o)) => (
                    palette.tile_color(*g, *r, *o),
                    map.tree.get(mx, my).copied().unwrap_or(false),
                ),
                _ => ([0, 0, 0], false),
            };
            for py in 0..ppt {
                for px in 0..ppt {
                    // A round marker: pixels within radius 3 of the tile center.
                    let (dx, dy) = (2 * px as i32 - 7, 2 * py as i32 - 7);
                    let rgb = if has_tree && dx * dx + dy * dy <= 36 {
                        palette.tree_color()
                    } else {
                        base
                    };
                    let i = ((ty * ppt + py) * side + tx * ppt + px) * 4;
                    bytes[i..i + 4].copy_from_slice(&[rgb[0], rgb[1], rgb[2], 255]);
                }
            }
        }
    }
    bytes
}

/// The baked chunk textures of one map.
pub struct MapView {
    chunks: Vec<Texture2D>,
    chunks_x: u32,
    chunks_y: u32,
}

impl MapView {
    /// Bake every chunk into a texture. Needs a running window.
    pub fn new(map: &LocalMapData, palette: &Palette) -> Self {
        let (chunks_x, chunks_y) = chunk_counts(map);
        let side = (CHUNK * PIXELS_PER_TILE) as u16;
        let mut chunks = Vec::new();
        for cy in 0..chunks_y {
            for cx in 0..chunks_x {
                let image = Image {
                    bytes: bake_chunk(map, palette, cx, cy),
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

    /// Draw the chunks that touch the viewport.
    pub fn draw(&self, camera: &Camera, map_size: (u32, u32)) {
        let view = view_size();
        let Some(rect) = camera.visible_tiles(view, map_size) else {
            return;
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
}

/// Current viewport size in pixels.
pub fn view_size() -> View {
    (screen_width(), screen_height())
}

/// Clear the screen.
pub fn clear() {
    clear_background(Color::from_rgba(12, 12, 16, 255));
}

/// Draw lines of text in a panel at the top left.
pub fn draw_overlay(lines: &[String]) {
    let height = 8.0 + 20.0 * lines.len() as f32;
    draw_rectangle(4.0, 4.0, 620.0, height, Color::from_rgba(0, 0, 0, 170));
    for (i, line) in lines.iter().enumerate() {
        draw_text(line, 10.0, 24.0 + 20.0 * i as f32, 20.0, WHITE);
    }
}

/// Frames per second.
pub fn fps() -> i32 {
    get_fps()
}

#[cfg(test)]
mod tests {
    use super::*;
    use worldgen::{EmbarkSummary, generate_local};

    fn palette() -> Palette {
        let c = |v: u8| [v, v, v];
        Palette {
            soil: c(1),
            sand: c(2),
            clay: c(3),
            water: c(4),
            sandstone: c(5),
            limestone: c(6),
            granite: c(7),
            slate: c(8),
            coal: c(9),
            copper_ore: c(10),
            iron_ore: c(11),
            tree: c(12),
        }
    }

    #[test]
    fn ore_beats_rock_beats_ground() {
        let p = palette();
        assert_eq!(
            p.tile_color(Ground::Stone, RockKind::Granite, Ore::Iron),
            [11; 3]
        );
        assert_eq!(
            p.tile_color(Ground::Stone, RockKind::Granite, Ore::None),
            [7; 3]
        );
        assert_eq!(
            p.tile_color(Ground::Water, RockKind::None, Ore::None),
            [4; 3]
        );
    }

    #[test]
    fn bake_chunk_has_expected_size_and_opaque_pixels() {
        let map = generate_local(&EmbarkSummary::temperate_forest());
        let bytes = bake_chunk(&map, &palette(), 0, 0);
        let side = (CHUNK * PIXELS_PER_TILE) as usize;
        assert_eq!(bytes.len(), side * side * 4);
        assert!(bytes.chunks(4).all(|px| px[3] == 255));
    }

    #[test]
    fn bake_chunk_draws_tree_marker() {
        let map = generate_local(&EmbarkSummary::temperate_forest());
        let bytes = bake_chunk(&map, &palette(), 0, 0);
        let any_tree_tile =
            (0..CHUNK as i32).any(|y| (0..CHUNK as i32).any(|x| map.tree.get(x, y) == Some(&true)));
        let has_marker = bytes.chunks(4).any(|px| px[0] == 12);
        assert_eq!(any_tree_tile, has_marker);
    }

    #[test]
    fn chunk_counts_cover_256_map() {
        let map = generate_local(&EmbarkSummary::dry_plain());
        assert_eq!(chunk_counts(&map), (8, 8));
    }

    #[test]
    fn shipped_materials_build_a_palette() {
        let text = include_str!("../../../assets/content/materials.ron");
        let materials = base::content::load_materials("materials.ron", text);
        assert!(materials.is_ok());
        let palette = Palette::from_materials(&materials.unwrap_or_default());
        assert!(palette.is_ok());
    }
}
