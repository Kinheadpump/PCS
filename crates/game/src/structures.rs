//! Drawing of buildings: floors, walls and other objects, roofs, blueprint ghosts with
//! progress, the build preview, and the rooms overlay. The sprites come from `buildings.png`
//! (gray, with an alpha code for the color class). The pure parts (which cell, which pixels)
//! have tests.

use crate::cells::buildings as bc;
use crate::font;
use crate::overlay::{Frame, screen_rect, thickness, tool_color, with_alpha};
use crate::palette::{self as pal, Rgb, ramp};
use crate::render::color as rgb_color;
use crate::sprites::{Png, Sprites};
use crate::tools::Tool;
use crate::ui;
use base::content::{BuildingKind, MaterialCategory};
use macroquad::prelude::{
    Color, DrawTextureParams, FilterMode, Image, Texture2D, WHITE, draw_rectangle, draw_texture_ex,
    vec2,
};
use sim::{BuildLayer, Sim};
use std::collections::BTreeMap;

/// Color of cloth (blanket) pixels.
pub const CLOTH: Rgb = [168, 72, 64];
/// Color of fire pixels (hearth).
pub const FIRE: Rgb = [240, 140, 40];
/// Color of metal pixels (tools).
pub const METAL: Rgb = [176, 180, 188];
/// Opacity of a roof. The rooms below stay visible.
pub const ROOF_ALPHA: f32 = 0.30;
/// Opacity of a blueprint ghost.
const GHOST_ALPHA: f32 = 0.55;
/// Opacity of the tint of a room in the rooms overlay.
const ROOM_ALPHA: f32 = 0.30;
/// Smallest zoom at which the progress bar of a blueprint shows.
const BAR_ZOOM: f32 = 8.0;
/// Ticks of one flame frame of a hearth.
const FLAME_TICKS: u64 = 14;

/// Tints of the rooms. Yellow is for selection only (ART_DIRECTION section 4).
const ROOM_TINTS: [Rgb; 6] = [
    [70, 190, 210],
    [170, 110, 230],
    [90, 210, 130],
    [240, 120, 150],
    [100, 150, 240],
    [240, 150, 90],
];

/// RGBA pixels (16 x 16) of a building cell in a material color. Material pixels go through the
/// ramp. Cloth, fire, and metal pixels use their own colors. Shadow pixels stay black.
pub fn building_image(sheet: &Png, cell: usize, material: Rgb) -> Vec<u8> {
    let mut bytes = sheet.cell_bytes(cell);
    for px in bytes.chunks_mut(4) {
        let tint = match px[3] {
            bc::MATERIAL_ALPHA => material,
            bc::CLOTH_ALPHA => CLOTH,
            bc::FIRE_ALPHA => FIRE,
            bc::METAL_ALPHA => METAL,
            _ => continue,
        };
        let c = ramp(tint, i32::from(px[0]));
        px[..3].copy_from_slice(&c);
        px[3] = 255;
    }
    bytes
}

/// The connection bits of a tile: one bit for each side where `joins` says yes.
pub fn neighbor_mask(x: u16, y: u16, joins: impl Fn(u16, u16) -> bool) -> u8 {
    let mut mask = 0;
    if y > 0 && joins(x, y - 1) {
        mask |= bc::NORTH;
    }
    if joins(x.saturating_add(1), y) && x < u16::MAX {
        mask |= bc::EAST;
    }
    if joins(x, y.saturating_add(1)) && y < u16::MAX {
        mask |= bc::SOUTH;
    }
    if x > 0 && joins(x - 1, y) {
        mask |= bc::WEST;
    }
    mask
}

/// True if the material is wood (wood sprites) and not stone.
pub fn is_wood(sim: &Sim, material: u16) -> bool {
    sim.rules()
        .materials
        .get(usize::from(material))
        .is_some_and(|m| m.category == MaterialCategory::Wood)
}

/// The sheet cell of a building. `mask` is the set of connected sides (walls and roofs);
/// `horizontal` is true for a door in a wall that runs east to west; `frame` is the flame
/// frame of a hearth.
pub fn cell_for(
    sim: &Sim,
    building: u16,
    material: u16,
    mask: u8,
    horizontal: bool,
    frame: usize,
) -> Option<usize> {
    let wood = is_wood(sim, material);
    let kind = sim.rules().building(building)?.kind;
    Some(match kind {
        BuildingKind::Wall => bc::wall_cell(wood, mask),
        BuildingKind::Roof => bc::roof_cell(mask),
        BuildingKind::Door if horizontal => bc::DOOR_H,
        BuildingKind::Door => bc::DOOR_V,
        BuildingKind::Floor if wood => bc::FLOOR_WOOD,
        BuildingKind::Floor => bc::FLOOR_STONE,
        BuildingKind::Bed => bc::BED,
        BuildingKind::Hearth if frame.is_multiple_of(2) => bc::HEARTH,
        BuildingKind::Hearth => bc::HEARTH_B,
        BuildingKind::Workshop => bc::WORKSHOP,
    })
}

