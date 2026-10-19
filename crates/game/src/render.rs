//! Map rendering. Tiles are baked into one texture per chunk (32 x 32 tiles, 16 pixels per tile)
//! from the sprite sheet, and only the visible chunks are drawn. The baking code is pure and has
//! no `macroquad` calls.

use crate::camera::{Camera, View};
use crate::sprites::{BASE_LEVEL, Sheet, TILE, cell};
use base::content::MaterialDef;
use base::rng::mix;
use macroquad::prelude::{
    Color, DrawTextureParams, FilterMode, Image, Texture2D, WHITE, clear_background, draw_line,
    draw_rectangle, draw_rectangle_lines, draw_text, draw_texture_ex, get_fps, measure_text,
    screen_height, screen_width, vec2,
};
use worldgen::{Ground, LocalMapData, Ore, RockKind};

/// Tiles per chunk side.
pub const CHUNK: u32 = 32;
/// Texture pixels per tile in a chunk texture.
pub const PIXELS_PER_TILE: u32 = TILE as u32;
/// Below this zoom (screen pixels per tile) the textures shrink, so they use linear filtering.
const NEAREST_MIN_ZOOM: f32 = 16.0;
/// Text size of the overlay.
const FONT_SIZE: f32 = 22.0;

type Rgb = [u8; 3];

