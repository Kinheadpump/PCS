//! Colonist sprites for `assetgen`. Pixel art is written as text rows and decoded here.
//!
//! The sheet `actors.png` has full color. One exception: the shirt uses gray values
//! (red = green = blue). The game multiplies those pixels with the colonist color, so
//! each colonist gets its own shirt. No other sprite color may be gray.
//!
//! Cells (16 x 16 pixels each): south frames 0..3, north frames 3..6, east frames 6..9.
//! Frame 0 of each facing is standing. Frames 1 and 2 are the two walking steps.
//! West is the east sprite flipped in the game. The feet touch the bottom of row 13.

use super::{COLUMNS, TILE};

/// Cell of the first south-facing frame.
pub const SOUTH: usize = 0;
/// Cell of the first north-facing frame.
#[allow(dead_code)] // The game repeats this number; the tests use it.
pub const NORTH: usize = 3;
/// Cell of the first east-facing frame.
pub const EAST: usize = 6;
/// Number of colonist sprites.
pub const COUNT: usize = 9;
/// Rows of the actor sheet.
pub const ROWS: usize = 1;

/// Color of one text symbol: RGBA. `None` means the symbol is unknown.
pub fn symbol(c: char) -> Option<[u8; 4]> {
    Some(match c {
        '.' => [0, 0, 0, 0],
        // Outline: a dark, slightly purple color. Never pure black.
        'o' => [30, 24, 40, 255],
        // Hair and hair light.
        'h' => [88, 58, 40, 255],
        'H' => [128, 88, 56, 255],
        // Skin, skin shade, and eye.
        's' => [236, 188, 148, 255],
        'S' => [200, 144, 112, 255],
        'e' => [40, 30, 44, 255],
        // Shirt (gray, tinted in the game): light, base, shade.
        'a' => [255, 255, 255, 255],
        'c' => [224, 224, 224, 255],
        'd' => [150, 150, 150, 255],
        // Pants, pants shade, boots.
        'p' => [72, 80, 112, 255],
        'P' => [50, 54, 82, 255],
        'b' => [92, 62, 44, 255],
        // Ground shadow.
        'z' => [0, 0, 0, 90],
        _ => return None,
    })
}

type Rows = [&'static str; 16];

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

fn frame(head: &[&'static str; 6], torso: &[&'static str; 4], legs: &[&'static str; 5]) -> Rows {
    let mut rows = [""; 16];
    rows[..6].copy_from_slice(head);
    rows[6..10].copy_from_slice(torso);
    rows[10..15].copy_from_slice(legs);
    rows[15] = "................";
    rows
}

/// The text rows of all colonist sprites, in cell order.
pub fn frames() -> Vec<Rows> {
    let mut out = Vec::with_capacity(COUNT);
    for legs in &LEGS_FRONT {
        out.push(frame(&HEAD_SOUTH, &TORSO_FRONT, &legs.clone()));
    }
    for legs in &LEGS_FRONT {
        out.push(frame(&HEAD_NORTH, &TORSO_BACK, &legs.clone()));
    }
    for legs in &LEGS_SIDE {
        out.push(frame(&HEAD_EAST, &TORSO_SIDE, &legs.clone()));
    }
    out
}

/// An RGBA image of the actor sprites.
pub struct ActorSheet {
    /// Width in pixels.
    pub width: usize,
    /// Height in pixels.
    pub height: usize,
    /// RGBA bytes, row-major.
    pub rgba: Vec<u8>,
}

/// Build the actor sheet.
pub fn make_sheet() -> ActorSheet {
    let (width, height) = (COLUMNS * TILE, ROWS * TILE);
    let mut rgba = vec![0u8; width * height * 4];
    for (cell, rows) in frames().iter().enumerate() {
        for (y, row) in rows.iter().enumerate() {
            for (x, ch) in row.chars().enumerate() {
                let Some(px) = symbol(ch) else { continue };
                let (sx, sy) = ((cell % COLUMNS) * TILE + x, (cell / COLUMNS) * TILE + y);
                let i = (sy * width + sx) * 4;
                rgba[i..i + 4].copy_from_slice(&px);
            }
        }
    }
    ActorSheet {
        width,
        height,
        rgba,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_row_has_16_known_symbols() {
        for rows in frames() {
            for row in rows {
                assert_eq!(row.chars().count(), 16, "row {row:?}");
                assert!(row.chars().all(|c| symbol(c).is_some()), "row {row:?}");
            }
        }
        assert_eq!(frames().len(), COUNT);
    }

    #[test]
    fn only_the_shirt_is_gray() {
        for c in "ohHsSepPbz".chars() {
            if let Some([r, g, b, a]) = symbol(c) {
                assert!(a == 0 || !(r == g && g == b) || c == 'z', "symbol {c}");
            }
        }
    }

    #[test]
    fn feet_stay_on_one_row_in_each_standing_frame() {
        for cell in [SOUTH, NORTH, EAST] {
            let rows = &frames()[cell];
            assert!(rows[13].contains('b'));
            assert!(!rows[14].contains('b'));
        }
    }
}