/// True if the building kind joins to walls: walls and doors.
fn is_wall_like(kind: BuildingKind) -> bool {
    matches!(kind, BuildingKind::Wall | BuildingKind::Door)
}

/// True if a finished wall or door stands on the tile. With `ghosts`, a blueprint of a wall or
/// a door counts too.
fn wall_at(sim: &Sim, x: u16, y: u16, ghosts: bool) -> bool {
    let b = sim.buildings();
    let kind_of = |def: u16| sim.rules().building(def).map(|d| d.kind);
    b.object_at(x, y)
        .and_then(|o| kind_of(o.def))
        .is_some_and(is_wall_like)
        || (ghosts
            && b.blueprint(x, y, BuildLayer::Object)
                .and_then(|bp| kind_of(bp.building))
                .is_some_and(is_wall_like))
}

/// True if a roof or a roof blueprint is on the tile.
fn roof_at(sim: &Sim, x: u16, y: u16, ghosts: bool) -> bool {
    let b = sim.buildings();
    b.roof_at(x, y).is_some() || (ghosts && b.blueprint(x, y, BuildLayer::Roof).is_some())
}

/// The flame frame at a tick.
pub fn flame_frame(tick: u64) -> usize {
    usize::from(tick / FLAME_TICKS % 2 == 1)
}

/// The sheet cell of a finished or planned building on a tile, read from the sim around it.
pub fn cell_at(
    sim: &Sim,
    building: u16,
    material: u16,
    x: u16,
    y: u16,
    ghosts: bool,
) -> Option<usize> {
    let kind = sim.rules().building(building)?.kind;
    let mask = match kind {
        BuildingKind::Wall => neighbor_mask(x, y, |nx, ny| wall_at(sim, nx, ny, ghosts)),
        BuildingKind::Roof => neighbor_mask(x, y, |nx, ny| roof_at(sim, nx, ny, ghosts)),
        _ => 0,
    };
    // A door fits a wall that runs east to west when a wall is east or west of it.
    let horizontal =
        wall_at(sim, x.saturating_add(1), y, ghosts) || (x > 0 && wall_at(sim, x - 1, y, ghosts));
    cell_for(
        sim,
        building,
        material,
        mask,
        horizontal,
        flame_frame(sim.tick()),
    )
}

/// The textures of the building cells, one for each cell and material color.
#[derive(Default)]
pub struct Structures {
    textures: BTreeMap<(usize, Rgb), Texture2D>,
}

impl Structures {
    /// Empty cache.
    pub fn new() -> Self {
        Self::default()
    }

    fn texture(&mut self, sprites: &Sprites, cell: usize, color: Rgb) -> &Texture2D {
        self.textures.entry((cell, color)).or_insert_with(|| {
            let image = Image {
                bytes: building_image(&sprites.buildings, cell, color),
                width: 16,
                height: 16,
            };
            let texture = Texture2D::from_image(&image);
            texture.set_filter(FilterMode::Nearest);
            texture
        })
    }

    fn draw_cell(
        &mut self,
        sprites: &Sprites,
        cell: usize,
        color: Rgb,
        rect: (f32, f32, f32, f32),
        tint: Color,
    ) {
        let texture = self.texture(sprites, cell, color);
        draw_texture_ex(
            texture,
            rect.0,
            rect.1,
            tint,
            DrawTextureParams {
                dest_size: Some(vec2(rect.2, rect.3)),
                ..Default::default()
            },
        );
    }

    /// Draw the finished buildings of one layer.
    fn draw_layer(&mut self, f: &Frame, sprites: &Sprites, layer: BuildLayer, tint: Color) {
        let Some(visible) = f.camera.visible_tiles(f.view, f.map_size) else {
            return;
        };
        let rules = f.sim.rules();
        for (x, y, l, b) in f.sim.buildings().iter() {
            if l != layer || !f.is_visible(&visible, x, y) {
                continue;
            }
            let Some(cell) = cell_at(f.sim, b.def, b.material, x, y, false) else {
                continue;
            };
            let color = rules
                .materials
                .get(usize::from(b.material))
                .map_or([200, 200, 200], |m| m.color);
            self.draw_cell(sprites, cell, color, f.rect(x, y), tint);
        }
    }

