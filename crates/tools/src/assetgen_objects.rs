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

// ------------------------------------------------------------------------------------ plants

/// Color of a plant grid character. `.` is transparent. The greens are the foliage ramp.
fn plant_color(c: char) -> Option<Rgb> {
    let f = &pal::FOLIAGE;
    Some(match c {
        'o' => f[0],
        '1' => f[1],
        '2' => f[2],
        '3' => f[3],
        '4' => f[4],
        // Soil: dark, mid, light.
        's' => [58, 40, 38],
        'S' => pal::BARK[1],
        'h' => pal::BARK[2],
        // Grain: light, mid, dark.
        'z' => [240, 208, 110],
        'y' => [212, 166, 60],
        'Y' => [160, 116, 44],
        // Turnip bulb: pale, purple, dark purple.
        'p' => [232, 220, 236],
        'P' => [174, 140, 190],
        'q' => [112, 84, 134],
        // Rye: ear tan, stalk blue-green, dark stalk.
        'a' => [196, 180, 126],
        'b' => [100, 142, 116],
        'e' => [64, 98, 88],
        // Sorghum: seed head red-brown light and dark, stalk.
        'r' => [168, 70, 50],
        'R' => [104, 40, 38],
        'k' => [104, 136, 74],
        // Dead: light, mid, dark.
        'c' => [166, 152, 126],
        'x' => [122, 106, 88],
        'X' => [76, 66, 60],
        // Berry, dark berry; pale root top, its shade; flax flower, its shade.
        'B' => [218, 54, 66],
        'D' => [132, 28, 48],
        'u' => [238, 226, 198],
        'U' => [184, 160, 126],
        'f' => [110, 156, 244],
        'F' => [58, 88, 190],
        _ => return None,
    })
}

/// Draw a plant from 16 text rows. The characters in `hide` are not drawn as they are: a not
/// ripe plant has no berries, root top, or flowers.
fn plant(img: &mut Image, cell: usize, rows: &[&str], hide: &str) {
    for (y, row) in rows.iter().enumerate() {
        for (x, c) in row.chars().enumerate() {
            let c = if hide.contains(c) {
                match c {
                    'B' | 'D' => '2',
                    'u' | 'U' => 'S',
                    _ => '.',
                }
            } else {
                c
            };
            if let Some(color) = plant_color(c) {
                img.set_cell(cell, x as i32, y as i32, px(color));
            }
        }
    }
}

const SPROUT: [&str; 16] = [
    "................",
    "................",
    "................",
    "................",
    "................",
    "................",
    "................",
    "................",
    "....oo....oo....",
    "...o34oo.o43o...",
    "....o3o2o3o.....",
    ".....oo2oo......",
    "....osSSSSso....",
    "...osSShhSSso...",
    "..osSSSSSSSSso..",
    "................",
];

const GROWING: [&str; 16] = [
    "................",
    "................",
    "................",
    "......oo........",
    ".....o34o..oo...",
    "..oo.o33o.o34o..",
    ".o34oo3ooo.33o..",
    ".o333o23o.o3o...",
    "..o33o2o3oo3o...",
    "...o3o2o33o.....",
    "....o32o3o......",
    ".....o2o2o......",
    "....osSSSSso....",
    "...osSShhSSso...",
    "..osSSSSSSSSso..",
    "................",
];

const BARLEY: [&str; 16] = [
    "................",
    "...zy.....zy....",
    "..zyYy...zyYy...",
    "..yYyY..zyyYy...",
    "...yYy...yYy.zy.",
    "....k.....k.yYyY",
    "....k..zy.k..Yy.",
    "...ok.zyYyk.....",
    "...ok..yYk.k....",
    "....kk..k.k.....",
    "....2k.2k2k.....",
    ".....2k2k2......",
    "....osSSSSso....",
    "...osSShhSSso...",
    "..osSSSSSSSSso..",
    "................",
];

const TURNIP: [&str; 16] = [
    "................",
    "................",
    "................",
    "................",
    "................",
    "....o3o..o3o....",
    "...o344o3o44o...",
    "..o33o3443o33o..",
    "..o3o.o23o.o3o..",
    "......o22o......",
    "......o2o.......",
    "....osppppso....",
    "...opppppPPso...",
    "..osPpppPPqqso..",
    "..osSqPPqqSSso..",
    "................",
];

const RYE: [&str; 16] = [
    "...a.....a......",
    "..aaa...aaa..a..",
    "..aaa..aaaa.aaa.",
    "..aaa...bab.aaa.",
    "...b....bab..b..",
    "...b..a.b...b...",
    "...b.aaab...b.a.",
    "..eb..aab..eb.aa",
    "..eb...b...eb..a",
    "..b.e..b..eb..b.",
    "..b.eb.b.eb..b..",
    "...beb.bbe..b...",
    "...osSbSSbsSo...",
    "..osSShhSSSSso..",
    "..osSSSSSSSSso..",
    "................",
];

const SORGHUM: [&str; 16] = [
    "................",
    ".......rr.......",
    "......rrRr......",
    "......rrRR......",
    "......rRrR......",
    ".......RR.......",
    ".......kk.......",
    "..oo...k2..oo...",
    ".o3kk..k2.k33o..",
    "..o33k.k2k3o....",
    "....o3kk2k3.....",
    ".....o3k2o......",
    "....osSk2sso....",
    "...osSShhSSso...",
    "..osSSSSSSSSso..",
    "................",
];

