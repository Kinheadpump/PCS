//! Item sprites and icons for `assetgen`. Gray sprites with alpha. The game tints them.
//!
//! Gray levels: outline 40, shade 150, base 214, light 244, sparkle 255. Level 224 is the plain
//! material color, so the base is a little darker than plain and the light is lighter. The light
//! comes from the north west. A soft contact shadow (black, alpha 70) lies under each pile.

use super::Image;
use super::cells::{icons as ic, items as it};

const OUT: i32 = 40;
const SHADE: i32 = 150;
const BASE: i32 = 214;
const LIGHT: i32 = 244;
const SPARK: i32 = 255;

fn g(img: &mut Image, cell: usize, x: i32, y: i32, level: i32) {
    let v = level.clamp(0, 255) as u8;
    img.set_cell(cell, x, y, [v, v, v, 255]);
}

fn shadow(img: &mut Image, cell: usize, cx: i32, cy: i32, rx: i32) {
    for x in cx - rx..=cx + rx {
        for dy in 0..2 {
            let w = if dy == 0 { rx } else { rx - 2 };
            if (x - cx).abs() <= w && img.get_cell(cell, x, cy + dy)[3] == 0 {
                img.set_cell(cell, x, cy + dy, [0, 0, 0, 70]);
            }
        }
    }
}

/// Add a 1 pixel outline around every opaque pixel (only on transparent pixels).
fn outline(img: &mut Image, cell: usize, level: i32, diagonal: bool) {
    let mut add = Vec::new();
    for y in -1..17 {
        for x in -1..17 {
            if img.get_cell(cell, x, y)[3] != 0 {
                continue;
            }
            let mut near = false;
            for (dx, dy) in [
                (1, 0),
                (-1, 0),
                (0, 1),
                (0, -1),
                (1, 1),
                (-1, -1),
                (1, -1),
                (-1, 1),
            ] {
                if !diagonal && dx != 0 && dy != 0 {
                    continue;
                }
                if img.get_cell(cell, x + dx, y + dy)[3] == 255 {
                    near = true;
                }
            }
            if near {
                add.push((x, y));
            }
        }
    }
    for (x, y) in add {
        g(img, cell, x, y, level);
    }
}

/// A rounded stone. `diamond` makes an angular chunk. `rx`, `ry` are radii in pixels.
#[allow(clippy::too_many_arguments)]
fn stone(
    img: &mut Image,
    cell: usize,
    cx: f32,
    cy: f32,
    rx: f32,
    ry: f32,
    diamond: bool,
    spark: bool,
) {
    let mut mask = [[false; 16]; 16];
    for (y, mask_row) in mask.iter_mut().enumerate() {
        for (x, cell_mask) in mask_row.iter_mut().enumerate() {
            let (dx, dy) = ((x as f32 + 0.5 - cx) / rx, (y as f32 + 0.5 - cy) / ry);
            *cell_mask = if diamond {
                dx.abs() + dy.abs() <= 1.0
            } else {
                dx * dx + dy * dy <= 1.0
            };
        }
    }
    let inside = |x: i32, y: i32| {
        (0..16).contains(&x) && (0..16).contains(&y) && mask[y as usize][x as usize]
    };
    for y in 0..16i32 {
        for x in 0..16i32 {
            if !inside(x, y) {
                continue;
            }
            let edge =
                !(inside(x - 1, y) && inside(x + 1, y) && inside(x, y - 1) && inside(x, y + 1));
            let rel = ((x as f32 + 0.5 - cx) / rx) + ((y as f32 + 0.5 - cy) / ry);
            let level = if edge {
                OUT
            } else if rel < -0.55 {
                LIGHT
            } else if rel > 0.45 {
                SHADE
            } else {
                BASE
            };
            g(img, cell, x, y, level);
        }
    }
    if spark {
        let (sx, sy) = ((cx - rx * 0.35) as i32, (cy - ry * 0.3) as i32);
        if inside(sx, sy) && inside(sx + 1, sy) {
            g(img, cell, sx, sy, SPARK);
            g(img, cell, sx + 1, sy, SPARK);
            g(img, cell, sx, sy + 1, SPARK);
        }
    }
}

