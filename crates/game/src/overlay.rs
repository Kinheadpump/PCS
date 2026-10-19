//! Drawing on top of the map: stockpile zones, items on the ground, designation markers,
//! the drag preview, and tile outlines. Item sprites are small text patterns, tinted by the
//! material color like the other sprites (ART_DIRECTION §4). The pure parts (screen rectangles,
//! sprite images, item grouping) have tests.

use crate::camera::{Camera, TileRect, View};
use crate::font;
use crate::tools::{Area, Tool, count_applicable};
use crate::ui;
use base::content::MaterialCategory;
use macroquad::prelude::{
    Color, DrawTextureParams, FilterMode, Image, Texture2D, WHITE, draw_rectangle,
    draw_rectangle_lines, draw_texture_ex, vec2,
};
use sim::{ColonistId, ItemLoc, JobKind, Sim, StockpileId};
use std::cell::RefCell;
use std::collections::BTreeMap;

/// The selection color. Yellow is for selection only (ART_DIRECTION §4).
pub const SELECT: Color = Color::new(1.0, 242.0 / 255.0, 51.0 / 255.0, 1.0);
/// Smallest zoom at which stack counts and zone dots show.
const DETAIL_ZOOM: f32 = 16.0;

/// A color from bytes.
fn rgba(r: u8, g: u8, b: u8, a: u8) -> Color {
    Color::from_rgba(r, g, b, a)
}

/// The marker color of a tool.
pub fn tool_color(tool: Tool) -> Color {
    match tool {
        Tool::Select => SELECT,
        Tool::Cut => rgba(110, 230, 150, 255),
        Tool::Mine => rgba(255, 160, 70, 255),
        Tool::Haul => rgba(100, 180, 255, 255),
        Tool::Stockpile => rgba(80, 200, 200, 255),
        Tool::Cancel => rgba(240, 90, 90, 255),
    }
}

fn with_alpha(c: Color, a: f32) -> Color {
    Color::new(c.r, c.g, c.b, a)
}

/// What the overlay needs to place things on the screen.
#[derive(Clone, Copy)]
pub struct Frame<'a> {
    /// The simulation (read only).
    pub sim: &'a Sim,
    /// The camera.
    pub camera: &'a Camera,
    /// The viewport size.
    pub view: View,
    /// The map size in tiles.
    pub map_size: (u32, u32),
}

impl Frame<'_> {
    fn visible(&self) -> Option<TileRect> {
        self.camera.visible_tiles(self.view, self.map_size)
    }

    fn rect(&self, x: u16, y: u16) -> (f32, f32, f32, f32) {
        screen_rect(self.camera, self.view, i32::from(x), i32::from(y), 1, 1)
    }

    fn is_visible(&self, rect: &TileRect, x: u16, y: u16) -> bool {
        (rect.min_x..=rect.max_x).contains(&i32::from(x))
            && (rect.min_y..=rect.max_y).contains(&i32::from(y))
    }
}

/// Screen rectangle `(x, y, w, h)` of a block of tiles, rounded to whole pixels. Blocks that
/// touch on the map touch on the screen: no gap, no overlap.
pub fn screen_rect(
    camera: &Camera,
    view: View,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
) -> (f32, f32, f32, f32) {
    let a = camera.tile_to_screen((x as f32, y as f32), view);
    let b = camera.tile_to_screen(((x + w) as f32, (y + h) as f32), view);
    let (x0, y0) = (a.0.round(), a.1.round());
    (x0, y0, b.0.round() - x0, b.1.round() - y0)
}

/// Border thickness for a tile of the given screen size.
fn thickness(tile_px: f32) -> f32 {
    (tile_px / 16.0).round().max(1.0)
}

// ---------------------------------------------------------------------------------------------
// Item sprites
// ---------------------------------------------------------------------------------------------

/// The shape of an item sprite.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Shape {
    /// Logs.
    Logs,
    /// Pieces of stone.
    Rocks,
    /// Chunks of ore, with bright specks.
    OreChunks,
    /// Anything else.
    Block,
}

/// The sprite shape of a material category.
pub fn shape_for(category: MaterialCategory) -> Shape {
    match category {
        MaterialCategory::Wood => Shape::Logs,
        MaterialCategory::Stone => Shape::Rocks,
        MaterialCategory::Ore | MaterialCategory::Metal | MaterialCategory::Fuel => {
            Shape::OreChunks
        }
        _ => Shape::Block,
    }
}