    /// Draw the floors.
    pub fn draw_floors(&mut self, f: &Frame, sprites: &Sprites) {
        self.draw_layer(f, sprites, BuildLayer::Floor, WHITE);
    }

    /// Draw the walls, doors, and furniture.
    pub fn draw_objects(&mut self, f: &Frame, sprites: &Sprites) {
        self.draw_layer(f, sprites, BuildLayer::Object, WHITE);
    }

    /// Draw the roofs, half see-through.
    pub fn draw_roofs(&mut self, f: &Frame, sprites: &Sprites) {
        self.draw_layer(
            f,
            sprites,
            BuildLayer::Roof,
            Color::new(1.0, 1.0, 1.0, ROOF_ALPHA),
        );
    }

    /// Draw the blueprints as ghosts, each with a progress bar.
    pub fn draw_blueprints(&mut self, f: &Frame, sprites: &Sprites) {
        let Some(visible) = f.camera.visible_tiles(f.view, f.map_size) else {
            return;
        };
        let rules = f.sim.rules();
        for (x, y, layer, bp) in f.sim.buildings().blueprints() {
            if !f.is_visible(&visible, x, y) {
                continue;
            }
            let rect = f.rect(x, y);
            if let Some(cell) = cell_at(f.sim, bp.building, bp.material, x, y, true) {
                let color = rules
                    .materials
                    .get(usize::from(bp.material))
                    .map_or([200, 200, 200], |m| m.color);
                let alpha = if layer == BuildLayer::Roof {
                    GHOST_ALPHA * 0.8
                } else {
                    GHOST_ALPHA
                };
                self.draw_cell(
                    sprites,
                    cell,
                    color,
                    rect,
                    Color::new(0.8, 0.92, 1.0, alpha),
                );
            }
            let blue = tool_color(Tool::Build);
            let t = thickness(f.camera.zoom);
            draw_rectangle(rect.0, rect.1, rect.2, t, with_alpha(blue, 0.85));
            draw_rectangle(
                rect.0,
                rect.1 + rect.3 - t,
                rect.2,
                t,
                with_alpha(blue, 0.85),
            );
            draw_rectangle(rect.0, rect.1, t, rect.3, with_alpha(blue, 0.85));
            draw_rectangle(
                rect.0 + rect.2 - t,
                rect.1,
                t,
                rect.3,
                with_alpha(blue, 0.85),
            );
            if f.camera.zoom >= BAR_ZOOM {
                let pct = crate::build_ui::blueprint_progress(f.sim, x, y, layer).unwrap_or(0);
                draw_progress(rect, pct, t);
            }
        }
    }

    /// Draw the ghosts of the build preview. A tile where the building cannot go gets a red
    /// tint. Returns nothing; the label goes through [`draw_build_label`].
    pub fn draw_build_preview(
        &mut self,
        f: &Frame,
        sprites: &Sprites,
        building: u16,
        material: u16,
        tiles: &[(u16, u16)],
    ) {
        let Some(visible) = f.camera.visible_tiles(f.view, f.map_size) else {
            return;
        };
        let rules = f.sim.rules();
        let color = rules
            .materials
            .get(usize::from(material))
            .map_or([200, 200, 200], |m| m.color);
        for &(x, y) in tiles {
            if !f.is_visible(&visible, x, y) {
                continue;
            }
            let rect = f.rect(x, y);
            let ok = crate::build_ui::can_place(f.sim, building, material, x, y).is_ok();
            if ok {
                // Walls and roofs in the drag join each other.
                let cell = preview_cell(f.sim, building, material, x, y, tiles);
                if let Some(cell) = cell {
                    self.draw_cell(sprites, cell, color, rect, Color::new(0.9, 1.0, 0.95, 0.72));
                }
                let blue = tool_color(Tool::Build);
                draw_rectangle(rect.0, rect.1, rect.2, rect.3, with_alpha(blue, 0.28));
                draw_frame(rect, thickness(f.camera.zoom), with_alpha(blue, 0.9));
            } else {
                draw_rectangle(
                    rect.0,
                    rect.1,
                    rect.2,
                    rect.3,
                    rgb_color([230, 70, 70], 120),
                );
            }
        }
    }
}

