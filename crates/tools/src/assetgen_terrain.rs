//! Ground textures for `assetgen`: seamless gray textures of 128 x 128 pixels, and the ore sprites.
//!
//! Every texture repeats with a period of 128 pixels in both directions. Tone patches come from
//! wrapped value noise, cut into 3 flat steps and cleaned of single pixels. Details (tufts, clods,
//! ripples, cracks, plates) sit on top of the patches. There is no per-pixel random noise.

use super::cells::{terrain as t, tiles};
use super::{Image, hash3, rng_for};

const S: i32 = t::SIZE as i32;

/// A square of values with wrap-around access.
struct Field {
    v: Vec<i32>,
}

impl Field {
    fn new(fill: i32) -> Self {
        Self {
            v: vec![fill; t::SIZE * t::SIZE],
        }
    }
    fn at(&self, x: i32, y: i32) -> i32 {
        self.v[(y.rem_euclid(S) * S + x.rem_euclid(S)) as usize]
    }
    fn set(&mut self, x: i32, y: i32, value: i32) {
        self.v[(y.rem_euclid(S) * S + x.rem_euclid(S)) as usize] = value;
    }
}

/// Smooth value noise with a lattice of `spacing` pixels that repeats every 128 pixels. 0..=255.
fn vnoise(seed: u64, salt: u64, x: i32, y: i32, spacing: i32) -> i32 {
    let n = S / spacing;
    let (ix, iy) = (x.div_euclid(spacing), y.div_euclid(spacing));
    let (fx, fy) = (x.rem_euclid(spacing), y.rem_euclid(spacing));
    let lat = |i: i32, j: i32| (hash3(seed, salt, i.rem_euclid(n), j.rem_euclid(n)) & 255) as i32;
    let smooth = |f: i32| f * f * (3 * spacing - 2 * f) * 1024 / (spacing * spacing * spacing);
    let (sx, sy) = (smooth(fx), smooth(fy));
    let top = lat(ix, iy) * (1024 - sx) + lat(ix + 1, iy) * sx;
    let bottom = lat(ix, iy + 1) * (1024 - sx) + lat(ix + 1, iy + 1) * sx;
    (top / 1024 * (1024 - sy) + bottom / 1024 * sy) / 1024
}

/// A tone field cut into 3 classes (0 dark, 1 middle, 2 light) and cleaned of single pixels.
fn tone_classes(seed: u64, salt: u64, spacing: i32, low: i32, high: i32) -> Field {
    let mut f = Field::new(1);
    for y in 0..S {
        for x in 0..S {
            let n = (vnoise(seed, salt, x, y, spacing) * 5
                + vnoise(seed, salt + 1, x, y, spacing / 2) * 2
                + vnoise(seed, salt + 2, x, y, spacing / 4))
                / 8;
            f.set(
                x,
                y,
                if n < low {
                    0
                } else if n > high {
                    2
                } else {
                    1
                },
            );
        }
    }
    for _ in 0..3 {
        let mut g = Field::new(1);
        for y in 0..S {
            for x in 0..S {
                let me = f.at(x, y);
                let n = [
                    f.at(x - 1, y),
                    f.at(x + 1, y),
                    f.at(x, y - 1),
                    f.at(x, y + 1),
                ];
                let same = n.iter().filter(|&&v| v == me).count();
                let majority = n
                    .iter()
                    .copied()
                    .find(|&c| n.iter().filter(|&&v| v == c).count() >= 3);
                g.set(
                    x,
                    y,
                    match (same, majority) {
                        (0..=1, Some(m)) => m,
                        _ => me,
                    },
                );
            }
        }
        f = g;
    }
    f
}

/// Jittered grid of feature points that repeat every 128 pixels.
struct Voronoi {
    cell: i32,
    points: Vec<(f32, f32)>,
    n: i32,
}

impl Voronoi {
    fn new(seed: u64, salt: u64, cell: i32, jitter: f32) -> Self {
        let n = S / cell;
        let mut points = Vec::new();
        for j in 0..n {
            for i in 0..n {
                let h = hash3(seed, salt, i, j);
                let fx = (h & 255) as f32 / 255.0 - 0.5;
                let fy = ((h >> 8) & 255) as f32 / 255.0 - 0.5;
                points.push((
                    (i * cell) as f32 + cell as f32 * (0.5 + fx * jitter),
                    (j * cell) as f32 + cell as f32 * (0.5 + fy * jitter),
                ));
            }
        }
        Self { cell, points, n }
    }

