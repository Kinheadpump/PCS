//! Review sheets for `assetgen`: the palette, a reference set of all sprites, and a repeat grid.
//! These files are for people. The game does not read them (guide sections 11 and 12).

use super::cells::{actors as a, items as it, objects as o, terrain as t};
use super::palette::{self as pal, Rgb};
use super::{Image, actors, items, objects, terrain};

/// Sample tint of each ground texture, close to the colors in `materials.ron`.
pub const SAMPLE_TINT: [Rgb; t::COUNT] = [
    [96, 150, 70],
    [140, 104, 70],
    [214, 196, 128],
    [176, 110, 80],
    [60, 110, 170],
    [140, 140, 146],
    [120, 118, 126],
    [100, 98, 108],
];

fn tinted(px: [u8; 4], tint: Rgb) -> [u8; 4] {
    let c = pal::ramp(tint, i32::from(px[0]));
    [c[0], c[1], c[2], px[3]]
}

fn fill(img: &mut Image, x: i32, y: i32, w: i32, h: i32, c: Rgb) {
    for j in 0..h {
        for i in 0..w {
            img.set(x + i, y + j, [c[0], c[1], c[2], 255]);
        }
    }
}

/// Draw a cell of a source sheet on a target. `tint` multiplies a gray cell. `scale` is a whole
/// number. Transparent pixels leave the target as it is.
fn blit(dst: &mut Image, src: &Image, cell: usize, x: i32, y: i32, scale: i32, tint: Option<Rgb>) {
    for j in 0..16 {
        for i in 0..16 {
            let mut px = src.get_cell(cell, i, j);
            if px[3] == 0 {
                continue;
            }
            if let Some(t) = tint {
                px = tinted(px, t);
            }
            let mut out = px;
            if px[3] != 255 {
                let under = dst.get(x + i * scale, y + j * scale);
                let al = i32::from(px[3]);
                for k in 0..3 {
                    out[k] =
                        ((i32::from(px[k]) * al + i32::from(under[k]) * (255 - al)) / 255) as u8;
                }
                out[3] = 255;
            }
            for dy in 0..scale {
                for dx in 0..scale {
                    dst.set(x + i * scale + dx, y + j * scale + dy, out);
                }
            }
        }
    }
}

/// Every named color as a swatch row.
pub fn palette_sheet() -> Image {
    let rows = pal::swatches();
    let (w, h) = (16 * 16 + 8, rows.len() as i32 * 12 + 8);
    let mut img = Image::new(w as usize, h as usize);
    fill(&mut img, 0, 0, w, h, [24, 24, 32]);
    for (r, (_, colors)) in rows.iter().enumerate() {
        for (i, c) in colors.iter().enumerate() {
            fill(&mut img, 4 + i as i32 * 10, 4 + r as i32 * 12, 9, 9, *c);
        }
    }
    // A ramp strip for each ground tint: levels 0 to 255 in 16 steps.
    img
}

/// A reference set: ground textures, trees, items, colonists, and icons at 2x on a dark ground.
pub fn reference_sheet(seed: u64) -> Image {
    let terrain_img = terrain::make_terrain(seed);
    let objects_img = objects::make(seed);
    let items_img = items::make_items(seed);
    let icons_img = items::make_icons();
    let actors_img = actors::make_sheet();
    let (w, h) = (16 * 2 * 16 + 16, 560);
    let mut img = Image::new(w as usize, h as usize);
    fill(&mut img, 0, 0, w, h, [88, 120, 72]);
    // Ground samples: 32 x 32 pixel patches of each texture.
    #[allow(clippy::needless_range_loop)]
    for index in 0..t::COUNT {
        let (ox, oy) = t::origin(index);
        for j in 0..48 {
            for i in 0..48 {
                let px = terrain_img.get((ox + i) as i32, (oy + j) as i32);
                img.set(
                    8 + index as i32 * 52 + i as i32,
                    8 + j as i32,
                    tinted(px, SAMPLE_TINT[index]),
                );
            }
        }
    }
    // Trees and decor at 3x.
    for cell in 0..o::ROWS * 16 {
        blit(
            &mut img,
            &objects_img,
            cell,
            8 + (cell as i32 % 16) * 50,
            64 + (cell as i32 / 16) * 52,
            3,
            None,
        );
    }
    // Items at 3x on a floor color.
    fill(&mut img, 0, 172, w, 108, [150, 138, 120]);
    for cell in 0..it::SHAPES * it::TIERS {
        let (cx, cy) = ((cell % 9) as i32, (cell / 9) as i32);
        let tint = match cell / it::TIERS {
            it::LOGS => [150, 104, 62],
            it::ROCKS => [150, 150, 156],
            it::ORE => [176, 112, 80],
            it::INGOTS => [196, 200, 214],
            it::LUMPS => [60, 58, 66],
            _ => [176, 130, 74],
        };
        blit(
            &mut img,
            &items_img,
            cell,
            8 + cx * 56,
            178 + cy * 52,
            3,
            Some(tint),
        );
    }
    // Colonists at 3x: one row per style, stand frames of the three facings and two work frames.
    for style in 0..pal::HAIR_STYLES {
        for (k, off) in [
            a::SOUTH,
            a::NORTH,
            a::EAST,
            a::WORK_SOUTH,
            a::WORK_SOUTH + 1,
            a::WORK_EAST,
            a::WORK_EAST + 1,
        ]
        .into_iter()
        .enumerate()
        {
            blit(
                &mut img,
                &actors_img,
                style * a::STYLE_CELLS + off,
                8 + k as i32 * 52,
                290 + style as i32 * 52,
                3,
                None,
            );
        }
    }
    // Icons at 3x.
    fill(&mut img, 0, 500, w, 56, [30, 32, 52]);
    for cell in 0..14 {
        blit(
            &mut img,
            &icons_img,
            cell,
            8 + cell as i32 * 36,
            506,
            2,
            Some([242, 238, 228]),
        );
    }
    img
}

/// Every ground texture repeated 3 x 3 times (384 x 384 pixels each), 4 per row. A reader looks
/// for a visible grid or a repeating blob. The grid of the repeat is 128 pixels.
pub fn repeat_sheet(seed: u64) -> Image {
    let src = terrain::make_terrain(seed);
    let side = (t::SIZE * 3) as i32;
    let mut img = Image::new((side * 4) as usize, (side * 2) as usize);
    #[allow(clippy::needless_range_loop)]
    for index in 0..t::COUNT {
        let (ox, oy) = t::origin(index);
        let (px, py) = ((index % 4) as i32 * side, (index / 4) as i32 * side);
        for y in 0..side {
            for x in 0..side {
                let s = src.get(
                    ox as i32 + x % t::SIZE as i32,
                    oy as i32 + y % t::SIZE as i32,
                );
                img.set(px + x, py + y, tinted(s, SAMPLE_TINT[index]));
            }
        }
    }
    img
}
