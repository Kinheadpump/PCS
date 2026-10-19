//! The shared palette and the color ramp (docs/ART_DIRECTION.md section 4).
//!
//! This file is pure integer code with no window calls. The game uses it. `tools assetgen`
//! includes the same file, so the preview sheets show the exact colors of the game.
//!
//! Most world sprites are gray. A gray value is a *level*. Level 224 is the plain material
//! color. A lower level is a shade. A higher level is a highlight. `ramp` turns a material
//! color and a level into a color with a hue shift: shades turn cool (more blue, less red),
//! highlights turn warm.
#![allow(dead_code)] // The game and the tools use different parts of this file.

/// A color: red, green, blue.
pub type Rgb = [u8; 3];

/// Level that means "the plain material color".
pub const BASE_LEVEL: i32 = 224;

/// Color of a gray sprite pixel with the given level, tinted with a material color.
///
/// Shade: the red channel falls fastest, then green, then blue. A little blue is added.
/// Highlight: the color moves toward white and a little toward yellow.
pub fn ramp(tint: Rgb, level: i32) -> Rgb {
    let level = level.clamp(0, 255);
    let c = tint.map(i32::from);
    if level <= BASE_LEVEL {
        // Dark amount in per mille: 0 at the plain level, 1000 at level 0.
        let dk = (BASE_LEVEL - level) * 1000 / BASE_LEVEL;
        let r = c[0] * (1000 - dk * 135 / 100).max(0) / 1000;
        let g = c[1] * (1000 - dk * 110 / 100).max(0) / 1000;
        let b = c[2] * (1000 - dk * 60 / 100).max(0) / 1000 + 40 * dk / 1000;
        [r, g, b].map(|v| v.clamp(0, 255) as u8)
    } else {
        // Light amount in per mille: 1000 at level 255.
        let l = (level - BASE_LEVEL) * 1000 / (255 - BASE_LEVEL);
        let lift = |v: i32| v + (255 - v) * l * 45 / 100_000;
        let r = lift(c[0]) + 14 * l / 1000;
        let g = lift(c[1]) + 4 * l / 1000;
        let b = lift(c[2]) - 10 * l / 1000;
        [r, g, b].map(|v| v.clamp(0, 255) as u8)
    }
}

/// A color as a shade of another color: `steps` shade steps darker (negative: lighter).
pub fn shade(color: Rgb, steps: i32) -> Rgb {
    ramp(color, BASE_LEVEL - 24 * steps)
}

// ---------------------------------------------------------------------------------------------
// Units
// ---------------------------------------------------------------------------------------------

/// Outline of units: a dark purple, never black.
pub const OUTLINE: Rgb = [30, 24, 40];
/// Eye.
pub const EYE: Rgb = [40, 30, 44];
/// Boots.
pub const BOOTS: Rgb = [92, 62, 44];
/// Pants and pants shade.
pub const PANTS: [Rgb; 2] = [[72, 80, 112], [50, 54, 82]];
/// Hair: base and light, for each hair color. Index 0 is the color in the sprite sheet.
pub const HAIR: [[Rgb; 2]; 5] = [
    [[88, 58, 40], [128, 88, 56]],
    [[46, 38, 52], [84, 72, 98]],
    [[196, 152, 72], [238, 200, 116]],
    [[156, 66, 36], [206, 112, 56]],
    [[150, 146, 158], [200, 198, 210]],
];
/// Skin: base and shade, for each skin tone. Index 0 is the color in the sprite sheet.
pub const SKIN: [[Rgb; 2]; 4] = [
    [[236, 188, 148], [200, 144, 112]],
    [[246, 208, 172], [214, 164, 130]],
    [[196, 140, 98], [158, 102, 72]],
    [[134, 90, 62], [104, 66, 46]],
];
/// Shirt colors, by slot order. The game multiplies the gray shirt of the sprite with them.
pub const SHIRTS: [Rgb; 7] = [
    [230, 89, 77],
    [242, 179, 64],
    [115, 204, 89],
    [77, 191, 217],
    [115, 128, 242],
    [204, 115, 230],
    [242, 242, 242],
];

/// The look of one colonist: indexes into the tables above.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Look {
    /// Shirt color index in `SHIRTS`.
    pub shirt: usize,
    /// Hair color index in `HAIR`.
    pub hair: usize,
    /// Skin tone index in `SKIN`.
    pub skin: usize,
    /// Hair style: a block of cells in the actor sheet.
    pub style: usize,
}

/// Number of hair styles in the actor sheet.
pub const HAIR_STYLES: usize = 4;

/// The look of the colonist in a slot. The shirt follows the slot order. The rest mixes slot
/// numbers, so neighbors differ and the same slot always has the same look.
pub fn look_for(slot: usize) -> Look {
    Look {
        shirt: slot % SHIRTS.len(),
        hair: (slot * 3 + slot / 7 + 1) % HAIR.len(),
        skin: (slot * 5 + 2) % SKIN.len(),
        style: (slot * 2 + slot / 4) % HAIR_STYLES,
    }
}

// ---------------------------------------------------------------------------------------------
// Foliage and decoration (full color sprites)
// ---------------------------------------------------------------------------------------------

