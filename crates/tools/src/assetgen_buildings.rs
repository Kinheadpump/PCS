//! Building sprites for `assetgen`: walls, doors, floors, roofs, bed, hearth, workshop.
//!
//! The sheet is gray. The alpha value of a pixel says how the game colors it (see
//! `cells::buildings`): 255 takes the material color, 254 cloth, 253 fire, 252 metal, and
//! a lower alpha is a black shadow. The light comes from the north west: the top and left
//! edges are light, the right edge and the south face are dark. Outlines are level 70.
//! Walls and roofs have one cell for each set of connected sides, so a row of walls is one
//! solid bar.

use super::Image;
use super::cells::buildings as b;

const OUT: i32 = 70;
const DEEP: i32 = 150;
const SEAM: i32 = 192;
const BASE: i32 = 222;
const LIGHT: i32 = 244;
const SPARK: i32 = 255;
/// Level of the front face of a wall.
const FACE: i32 = 172;
/// Level of the seams on the front face.
const FACE_SEAM: i32 = 140;
/// Shadow alpha.
const SHADOW: u8 = 70;
/// Height of the front face in pixels.
const FACE_H: i32 = 5;

fn put(img: &mut Image, cell: usize, x: i32, y: i32, level: i32, alpha: u8) {
    let v = level.clamp(0, 255) as u8;
    img.set_cell(cell, x, y, [v, v, v, alpha]);
}

fn mat(img: &mut Image, cell: usize, x: i32, y: i32, level: i32) {
    put(img, cell, x, y, level, b::MATERIAL_ALPHA);
}

/// Fill the rectangle `(x0, y0, x1, y1)` (both corners included).
fn rect(img: &mut Image, cell: usize, area: (i32, i32, i32, i32), level: i32, alpha: u8) {
    let (x0, y0, x1, y1) = area;
    for y in y0..=y1 {
        for x in x0..=x1 {
            put(img, cell, x, y, level, alpha);
        }
    }
}

fn opaque(img: &Image, cell: usize, x: i32, y: i32) -> bool {
    img.get_cell(cell, x, y)[3] >= b::METAL_ALPHA
}

/// A 1 pixel outline on transparent pixels next to a solid pixel (four directions).
fn outline(img: &mut Image, cell: usize) {
    let mut add = Vec::new();
    for y in 0..16 {
        for x in 0..16 {
            if img.get_cell(cell, x, y)[3] != 0 {
                continue;
            }
            let near = [(1, 0), (-1, 0), (0, 1), (0, -1)]
                .iter()
                .any(|&(dx, dy)| opaque(img, cell, x + dx, y + dy));
            if near {
                add.push((x, y));
            }
        }
    }
    for (x, y) in add {
        mat(img, cell, x, y, OUT);
    }
}

/// A one pixel shadow to the south and east of the solid pixels.
fn shadow(img: &mut Image, cell: usize) {
    let mut add = Vec::new();
    for y in 0..16 {
        for x in 0..16 {
            if img.get_cell(cell, x, y)[3] != 0 {
                continue;
            }
            if opaque(img, cell, x - 1, y) || opaque(img, cell, x, y - 1) {
                add.push((x, y));
            }
        }
    }
    for (x, y) in add {
        put(img, cell, x, y, 0, SHADOW);
    }
}

// -------------------------------------------------------------------------------- walls

/// The level of the top of a stone wall at a pixel (courses of 4 pixels, joints every 8).
fn stone_cap(x: i32, y: i32) -> i32 {
    let course = y / 4;
    let joint_x = if course % 2 == 0 { 3 } else { 7 };
    if y % 4 == 3 || x % 8 == joint_x {
        SEAM
    } else if y % 4 == 0 {
        BASE + 6
    } else {
        BASE
    }
}

/// The level of the top of a wood wall: vertical planks of 4 pixels.
fn wood_cap(x: i32, y: i32) -> i32 {
    match x % 4 {
        3 => SEAM,
        0 => BASE + 8,
        1 if y % 7 == 2 => BASE - 10,
        _ => BASE,
    }
}

fn stone_face(x: i32, y: i32) -> i32 {
    let course = y / 4;
    let joint_x = if course % 2 == 0 { 3 } else { 7 };
    if y % 4 == 3 || x % 8 == joint_x {
        FACE_SEAM
    } else {
        FACE
    }
}

