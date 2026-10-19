//! Drawing on top of the map: stockpile zones, items on the ground, designation markers,
//! the drag preview, and tile outlines. Item sprites come from `items.png` (gray, with three pile
//! sizes) and get the material color through `palette::ramp`. The pure parts (screen rectangles,
//! item images, item grouping) have tests.

use crate::camera::{Camera, TileRect, View};
use crate::cells::items as it;
use crate::font;
use crate::icons;
use crate::palette::{self as pal, Rgb, ramp};
use crate::render::color as rgb_color;
use crate::sprites::{Png, Sprites};
use crate::tools::{Area, Tool, count_applicable};
use crate::ui;
use base::content::MaterialCategory;
use macroquad::prelude::{
    Color, DrawTextureParams, FilterMode, Image, Texture2D, WHITE, draw_rectangle, draw_texture_ex,
    vec2,
};
use sim::{ColonistId, ItemLoc, JobKind, Sim, StockpileId};
use std::cell::RefCell;
use std::collections::BTreeMap;

/// The selection color. Yellow is for selection only (ART_DIRECTION section 4).
pub const SELECT: Color = Color::new(1.0, 242.0 / 255.0, 51.0 / 255.0, 1.0);
/// Smallest zoom at which stack counts show.
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

/// Border thickness for a tile of the given screen size: one sprite pixel.
fn thickness(tile_px: f32) -> f32 {
    (tile_px / 16.0).round().max(1.0)
}

// ---------------------------------------------------------------------------------------------
// Item sprites
// ---------------------------------------------------------------------------------------------

/// The shape of an item sprite. Each shape has three pile sizes in `items.png`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Shape {
    /// Logs.
    Logs,
    /// Pieces of stone.
    Rocks,
    /// Chunks of ore, with bright specks.
    OreChunks,
    /// Ingots of metal.
    Ingots,
    /// Lumps of fuel.
    Lumps,
    /// Anything else: a crate.
    Block,
}

impl Shape {
    /// The row of the shape in the sheet (see `cells::items`).
    pub fn index(self) -> usize {
        match self {
            Shape::Logs => it::LOGS,
            Shape::Rocks => it::ROCKS,
            Shape::OreChunks => it::ORE,
            Shape::Ingots => it::INGOTS,
            Shape::Lumps => it::LUMPS,
            Shape::Block => it::CRATE,
        }
    }
}

/// The sprite shape of a material category.
pub fn shape_for(category: MaterialCategory) -> Shape {
    match category {
        MaterialCategory::Wood => Shape::Logs,
        MaterialCategory::Stone => Shape::Rocks,
        MaterialCategory::Ore => Shape::OreChunks,
        MaterialCategory::Metal => Shape::Ingots,
        MaterialCategory::Fuel => Shape::Lumps,
        _ => Shape::Block,
    }
}

/// RGBA pixels (16 x 16) of an item sprite in a material color. Gray pixels go through the ramp:
/// level 224 is the plain color, lower levels turn darker and cooler, higher levels turn lighter
/// and warmer. Shadow pixels (alpha below 255) stay black.
pub fn item_image(items: &Png, shape: Shape, tier: usize, color: Rgb) -> Vec<u8> {
    let cell = it::cell(shape.index(), tier);
    let mut bytes = items.cell_bytes(cell);
    for px in bytes.chunks_mut(4) {
        if px[3] == 255 {
            let c = ramp(color, i32::from(px[0]));
            px[..3].copy_from_slice(&c);
        }
    }
    bytes
}

/// The loaded sheet and one texture for each shape, pile size, and color.
type Cache = (Option<Sprites>, BTreeMap<(Shape, usize, Rgb), Texture2D>);

thread_local! {
    /// The item sheet and one texture for each shape, pile size, and color, made on first use.
    static ITEM_TEXTURES: RefCell<Cache> =
        const { RefCell::new((None, BTreeMap::new())) };
}

