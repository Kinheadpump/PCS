//! Colonist sprites for `assetgen`. Pixel art is written as text rows and decoded here.
//!
//! The sheet `actors.png` has full color. The shirt uses gray values (red = green = blue). The
//! game multiplies those pixels with the shirt color. Hair and skin use the colors of index 0 in
//! `palette.rs`; the game swaps them per colonist (palette swap). No other color may be gray.
//!
//! The sheet has 4 hair styles. Each style has 15 cells: 9 walk frames (stand, step 1, step 2
//! for south, north, east) and 6 work frames (2 for each facing). West is east flipped.
//! The work pose has no tool: the arms move and the body bobs (D-039).

use super::Image;
use super::cells::actors as a;
use super::palette as pal;

/// Color of one text symbol: RGBA. `None` means the symbol is unknown.
pub fn symbol(c: char) -> Option<[u8; 4]> {
    let rgb = |c: pal::Rgb| [c[0], c[1], c[2], 255];
    Some(match c {
        '.' => [0, 0, 0, 0],
        'o' => rgb(pal::OUTLINE),
        'h' => rgb(pal::HAIR[0][0]),
        'H' => rgb(pal::HAIR[0][1]),
        's' => rgb(pal::SKIN[0][0]),
        'S' => rgb(pal::SKIN[0][1]),
        'e' => rgb(pal::EYE),
        // Shirt (gray, tinted in the game): light, base, shade.
        'a' => [255, 255, 255, 255],
        'c' => [224, 224, 224, 255],
        'd' => [150, 150, 150, 255],
        'p' => rgb(pal::PANTS[0]),
        'P' => rgb(pal::PANTS[1]),
        'b' => rgb(pal::BOOTS),
        'z' => [0, 0, 0, 90],
        _ => return None,
    })
}

type Grid = [[char; 16]; 16];

const HEAD_SOUTH: [&str; 6] = [
    "................",
    ".....oooooo.....",
    "....ohHhhhho....",
    "....ohhhhhho....",
    "....osesseso....",
    ".....osssso.....",
];

const HEAD_NORTH: [&str; 6] = [
    "................",
    ".....oooooo.....",
    "....ohhHhhho....",
    "....ohhhhhho....",
    "....ohhhhhho....",
    ".....ohhhho.....",
];

const HEAD_EAST: [&str; 6] = [
    "................",
    ".....oooooo.....",
    "....ohhHhhho....",
    "....ohhhhsso....",
    "....ohhsseso....",
    ".....ohssso.....",
];

const TORSO_FRONT: [&str; 4] = [
    "...oaccccccdo...",
    "...ocdccccddo...",
    "...ocdccccddo...",
    "...osdccccdso...",
];

const TORSO_BACK: [&str; 4] = [
    "...oaccccccdo...",
    "...ocdccccddo...",
    "...ocdccccddo...",
    "...ocdccccddo...",
];

const TORSO_SIDE: [&str; 4] = [
    "....occcccco....",
    "....oaccccdo....",
    "....oacddcdo....",
    "....ocdssddo....",
];

/// Legs for front and back views: stand, step 1, step 2.
const LEGS_FRONT: [[&str; 5]; 3] = [
    [
        "....oppppppo....",
        "....oppPPppo....",
        "....oppooppo....",
        "....obboobbo....",
        "...zzzzzzzzzz...",
    ],
    [
        "....oppppppo....",
        "....oppPPppo....",
        "....oppoobbo....",
        "....obbo........",
        "...zzzzzzzzzz...",
    ],
    [
        "....oppppppo....",
        "....oppPPppo....",
        "....obbooppo....",
        "........obbo....",
        "...zzzzzzzzzz...",
    ],
];