    /// The nearest feature to a pixel: its index and the two smallest distances.
    fn nearest(&self, x: i32, y: i32) -> (usize, f32, f32) {
        let (cx, cy) = (x.div_euclid(self.cell), y.div_euclid(self.cell));
        let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
        let (mut best, mut d1, mut d2) = (0, f32::MAX, f32::MAX);
        for dj in -1..=1 {
            for di in -1..=1 {
                let (i, j) = (cx + di, cy + dj);
                let (wi, wj) = (i.rem_euclid(self.n), j.rem_euclid(self.n));
                let (fx, fy) = self.points[(wj * self.n + wi) as usize];
                let (fx, fy) = (
                    fx + (i - wi) as f32 * self.cell as f32,
                    fy + (j - wj) as f32 * self.cell as f32,
                );
                let d = ((px - fx).powi(2) + (py - fy).powi(2)).sqrt();
                if d < d1 {
                    d2 = d1;
                    d1 = d;
                    best = (wj * self.n + wi) as usize;
                } else if d < d2 {
                    d2 = d;
                }
            }
        }
        (best, d1, d2)
    }

    fn point(&self, index: usize) -> (f32, f32) {
        self.points[index]
    }
}

fn level(class: i32, steps: [i32; 3]) -> i32 {
    steps[class.clamp(0, 2) as usize]
}

fn grass(seed: u64) -> Field {
    let classes = tone_classes(seed, 10, 32, 100, 156);
    let mut f = Field::new(0);
    for y in 0..S {
        for x in 0..S {
            f.set(x, y, level(classes.at(x, y), [208, 222, 234]));
        }
    }
    // Tufts on a jittered grid: even spacing, no clumps. Each tuft is a root and 2 or 3 blades.
    for j in 0..S / 8 {
        for i in 0..S / 8 {
            let h = hash3(seed, 11, i, j);
            if h % 10 >= 6 {
                continue;
            }
            let (x, y) = (
                i * 8 + 2 + (h >> 4) as i32 % 4,
                j * 8 + 3 + (h >> 8) as i32 % 4,
            );
            let base = f.at(x, y);
            f.set(x, y, base - 30);
            f.set(x, y - 1, 250);
            f.set(x + 1, y - 2, 252);
            if (h >> 12) & 1 == 1 {
                f.set(x - 1, y - 2, 246);
            }
        }
    }
    f
}

fn soil(seed: u64) -> Field {
    let classes = tone_classes(seed, 20, 32, 96, 160);
    let mut f = Field::new(0);
    for y in 0..S {
        for x in 0..S {
            f.set(x, y, level(classes.at(x, y), [204, 218, 230]));
        }
    }
    for j in 0..S / 8 {
        for i in 0..S / 8 {
            let h = hash3(seed, 21, i, j);
            match h % 10 {
                0..=4 => {
                    // A clod: light top left, dark bottom right.
                    let (x, y) = (
                        i * 8 + 1 + (h >> 4) as i32 % 5,
                        j * 8 + 1 + (h >> 8) as i32 % 5,
                    );
                    for (dx, dy, v) in [
                        (0, 0, 240),
                        (1, 0, 226),
                        (0, 1, 198),
                        (1, 1, 178),
                        (2, 1, 170),
                    ] {
                        f.set(x + dx, y + dy, v);
                    }
                }
                5 | 6 => {
                    let (x, y) = (
                        i * 8 + 1 + (h >> 4) as i32 % 5,
                        j * 8 + 1 + (h >> 8) as i32 % 5,
                    );
                    f.set(x, y, 244);
                    f.set(x + 1, y, 244);
                    f.set(x + 1, y + 1, 186);
                }
                _ => {}
            }
        }
    }
    f
}

fn sand(seed: u64) -> Field {
    let classes = tone_classes(seed, 30, 64, 90, 166);
    let mut f = Field::new(0);
    for y in 0..S {
        for x in 0..S {
            f.set(x, y, level(classes.at(x, y), [226, 232, 238]));
        }
    }
    // Ripples: contour lines of a smooth field. A light line has a dark line below it.
    let band = |x: i32, y: i32| {
        (vnoise(seed, 31, x, y, 32) * 3 + vnoise(seed, 32, x + y / 2, y, 16)) / 4 / 22
    };
    let mut line = Field::new(0);
    for y in 0..S {
        for x in 0..S {
            let b = band(x, y);
            if b % 2 == 0 && (band(x + 1, y) != b || band(x, y + 1) != b) {
                line.set(x, y, 1);
            }
        }
    }
    for y in 0..S {
        for x in 0..S {
            if line.at(x, y) == 1 {
                f.set(x, y, 248);
            } else if line.at(x - 1, y - 1) == 1 && line.at(x, y - 1) == 0 {
                f.set(x, y, 206);
            }
        }
    }
    f
}