fn wood_face(x: i32, _y: i32) -> i32 {
    if x % 4 == 3 { FACE_SEAM } else { FACE }
}

/// Draw one wall cell for a set of connected sides.
fn wall(img: &mut Image, cell: usize, wood: bool, mask: u8) {
    let has = |bit: u8| mask & bit != 0;
    let face_rows = if has(b::SOUTH) { 0 } else { FACE_H };
    let cap_end = 16 - face_rows;
    for y in 0..16 {
        for x in 0..16 {
            let on_face = y >= cap_end;
            let mut level = match (on_face, wood) {
                (false, false) => stone_cap(x, y),
                (false, true) => wood_cap(x, y),
                (true, false) => stone_face(x, y),
                (true, true) => wood_face(x, y),
            };
            if !on_face {
                if y == 1 && !has(b::NORTH) {
                    level = LIGHT;
                }
                if x == 1 && !has(b::WEST) {
                    level = LIGHT;
                }
                if x == 14 && !has(b::EAST) {
                    level = level.min(DEEP + 20);
                }
                if y == cap_end - 1 && face_rows > 0 {
                    level = level.min(DEEP + 30);
                }
            }
            let edge = (y == 0 && !has(b::NORTH))
                || (x == 0 && !has(b::WEST))
                || (x == 15 && !has(b::EAST))
                || (y == 15 && face_rows > 0);
            if edge {
                level = OUT;
            }
            mat(img, cell, x, y, level);
        }
    }
    if face_rows > 0 {
        // The edge between the top and the face.
        for x in 0..16 {
            if (x == 0 && !has(b::WEST)) || (x == 15 && !has(b::EAST)) {
                continue;
            }
            mat(img, cell, x, cap_end, DEEP);
        }
    }
}

/// Posts and a leaf. `horizontal` is a door in a wall that runs east to west.
fn door(img: &mut Image, cell: usize, horizontal: bool) {
    if horizontal {
        for (x0, x1) in [(0, 2), (13, 15)] {
            for y in 0..16 {
                for x in x0..=x1 {
                    let on_face = y >= 16 - FACE_H;
                    let mut level = if on_face {
                        wood_face(x, y)
                    } else {
                        wood_cap(x, y)
                    };
                    if y == 0 || x == 0 || x == 15 || y == 15 {
                        level = OUT;
                    } else if y == 1 || x == 1 {
                        level = LIGHT;
                    }
                    if y == 16 - FACE_H {
                        level = DEEP;
                    }
                    mat(img, cell, x, y, level);
                }
            }
        }
        // The leaf: 4 rows of top and 2 rows of front.
        for x in 3..13 {
            for y in 5..=10 {
                let level = match y {
                    5 => OUT,
                    6 => LIGHT,
                    7 | 8 => {
                        if x % 3 == 0 {
                            SEAM
                        } else {
                            BASE
                        }
                    }
                    9 => FACE,
                    _ => OUT,
                };
                mat(img, cell, x, y, level);
            }
        }
        mat(img, cell, 10, 7, SPARK);
        mat(img, cell, 11, 7, SPARK);
    } else {
        // Top post and bottom post.
        for (y0, y1) in [(0, 3), (12, 15)] {
            for y in y0..=y1 {
                for x in 3..13 {
                    let on_face = y >= 14;
                    let mut level = if on_face {
                        wood_face(x, y)
                    } else {
                        wood_cap(x, y)
                    };
                    if y == y0 && y0 == 0 || y == 15 || x == 3 || x == 12 {
                        level = OUT;
                    } else if y == 1 && y0 == 0 || x == 4 {
                        level = LIGHT;
                    }
                    mat(img, cell, x, y, level);
                }
            }
        }
        // The leaf: planks that run north to south.
        for y in 4..12 {
            for x in 5..11 {
                let level = if x == 5 || x == 10 {
                    OUT
                } else if x == 6 {
                    LIGHT
                } else if x == 9 {
                    DEEP + 20
                } else if y % 4 == 3 {
                    SEAM
                } else {
                    BASE
                };
                mat(img, cell, x, y, level);
            }
        }
        mat(img, cell, 8, 8, SPARK);
        mat(img, cell, 8, 9, SPARK);
    }
}

// -------------------------------------------------------------------------------- roofs

