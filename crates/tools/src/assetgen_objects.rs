//! Trees and ground decoration for `assetgen`: full color sprites of 16 x 16 pixels.
//!
//! Camera: three-quarter top-down. A tree shows its canopy above and a short trunk below.
//! The light comes from the north west: each canopy lump is lighter at the north west and darker
//! at the south east. The outline is the darkest leaf color, not black. The lit side of the
//! outline is one step lighter. The ground shadow of a tree is made by the map baker.

use super::cells::objects as o;
use super::palette::{self as pal, Rgb};
use super::{Image, rng_for};
use base::rng::Pcg32;

fn px(c: Rgb) -> [u8; 4] {
    [c[0], c[1], c[2], 255]
}

/// Draw a trunk of 4 pixels width: outline, light, shade, outline. It starts at row `top`.
fn trunk(img: &mut Image, cell: usize, x: i32, top: i32, bark: &[Rgb; 3]) {
    for y in top..15 {
        let last = y == 14;
        let cols = [bark[0], bark[2], bark[1], bark[0]];
        for (i, c) in cols.iter().enumerate() {
            let c = if last { bark[0] } else { *c };
            img.set_cell(cell, x + i as i32, y, px(c));
        }
    }
}

/// Draw a canopy made of round lumps. `lumps` are `(x, y, radius)` in half pixels.
fn canopy(img: &mut Image, cell: usize, lumps: &[(i32, i32, i32)], ramp: &[Rgb], rng: &mut Pcg32) {
    let mut depth = [[0i32; 16]; 16];
    let mut lit = [[0i32; 16]; 16];
    for y in 0..16i32 {
        for x in 0..16i32 {
            let (hx, hy) = (2 * x + 1, 2 * y + 1);
            let mut best = 0;
            for &(cx, cy, r) in lumps {
                let (dx, dy) = (hx - cx, hy - cy);
                let d = ((dx * dx + dy * dy) as f32).sqrt() as i32;
                if r - d > best {
                    best = r - d;
                    // 100 at the north west of a lump, -100 at the south east.
                    lit[y as usize][x as usize] = -(dx + dy) * 100 / (2 * r);
                }
            }
            depth[y as usize][x as usize] = best;
        }
    }
    let inside = |x: i32, y: i32| {
        (0..16).contains(&x) && (0..16).contains(&y) && depth[y as usize][x as usize] > 0
    };
    let bottom = (0..16i32)
        .filter(|&y| (0..16).any(|x| inside(x, y)))
        .max()
        .unwrap_or(0);
    // Leaf bunches: 2 x 2 blocks that are one step lighter. They are chosen once per sprite.
    let mut bunch = [[false; 8]; 8];
    for row in &mut bunch {
        for b in row.iter_mut() {
            *b = rng.below(4) == 0;
        }
    }
    for y in 0..16i32 {
        for x in 0..16i32 {
            if !inside(x, y) {
                continue;
            }
            let (d, l) = (depth[y as usize][x as usize], lit[y as usize][x as usize]);
            let edge =
                !(inside(x - 1, y) && inside(x + 1, y) && inside(x, y - 1) && inside(x, y + 1));
            let mut step: i32 = if l > 45 {
                4
            } else if l > -5 {
                3
            } else if l > -50 {
                2
            } else {
                1
            };
            if bunch[y as usize / 2][x as usize / 2] && step < 4 && l > -50 {
                step += 1;
            }
            // A dark crease where two lumps meet, and a dark underside.
            if d <= 2 || y >= bottom - 1 {
                step = step.min(1);
            }
            let color = if edge {
                if l > 20 { ramp[1] } else { ramp[0] }
            } else {
                ramp[step as usize]
            };
            img.set_cell(cell, x, y, px(color));
        }
    }
}

fn oak(img: &mut Image, cell: usize, seed: u64, kind: usize) {
    let mut rng = rng_for(seed, 200 + cell as u64);
    let mut lumps: Vec<(i32, i32, i32)> = match kind {
        // Broad: five lumps.
        0 => vec![
            (16, 14, 11),
            (10, 14, 8),
            (22, 14, 8),
            (12, 8, 7),
            (21, 8, 7),
        ],
        // Wide and low.
        1 => vec![(16, 15, 10), (8, 15, 7), (24, 15, 7), (15, 9, 8)],
        // Tall and narrow.
        2 => vec![(16, 14, 9), (15, 8, 8), (18, 20, 7), (11, 11, 6)],
        // Small.
        _ => vec![(16, 18, 8), (12, 15, 6), (20, 15, 6)],
    };
    for l in &mut lumps {
        l.0 += rng.below(3) as i32 - 1;
        l.1 += rng.below(3) as i32 - 1;
    }
    let top = if kind == 3 { 9 } else { 11 };
    trunk(img, cell, 6, top, &pal::BARK);
    canopy(img, cell, &lumps, &pal::FOLIAGE, &mut rng);
}

fn birch(img: &mut Image, cell: usize, seed: u64, kind: usize) {
    let mut rng = rng_for(seed, 200 + cell as u64);
    let lumps: Vec<(i32, i32, i32)> = if kind == 0 {
        vec![(16, 12, 9), (11, 10, 7), (21, 11, 7), (16, 18, 6)]
    } else {
        vec![(15, 13, 9), (21, 11, 7), (10, 12, 6), (14, 6, 6)]
    };
    // The trunk is light with two dark marks.
    trunk(img, cell, 6, 10, &pal::BIRCH_BARK);
    img.set_cell(cell, 8, 13, px(pal::BIRCH_BARK[0]));
    img.set_cell(cell, 7, 12, px(pal::BIRCH_BARK[0]));
    canopy(img, cell, &lumps, &pal::BIRCH, &mut rng);
}