fn clay(seed: u64) -> Field {
    let v = Voronoi::new(seed, 40, 32, 0.8);
    let mut f = Field::new(0);
    for y in 0..S {
        for x in 0..S {
            let (id, d1, d2) = v.nearest(x, y);
            let tone = (hash3(seed, 41, id as i32, 0) % 3) as i32;
            f.set(x, y, [216, 224, 231][tone as usize]);
            if d2 - d1 < 1.5 {
                f.set(x, y, 168);
            }
        }
    }
    // Light chips beside the cracks.
    for j in 0..S / 16 {
        for i in 0..S / 16 {
            let h = hash3(seed, 42, i, j);
            if h.is_multiple_of(3) {
                let (x, y) = (i * 16 + (h >> 4) as i32 % 14, j * 16 + (h >> 9) as i32 % 14);
                if f.at(x, y) != 168 && f.at(x + 1, y) != 168 {
                    f.set(x, y, 242);
                    f.set(x + 1, y, 242);
                }
            }
        }
    }
    f
}

fn water(seed: u64) -> Field {
    let classes = tone_classes(seed, 50, 32, 110, 150);
    let mut f = Field::new(0);
    for y in 0..S {
        for x in 0..S {
            f.set(x, y, level(classes.at(x, y), [208, 216, 224]));
        }
    }
    // Short light ripples with a dark line below them. Static, no animation.
    for j in 0..S / 8 {
        for i in 0..S / 16 {
            let h = hash3(seed, 51, i, j);
            if h % 10 >= 4 {
                continue;
            }
            let len = 3 + (h >> 4) as i32 % 4;
            let (x, y) = (i * 16 + (h >> 8) as i32 % 9, j * 8 + (h >> 12) as i32 % 6);
            for d in 0..len {
                f.set(x + d, y, 244);
            }
            for d in 1..len {
                f.set(x + d, y + 1, 184);
            }
        }
    }
    f
}

/// Plates of a Voronoi pattern. `bevel` gives light edges on the north west and dark seams on
/// the south east of each plate.
fn plates(seed: u64, salt: u64, cell: i32, bevel: bool) -> Field {
    let v = Voronoi::new(seed, salt, cell, 0.85);
    let mut ids = Field::new(0);
    let mut dist = Field::new(0);
    for y in 0..S {
        for x in 0..S {
            let (id, d1, _) = v.nearest(x, y);
            ids.set(x, y, id as i32);
            dist.set(x, y, (d1 * 100.0) as i32);
        }
    }
    let mut f = Field::new(0);
    for y in 0..S {
        for x in 0..S {
            let id = ids.at(x, y);
            let step = (hash3(seed, salt + 1, id, 0) % 5) as i32 - 2;
            let (fx, fy) = v.point(id as usize);
            // Light from the north west: the north west half of a plate is one step lighter.
            let rel = ((x as f32 + 0.5 - fx) + (y as f32 + 0.5 - fy)) / cell as f32;
            let lit = if rel < -0.18 {
                6
            } else if rel > 0.3 {
                -6
            } else {
                0
            };
            let mut val = if bevel {
                226 + step * 4 + lit
            } else {
                224 + step * 3
            };
            let west = ids.at(x - 1, y) != id || ids.at(x, y - 1) != id;
            let east = ids.at(x + 1, y) != id || ids.at(x, y + 1) != id;
            if bevel {
                if east {
                    val = 166;
                } else if west {
                    val = 246;
                }
            } else if east || west {
                val = 186;
            }
            f.set(x, y, val);
        }
    }
    // A few cracks and light chips.
    let mut rng = rng_for(seed, salt + 2);
    for _ in 0..7 {
        let (mut x, mut y) = (rng.below(128) as i32, rng.below(128) as i32);
        for _ in 0..(4 + rng.below(4)) {
            if f.at(x, y) > 150 {
                f.set(x, y, 170);
            }
            x += rng.below(3) as i32 - 1;
            y += 1;
        }
    }
    for _ in 0..28 {
        let (x, y) = (rng.below(128) as i32, rng.below(128) as i32);
        if f.at(x, y) < 250 && f.at(x + 1, y) < 250 && f.at(x, y) > 150 && f.at(x + 1, y) > 150 {
            f.set(x, y, 248);
            f.set(x + 1, y, 248);
        }
    }
    f
}