/// Legs for the side view: stand, step 1, step 2.
const LEGS_SIDE: [[&str; 5]; 3] = [
    [
        "....oppppppo....",
        "....oppppppo....",
        "....opppppoo....",
        "....obbbbbo.....",
        "...zzzzzzzzzz...",
    ],
    [
        "....oppppppo....",
        "...oPPooppo.....",
        "..oPPo..oppo....",
        "..obbo..obbo....",
        "...zzzzzzzzzz...",
    ],
    [
        "....oppppppo....",
        "....oppooPPo....",
        "...oppo..oPPo...",
        "...obbo..obbo...",
        "...zzzzzzzzzz...",
    ],
];

#[derive(Clone, Copy, PartialEq, Eq)]
enum Facing {
    South,
    North,
    East,
}

fn put(g: &mut Grid, x: usize, y: usize, c: char) {
    if x < 16 && y < 16 {
        g[y][x] = c;
    }
}

fn base(facing: Facing, legs: usize) -> Grid {
    let (head, torso, set) = match facing {
        Facing::South => (&HEAD_SOUTH, &TORSO_FRONT, &LEGS_FRONT),
        Facing::North => (&HEAD_NORTH, &TORSO_BACK, &LEGS_FRONT),
        Facing::East => (&HEAD_EAST, &TORSO_SIDE, &LEGS_SIDE),
    };
    let mut rows: Vec<&str> = Vec::new();
    rows.extend(head.iter());
    rows.extend(torso.iter());
    rows.extend(set[legs].iter());
    let mut g = [['.'; 16]; 16];
    for (y, row) in rows.iter().enumerate() {
        for (x, c) in row.chars().enumerate() {
            g[y][x] = c;
        }
    }
    g
}

/// Change the hair of a frame to a style. Style 0 is short hair.
fn style_hair(g: &mut Grid, facing: Facing, style: usize) {
    match (style, facing) {
        // Long hair: falls to the shoulders.
        (1, Facing::South | Facing::North) => {
            for y in 3..8 {
                put(g, 3, y, 'o');
                put(g, 12, y, 'o');
                put(g, 4, y, 'h');
                put(g, 11, y, 'h');
            }
            put(g, 4, 3, 'o');
            put(g, 11, 3, 'o');
            put(g, 4, 5, 'h');
            put(g, 11, 5, 'h');
            put(g, 5, 5, 'o');
            put(g, 10, 5, 'o');
            put(g, 4, 8, 'o');
            put(g, 11, 8, 'o');
        }
        (1, Facing::East) => {
            for y in 3..8 {
                put(g, 3, y, 'o');
                put(g, 4, y, 'h');
            }
            put(g, 4, 8, 'o');
            put(g, 3, 8, 'o');
        }
        // Spiky hair: two tufts above the head.
        (2, _) => {
            put(g, 6, 0, 'o');
            put(g, 9, 0, 'o');
            put(g, 6, 1, 'h');
            put(g, 9, 1, 'H');
            put(g, 5, 1, 'o');
            put(g, 10, 1, 'o');
        }
        // Ponytail: a tail that hangs at one side.
        (3, Facing::South | Facing::North) => {
            for y in 2..6 {
                put(g, 12, y, 'h');
                put(g, 13, y, 'o');
            }
            put(g, 12, 6, 'h');
            put(g, 12, 7, 'o');
            put(g, 11, 2, 'h');
            put(g, 12, 1, 'o');
        }
        (3, Facing::East) => {
            for y in 2..7 {
                put(g, 3, y, 'h');
                put(g, 2, y, 'o');
            }
            put(g, 3, 7, 'o');
            put(g, 4, 2, 'h');
        }
        _ => {}
    }
}