/// Tile colors, taken from `materials.ron`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Palette {
    grass: Rgb,
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
        // The tree canopy is a darker green than the grass.
        let tree = [
            grass[0] / 2,
            (u16::from(grass[1]) * 3 / 4) as u8,
            grass[2] / 2,
        ];
        Ok(Self {
            grass,
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

    /// Color of the tree canopy.
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

/// Neighbor directions in the order of the edge masks: north, east, south, west.
const DIRS: [(i32, i32); 4] = [(0, -1), (1, 0), (0, 1), (-1, 0)];

/// What a tile is, for the purpose of drawing.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Outside,
    Wall,
    Water,
    Open,
}

fn kind_at(map: &LocalMapData, x: i32, y: i32) -> Kind {
    match (map.ground.get(x, y), map.rock.get(x, y)) {
        (Some(_), Some(r)) if *r != RockKind::None => Kind::Wall,
        (Some(Ground::Water), _) => Kind::Water,
        (Some(_), _) => Kind::Open,
        _ => Kind::Outside,
    }
}

fn tile_hash(x: i32, y: i32, salt: u64) -> u64 {
    mix((u64::from(x as u32) << 32 | u64::from(y as u32))
        ^ salt.wrapping_mul(0x9E37_79B9_7F4A_7C15))
}

/// One tile under construction: 16 x 16 RGB.
struct TileBuf {
    rgb: [[i32; 3]; TILE * TILE],
}

impl TileBuf {
    /// Blend a sprite onto the tile. `tint` multiplies the grayscale value.
    fn sprite(&mut self, sheet: &Sheet, cell: usize, tint: Rgb, bright: i32) {
        for y in 0..TILE {
            for x in 0..TILE {
                let (v, a) = sheet.pixel(cell, x, y);
                if a == 0 {
                    continue;
                }
                let px = &mut self.rgb[y * TILE + x];
                for (c, p) in px.iter_mut().enumerate() {
                    let lit =
                        (i32::from(tint[c]) * v as i32 / BASE_LEVEL as i32 + bright).clamp(0, 255);
                    *p = (*p * (255 - a as i32) + lit * a as i32) / 255;
                }
            }
        }
    }

    /// Blend an edge mask in one flat color. `strength` is 0..=255.
    fn edge(&mut self, sheet: &Sheet, dir: usize, color: [i32; 3], strength: i32) {
        for y in 0..TILE {
            for x in 0..TILE {
                let (_, a) = sheet.pixel(cell::EDGE + dir, x, y);
                let a = a as i32 * strength / 255;
                if a == 0 {
                    continue;
                }
                let px = &mut self.rgb[y * TILE + x];
                for (c, p) in px.iter_mut().enumerate() {
                    *p = (*p * (255 - a) + color[c] * a) / 255;
                }
            }
        }
    }

    /// Ore speckles: dark outline to bright highlight in the ore color.
    fn ore(&mut self, sheet: &Sheet, cell: usize, tint: Rgb) {
        for y in 0..TILE {
            for x in 0..TILE {
                let (v, a) = sheet.pixel(cell, x, y);
                if a == 0 {
                    continue;
                }
                let px = &mut self.rgb[y * TILE + x];
                for (c, p) in px.iter_mut().enumerate() {
                    let t = i32::from(tint[c]);
                    let dark = t * 35 / 100;
                    let light = t + (255 - t) * 45 / 100;
                    *p = dark + (light - dark) * v as i32 / 255;
                }
            }
        }
    }
}

/// Draw one tile of the map into `buf`.
fn draw_tile(
    map: &LocalMapData,
    palette: &Palette,
    sheet: &Sheet,
    mx: i32,
    my: i32,
    buf: &mut TileBuf,
) {
    let (Some(g), Some(r), Some(o)) = (
        map.ground.get(mx, my),
        map.rock.get(mx, my),
        map.ore.get(mx, my),
    ) else {
        return;
    };
    let h = tile_hash(mx, my, 1);
    let variant = (h & 3) as usize;
    let kind = kind_at(map, mx, my);
    let near = |dx: i32, dy: i32| kind_at(map, mx + dx, my + dy);
    // A small brightness change per tile hides the repeat of the 4 variants.
    let bright = ((h >> 8) % 9) as i32 - 4 + (tile_hash(mx / 4, my / 4, 3) % 17) as i32 - 8;

    if kind == Kind::Wall {
        let tint = palette.tile_color(*g, *r, Ore::None);
        buf.sprite(sheet, cell::WALL + variant, tint, bright);
        if *o != Ore::None {
            let ore_tint = palette.tile_color(*g, RockKind::None, *o);
            buf.ore(sheet, cell::ORE + ((h >> 4) & 3) as usize, ore_tint);
        }
        let open = |dx: i32, dy: i32| !matches!(near(dx, dy), Kind::Wall | Kind::Outside);
        // Light on the north and west edges, shade on the east edge, a face on the south.
        if open(0, -1) {
            buf.edge(sheet, 0, [255; 3], 70);
        }
        if open(-1, 0) {
            buf.edge(sheet, 3, [255; 3], 50);
        }
        if open(1, 0) {
            buf.edge(sheet, 1, [0; 3], 70);
        }
        if open(0, 1) {
            buf.sprite(sheet, cell::WALL_FACE + ((h >> 6) & 1) as usize, tint, 0);
        }
        return;
    }

    // Ground. Soil shows as grass with patches of bare soil.
    let patch = tile_hash(mx / 2, my / 2, 2) % 100;
    let (first, tint) = match g {
        Ground::Soil if patch < 90 => (cell::GRASS, palette.grass),
        Ground::Soil => (cell::SOIL, palette.soil),
        Ground::Sand => (cell::SAND, palette.sand),
        Ground::Clay => (cell::CLAY, palette.clay),
        Ground::Water => (cell::WATER, palette.water),
        Ground::Stone => (cell::FLOOR, palette.slate),
    };
    buf.sprite(sheet, first + variant, tint, bright);
    if kind == Kind::Water {
        // Foam along the shore.
        for (dir, (dx, dy)) in DIRS.iter().enumerate() {
            if near(*dx, *dy) == Kind::Open {
                buf.edge(sheet, dir, [225, 240, 250], 150);
            }
        }
    } else {
        for (dir, (dx, dy)) in DIRS.iter().enumerate() {
            match near(*dx, *dy) {
                // Wet ground next to water.
                Kind::Water => buf.edge(sheet, dir, [20, 40, 70], 70),
                // Shadow of a wall: stronger on the north side.
                Kind::Wall if dir == 0 => buf.edge(sheet, dir, [0; 3], 120),
                Kind::Wall if dir == 3 => buf.edge(sheet, dir, [0; 3], 60),
                _ => {}
            }
        }
    }
    if map.tree.get(mx, my).copied().unwrap_or(false) {
        buf.sprite(sheet, cell::TREE_SHADOW, [0; 3], 0);
        let shade = 90 + ((h >> 12) % 21) as i32;
        let tree = palette.tree_color();
        let tint = tree.map(|c| (i32::from(c) * shade / 100).min(255) as u8);
        buf.sprite(sheet, cell::TREE + ((h >> 10) & 1) as usize, tint, 0);
    }
}

/// RGBA pixels of one chunk (row-major). Tiles outside the map are black.
pub fn bake_chunk(
    map: &LocalMapData,
    palette: &Palette,
    sheet: &Sheet,
    chunk_x: u32,
    chunk_y: u32,
) -> Vec<u8> {
    let side = (CHUNK * PIXELS_PER_TILE) as usize;
    let mut bytes = vec![0u8; side * side * 4];
    for ty in 0..CHUNK as usize {
        for tx in 0..CHUNK as usize {
            let mx = ((chunk_x * CHUNK) as usize + tx) as i32;
            let my = ((chunk_y * CHUNK) as usize + ty) as i32;
            let mut buf = TileBuf {
                rgb: [[0; 3]; TILE * TILE],
            };
            draw_tile(map, palette, sheet, mx, my, &mut buf);
            for py in 0..TILE {
                for px in 0..TILE {
                    let rgb = buf.rgb[py * TILE + px];
                    let i = ((ty * TILE + py) * side + tx * TILE + px) * 4;
                    bytes[i..i + 4].copy_from_slice(&[
                        rgb[0].clamp(0, 255) as u8,
                        rgb[1].clamp(0, 255) as u8,
                        rgb[2].clamp(0, 255) as u8,
                        255,
                    ]);
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
    pub fn new(map: &LocalMapData, palette: &Palette, sheet: &Sheet) -> Self {
        let (chunks_x, chunks_y) = chunk_counts(map);
        let side = (CHUNK * PIXELS_PER_TILE) as u16;
        let mut chunks = Vec::new();
        for cy in 0..chunks_y {
            for cx in 0..chunks_x {
                let image = Image {
                    bytes: bake_chunk(map, palette, sheet, cx, cy),
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
        let color = Color::from_rgba(0, 0, 0, 40);
        for x in rect.min_x..=rect.max_x + 1 {
            let sx = camera.tile_to_screen((x as f32, 0.0), view).0.round();
            draw_line(sx, 0.0, sx, view.1, 1.0, color);
        }
        for y in rect.min_y..=rect.max_y + 1 {
            let sy = camera.tile_to_screen((0.0, y as f32), view).1.round();
            draw_line(0.0, sy, view.0, sy, 1.0, color);
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

/// Draw lines of text in a semi-transparent panel at the top left.
pub fn draw_overlay(lines: &[String]) {
    let line_height = FONT_SIZE + 6.0;
    let width = lines
        .iter()
        .map(|l| measure_text(l, None, FONT_SIZE as u16, 1.0).width)
        .fold(0.0_f32, f32::max);
    let (x, y) = (8.0, 8.0);
    let (w, h) = (width + 20.0, line_height * lines.len() as f32 + 12.0);
    draw_rectangle(x, y, w, h, Color::from_rgba(10, 12, 18, 200));
    draw_rectangle_lines(x, y, w, h, 1.0, Color::from_rgba(255, 255, 255, 50));
    for (i, line) in lines.iter().enumerate() {
        let ty = y + 6.0 + line_height * (i as f32 + 0.8);
        // A dark offset copy under the text keeps it readable on any background.
        draw_text(
            line,
            x + 11.0,
            ty + 1.0,
            FONT_SIZE,
            Color::from_rgba(0, 0, 0, 200),
        );
        draw_text(line, x + 10.0, ty, FONT_SIZE, WHITE);
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
            grass: c(0),
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

    fn real_palette() -> Palette {
        let text = include_str!("../../../assets/content/materials.ron");
        let materials = base::content::load_materials("materials.ron", text).unwrap_or_default();
        Palette::from_materials(&materials).unwrap_or_else(|_| palette())
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
        let Ok(sheet) = Sheet::shipped() else {
            return;
        };
        let map = generate_local(&EmbarkSummary::temperate_forest());
        let bytes = bake_chunk(&map, &real_palette(), &sheet, 0, 0);
        let side = (CHUNK * PIXELS_PER_TILE) as usize;
        assert_eq!(bytes.len(), side * side * 4);
        assert!(bytes.chunks(4).all(|px| px[3] == 255));
    }

    #[test]
    fn bake_is_repeatable_and_trees_change_pixels() {
        let Ok(sheet) = Sheet::shipped() else {
            return;
        };
        let p = real_palette();
        let mut map = generate_local(&EmbarkSummary::dry_plain());
        let a = bake_chunk(&map, &p, &sheet, 0, 0);
        assert_eq!(a, bake_chunk(&map, &p, &sheet, 0, 0));
        let open = (0..CHUNK as i32)
            .flat_map(|y| (0..CHUNK as i32).map(move |x| (x, y)))
            .find(|&(x, y)| kind_at(&map, x, y) == Kind::Open);
        let Some((x, y)) = open else { return };
        let was = map.tree.get(x, y).copied().unwrap_or(false);
        map.tree.set(x, y, !was);
        assert_ne!(a, bake_chunk(&map, &p, &sheet, 0, 0));
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