// Symbols: `.` empty, `o` outline, `d` deep shade, `b` base, `l` light, `s` speck, `h` shadow.
// A row shorter than 16 is filled with empty pixels. The light comes from the north west.

const LOGS: [&str; 16] = [
    "",
    "",
    "",
    "...oooooooooo...",
    "..odlllllllbdo..",
    "..obbbbbbbbddo.h",
    ".oooooooooooooh.",
    ".odlllllllllbdoh",
    ".obbbbbbbbbbddoh",
    ".oooooooooooooo.",
    "..odllllllllbdoh",
    "..obbbbbbbbbddoh",
    "..ooooooooooooh.",
    "...hhhhhhhhhhhh.",
    "",
    "",
];

const ROCKS: [&str; 16] = [
    "",
    "",
    "",
    "",
    "....oooo........",
    "...ollllo.......",
    "..olbbbbdo.oooo.",
    "..obbbbbddoolbdo",
    "..obbbddddooddoh",
    "...ooddoooooooh.",
    "....hooooohhhhh.",
    "",
    "",
    "",
    "",
    "",
];

const ORE: [&str; 16] = [
    "",
    "",
    "",
    "",
    "....oooo........",
    "...olsslo.......",
    "..olbbsbdo.oooo.",
    "..obsbbbddoolsdo",
    "..obbbddsdooddoh",
    "...ooddoooooooh.",
    "....hooooohhhhh.",
    "",
    "",
    "",
    "",
    "",
];

const BLOCK: [&str; 16] = [
    "",
    "",
    "",
    "",
    "",
    "",
    "...ooooooooo....",
    "..ollllllllbo...",
    "..obbbbbbbbdoh..",
    "..obbbbbbbbdoh..",
    "..obbbbbbbbdoh..",
    "..oddddddddooh..",
    "...oooooooooh...",
    "....hhhhhhhhh...",
    "",
    "",
];

fn pattern(shape: Shape) -> &'static [&'static str; 16] {
    match shape {
        Shape::Logs => &LOGS,
        Shape::Rocks => &ROCKS,
        Shape::OreChunks => &ORE,
        Shape::Block => &BLOCK,
    }
}

/// RGBA pixels (16 x 16) of an item sprite in a material color. The value 224 of the
/// grayscale scale is the plain color (ART_DIRECTION §4).
pub fn item_image(shape: Shape, color: [u8; 3]) -> Vec<u8> {
    let tint =
        |level: u32| -> [u8; 3] { color.map(|c| (u32::from(c) * level / 224).min(255) as u8) };
    let speck = color.map(|c| (u32::from(c) + (255 - u32::from(c)) * 6 / 10) as u8);
    let mut bytes = vec![0u8; 16 * 16 * 4];
    for (y, row) in pattern(shape).iter().enumerate() {
        for (x, symbol) in row.chars().take(16).enumerate() {
            let rgba = match symbol {
                'o' => {
                    let [r, g, b] = tint(70);
                    [r, g, b, 255]
                }
                'd' => {
                    let [r, g, b] = tint(160);
                    [r, g, b, 255]
                }
                'b' => {
                    let [r, g, b] = tint(224);
                    [r, g, b, 255]
                }
                'l' => {
                    let [r, g, b] = tint(250);
                    [r, g, b, 255]
                }
                's' => [speck[0], speck[1], speck[2], 255],
                'h' => [0, 0, 0, 89],
                _ => continue,
            };
            let i = (y * 16 + x) * 4;
            bytes[i..i + 4].copy_from_slice(&rgba);
        }
    }
    bytes
}

thread_local! {
    /// One texture for each shape and color, made on first use (needs the window).
    static ITEM_TEXTURES: RefCell<BTreeMap<(Shape, [u8; 3]), Texture2D>> =
        const { RefCell::new(BTreeMap::new()) };
}

fn draw_item_sprite(shape: Shape, color: [u8; 3], rect: (f32, f32, f32, f32)) {
    ITEM_TEXTURES.with(|cell| {
        let mut textures = cell.borrow_mut();
        let texture = textures.entry((shape, color)).or_insert_with(|| {
            let image = Image {
                bytes: item_image(shape, color),
                width: 16,
                height: 16,
            };
            let texture = Texture2D::from_image(&image);
            texture.set_filter(FilterMode::Nearest);
            texture
        });
        draw_texture_ex(
            texture,
            rect.0,
            rect.1,
            WHITE,
            DrawTextureParams {
                dest_size: Some(vec2(rect.2, rect.3)),
                ..Default::default()
            },
        );
    });
}