const DEAD: [&str; 16] = [
    "................",
    "................",
    "................",
    "................",
    "......c.........",
    ".....cx...c.....",
    "....cx.....xc...",
    "....x...x...x...",
    "....x...x..cx...",
    "...cx..cx..x....",
    "...X...x...X....",
    "...X..xX..X.....",
    "....osXSXsso....",
    "...osSShhSSso...",
    "..osSSSSSSSSso..",
    "................",
];

const BERRY_BUSH: [&str; 16] = [
    "................",
    "................",
    "................",
    "................",
    "....oooooo......",
    "..oo334433oo....",
    ".o3344B4343Boo..",
    ".o334433D4334o..",
    "o334B4433443B1o.",
    "o33443D4334433o.",
    "o2334433B433321o",
    ".o23343344B32o1.",
    ".oo2233222221oo.",
    "..osoo11111ooso.",
    "...ssssssssss...",
    "................",
];

const WILD_ROOT: [&str; 16] = [
    "................",
    "................",
    "................",
    "................",
    "................",
    "....o3o.........",
    "...o34o...o3o...",
    "..o33o33o.o43o..",
    "..o3o.o34o.o3o..",
    "...o3o.o3oo3o...",
    "....o3o23o3o....",
    ".....o3o2o3o....",
    "....oo2uuUoo....",
    "...osSuuUUSso...",
    "..osSSSUUSSSso..",
    "................",
];

const WILD_FLAX: [&str; 16] = [
    "................",
    "................",
    "...fF.....Ff....",
    "..fFf.fF..fF....",
    "...k..fFf..k.fF.",
    "...k...k...k.Ff.",
    "..ok...k...k.k..",
    "..k.k..k..k..k..",
    "..k.k..k..k..ok.",
    "...k..ok..k.k...",
    "...k..k.k.k.k...",
    "...2k.k.2.k2....",
    "...2o2k2o2k2....",
    "...osSkSkSsso...",
    "..osSSSSSSSSso..",
    "................",
];

/// Draw all plant cells.
fn plants(img: &mut Image) {
    plant(img, o::CROP_SPROUT, &SPROUT, "");
    plant(img, o::CROP_GROWING, &GROWING, "");
    let ripe: [&[&str; 16]; 4] = [&BARLEY, &TURNIP, &RYE, &SORGHUM];
    for (k, rows) in ripe.iter().enumerate() {
        plant(img, o::CROP_RIPE + k, *rows, "");
    }
    plant(img, o::CROP_DEAD, &DEAD, "");
    let wild: [(&[&str; 16], &str); 3] =
        [(&BERRY_BUSH, "BD"), (&WILD_ROOT, "uU"), (&WILD_FLAX, "fF")];
    for (k, (rows, hide)) in wild.iter().enumerate() {
        plant(img, o::WILD + 2 * k, *rows, "");
        plant(img, o::WILD + 2 * k + 1, *rows, hide);
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
    plants(&mut img);
    img
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cell_pixels(img: &Image, cell: usize) -> Vec<[u8; 4]> {
        (0..16)
            .flat_map(|y| (0..16).map(move |x| (x, y)))
            .map(|(x, y)| img.get_cell(cell, x, y))
            .collect()
    }

    #[test]
    fn plant_grids_have_16_rows_of_16_columns() {
        let grids: [&[&str; 16]; 10] = [
            &SPROUT,
            &GROWING,
            &BARLEY,
            &TURNIP,
            &RYE,
            &SORGHUM,
            &DEAD,
            &BERRY_BUSH,
            &WILD_ROOT,
            &WILD_FLAX,
        ];
        for (i, grid) in grids.iter().enumerate() {
            for row in grid.iter() {
                assert_eq!(row.chars().count(), 16, "grid {i}: {row}");
            }
        }
    }

    #[test]
    fn plant_cells_are_not_empty_and_differ() {
        let img = make(1);
        let first = o::CROP_SPROUT;
        let last = o::WILD + 2 * o::WILD_COUNT;
        let mut seen: Vec<Vec<[u8; 4]>> = Vec::new();
        for cell in first..last {
            let px = cell_pixels(&img, cell);
            let solid = px.iter().filter(|p| p[3] == 255).count();
            assert!(
                (20..=150).contains(&solid),
                "cell {cell} has {solid} pixels"
            );
            // A 1 pixel margin stays free at the top and left edge.
            assert_eq!(img.get_cell(cell, 0, 0)[3], 0, "cell {cell}");
            assert!(!seen.contains(&px), "cell {cell} repeats another cell");
            seen.push(px);
        }
    }

    #[test]
    fn ripe_wild_plant_differs_from_not_ripe() {
        let img = make(1);
        for k in 0..o::WILD_COUNT {
            let ripe = cell_pixels(&img, o::WILD + 2 * k);
            let green = cell_pixels(&img, o::WILD + 2 * k + 1);
            assert_ne!(ripe, green, "wild plant {k}");
        }
    }

    #[test]
    fn plants_are_deterministic() {
        assert_eq!(make(1).rgba, make(1).rgba);
    }
}