/// The cell of a preview tile. Walls and roofs also join the other tiles of the drag.
fn preview_cell(
    sim: &Sim,
    building: u16,
    material: u16,
    x: u16,
    y: u16,
    tiles: &[(u16, u16)],
) -> Option<usize> {
    let kind = sim.rules().building(building)?.kind;
    let in_drag = |nx: u16, ny: u16| tiles.contains(&(nx, ny));
    match kind {
        BuildingKind::Wall => {
            let mask = neighbor_mask(x, y, |nx, ny| in_drag(nx, ny) || wall_at(sim, nx, ny, true));
            cell_for(sim, building, material, mask, true, 0)
        }
        BuildingKind::Roof => {
            let mask = neighbor_mask(x, y, |nx, ny| in_drag(nx, ny) || roof_at(sim, nx, ny, true));
            cell_for(sim, building, material, mask, true, 0)
        }
        _ => cell_at(sim, building, material, x, y, true),
    }
}

/// A frame of `t` pixels along the edge of a rectangle.
fn draw_frame(rect: (f32, f32, f32, f32), t: f32, color: Color) {
    draw_rectangle(rect.0, rect.1, rect.2, t, color);
    draw_rectangle(rect.0, rect.1 + rect.3 - t, rect.2, t, color);
    draw_rectangle(rect.0, rect.1, t, rect.3, color);
    draw_rectangle(rect.0 + rect.2 - t, rect.1, t, rect.3, color);
}

/// The bar under a blueprint: dark track, green fill.
fn draw_progress(rect: (f32, f32, f32, f32), pct: u32, t: f32) {
    let bar_h = (t * 2.0).max(3.0);
    let inset = (rect.2 / 8.0).round();
    let (x, w) = (rect.0 + inset, rect.2 - 2.0 * inset);
    let y = rect.1 + rect.3 - inset - bar_h;
    draw_rectangle(
        x - 1.0,
        y - 1.0,
        w + 2.0,
        bar_h + 2.0,
        rgb_color(pal::ui::INK, 230),
    );
    draw_rectangle(x, y, w, bar_h, rgb_color([58, 62, 90], 255));
    let fill = (w * pct.min(100) as f32 / 100.0).round();
    draw_rectangle(x, y, fill, bar_h, rgb_color([104, 214, 130], 255));
}

/// Draw a small tag with a text near a point, for the build preview.
pub fn draw_build_label(f: &Frame, area_top_left: (u16, u16), text: &str) {
    let (x, y, _, _) = screen_rect(
        f.camera,
        f.view,
        i32::from(area_top_left.0),
        i32::from(area_top_left.1),
        1,
        1,
    );
    let tw = ui::text_px(text);
    let color = tool_color(Tool::Build);
    let (lx, ly) = (x, (y - 26.0).max(2.0));
    ui::draw_tag(lx, ly, tw + 12.0, 22.0, color);
    font::draw_text(
        text,
        lx + 6.0,
        ly + 4.0,
        ui::SCALE,
        rgb_color(pal::ui::TEXT, 255),
    );
}

/// The tint of a room in the rooms overlay, by room index.
pub fn room_tint(index: usize) -> Rgb {
    ROOM_TINTS[index % ROOM_TINTS.len()]
}