/// One stack to draw: a material on a tile, the sum of the items there.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Stack {
    /// Tile column.
    pub x: u16,
    /// Tile row.
    pub y: u16,
    /// Material index.
    pub material: u16,
    /// Number of units.
    pub count: u32,
    /// True if any of the items has a haul order.
    pub to_haul: bool,
}

/// The stacks on the tiles of the rectangle, sorted by tile, then by material.
pub fn stacks_in(sim: &Sim, rect: &TileRect) -> Vec<Stack> {
    let mut map: BTreeMap<(u16, u16, u16), (u32, bool)> = BTreeMap::new();
    for (_, item) in sim.items().iter() {
        let ItemLoc::Tile(x, y) = item.loc else {
            continue;
        };
        if !(rect.min_x..=rect.max_x).contains(&i32::from(x))
            || !(rect.min_y..=rect.max_y).contains(&i32::from(y))
        {
            continue;
        }
        let entry = map.entry((y, x, item.material)).or_insert((0, false));
        entry.0 += item.count;
        entry.1 |= item.to_haul;
    }
    map.into_iter()
        .map(|((y, x, material), (count, to_haul))| Stack {
            x,
            y,
            material,
            count,
            to_haul,
        })
        .collect()
}

/// Text of a stack count, or `None` for one unit.
pub fn stack_label(count: u32) -> Option<String> {
    (count > 1).then(|| count.to_string())
}

fn material_look(sim: &Sim, material: u16) -> (Shape, [u8; 3]) {
    sim.rules()
        .materials
        .get(usize::from(material))
        .map_or((Shape::Block, [200, 200, 200]), |m| {
            (shape_for(m.category), m.color)
        })
}

/// Draw text with a dark shadow so it reads on any ground.
fn shadowed_text(text: &str, x: f32, y: f32, scale: u32, color: Color) {
    font::draw_text(
        text,
        x + scale as f32,
        y + scale as f32,
        scale,
        rgba(10, 10, 16, 230),
    );
    font::draw_text(text, x, y, scale, color);
}

/// Draw the items on the ground, with a stack count where there is more than one unit.
pub fn draw_items(f: &Frame) {
    let Some(visible) = f.visible() else { return };
    let stacks = stacks_in(f.sim, &visible);
    let mut last_tile = (u16::MAX, u16::MAX);
    let mut nth = 0.0_f32;
    for stack in stacks {
        if (stack.x, stack.y) == last_tile {
            nth += 1.0;
        } else {
            last_tile = (stack.x, stack.y);
            nth = 0.0;
        }
        let (x, y, w, h) = f.rect(stack.x, stack.y);
        let shift = (nth * w * 0.14).round();
        let (shape, color) = material_look(f.sim, stack.material);
        draw_item_sprite(shape, color, (x + shift, y - shift, w, h));
        if f.camera.zoom >= DETAIL_ZOOM
            && let Some(label) = stack_label(stack.count)
        {
            let scale = if f.camera.zoom >= 48.0 { 2 } else { 1 };
            let tw = (font::text_width(&label) * scale) as f32;
            let th = (font::GLYPH_H * scale) as f32;
            shadowed_text(
                &label,
                (x + w - tw - 2.0).round(),
                (y + h - th - 2.0).round(),
                scale,
                rgba(250, 250, 250, 255),
            );
        }
    }
}

/// Draw the item that a colonist carries, above its head.
pub fn draw_carried(f: &Frame, colonist_screen: (f32, f32), sprite_size: f32, id: ColonistId) {
    let Some(item) = f
        .sim
        .colonist(id)
        .and_then(|c| c.carrying)
        .and_then(|item| f.sim.items().get(item))
    else {
        return;
    };
    let (shape, color) = material_look(f.sim, item.material);
    let side = (sprite_size * 0.55).round();
    let x = (colonist_screen.0 - side / 2.0).round();
    let y = (colonist_screen.1 - sprite_size * 0.5 - side * 0.7).round();
    draw_item_sprite(shape, color, (x, y, side, side));
}

// ---------------------------------------------------------------------------------------------
// Zones
// ---------------------------------------------------------------------------------------------