/// A log lying to the east with the round end at the west. 5 rows high.
fn log(img: &mut Image, cell: usize, x: i32, y: i32, len: i32) {
    for i in 0..len {
        let col = [OUT, LIGHT, BASE, SHADE, OUT];
        for (r, &lv) in col.iter().enumerate() {
            let end = i == 0 || i == len - 1;
            let corner = end && (r == 0 || r == 4);
            if !corner {
                g(img, cell, x + i, y + r as i32, lv);
            }
        }
    }
    // The cut end: a light ring around a dark center.
    g(img, cell, x + 1, y + 1, SPARK);
    g(img, cell, x + 1, y + 2, 190);
    g(img, cell, x + 1, y + 3, SPARK);
    g(img, cell, x + 2, y + 2, LIGHT);
    // One short bark mark.
    g(img, cell, x + len / 2 + 1, y + 2, SHADE);
    g(img, cell, x + len / 2 + 2, y + 2, SHADE);
}

/// A metal ingot: a flat bar with a light top and a dark side. 9 x 4 pixels.
fn ingot(img: &mut Image, cell: usize, x: i32, y: i32) {
    for i in 0..9 {
        let slant = i32::from(i == 0 || i == 8);
        g(img, cell, x + i, y + slant, OUT);
        g(
            img,
            cell,
            x + i,
            y + 1 + slant,
            if i < 7 { SPARK } else { LIGHT },
        );
        g(img, cell, x + i, y + 2, if slant == 1 { OUT } else { BASE });
        g(img, cell, x + i, y + 3, OUT);
    }
    for i in 1..8 {
        g(img, cell, x + i, y + 2, if i < 6 { BASE } else { SHADE });
    }
}

/// A wooden box. `w` and `h` include the outline.
fn crate_box(img: &mut Image, cell: usize, x: i32, y: i32, w: i32, h: i32) {
    for j in 0..h {
        for i in 0..w {
            let edge = i == 0 || j == 0 || i == w - 1 || j == h - 1;
            let level = if edge {
                OUT
            } else if j == 1 || i == 1 {
                LIGHT
            } else if j == h - 2 || i == w - 2 || (j - 2) % 3 == 2 {
                SHADE
            } else {
                BASE
            };
            g(img, cell, x + i, y + j, level);
        }
    }
    // Corner studs.
    for (dx, dy) in [(2, 2), (w - 3, 2), (2, h - 3), (w - 3, h - 3)] {
        g(img, cell, x + dx, y + dy, OUT + 40);
    }
}

fn pile_logs(img: &mut Image, tier: usize) {
    let cell = it::cell(it::LOGS, tier);
    shadow(img, cell, 8, 14, 7);
    match tier {
        0 => log(img, cell, 2, 6, 12),
        1 => {
            log(img, cell, 3, 4, 11);
            log(img, cell, 2, 8, 12);
        }
        _ => {
            log(img, cell, 3, 1, 11);
            log(img, cell, 2, 5, 12);
            log(img, cell, 3, 9, 11);
        }
    }
}