/// Draw the rooms overlay: every closed room gets a tint of its own.
pub fn draw_rooms(f: &Frame) {
    let Some(visible) = f.camera.visible_tiles(f.view, f.map_size) else {
        return;
    };
    for (i, room) in f.sim.rooms().iter().enumerate() {
        let color = rgb_color(room_tint(i), (ROOM_ALPHA * 255.0) as u8);
        for &(x, y) in &room.tiles {
            if f.is_visible(&visible, x, y) {
                let r = f.rect(x, y);
                draw_rectangle(r.0, r.1, r.2, r.3, color);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sim::scenario::sim_from_ascii;

    fn sheet() -> Option<Png> {
        Sprites::shipped().ok().map(|s| s.buildings)
    }

    #[test]
    fn the_mask_has_one_bit_for_each_joined_side() {
        let all = neighbor_mask(5, 5, |_, _| true);
        assert_eq!(all, bc::NORTH | bc::EAST | bc::SOUTH | bc::WEST);
        let east_west = neighbor_mask(5, 5, |nx, ny| ny == 5 && nx != 5);
        assert_eq!(east_west, bc::EAST | bc::WEST);
        // The map edge has no neighbor to the north or the west.
        assert_eq!(neighbor_mask(0, 0, |_, _| true), bc::EAST | bc::SOUTH);
    }

    #[test]
    fn a_wall_joins_its_wall_neighbors_and_a_door_follows_the_wall() {
        let mut sim = sim_from_ascii("......\n......\n......", 1).unwrap();
        let rules = sim.rules();
        let wall = rules.building_index("wall").unwrap();
        let door = rules.building_index("door").unwrap();
        let granite = rules.material_index("granite").unwrap();
        let log = rules.log;
        assert!(sim.debug_build(wall, 1, 1, granite));
        assert!(sim.debug_build(door, 2, 1, log));
        assert!(sim.debug_build(wall, 3, 1, granite));
        // The wall at x = 1 joins the door at its east side only.
        assert_eq!(
            cell_at(&sim, wall, granite, 1, 1, false),
            Some(bc::wall_cell(false, bc::EAST))
        );
        // The door in a wall that runs east to west is a horizontal door.
        assert_eq!(cell_at(&sim, door, log, 2, 1, false), Some(bc::DOOR_H));
        // A door with no wall beside it is a vertical door.
        assert!(sim.debug_build(door, 5, 2, log));
        assert_eq!(cell_at(&sim, door, log, 5, 2, false), Some(bc::DOOR_V));
    }

    #[test]
    fn wood_and_stone_use_their_own_sprites() {
        let sim = sim_from_ascii("..", 1).unwrap();
        let rules = sim.rules();
        let wall = rules.building_index("wall").unwrap();
        let floor = rules.building_index("floor").unwrap();
        let (granite, log) = (rules.material_index("granite").unwrap(), rules.log);
        assert_eq!(
            cell_for(&sim, wall, granite, 0, true, 0),
            Some(bc::WALL_STONE)
        );
        assert_eq!(cell_for(&sim, wall, log, 0, true, 0), Some(bc::WALL_WOOD));
        assert_eq!(
            cell_for(&sim, floor, granite, 0, true, 0),
            Some(bc::FLOOR_STONE)
        );
        assert_eq!(cell_for(&sim, floor, log, 0, true, 0), Some(bc::FLOOR_WOOD));
    }

    #[test]
    fn the_hearth_has_two_flame_frames() {
        assert_eq!(flame_frame(0), 0);
        assert_eq!(flame_frame(FLAME_TICKS), 1);
        assert_eq!(flame_frame(2 * FLAME_TICKS), 0);
    }

    #[test]
    fn the_material_color_changes_only_the_material_pixels() {
        let Some(sheet) = sheet() else { return };
        let a = building_image(&sheet, bc::WALL_STONE, [120, 120, 130]);
        let b = building_image(&sheet, bc::WALL_STONE, [200, 80, 60]);
        assert_ne!(a, b);
        let raw = sheet.cell_bytes(bc::WALL_STONE);
        for ((pa, pb), pr) in a.chunks(4).zip(b.chunks(4)).zip(raw.chunks(4)) {
            if pr[3] == bc::MATERIAL_ALPHA {
                assert_eq!(pa[3], 255);
            } else {
                // Shadow and see-through pixels do not change.
                assert_eq!(pa, pb);
            }
        }
    }

    #[test]
    fn hearth_fire_and_blanket_have_fixed_colors() {
        let Some(sheet) = sheet() else { return };
        let a = building_image(&sheet, bc::HEARTH, [120, 120, 130]);
        let b = building_image(&sheet, bc::HEARTH, [20, 20, 20]);
        let fire = |img: &[u8]| -> Vec<[u8; 4]> {
            sheet
                .cell_bytes(bc::HEARTH)
                .chunks(4)
                .zip(img.chunks(4))
                .filter(|(raw, _)| raw[3] == bc::FIRE_ALPHA)
                .map(|(_, p)| [p[0], p[1], p[2], p[3]])
                .collect()
        };
        assert!(!fire(&a).is_empty());
        assert_eq!(fire(&a), fire(&b));
    }

    #[test]
    fn every_building_cell_has_visible_pixels() {
        let Some(sheet) = sheet() else { return };
        for cell in 0..bc::WORKSHOP + 1 {
            let image = building_image(&sheet, cell, [150, 150, 150]);
            assert!(image.chunks(4).any(|p| p[3] > 0), "cell {cell} is empty");
        }
    }

    #[test]
    fn rooms_get_different_tints() {
        assert_ne!(room_tint(0), room_tint(1));
        assert_eq!(room_tint(0), room_tint(ROOM_TINTS.len()));
    }
}