/// The wall face: courses of blocks. Row 0 is a light lip, the last row is a dark contact line.
fn face(seed: u64) -> Field {
    let mut f = Field::new(0);
    const ROWS_V: [i32; t::FACE_H] = [240, 178, 170, 122, 152, 108];
    let mut rng = rng_for(seed, 60);
    // Block seams of the two courses: widths 9 to 15 pixels that fill 128 exactly.
    let mut seams: Vec<Vec<i32>> = Vec::new();
    for _ in 0..2 {
        let mut xs = vec![rng.below(8) as i32];
        while *xs.last().unwrap_or(&0) < 120 {
            let next = xs.last().copied().unwrap_or(0) + 9 + rng.below(7) as i32;
            xs.push(next);
        }
        seams.push(xs);
    }
    for y in 0..S {
        for x in 0..S {
            let row = y as usize;
            if row >= t::FACE_H {
                f.set(x, y, 0);
                continue;
            }
            let course = usize::from(row >= 4);
            let mut v = ROWS_V[row];
            let block = seams[course].iter().filter(|&&s| s <= x).count() as i32;
            v += ((hash3(seed, 61 + course as u64, block, 0) % 3) as i32 - 1) * 5;
            if row != 0 && row != 3 && row != 5 && seams[course].contains(&x) {
                v -= 44;
            }
            f.set(x, y, v);
        }
    }
    f
}

/// Make `terrain.png`: eight textures in a sheet of 4 columns and 2 rows.
pub fn make_terrain(seed: u64) -> Image {
    let mut img = Image::new(t::SIZE * t::COLS, t::SIZE * t::ROWS);
    let fields = [
        (t::GRASS, grass(seed)),
        (t::SOIL, soil(seed)),
        (t::SAND, sand(seed)),
        (t::CLAY, clay(seed)),
        (t::WATER, water(seed)),
        (t::FLOOR, plates(seed, 70, 16, false)),
        (t::ROCK, plates(seed, 80, 32, true)),
        (t::FACE, face(seed)),
    ];
    for (index, field) in fields {
        let (ox, oy) = t::origin(index);
        for y in 0..S {
            for x in 0..S {
                img.set_gray(ox as i32 + x, oy as i32 + y, field.at(x, y), 255);
            }
        }
    }
    img
}

// Symbols of an ore lump: `o` outline, `d` shade, `b` base, `l` light, `s` sparkle.
const LUMP_A: [&str; 5] = [".oooo.", "olslbo", "olbbdo", "obbddo", ".oddo."];
const LUMP_B: [&str; 4] = [".ooo.", "olsdo", "obbdo", ".ooo."];
const LUMP_C: [&str; 4] = ["oooo.", "olbbo", "obddo", ".ooo."];

fn lump_level(c: char) -> Option<i32> {
    Some(match c {
        'o' => 30,
        'd' => 150,
        'b' => 214,
        'l' => 240,
        's' => 255,
        _ => return None,
    })
}

/// Make `tiles.png`: 8 ore overlays. Each has 3 lumps with a dark rim, so ore reads at once.
pub fn make_tiles(seed: u64) -> Image {
    let mut img = Image::cell_sheet(tiles::ROWS);
    for v in 0..tiles::ORE_COUNT {
        let mut rng = rng_for(seed, 100 + v as u64);
        // Three places on a loose 2 x 2 grid, so that lumps do not touch.
        let mut slots = [(1, 1), (9, 1), (1, 9), (9, 9)];
        for i in (1..slots.len()).rev() {
            slots.swap(i, rng.below(i as u32 + 1) as usize);
        }
        for (k, &(sx, sy)) in slots.iter().take(3).enumerate() {
            let shape: &[&str] = match (v + k) % 3 {
                0 => &LUMP_A,
                1 => &LUMP_B,
                _ => &LUMP_C,
            };
            let (ox, oy) = (sx + rng.below(2) as i32, sy + rng.below(2) as i32);
            for (y, row) in shape.iter().enumerate() {
                for (x, c) in row.chars().enumerate() {
                    if let Some(l) = lump_level(c) {
                        img.set_cell(
                            tiles::ORE + v,
                            ox + x as i32,
                            oy + y as i32,
                            [l as u8, l as u8, l as u8, 255],
                        );
                    }
                }
            }
        }
    }
    img
}