fn pile_stones(img: &mut Image, shape: usize, tier: usize) {
    let cell = it::cell(shape, tier);
    let ore = shape == it::ORE;
    let coal = shape == it::LUMPS;
    shadow(img, cell, 8, 13, 6 + tier as i32);
    let spots: &[(f32, f32, f32, f32)] = match (coal, tier) {
        (_, 0) => &[(8.0, 9.0, 5.0, 4.0)],
        (true, 1) => &[
            (5.0, 10.0, 3.0, 2.5),
            (10.5, 10.5, 3.5, 3.0),
            (8.0, 6.5, 3.0, 2.5),
            (12.0, 6.0, 2.0, 2.0),
        ],
        (true, _) => &[
            (4.5, 11.0, 3.5, 3.0),
            (11.0, 11.0, 4.0, 3.5),
            (8.0, 7.0, 4.0, 3.0),
            (4.0, 6.0, 2.5, 2.5),
            (12.5, 5.5, 2.5, 2.5),
        ],
        (_, 1) => &[
            (5.0, 10.0, 4.0, 3.0),
            (11.0, 10.5, 4.5, 3.5),
            (8.0, 5.5, 3.5, 3.0),
        ],
        _ => &[
            (4.5, 11.0, 4.5, 3.5),
            (11.5, 11.0, 4.5, 3.5),
            (8.0, 7.5, 5.0, 4.0),
            (3.5, 5.5, 3.0, 2.5),
            (12.5, 5.5, 3.0, 2.5),
        ],
    };
    for (i, &(cx, cy, rx, ry)) in spots.iter().enumerate() {
        // Draw the far pieces first so the near pieces cover them.
        let _ = i;
        stone(img, cell, cx, cy, rx, ry, ore, ore || coal);
    }
}

fn pile_ingots(img: &mut Image, tier: usize) {
    let cell = it::cell(it::INGOTS, tier);
    shadow(img, cell, 8, 14, 7);
    match tier {
        0 => ingot(img, cell, 3, 8),
        1 => {
            ingot(img, cell, 1, 9);
            ingot(img, cell, 6, 9);
            ingot(img, cell, 3, 6);
        }
        _ => {
            ingot(img, cell, 0, 10);
            ingot(img, cell, 7, 10);
            ingot(img, cell, 3, 7);
            ingot(img, cell, 6, 7);
            ingot(img, cell, 1, 4);
            ingot(img, cell, 5, 4);
        }
    }
}

fn pile_crates(img: &mut Image, tier: usize) {
    let cell = it::cell(it::CRATE, tier);
    shadow(img, cell, 8, 14, 6);
    match tier {
        0 => crate_box(img, cell, 4, 6, 8, 8),
        1 => crate_box(img, cell, 2, 4, 11, 10),
        _ => {
            crate_box(img, cell, 1, 6, 11, 9);
            crate_box(img, cell, 6, 1, 9, 8);
        }
    }
}

/// Make `items.png`: 6 shapes with 3 pile sizes each.
pub fn make_items(_seed: u64) -> Image {
    let mut img = Image::cell_sheet(it::ROWS);
    for tier in 0..it::TIERS {
        pile_logs(&mut img, tier);
        pile_stones(&mut img, it::ROCKS, tier);
        pile_stones(&mut img, it::ORE, tier);
        pile_ingots(&mut img, tier);
        pile_stones(&mut img, it::LUMPS, tier);
        pile_crates(&mut img, tier);
    }
    img
}

// ------------------------------------------------------------------------------------ icons

fn rows(img: &mut Image, cell: usize, rows: &[&str]) {
    for (y, row) in rows.iter().enumerate() {
        for (x, c) in row.chars().enumerate() {
            let level = match c {
                'o' => OUT,
                'w' => SPARK,
                'm' => 176,
                'd' => SHADE,
                _ => continue,
            };
            g(img, cell, x as i32, y as i32, level);
        }
    }
}

const SELECT: [&str; 14] = [
    "..o.............",
    "..oo............",
    "..owo...........",
    "..owwo..........",
    "..owwwo.........",
    "..owwwwo........",
    "..owwwwwo.......",
    "..owwwwwwo......",
    "..owwwwoooo.....",
    "..owdwwo........",
    "..oodwwo........",
    "..o.odwwo.......",
    ".....owwo.......",
    "......oo........",
];

const CUT: [&str; 12] = [
    "................",
    ".........ooo....",
    "........owwwo...",
    ".......owwwwwo..",
    "......owwwwwwo..",
    ".....omwwwwwo...",
    "....omommwwo....",
    "...omo.ooo......",
    "..omo...........",
    ".omo............",
    "omo.............",
    "oo..............",
];