/// Draw the stockpile zones as tinted areas.
pub fn draw_zones(f: &Frame, selected: Option<StockpileId>) {
    let Some(visible) = f.visible() else { return };
    let teal = tool_color(Tool::Stockpile);
    for (id, zone) in f.sim.stockpiles().iter() {
        let (x0, y0) = (i32::from(zone.x0), i32::from(zone.y0));
        let (x1, y1) = (i32::from(zone.x1), i32::from(zone.y1));
        if x1 < visible.min_x || x0 > visible.max_x || y1 < visible.min_y || y0 > visible.max_y {
            continue;
        }
        let (x, y, w, h) = screen_rect(f.camera, f.view, x0, y0, x1 - x0 + 1, y1 - y0 + 1);
        draw_rectangle(x, y, w, h, with_alpha(teal, 0.20));
        if f.camera.zoom >= DETAIL_ZOOM {
            let dot = thickness(f.camera.zoom) * 2.0;
            for ty in y0.max(visible.min_y)..=y1.min(visible.max_y) {
                for tx in x0.max(visible.min_x)..=x1.min(visible.max_x) {
                    let r = screen_rect(f.camera, f.view, tx, ty, 1, 1);
                    draw_rectangle(
                        (r.0 + r.2 / 2.0 - dot / 2.0).round(),
                        (r.1 + r.3 / 2.0 - dot / 2.0).round(),
                        dot,
                        dot,
                        with_alpha(teal, 0.45),
                    );
                }
            }
        }
        let is_selected = selected == Some(id);
        let (color, line) = if is_selected {
            (SELECT, thickness(f.camera.zoom).max(2.0))
        } else {
            (with_alpha(teal, 0.85), thickness(f.camera.zoom).max(1.0))
        };
        draw_rectangle_lines(x, y, w, h, line * 2.0, color);
    }
}

// ---------------------------------------------------------------------------------------------
// Designations
// ---------------------------------------------------------------------------------------------