fn draw_item_sprite(shape: Shape, tier: usize, color: Rgb, rect: (f32, f32, f32, f32)) {
    ITEM_TEXTURES.with(|store| {
        let mut store = store.borrow_mut();
        let (sprites, textures) = &mut *store;
        if sprites.is_none() {
            *sprites = Sprites::shipped().ok();
        }
        let Some(sprites) = sprites.as_ref() else {
            return;
        };
        let texture = textures.entry((shape, tier, color)).or_insert_with(|| {
            let image = Image {
                bytes: item_image(&sprites.items, shape, tier, color),
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

fn material_look(sim: &Sim, material: u16) -> (Shape, Rgb) {
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
        rgb_color(pal::ui::INK, 230),
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
        let tier = it::tier_for(stack.count);
        draw_item_sprite(shape, tier, color, (x + shift, y - shift, w, h));
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
                rgb_color(pal::ui::TEXT, 255),
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
    draw_item_sprite(shape, 0, color, (x, y, side, side));
}

// ---------------------------------------------------------------------------------------------
// Zones
// ---------------------------------------------------------------------------------------------

/// Draw a dashed rectangle border of `t` pixels. Dashes and gaps are `dash` pixels long.
pub fn dashed_rect(x: f32, y: f32, w: f32, h: f32, t: f32, dash: f32, color: Color) {
    let run = |start: f32, len: f32, at: &dyn Fn(f32, f32) -> (f32, f32, f32, f32)| {
        let mut p = 0.0;
        while p < len {
            let l = dash.min(len - p);
            let (rx, ry, rw, rh) = at(start + p, l);
            draw_rectangle(rx, ry, rw, rh, color);
            p += 2.0 * dash;
        }
    };
    run(x, w, &|p, l| (p, y, l, t));
    run(x, w, &|p, l| (p, y + h - t, l, t));
    run(y, h, &|p, l| (x, p, t, l));
    run(y, h, &|p, l| (x + w - t, p, t, l));
}

/// Draw the stockpile zones: a checked floor tint and a dashed border.
pub fn draw_zones(f: &Frame, selected: Option<StockpileId>) {
    let Some(visible) = f.visible() else { return };
    let teal = tool_color(Tool::Stockpile);
    for (id, zone) in f.sim.stockpiles().iter() {
        let (x0, y0) = (i32::from(zone.x0), i32::from(zone.y0));
        let (x1, y1) = (i32::from(zone.x1), i32::from(zone.y1));
        if x1 < visible.min_x || x0 > visible.max_x || y1 < visible.min_y || y0 > visible.max_y {
            continue;
        }
        for ty in y0.max(visible.min_y)..=y1.min(visible.max_y) {
            for tx in x0.max(visible.min_x)..=x1.min(visible.max_x) {
                let (rx, ry, rw, rh) = screen_rect(f.camera, f.view, tx, ty, 1, 1);
                let alpha = if (tx + ty) % 2 == 0 { 0.10 } else { 0.20 };
                draw_rectangle(rx, ry, rw, rh, with_alpha(teal, alpha));
            }
        }
        let (x, y, w, h) = screen_rect(f.camera, f.view, x0, y0, x1 - x0 + 1, y1 - y0 + 1);
        let t = thickness(f.camera.zoom);
        let is_selected = selected == Some(id);
        let color = if is_selected {
            SELECT
        } else {
            with_alpha(teal, 0.95)
        };
        let dash = (f.camera.zoom / 4.0).round().max(2.0);
        dashed_rect(x, y, w, h, t * 2.0, dash, rgb_color(pal::ui::INK, 150));
        dashed_rect(x, y, w, h, t, dash, color);
    }
}

// ---------------------------------------------------------------------------------------------
// Designations
// ---------------------------------------------------------------------------------------------

/// Draw a marker on one tile: a light tint, four corner brackets, and a badge at the top right.
/// A claimed order has a stronger tint. The marker stays calm so colonists and items read first.
fn draw_marker(rect: (f32, f32, f32, f32), tool: Tool, claimed: bool) {
    let (x, y, w, h) = rect;
    let color = tool_color(tool);
    draw_rectangle(
        x,
        y,
        w,
        h,
        with_alpha(color, if claimed { 0.30 } else { 0.14 }),
    );
    if w < 8.0 {
        return;
    }
    icons::draw(crate::cells::icons::BRACKETS, x, y, w, color);
    if w >= 20.0
        && let Some(badge) = icons::badge_for(tool)
    {
        icons::draw(badge, x, y, w, color);
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
            // Building orders have no tool in the interface yet (Sprint 4 game side).
            _ => continue,
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

/// Draw the rectangle that a drag will use: fill, dashed border, tiles that the tool works on, and
/// a label.
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
    draw_rectangle(x, y, w, h, with_alpha(color, 0.14));
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
    let t = thickness(f.camera.zoom);
    let dash = (f.camera.zoom / 4.0).round().max(2.0);
    dashed_rect(x, y, w, h, t * 2.0, dash, rgb_color(pal::ui::INK, 170));
    dashed_rect(x, y, w, h, t, dash, color);
    let label = preview_label(&area, count_applicable(f.sim, tool, area));
    let tw = ui::text_px(&label);
    let (lx, ly) = (x, (y - 26.0).max(2.0));
    ui::draw_tag(lx, ly, tw + 12.0, 22.0, color);
    font::draw_text(
        &label,
        lx + 6.0,
        ly + 4.0,
        ui::SCALE,
        rgb_color(pal::ui::TEXT, 255),
    );
}

/// Draw an outline around one tile: four corner brackets in `color`.
pub fn draw_tile_outline(f: &Frame, tile: (u16, u16), color: Color) {
    let (x, y, w, _) = f.rect(tile.0, tile.1);
    icons::draw(crate::cells::icons::BRACKETS, x, y, w, color);
}

/// The outline color of the tile under the mouse when a tool is active.
pub fn hover_color() -> Color {
    rgba(255, 255, 255, 200)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::camera::Camera;
    use base::content::MaterialCategory as C;

    fn items() -> Option<Png> {
        Sprites::shipped().ok().map(|s| s.items)
    }

    #[test]
    fn item_images_are_16_by_16_rgba_with_an_outline_and_a_filled_body() {
        let Some(sheet) = items() else { return };
        for shape in [
            Shape::Logs,
            Shape::Rocks,
            Shape::OreChunks,
            Shape::Ingots,
            Shape::Lumps,
            Shape::Block,
        ] {
            for tier in 0..it::TIERS {
                let bytes = item_image(&sheet, shape, tier, [120, 84, 52]);
                assert_eq!(bytes.len(), 16 * 16 * 4);
                let solid = bytes.chunks(4).filter(|p| p[3] == 255).count();
                assert!(solid > 20, "{shape:?} {tier} has {solid} solid pixels");
                // Pixels outside the shape stay empty: the corners.
                assert_eq!(bytes[3], 0);
                assert_eq!(bytes[bytes.len() - 1], 0);
            }
        }
    }

    #[test]
    fn bigger_piles_have_more_pixels() {
        let Some(sheet) = items() else { return };
        let solid = |tier| {
            item_image(&sheet, Shape::Rocks, tier, [150, 150, 150])
                .chunks(4)
                .filter(|p| p[3] == 255)
                .count()
        };
        assert!(solid(0) < solid(1) && solid(1) < solid(2));
    }

    #[test]
    fn the_outline_is_darker_than_the_body_and_the_light_is_lighter() {
        let Some(sheet) = items() else { return };
        let color = [200, 120, 60];
        let bytes = item_image(&sheet, Shape::Block, 1, color);
        let lum = |p: &[u8]| u32::from(p[0]) * 3 + u32::from(p[1]) * 6 + u32::from(p[2]);
        let solid: Vec<&[u8]> = bytes.chunks(4).filter(|p| p[3] == 255).collect();
        let darkest = solid.iter().map(|p| lum(p)).min().unwrap_or(0);
        let lightest = solid.iter().map(|p| lum(p)).max().unwrap_or(0);
        assert!(darkest * 2 < lum(&[color[0], color[1], color[2], 255]));
        assert!(lightest > lum(&[color[0], color[1], color[2], 255]));
    }

    #[test]
    fn shape_follows_the_material_category() {
        assert_eq!(shape_for(C::Wood), Shape::Logs);
        assert_eq!(shape_for(C::Stone), Shape::Rocks);
        assert_eq!(shape_for(C::Ore), Shape::OreChunks);
        assert_eq!(shape_for(C::Metal), Shape::Ingots);
        assert_eq!(shape_for(C::Fuel), Shape::Lumps);
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