const MINE: [&str; 12] = [
    "................",
    "...oooooooo.....",
    "..owwwwwwwwo....",
    ".owwoooooowwo...",
    ".owo..omo.owo...",
    ".oo...omo..oo...",
    "......omo.......",
    "......omo.......",
    "......omo.......",
    "......omo.......",
    "......omo.......",
    "......ooo.......",
];

const HAUL: [&str; 13] = [
    "......ooo.......",
    "......owo.......",
    "......owo.......",
    "....ooowooo.....",
    "....owwwwwo.....",
    ".....owwwo......",
    "......owo.......",
    "..oooooooooooo..",
    "..owwwwwwwwwwo..",
    "..owmmmmmmmmwo..",
    "..owmmmmmmmmwo..",
    "..owwwwwwwwwwo..",
    "..oooooooooooo..",
];

fn plot(img: &mut Image, cell: usize, pts: &[(i32, i32)], level: i32) {
    for &(x, y) in pts {
        g(img, cell, x, y, level);
    }
}

fn zone_icon(img: &mut Image, cell: usize) {
    let mut ring = Vec::new();
    for i in 2..14 {
        ring.extend([(i, 2), (i, 13), (2, i), (13, i)]);
    }
    let dashed: Vec<(i32, i32)> = ring
        .into_iter()
        .filter(|&(x, y)| ((x + y) / 2) % 2 == 0)
        .collect();
    plot(img, cell, &dashed, SPARK);
    // A small filled square inside, the stockpile.
    for y in 6..10 {
        for x in 6..10 {
            g(img, cell, x, y, if x == 6 || y == 6 { SPARK } else { 176 });
        }
    }
    outline(img, cell, OUT, false);
}

fn cancel_icon(img: &mut Image, cell: usize) {
    let mut pts = Vec::new();
    for i in 0..10 {
        pts.extend([
            (3 + i, 3 + i),
            (4 + i, 3 + i),
            (12 - i, 3 + i),
            (13 - i, 3 + i),
        ]);
    }
    plot(img, cell, &pts, SPARK);
    outline(img, cell, OUT, true);
}

fn work_icon(img: &mut Image, cell: usize) {
    for (cx, top) in [(5, 3), (11, 5)] {
        // Head 3 x 3 and body 4 x 5.
        for y in 0..3 {
            for x in 0..3 {
                g(img, cell, cx - 1 + x, top + y, SPARK);
            }
        }
        for y in 0..5 {
            for x in 0..4 {
                g(
                    img,
                    cell,
                    cx - 2 + x,
                    top + 4 + y,
                    if x == 3 { 176 } else { SPARK },
                );
            }
        }
    }
    outline(img, cell, OUT, true);
}

fn speed_icons(img: &mut Image) {
    for (a, b) in [(4, 6), (9, 11)] {
        for y in 3..13 {
            for x in a..=b {
                g(img, ic::PAUSE, x, y, SPARK);
            }
        }
    }
    outline(img, ic::PAUSE, OUT, false);
    for y in 3..13i32 {
        let half = 5 - (y - 8).abs();
        for x in 4..4 + half.max(0) * 2 {
            if x < 4 + (6 - (y - 8).abs() * 6 / 5) {
                g(img, ic::PLAY, x, y, SPARK);
            }
        }
    }
    outline(img, ic::PLAY, OUT, false);
    for off in [0, 5] {
        for y in 3..13i32 {
            for x in 2 + off..2 + off + (5 - (y - 8).abs()).max(0) {
                g(img, ic::FAST, x, y, SPARK);
            }
        }
    }
    outline(img, ic::FAST, OUT, false);
}

fn close_icon(img: &mut Image) {
    let mut pts = Vec::new();
    for i in 0..8 {
        pts.extend([
            (4 + i, 4 + i),
            (5 + i, 4 + i),
            (11 - i, 4 + i),
            (12 - i, 4 + i),
        ]);
    }
    plot(img, ic::CLOSE, &pts, SPARK);
    outline(img, ic::CLOSE, OUT, true);
}