fn roof(img: &mut Image, cell: usize, mask: u8) {
    let has = |bit: u8| mask & bit != 0;
    for y in 0..16 {
        for x in 0..16 {
            let row = y / 4;
            let seam_x = if row % 2 == 0 { 3 } else { 7 };
            let mut level = if y % 4 == 3 {
                DEEP + 20
            } else if x % 8 == seam_x {
                SEAM
            } else if y % 4 == 0 {
                BASE + 10
            } else {
                BASE - 6
            };
            if (y == 0 && !has(b::NORTH))
                || (x == 0 && !has(b::WEST))
                || (x == 15 && !has(b::EAST))
                || (y == 15 && !has(b::SOUTH))
            {
                level = OUT;
            }
            mat(img, cell, x, y, level);
        }
    }
}

// -------------------------------------------------------------------------------- floors

fn floor_stone(img: &mut Image, cell: usize) {
    for y in 0..16 {
        for x in 0..16 {
            // Flags: three rows with joints at different columns.
            let (row, joints): (i32, [i32; 2]) = match y {
                0..=4 => (0, [4, 11]),
                5..=10 => (1, [1, 8]),
                _ => (2, [5, 13]),
            };
            let joint_row = y == 5 || y == 11;
            let joint_col = joints.contains(&x);
            let mut level = BASE + if (x + y * 3) % 7 == 0 { 6 } else { 0 };
            if joint_row || joint_col {
                level = SEAM + 6;
            } else if (y == 0 || y == 6 || y == 12) && row >= 0 {
                level = BASE + 8;
            }
            mat(img, cell, x, y, level);
        }
    }
}

fn floor_wood(img: &mut Image, cell: usize) {
    for y in 0..16 {
        for x in 0..16 {
            let plank = y / 4;
            let end_x = (plank * 5 + 3) % 16;
            let mut level = BASE;
            if y % 4 == 3 || x == end_x {
                level = SEAM + 4;
            } else if y % 4 == 0 {
                level = BASE + 6;
            } else if (x + plank * 3) % 9 == 4 && y % 4 == 1 {
                level = BASE - 8;
            }
            mat(img, cell, x, y, level);
        }
    }
}

// ------------------------------------------------------------------------------ furniture

fn bed(img: &mut Image, cell: usize) {
    // Frame.
    rect(img, cell, (2, 1, 13, 14), BASE, b::MATERIAL_ALPHA);
    rect(img, cell, (2, 1, 13, 2), DEEP + 30, b::MATERIAL_ALPHA); // headboard
    rect(img, cell, (2, 1, 13, 1), LIGHT, b::MATERIAL_ALPHA);
    rect(img, cell, (2, 13, 13, 14), DEEP + 20, b::MATERIAL_ALPHA); // footboard
    rect(img, cell, (2, 3, 2, 12), LIGHT - 8, b::MATERIAL_ALPHA);
    rect(img, cell, (13, 3, 13, 12), DEEP + 30, b::MATERIAL_ALPHA);
    // Pillow.
    rect(img, cell, (4, 3, 11, 5), 250, b::CLOTH_ALPHA);
    rect(img, cell, (4, 5, 11, 5), 214, b::CLOTH_ALPHA);
    // Blanket.
    rect(img, cell, (3, 6, 12, 12), 222, b::CLOTH_ALPHA);
    rect(img, cell, (3, 6, 12, 6), 246, b::CLOTH_ALPHA); // folded edge
    rect(img, cell, (3, 7, 12, 7), 200, b::CLOTH_ALPHA);
    rect(img, cell, (12, 8, 12, 12), 190, b::CLOTH_ALPHA);
    for x in [5, 8, 10] {
        put(img, cell, x, 10, 240, b::CLOTH_ALPHA);
    }
    outline(img, cell);
    shadow(img, cell);
}

/// Draw the stones of the ring and the ash.
fn hearth_base(img: &mut Image, cell: usize) {
    for y in 0..16 {
        for x in 0..16 {
            let (dx, dy) = (x as f32 + 0.5 - 8.0, y as f32 + 0.5 - 8.0);
            let d = (dx * dx + dy * dy).sqrt();
            if d <= 6.3 {
                let stone = ((x + 2 * y) / 3 + (y / 3)) % 3;
                let mut level = BASE - 8 + stone * 8;
                if dx + dy < -4.0 {
                    level = LIGHT;
                } else if dx + dy > 4.5 {
                    level = DEEP + 25;
                }
                if (x + y) % 5 == 0 && d > 4.0 {
                    level = SEAM - 6;
                }
                if d <= 3.6 {
                    level = 78;
                }
                mat(img, cell, x, y, level);
            }
        }
    }
}