/// Ramp of leaves, from the darkest outline to the highlight.
pub const FOLIAGE: [Rgb; 6] = [
    [22, 54, 48],
    [34, 86, 60],
    [50, 118, 66],
    [76, 148, 72],
    [118, 178, 80],
    [170, 206, 100],
];
/// Ramp of pine needles: bluer than the leaves.
pub const PINE: [Rgb; 5] = [
    [18, 48, 52],
    [28, 76, 66],
    [40, 104, 78],
    [62, 134, 88],
    [104, 168, 100],
];
/// Ramp of birch leaves: yellower than the leaves.
pub const BIRCH: [Rgb; 5] = [
    [34, 78, 52],
    [64, 118, 56],
    [104, 156, 62],
    [150, 188, 78],
    [206, 220, 118],
];
/// Ramp of trunks.
pub const BARK: [Rgb; 3] = [[48, 30, 34], [84, 56, 44], [124, 88, 58]];
/// Birch bark: light with dark marks.
pub const BIRCH_BARK: [Rgb; 3] = [[56, 48, 60], [180, 176, 170], [230, 226, 214]];
/// Flower petal colors: red, yellow, white, blue.
pub const PETALS: [Rgb; 4] = [
    [226, 70, 82],
    [250, 214, 70],
    [246, 242, 236],
    [96, 140, 240],
];

// ---------------------------------------------------------------------------------------------
// Interface
// ---------------------------------------------------------------------------------------------

/// Colors of the interface. All are fixed, none is tinted.
pub mod ui {
    use super::Rgb;
    /// Outer line of a panel and the text shadow.
    pub const INK: Rgb = [14, 12, 24];
    /// Panel fill.
    pub const PANEL: Rgb = [30, 32, 52];
    /// Light bevel line (top and left) of a panel.
    pub const PANEL_LIGHT: Rgb = [74, 80, 118];
    /// Dark bevel line (bottom and right) of a panel.
    pub const PANEL_DARK: Rgb = [18, 18, 32];
    /// Button fill, hover fill, and active fill.
    pub const BUTTON: Rgb = [46, 50, 78];
    /// Button fill under the mouse.
    pub const BUTTON_HOVER: Rgb = [66, 74, 112];
    /// Button fill of the chosen tool or speed.
    pub const BUTTON_ACTIVE: Rgb = [36, 112, 96];
    /// Main text.
    pub const TEXT: Rgb = [242, 238, 228];
    /// Titles.
    pub const ACCENT: Rgb = [150, 218, 226];
    /// Hints and section headings.
    pub const DIM: Rgb = [146, 154, 184];
    /// Keys of key and value rows.
    pub const KEY: Rgb = [186, 190, 212];
    /// The selection color. Yellow is for selection only.
    pub const SELECT: Rgb = [255, 242, 51];
}

/// Every named color of this file with a short name, for the swatch sheet.
pub fn swatches() -> Vec<(&'static str, Vec<Rgb>)> {
    vec![
        ("foliage", FOLIAGE.to_vec()),
        ("pine", PINE.to_vec()),
        ("birch", BIRCH.to_vec()),
        ("bark", BARK.to_vec()),
        ("petals", PETALS.to_vec()),
        ("hair", HAIR.iter().flatten().copied().collect()),
        ("skin", SKIN.iter().flatten().copied().collect()),
        ("shirts", SHIRTS.to_vec()),
        ("unit", vec![OUTLINE, EYE, BOOTS, PANTS[0], PANTS[1]]),
        (
            "ui",
            vec![
                ui::INK,
                ui::PANEL,
                ui::PANEL_LIGHT,
                ui::PANEL_DARK,
                ui::BUTTON,
                ui::BUTTON_HOVER,
                ui::BUTTON_ACTIVE,
                ui::TEXT,
                ui::ACCENT,
                ui::DIM,
                ui::KEY,
                ui::SELECT,
            ],
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_plain_level_is_the_plain_color() {
        for c in [
            [96, 150, 70],
            [214, 196, 128],
            [34, 34, 38],
            [255, 255, 255],
        ] {
            assert_eq!(ramp(c, BASE_LEVEL), c);
        }
    }

    #[test]
    fn shades_turn_cooler_and_highlights_turn_warmer() {
        let c = [96, 150, 70];
        let dark = ramp(c, 150);
        assert!(dark[0] < c[0] && dark[1] < c[1]);
        // The blue share of a shade is larger than the blue share of the plain color.
        let share = |p: Rgb| i32::from(p[2]) * 1000 / (i32::from(p[0]) + i32::from(p[1]) + 1);
        assert!(share(dark) > share(c));
        let light = ramp(c, 255);
        assert!(light[0] > c[0] + 40 && light[1] > c[1]);
        assert!(i32::from(light[0]) - i32::from(c[0]) > i32::from(light[2]) - i32::from(c[2]));
    }

    #[test]
    fn levels_never_get_darker_when_the_level_rises() {
        let c = [128, 124, 122];
        let lum = |p: Rgb| u32::from(p[0]) * 3 + u32::from(p[1]) * 6 + u32::from(p[2]);
        let mut last = 0;
        for level in (0..=255).step_by(5) {
            let now = lum(ramp(c, level));
            assert!(now >= last, "level {level}");
            last = now;
        }
    }

    #[test]
    fn looks_repeat_for_the_same_slot_and_differ_between_neighbors() {
        assert_eq!(look_for(3), look_for(3));
        for slot in 0..20 {
            assert_ne!(look_for(slot), look_for(slot + 1));
            let l = look_for(slot);
            assert!(l.shirt < SHIRTS.len() && l.hair < HAIR.len());
            assert!(l.skin < SKIN.len() && l.style < HAIR_STYLES);
        }
    }

    #[test]
    fn no_unit_color_is_gray_except_the_shirt_tints() {
        for c in HAIR.iter().flatten().chain(SKIN.iter().flatten()) {
            assert!(!(c[0] == c[1] && c[1] == c[2]), "{c:?}");
        }
    }
}