fn chevron_icons(img: &mut Image) {
    let mut down = Vec::new();
    let mut right = Vec::new();
    for i in 0..5 {
        down.extend([
            (3 + i, 5 + i),
            (4 + i, 5 + i),
            (12 - i, 5 + i),
            (11 - i, 5 + i),
        ]);
        right.extend([
            (5 + i, 3 + i),
            (5 + i, 4 + i),
            (5 + i, 12 - i),
            (5 + i, 11 - i),
        ]);
    }
    plot(img, ic::CHEVRON_DOWN, &down, SPARK);
    outline(img, ic::CHEVRON_DOWN, OUT, true);
    plot(img, ic::CHEVRON_RIGHT, &right, SPARK);
    outline(img, ic::CHEVRON_RIGHT, OUT, true);
}

fn person_icon(img: &mut Image) {
    for y in 2..6 {
        for x in 6..10 {
            g(img, ic::PERSON, x, y, SPARK);
        }
    }
    for y in 7..13 {
        for x in 4..12 {
            g(
                img,
                ic::PERSON,
                x,
                y,
                if x == 11 || y == 12 { 176 } else { SPARK },
            );
        }
    }
    outline(img, ic::PERSON, OUT, true);
}

fn snowflake_icon(img: &mut Image) {
    let mut pts = Vec::new();
    for i in 1..15 {
        pts.extend([(i, 7), (i, 8), (7, i), (8, i)]);
    }
    for i in 3..13 {
        pts.extend([(i, i), (15 - i, i)]);
    }
    // Small barbs at the ends of the four main arms.
    pts.extend([
        (5, 3),
        (10, 3),
        (5, 12),
        (10, 12),
        (3, 5),
        (3, 10),
        (12, 5),
        (12, 10),
    ]);
    plot(img, ic::SNOWFLAKE, &pts, SPARK);
    outline(img, ic::SNOWFLAKE, OUT, true);
}

fn fastest_icon(img: &mut Image) {
    for off in [0, 4, 8] {
        for y in 4..13i32 {
            for x in 2 + off..2 + off + (4 - (y - 8).abs()).max(0) {
                g(img, ic::FASTEST, x, y, SPARK);
            }
        }
    }
    outline(img, ic::FASTEST, OUT, false);
}

/// A 9 x 9 badge at the top right of its cell: a dark plate with a white glyph.
fn badge(img: &mut Image, cell: usize, glyph: &[(i32, i32)]) {
    for y in 0..9 {
        for x in 0..9 {
            let corner = (x == 0 || x == 8) && (y == 0 || y == 8);
            if !corner {
                g(img, cell, 7 + x, y, OUT);
            }
        }
    }
    for &(x, y) in glyph {
        g(img, cell, 7 + x, y, SPARK);
    }
}

fn badges(img: &mut Image) {
    let cut: Vec<(i32, i32)> = (1..8)
        .flat_map(|r| [(8 - r, r), (7 - r + 1, r)])
        .filter(|&(x, _)| x > 0 && x < 8)
        .collect();
    badge(img, ic::BADGE_CUT, &cut);
    let mine: Vec<(i32, i32)> = (1..8).flat_map(|r| [(r, r), (8 - r, r)]).collect();
    badge(img, ic::BADGE_MINE, &mine);
    let mut haul: Vec<(i32, i32)> = (1..6).map(|y| (4, y)).collect();
    haul.extend([
        (2, 5),
        (3, 5),
        (5, 5),
        (6, 5),
        (3, 6),
        (4, 6),
        (5, 6),
        (4, 7),
    ]);
    badge(img, ic::BADGE_HAUL, &haul);
}

fn brackets(img: &mut Image) {
    let cell = ic::BRACKETS;
    for i in 0..4 {
        for (x, y) in [
            (i, 0),
            (0, i),
            (15 - i, 0),
            (15, i),
            (i, 15),
            (0, 15 - i),
            (15 - i, 15),
            (15, 15 - i),
        ] {
            g(img, cell, x, y, SPARK);
        }
    }
    // A dark inner edge so the brackets show on light ground too.
    for i in 1..4 {
        for (x, y) in [
            (i, 1),
            (1, i),
            (15 - i, 1),
            (14, i),
            (i, 14),
            (1, 15 - i),
            (15 - i, 14),
            (14, 15 - i),
        ] {
            if img.get_cell(cell, x, y)[3] == 0 {
                g(img, cell, x, y, OUT);
            }
        }
    }
}