/// A pine: stacked tiers that grow wider toward the bottom.
fn pine(img: &mut Image, cell: usize, kind: usize) {
    let ramp = &pal::PINE;
    // Trunk first, tiers on top. Tiers are drawn from the bottom tier to the top tier.
    for y in 12..15 {
        let cols = [pal::BARK[0], pal::BARK[2], pal::BARK[1], pal::BARK[0]];
        for (i, c) in cols.iter().enumerate() {
            img.set_cell(
                cell,
                6 + i as i32,
                y,
                px(if y == 14 { pal::BARK[0] } else { *c }),
            );
        }
    }
    // (top row, bottom row, half width at the top, half width at the bottom)
    let tiers: &[(i32, i32, i32, i32)] = if kind == 0 {
        &[(7, 12, 2, 6), (4, 8, 1, 5), (1, 5, 0, 3)]
    } else {
        &[(8, 12, 2, 5), (5, 9, 1, 4), (3, 6, 1, 3), (0, 3, 0, 2)]
    };
    for &(y0, y1, w0, w1) in tiers {
        for y in y0..=y1 {
            let hw = w0 + (w1 - w0) * (y - y0) / (y1 - y0).max(1);
            for x in (8 - hw - 1)..(8 + hw + 1) {
                let dx = x - 8 + i32::from(x < 8);
                let edge = x == 8 - hw - 1 || x == 8 + hw || y == y0 && hw == 0;
                let bottom_row = y == y1;
                let color = if edge || bottom_row {
                    if dx < 0 && !bottom_row {
                        ramp[1]
                    } else {
                        ramp[0]
                    }
                } else if y == y0 + 1 && dx.abs() <= 1 {
                    ramp[4]
                } else if dx < -hw / 3 {
                    ramp[3]
                } else if dx > hw / 2 {
                    ramp[1]
                } else {
                    ramp[2]
                };
                img.set_cell(cell, x, y, px(color));
            }
        }
    }
}

fn flower(img: &mut Image, cell: usize, color: Rgb) {
    let stem = pal::FOLIAGE[2];
    let leaf = pal::FOLIAGE[3];
    for (cx, cy) in [(4, 5), (11, 10)] {
        img.set_cell(cell, cx, cy + 2, px(stem));
        img.set_cell(cell, cx, cy + 3, px(stem));
        img.set_cell(cell, cx + 1, cy + 3, px(leaf));
        for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
            img.set_cell(cell, cx + dx, cy + dy, px(color));
        }
        // A light top left pixel and a dark bottom right pixel.
        img.set_cell(cell, cx, cy, px(pal::shade(color, -2)));
        img.set_cell(cell, cx + 1, cy + 1, px(pal::shade(color, 1)));
    }
}

fn pebbles(img: &mut Image, cell: usize, kind: usize) {
    let stones: &[(i32, i32)] = match kind {
        0 => &[(3, 4), (10, 10), (12, 4)],
        1 => &[(5, 9), (11, 5)],
        _ => &[(2, 10), (8, 4), (11, 11)],
    };
    for &(x, y) in stones {
        let rows = [
            (1, 2, [196, 194, 200]),
            (0, 4, [150, 148, 158]),
            (1, 3, [104, 102, 116]),
        ];
        for (r, (dx, w, c)) in rows.iter().enumerate() {
            for i in 0..*w {
                img.set_cell(cell, x + dx + i, y + r as i32, px(*c));
            }
        }
    }
}

fn tuft(img: &mut Image, cell: usize, kind: usize) {
    let (dark, mid, light) = (pal::FOLIAGE[1], pal::FOLIAGE[3], pal::FOLIAGE[4]);
    let spots: &[(i32, i32)] = if kind == 0 {
        &[(4, 9), (11, 5)]
    } else {
        &[(7, 8), (12, 12), (3, 4)]
    };
    for &(x, y) in spots {
        img.set_cell(cell, x, y, px(dark));
        img.set_cell(cell, x + 1, y, px(dark));
        img.set_cell(cell, x - 1, y - 1, px(mid));
        img.set_cell(cell, x, y - 1, px(mid));
        img.set_cell(cell, x + 1, y - 1, px(light));
        img.set_cell(cell, x - 1, y - 2, px(light));
        img.set_cell(cell, x + 2, y - 2, px(mid));
    }
}

/// Make `objects.png`.
pub fn make(seed: u64) -> Image {
    let mut img = Image::cell_sheet(o::ROWS);
    for k in 0..3 {
        oak(&mut img, o::TREE + k, seed, k);
    }
    oak(&mut img, o::TREE + 3, seed, 3);
    pine(&mut img, o::TREE + 4, 0);
    pine(&mut img, o::TREE + 5, 1);
    birch(&mut img, o::TREE + 6, seed, 0);
    birch(&mut img, o::TREE + 7, seed, 1);
    for k in 0..o::FLOWER_COUNT {
        flower(&mut img, o::FLOWER + k, pal::PETALS[k]);
    }
    for k in 0..o::PEBBLE_COUNT {
        pebbles(&mut img, o::PEBBLE + k, k);
    }
    for k in 0..o::TUFT_COUNT {
        tuft(&mut img, o::TUFT + k, k);
    }
    img
}