/// A 5 x 5 pixel icon: `#` is set.
pub type Icon = [&'static str; 5];

/// The icon of a designation or tool: a slash for cut, an X for mine, an arrow for haul.
pub fn icon_for(tool: Tool) -> Icon {
    match tool {
        Tool::Cut => ["....#", "...#.", "..#..", ".#...", "#...."],
        Tool::Haul => ["..#..", "..#..", "#.#.#", ".###.", "..#.."],
        _ => ["#...#", ".#.#.", "..#..", ".#.#.", "#...#"],
    }
}

fn draw_icon(icon: &Icon, x: f32, y: f32, px: f32, color: Color) {
    let dark = rgba(10, 10, 16, 220);
    for pass in 0..2 {
        for (row, line) in icon.iter().enumerate() {
            for (col, c) in line.chars().enumerate() {
                if c == '#' {
                    let (ox, oy) = if pass == 0 { (px, px) } else { (0.0, 0.0) };
                    draw_rectangle(
                        x + col as f32 * px + ox,
                        y + row as f32 * px + oy,
                        px,
                        px,
                        if pass == 0 { dark } else { color },
                    );
                }
            }
        }
    }
}

/// Draw a marker on one tile: a light tint, a thin inset border, and an icon at the top right.
/// A claimed order has a stronger tint. The marker stays calm so colonists and items read first.
fn draw_marker(rect: (f32, f32, f32, f32), tool: Tool, claimed: bool) {
    let (x, y, w, h) = rect;
    let color = tool_color(tool);
    draw_rectangle(
        x,
        y,
        w,
        h,
        with_alpha(color, if claimed { 0.30 } else { 0.16 }),
    );
    if w < 8.0 {
        return;
    }
    let t = thickness(w);
    let inset = t * 2.0;
    draw_rectangle_lines(
        x + inset,
        y + inset,
        w - 2.0 * inset,
        h - 2.0 * inset,
        t,
        with_alpha(color, 0.55),
    );
    if w >= 24.0 {
        let px = (w / 16.0).floor().max(1.0);
        let icon_w = 5.0 * px;
        draw_icon(
            &icon_for(tool),
            x + w - icon_w - inset - px,
            y + inset + px * 2.0,
            px,
            color,
        );
    }
}

/// Draw the markers of designated tiles and of items with a haul order.
pub fn draw_designations(f: &Frame) {
    let Some(visible) = f.visible() else { return };
    for ((x, y), kind) in f.sim.designations() {
        if !f.is_visible(&visible, x, y) {
            continue;
        }
        let tool = match kind {
            JobKind::Cut => Tool::Cut,
            JobKind::Mine => Tool::Mine,
            JobKind::Haul => Tool::Haul,
        };
        let claimed = f
            .sim
            .jobs()
            .find(kind, x, y)
            .and_then(|j| f.sim.jobs().get(j))
            .is_some_and(|j| j.claimed_by.is_some());
        draw_marker(f.rect(x, y), tool, claimed);
    }
    for stack in stacks_in(f.sim, &visible) {
        if stack.to_haul && f.sim.designation(stack.x, stack.y).is_none() {
            draw_marker(f.rect(stack.x, stack.y), Tool::Haul, false);
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Preview and outlines
// ---------------------------------------------------------------------------------------------

/// Text of a drag preview, for example `4 x 3 (7)`.
pub fn preview_label(area: &Area, count: u32) -> String {
    format!("{} x {} ({count})", area.width(), area.height())
}

/// Draw the rectangle that a drag will use: fill, border, tiles that the tool works on, and a label.
pub fn draw_preview(f: &Frame, tool: Tool, area: Area) {
    let color = tool_color(tool);
    let (x0, y0) = (i32::from(area.x0), i32::from(area.y0));
    let (x, y, w, h) = screen_rect(
        f.camera,
        f.view,
        x0,
        y0,
        i32::from(area.x1) - x0 + 1,
        i32::from(area.y1) - y0 + 1,
    );
    draw_rectangle(x, y, w, h, with_alpha(color, 0.18));
    if let (Some(visible), true) = (f.visible(), f.camera.zoom >= 8.0)
        && tool != Tool::Stockpile
    {
        for (tx, ty) in area.tiles() {
            if f.is_visible(&visible, tx, ty) && crate::tools::applies(f.sim, tool, tx, ty) {
                let r = f.rect(tx, ty);
                draw_rectangle(r.0, r.1, r.2, r.3, with_alpha(color, 0.30));
            }
        }
    }
    draw_rectangle_lines(x, y, w, h, 3.0, color);
    let label = preview_label(&area, count_applicable(f.sim, tool, area));
    let tw = ui::text_px(&label);
    let (lx, ly) = (x, (y - 24.0).max(2.0));
    draw_rectangle(lx, ly, tw + 12.0, 22.0, rgba(10, 12, 18, 220));
    draw_rectangle_lines(lx, ly, tw + 12.0, 22.0, 1.0, with_alpha(color, 0.9));
    font::draw_text(&label, lx + 6.0, ly + 3.0, ui::SCALE, WHITE);
}

/// Draw an outline around one tile.
pub fn draw_tile_outline(f: &Frame, tile: (u16, u16), color: Color) {
    let (x, y, w, h) = f.rect(tile.0, tile.1);
    draw_rectangle_lines(x, y, w, h, thickness(w).max(2.0), color);
}

/// The outline color of the tile under the mouse when a tool is active.
pub fn hover_color() -> Color {
    rgba(255, 255, 255, 170)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::camera::Camera;
    use base::content::MaterialCategory as C;

    #[test]
    fn every_sprite_row_has_at_most_16_pixels_and_uses_known_symbols() {
        for shape in [Shape::Logs, Shape::Rocks, Shape::OreChunks, Shape::Block] {
            let rows = pattern(shape);
            assert_eq!(rows.len(), 16);
            for row in rows {
                assert!(row.chars().count() <= 16, "{shape:?}: {row}");
                assert!(
                    row.chars().all(|c| ".odblsh".contains(c)),
                    "{shape:?}: {row}"
                );
            }
        }
    }

    #[test]
    fn item_images_are_16_by_16_rgba_with_an_outline_and_a_filled_body() {
        for shape in [Shape::Logs, Shape::Rocks, Shape::OreChunks, Shape::Block] {
            let bytes = item_image(shape, [120, 84, 52]);
            assert_eq!(bytes.len(), 16 * 16 * 4);
            let solid = bytes.chunks(4).filter(|p| p[3] == 255).count();
            assert!(solid > 40, "{shape:?} has {solid} solid pixels");
            // Pixels outside the shape stay empty: the corners.
            assert_eq!(bytes[3], 0);
            assert_eq!(bytes[bytes.len() - 1], 0);
        }
    }

    #[test]
    fn the_outline_is_darker_than_the_base_and_the_base_is_the_plain_color() {
        let color = [224, 112, 56];
        let bytes = item_image(Shape::Block, color);
        let px = |x: usize, y: usize| &bytes[(y * 16 + x) * 4..(y * 16 + x) * 4 + 4];
        // Row 6 starts with the outline; row 9 has a base pixel at column 5.
        let outline = px(4, 6);
        let base = px(5, 9);
        assert_eq!(base[..3], color);
        assert!(outline[0] < base[0] / 2, "the outline is dark");
    }

    #[test]
    fn ore_specks_are_lighter_than_the_ore_color() {
        let color = [150, 84, 66];
        let bytes = item_image(Shape::OreChunks, color);
        let lighter = bytes
            .chunks(4)
            .filter(|p| p[3] == 255 && p[0] > color[0] + 40 && p[1] > color[1] + 40)
            .count();
        assert!(lighter >= 2);
    }

    #[test]
    fn shape_follows_the_material_category() {
        assert_eq!(shape_for(C::Wood), Shape::Logs);
        assert_eq!(shape_for(C::Stone), Shape::Rocks);
        assert_eq!(shape_for(C::Ore), Shape::OreChunks);
        assert_eq!(shape_for(C::Food), Shape::Block);
    }

    #[test]
    fn stack_labels_show_only_for_more_than_one_unit() {
        assert_eq!(stack_label(1), None);
        assert_eq!(stack_label(12), Some("12".to_owned()));
    }

    #[test]
    fn neighbor_tiles_touch_on_the_screen_at_any_zoom() {
        let view = (1280.0, 720.0);
        for zoom in [7.3, 16.0, 21.7, 32.0, 64.0] {
            let mut camera = Camera::new((256, 256), zoom);
            camera.center = (100.37, 80.91);
            for x in 90..100 {
                let a = screen_rect(&camera, view, x, 70, 1, 1);
                let b = screen_rect(&camera, view, x + 1, 70, 1, 1);
                assert_eq!(a.0 + a.2, b.0, "gap at zoom {zoom}");
            }
            let block = screen_rect(&camera, view, 90, 70, 3, 2);
            let first = screen_rect(&camera, view, 90, 70, 1, 1);
            let last = screen_rect(&camera, view, 92, 71, 1, 1);
            assert_eq!(block.0, first.0);
            assert_eq!(block.0 + block.2, last.0 + last.2);
            assert_eq!(block.1 + block.3, last.1 + last.3);
        }
    }

    #[test]
    fn stacks_add_the_units_of_a_material_on_a_tile() {
        let mut sim = sim::scenario::sim_from_ascii("c...", 1).unwrap();
        let log = sim.rules().log;
        sim.debug_spawn_item(log, 3, 2, 0);
        sim.debug_spawn_item(log, 4, 2, 0);
        sim.debug_spawn_item(log, 1, 3, 0);
        let all = TileRect {
            min_x: 0,
            min_y: 0,
            max_x: 3,
            max_y: 0,
        };
        let stacks = stacks_in(&sim, &all);
        let units: u32 = stacks.iter().map(|s| s.count).sum();
        assert_eq!(units, 8);
        assert_eq!(
            stacks
                .iter()
                .filter(|s| s.x == 2)
                .map(|s| s.count)
                .sum::<u32>(),
            7
        );
        assert!(
            stacks
                .windows(2)
                .all(|w| (w[0].y, w[0].x) <= (w[1].y, w[1].x))
        );
        let narrow = TileRect { min_x: 3, ..all };
        assert_eq!(stacks_in(&sim, &narrow).len(), 1);
    }

    #[test]
    fn icons_are_5_by_5_and_distinct() {
        let all = [
            icon_for(Tool::Cut),
            icon_for(Tool::Mine),
            icon_for(Tool::Haul),
        ];
        for icon in all {
            assert!(icon.iter().all(|r| r.chars().count() == 5));
            assert!(icon.iter().any(|r| r.contains('#')));
        }
        assert_ne!(all[0], all[1]);
        assert_ne!(all[1], all[2]);
    }

    #[test]
    fn tool_colors_are_distinct_and_only_select_is_yellow() {
        let yellow = |c: Color| c.r > 0.9 && c.g > 0.9 && c.b < 0.3;
        for tool in Tool::ALL {
            assert_eq!(yellow(tool_color(tool)), tool == Tool::Select, "{tool:?}");
        }
        let mut seen: Vec<[u8; 3]> = Vec::new();
        for tool in Tool::ALL {
            let c = tool_color(tool);
            let key = [
                (c.r * 255.0) as u8,
                (c.g * 255.0) as u8,
                (c.b * 255.0) as u8,
            ];
            assert!(!seen.contains(&key));
            seen.push(key);
        }
    }

    #[test]
    fn the_preview_label_gives_size_and_count() {
        let area = Area::from_corners((2, 2), (5, 4));
        assert_eq!(preview_label(&area, 7), "4 x 3 (7)");
    }
}