/// A work frame: `phase` 0 has the hands up, phase 1 has the hands down and the body low.
fn work_frame(facing: Facing, style: usize, phase: usize) -> Grid {
    let mut g = base(facing, 0);
    style_hair(&mut g, facing, style);
    if phase == 1 {
        // The body bobs: rows 1 to 9 move one pixel down.
        for y in (1..10).rev() {
            g[y + 1] = g[y];
        }
        g[1] = ['.'; 16];
    }
    let (hy, dy) = if phase == 0 { (6, 0) } else { (9, 1) };
    let _ = dy;
    match facing {
        Facing::South | Facing::North => {
            for (x, ox) in [(2, 1), (13, 14)] {
                put(&mut g, x, hy, 's');
                put(&mut g, x, hy + 1, 's');
                put(&mut g, ox, hy, 'o');
                put(&mut g, ox, hy + 1, 'o');
                put(&mut g, x, hy - 1, 'o');
                put(&mut g, x, hy + 2, 'o');
            }
            put(&mut g, 3, hy + 1, 'c');
            put(&mut g, 12, hy + 1, 'c');
        }
        Facing::East => {
            let y = hy;
            put(&mut g, 12, y, 's');
            put(&mut g, 13, y, 's');
            put(&mut g, 12, y + 1, 's');
            put(&mut g, 14, y, 'o');
            put(&mut g, 13, y + 1, 'o');
            put(&mut g, 12, y - 1, 'o');
            put(&mut g, 13, y - 1, 'o');
            put(&mut g, 12, y + 2, 'o');
        }
    }
    g
}

fn walk_frame(facing: Facing, style: usize, legs: usize) -> Grid {
    let mut g = base(facing, legs);
    style_hair(&mut g, facing, style);
    g
}

/// All 15 frames of one style, in cell order.
pub fn frames(style: usize) -> Vec<Grid> {
    let mut out = Vec::with_capacity(a::STYLE_CELLS);
    for facing in [Facing::South, Facing::North, Facing::East] {
        for legs in 0..3 {
            out.push(walk_frame(facing, style, legs));
        }
    }
    for facing in [Facing::South, Facing::North, Facing::East] {
        for phase in 0..2 {
            out.push(work_frame(facing, style, phase));
        }
    }
    out
}

/// Build the actor sheet.
pub fn make_sheet() -> Image {
    let mut img = Image::cell_sheet(a::ROWS);
    for style in 0..pal::HAIR_STYLES {
        for (i, g) in frames(style).iter().enumerate() {
            let cell = style * a::STYLE_CELLS + i;
            for (y, row) in g.iter().enumerate() {
                for (x, &ch) in row.iter().enumerate() {
                    if let Some(px) = symbol(ch).filter(|p| p[3] != 0) {
                        img.set_cell(cell, x as i32, y as i32, px);
                    }
                }
            }
        }
    }
    img
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_symbol_is_known() {
        for style in 0..pal::HAIR_STYLES {
            let f = frames(style);
            assert_eq!(f.len(), a::STYLE_CELLS);
            for g in f {
                for row in g {
                    assert!(row.iter().all(|&c| symbol(c).is_some()));
                }
            }
        }
    }

    #[test]
    fn only_the_shirt_is_gray() {
        for c in "ohHsSepPb".chars() {
            let [r, g, b, _] = symbol(c).unwrap_or_default();
            assert!(!(r == g && g == b), "symbol {c}");
        }
    }

    #[test]
    fn feet_stay_on_one_row_in_each_standing_frame() {
        for style in 0..pal::HAIR_STYLES {
            let f = frames(style);
            for cell in [a::SOUTH, a::NORTH, a::EAST] {
                assert!(f[cell][13].contains(&'b'));
                assert!(!f[cell][14].contains(&'b'));
            }
        }
    }

    #[test]
    fn styles_differ_and_work_frames_differ_from_standing() {
        let s0 = frames(0);
        for style in 1..pal::HAIR_STYLES {
            assert_ne!(frames(style)[a::SOUTH], s0[a::SOUTH], "style {style}");
        }
        assert_ne!(s0[a::WORK_SOUTH], s0[a::WORK_SOUTH + 1]);
        assert_ne!(s0[a::WORK_SOUTH], s0[a::SOUTH]);
    }
}