fn flame(img: &mut Image, cell: usize, shift: i32) {
    // Outer flame (orange), then the bright core.
    let outer = [
        (8, 4),
        (7, 5),
        (8, 5),
        (9, 5),
        (6, 6),
        (7, 6),
        (8, 6),
        (9, 6),
        (10, 6),
        (6, 7),
        (7, 7),
        (8, 7),
        (9, 7),
        (10, 7),
        (7, 8),
        (8, 8),
        (9, 8),
    ];
    for (x, y) in outer {
        let sx = if y <= 5 { shift } else { 0 };
        put(img, cell, x + sx, y + 1, 214, b::FIRE_ALPHA);
    }
    for (x, y) in [(8, 6), (8, 7), (7, 7), (9, 7), (8, 8)] {
        put(img, cell, x, y + 1, SPARK, b::FIRE_ALPHA);
    }
    put(img, cell, 8 + shift, 5, 240, b::FIRE_ALPHA);
}

fn hearth(img: &mut Image, cell: usize, shift: i32) {
    hearth_base(img, cell);
    flame(img, cell, shift);
    outline(img, cell);
    shadow(img, cell);
}

fn workshop(img: &mut Image, cell: usize) {
    // Legs and the front apron.
    rect(img, cell, (2, 11, 13, 13), FACE, b::MATERIAL_ALPHA);
    rect(img, cell, (2, 11, 13, 11), DEEP, b::MATERIAL_ALPHA);
    rect(img, cell, (2, 13, 3, 14), DEEP + 20, b::MATERIAL_ALPHA);
    rect(img, cell, (12, 13, 13, 14), DEEP + 20, b::MATERIAL_ALPHA);
    // Table top.
    rect(img, cell, (1, 3, 14, 10), BASE, b::MATERIAL_ALPHA);
    rect(img, cell, (1, 3, 14, 3), LIGHT, b::MATERIAL_ALPHA);
    rect(img, cell, (1, 3, 1, 10), LIGHT - 6, b::MATERIAL_ALPHA);
    rect(img, cell, (14, 4, 14, 10), DEEP + 30, b::MATERIAL_ALPHA);
    rect(img, cell, (2, 10, 13, 10), SEAM, b::MATERIAL_ALPHA);
    for x in [5, 9] {
        rect(img, cell, (x, 5, x, 9), SEAM + 8, b::MATERIAL_ALPHA);
    }
    // Saw on the left: a blade of metal with a wooden grip.
    rect(img, cell, (2, 6, 7, 7), 236, b::METAL_ALPHA);
    rect(img, cell, (2, 6, 7, 6), SPARK, b::METAL_ALPHA);
    for x in [3, 5, 7] {
        put(img, cell, x, 8, 200, b::METAL_ALPHA);
    }
    rect(img, cell, (8, 6, 9, 7), DEEP + 30, b::MATERIAL_ALPHA);
    // Hammer on the right.
    rect(img, cell, (11, 4, 13, 5), 236, b::METAL_ALPHA);
    rect(img, cell, (11, 4, 13, 4), SPARK, b::METAL_ALPHA);
    rect(img, cell, (12, 6, 12, 9), DEEP + 30, b::MATERIAL_ALPHA);
    outline(img, cell);
    shadow(img, cell);
}

/// Make `buildings.png`.
pub fn make_buildings() -> Image {
    let mut img = Image::cell_sheet(b::ROWS);
    for mask in 0..16u8 {
        wall(&mut img, b::wall_cell(false, mask), false, mask);
        wall(&mut img, b::wall_cell(true, mask), true, mask);
        roof(&mut img, b::roof_cell(mask), mask);
    }
    door(&mut img, b::DOOR_H, true);
    door(&mut img, b::DOOR_V, false);
    floor_stone(&mut img, b::FLOOR_STONE);
    floor_wood(&mut img, b::FLOOR_WOOD);
    bed(&mut img, b::BED);
    hearth(&mut img, b::HEARTH, 0);
    hearth(&mut img, b::HEARTH_B, 1);
    workshop(&mut img, b::WORKSHOP);
    img
}