const THERMOMETER: [&str; 16] = [
    "................",
    "......oooo......",
    ".....owwwwo.....",
    ".....owmddo.....",
    ".....owmddo.....",
    ".....owmwdo.....",
    ".....owmwdo.....",
    ".....owmwdo.....",
    ".....owmwdo.....",
    "....oowmwdoo....",
    "...owwwwwwwmo...",
    "...owwwwwwwdo...",
    "...owwwwwwmdo...",
    "....owwwwmdo....",
    ".....oooooo.....",
    "................",
];

const RAIN: [&str; 16] = [
    "................",
    ".......oo.......",
    "......owwo......",
    "......owwmo.....",
    ".....owwwmo.....",
    ".....owwwwmo....",
    "....owwwwwmdo...",
    "....owwwwwmdo...",
    "...owwwwwwmddo..",
    "...owwwwwmmddo..",
    "...owwwwmmmddo..",
    "...owmmmmmmddo..",
    "....owmmmmddo...",
    ".....oodddoo....",
    ".......ooo......",
    "................",
];

const FLAME: [&str; 16] = [
    "................",
    ".......oo.......",
    "......owwo......",
    "......owwwo.....",
    ".....owwwwo.oo..",
    ".....owwwwwowo..",
    "....owwwwwwwwo..",
    "....owwmwwwwwo..",
    "...owwmmmwwwwo..",
    "...owwmmmmwwwo..",
    "...owmmmmmmwwo..",
    "...owmmddmmmwo..",
    "....owmmddmmo...",
    ".....owmmmmo....",
    "......oooooo....",
    "................",
];

const BUILD: [&str; 16] = [
    "................",
    "......oooooo....",
    ".....owwwwwmo...",
    "....owwwwwwmdo..",
    "....owwwwwwmdo..",
    "....owwwwwmddo..",
    ".....ooooommo...",
    "........owmdo...",
    "........owmdo...",
    "........owmdo...",
    "........owmdo...",
    "........owmdo...",
    "........owmdo...",
    "........owmdo...",
    ".........ooo....",
    "................",
];

const REMOVE: [&str; 16] = [
    "................",
    "...oo......oo...",
    "..owwo....owwo..",
    "..owmwo..owmwo..",
    "...owmwooowmo...",
    "....owwwwwmo....",
    ".....owwwwo.....",
    ".....owwwwo.....",
    "....owwmmwmo....",
    "...owmwoowmdo...",
    "..owmwo..owmdo..",
    "..owwo....owdo..",
    "...oo......oo...",
    "................",
    "................",
    "................",
];

/// Make `icons.png`.
pub fn make_icons() -> Image {
    let mut img = Image::cell_sheet(ic::ROWS);
    rows(&mut img, ic::SELECT, &SELECT);
    rows(&mut img, ic::CUT, &CUT);
    rows(&mut img, ic::MINE, &MINE);
    rows(&mut img, ic::HAUL, &HAUL);
    rows(&mut img, ic::BUILD, &BUILD);
    rows(&mut img, ic::REMOVE, &REMOVE);
    zone_icon(&mut img, ic::ZONE);
    cancel_icon(&mut img, ic::CANCEL);
    work_icon(&mut img, ic::WORK);
    speed_icons(&mut img);
    close_icon(&mut img);
    chevron_icons(&mut img);
    person_icon(&mut img);
    fastest_icon(&mut img);
    rows(&mut img, ic::THERMOMETER, &THERMOMETER);
    rows(&mut img, ic::RAIN, &RAIN);
    rows(&mut img, ic::FLAME, &FLAME);
    snowflake_icon(&mut img);
    badges(&mut img);
    brackets(&mut img);
    img
}
