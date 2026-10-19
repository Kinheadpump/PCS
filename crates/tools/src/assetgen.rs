//! `assetgen`: makes the 16 x 16 pixel tile sheet with code (D-034).
//!
//! The sheet is mostly grayscale. The game multiplies each pixel with the material color.
//! A pixel value of `BASE_LEVEL` (224) means "the plain material color". The same seed
//! gives the same bytes. The PNG writer is our own: stored deflate, CRC32, Adler-32.

use base::rng::Pcg32;

#[path = "assetgen_actors.rs"]
pub mod actors;

/// Sprite side in pixels.
pub const TILE: usize = 16;
/// Sprites per sheet row.
pub const COLUMNS: usize = 16;
/// Rows in the sheet.
pub const ROWS: usize = 3;
/// Value that means "the plain material color".
pub const BASE_LEVEL: i32 = 224;
/// Default seed of the shipped sheet.
pub const DEFAULT_SEED: u64 = 1;

/// Variants of each ground sprite.
const VARIANTS: usize = 4;

/// Sheet cell numbers. `game/src/sprites.rs` repeats these numbers.
pub mod cell {
    /// First grass variant (4 variants).
    pub const GRASS: usize = 0;
    /// First soil variant.
    pub const SOIL: usize = 4;
    /// First sand variant.
    pub const SAND: usize = 8;
    /// First clay variant.
    pub const CLAY: usize = 12;
    /// First water variant.
    pub const WATER: usize = 16;
    /// First stone floor variant.
    pub const FLOOR: usize = 20;
    /// First rock wall top variant.
    pub const WALL: usize = 24;
    /// First ore speckle variant (overlay).
    pub const ORE: usize = 28;
    /// First tree canopy variant (2 variants).
    pub const TREE: usize = 32;
    /// Tree shadow.
    pub const TREE_SHADOW: usize = 34;
    /// Edge masks: north, east, south, west (white, alpha ramp from the edge).
    pub const EDGE: usize = 36;
    /// First wall face variant (2 variants, bottom rows only).
    pub const WALL_FACE: usize = 40;
}

/// An RGBA image.
pub struct Sheet {
    /// Width in pixels.
    pub width: usize,
    /// Height in pixels.
    pub height: usize,
    /// RGBA bytes, row-major.
    pub rgba: Vec<u8>,
}

impl Sheet {
    fn new() -> Self {
        let (width, height) = (COLUMNS * TILE, ROWS * TILE);
        Self {
            width,
            height,
            rgba: vec![0; width * height * 4],
        }
    }

    fn put(&mut self, cell: usize, x: i32, y: i32, value: i32, alpha: u8) {
        if !(0..TILE as i32).contains(&x) || !(0..TILE as i32).contains(&y) {
            return;
        }
        let px = (cell % COLUMNS) * TILE + x as usize;
        let py = (cell / COLUMNS) * TILE + y as usize;
        let v = value.clamp(0, 255) as u8;
        let i = (py * self.width + px) * 4;
        self.rgba[i..i + 4].copy_from_slice(&[v, v, v, alpha]);
    }
}

fn rng_for(seed: u64, cell: usize) -> Pcg32 {
    Pcg32::from_seed(seed ^ (cell as u64 + 1).wrapping_mul(0x9E37_79B9_7F4A_7C15))
}

/// A random value in `-spread..=spread`.
fn jitter(rng: &mut Pcg32, spread: i32) -> i32 {
    rng.below(2 * spread as u32 + 1) as i32 - spread
}

fn fill_noise(sheet: &mut Sheet, cell: usize, rng: &mut Pcg32, base: i32, spread: i32) {
    for y in 0..TILE as i32 {
        for x in 0..TILE as i32 {
            sheet.put(cell, x, y, base + jitter(rng, spread), 255);
        }
    }
}

fn grass(sheet: &mut Sheet, cell: usize, seed: u64) {
    let mut rng = rng_for(seed, cell);
    // Quiet base: two close values only, so the ground does not fight with the units.
    fill_noise(sheet, cell, &mut rng, BASE_LEVEL - 2, 3);
    // Tufts: a dark root pixel with a light blade above it and one beside it.
    for _ in 0..5 {
        let x = 1 + rng.below(13) as i32;
        let y = 3 + rng.below(11) as i32;
        sheet.put(cell, x, y, 190, 255);
        sheet.put(cell, x, y - 1, 244, 255);
        sheet.put(cell, x + 1, y - 2, 252, 255);
        sheet.put(cell, x + 1, y, 198, 255);
    }
}

fn soil(sheet: &mut Sheet, cell: usize, seed: u64) {
    let mut rng = rng_for(seed, cell);
    fill_noise(sheet, cell, &mut rng, BASE_LEVEL, 13);
    for _ in 0..4 {
        let x = rng.below(14) as i32;
        let y = rng.below(14) as i32;
        sheet.put(cell, x, y, 176, 255);
        sheet.put(cell, x + 1, y, 190, 255);
        sheet.put(cell, x, y + 1, 168, 255);
        sheet.put(cell, x - 1, y - 1, 244, 255);
    }
}

fn sand(sheet: &mut Sheet, cell: usize, seed: u64) {
    let mut rng = rng_for(seed, cell);
    fill_noise(sheet, cell, &mut rng, BASE_LEVEL + 6, 7);
    for _ in 0..3 {
        let x = rng.below(11) as i32;
        let y = 1 + rng.below(14) as i32;
        for d in 0..4 {
            sheet.put(cell, x + d, y, 250, 255);
            sheet.put(cell, x + d + 1, y + 1, 200, 255);
        }
    }
}

fn clay(sheet: &mut Sheet, cell: usize, seed: u64) {
    let mut rng = rng_for(seed, cell);
    fill_noise(sheet, cell, &mut rng, BASE_LEVEL, 8);
    for _ in 0..2 {
        let (mut x, mut y) = (rng.below(16) as i32, rng.below(16) as i32);
        for _ in 0..5 {
            sheet.put(cell, x, y, 168, 255);
            x += rng.below(3) as i32 - 1;
            y += 1;
        }
    }
}

fn water(sheet: &mut Sheet, cell: usize, seed: u64) {
    let mut rng = rng_for(seed, cell);
    fill_noise(sheet, cell, &mut rng, BASE_LEVEL - 8, 5);
    for _ in 0..3 {
        let x = rng.below(12) as i32;
        let y = 1 + rng.below(14) as i32;
        for d in 0..4 {
            sheet.put(cell, x + d, y, 252, 255);
        }
        for d in 1..3 {
            sheet.put(cell, x + d, y + 1, 192, 255);
        }
    }
}

fn floor(sheet: &mut Sheet, cell: usize, seed: u64) {
    let mut rng = rng_for(seed, cell);
    fill_noise(sheet, cell, &mut rng, BASE_LEVEL, 10);
    let (mut x, mut y) = (rng.below(16) as i32, 0);
    for _ in 0..16 {
        sheet.put(cell, x, y, 170, 255);
        x += rng.below(3) as i32 - 1;
        y += 1;
        if rng.below(3) == 0 {
            sheet.put(cell, x + 1, y, 242, 255);
        }
    }
}

fn wall(sheet: &mut Sheet, cell: usize, seed: u64) {
    let mut rng = rng_for(seed, cell);
    // Rock plates in three staggered rows. Each plate has a dark seam on its top and left,
    // a light line under the seam, and a dark bottom line (light from the north west).
    const ROWS_Y: [(i32, i32); 2] = [(0, 8), (8, 16)];
    const SEAMS_X: [[i32; 2]; 2] = [[0, 10], [5, 13]];
    for (r, &(y0, y1)) in ROWS_Y.iter().enumerate() {
        for (k, &x0) in SEAMS_X[r].iter().enumerate() {
            let x1 = if k == 0 {
                SEAMS_X[r][1]
            } else {
                SEAMS_X[r][0] + TILE as i32
            };
            let tone = jitter(&mut rng, 9);
            for y in y0..y1 {
                for x in x0..x1 {
                    let px = x.rem_euclid(TILE as i32);
                    let mut v = BASE_LEVEL + 6 + tone + jitter(&mut rng, 2);
                    if y == y0 || x == x0 {
                        v = BASE_LEVEL - 30;
                    } else if y == y0 + 1 || x == x0 + 1 {
                        v += 8;
                    } else if y == y1 - 1 || x == x1 - 1 {
                        v -= 10;
                    }
                    sheet.put(cell, px, y, v, 255);
                }
            }
        }
    }
    // One short crack and one chip, so the plates do not look printed.
    let (mut x, mut y) = (2 + rng.below(12) as i32, 1 + rng.below(4) as i32);
    for _ in 0..3 {
        sheet.put(cell, x, y, BASE_LEVEL - 70, 255);
        x += rng.below(3) as i32 - 1;
        y += 1;
    }
}

fn ore(sheet: &mut Sheet, cell: usize, seed: u64) {
    let mut rng = rng_for(seed, cell);
    // Two clusters of 4 x 4 pixels with a dark rim, so one ore tile reads at a glance.
    for k in 0..2 {
        let x = 1 + k * 7 + rng.below(3) as i32;
        let y = 1 + (1 - k) * 6 + rng.below(3) as i32;
        for dy in -1..=4 {
            for dx in -1..=4 {
                let inside = (0..4).contains(&dx) && (0..4).contains(&dy);
                if inside {
                    let corner = (dx == 0 || dx == 3) && (dy == 0 || dy == 3);
                    let v = if dx <= 1 && dy <= 1 {
                        255
                    } else if corner {
                        170
                    } else {
                        225
                    };
                    sheet.put(cell, x + dx, y + dy, v, 255);
                } else if dx >= 0 && dy >= 0 {
                    sheet.put(cell, x + dx, y + dy, 30, 255);
                }
            }
        }
    }
}

/// Value of the canopy outline. The game multiplies it with the tree color.
const TREE_OUTLINE: i32 = 88;

fn tree(sheet: &mut Sheet, cell: usize, seed: u64) {
    let mut rng = rng_for(seed, cell);
    // Four round clumps. Centers and radii are in half pixels, so the shape stays lumpy.
    let mut clumps: Vec<(i32, i32, i32)> = vec![
        (16, 17, 10),
        (12, 13, 7),
        (20, 13, 7),
        (13, 20, 7),
        (20, 20, 7),
    ];
    for c in &mut clumps {
        c.0 += jitter(&mut rng, 2);
        c.1 += jitter(&mut rng, 2);
    }
    // Per pixel: the deepest clump decides the shade. 0 means outside.
    let mut depth = [[0i32; TILE]; TILE];
    let mut shade = [[0i32; TILE]; TILE];
    for y in 0..TILE as i32 {
        for x in 0..TILE as i32 {
            let (px, py) = (2 * x + 1, 2 * y + 1);
            let mut best = 0;
            for &(cx, cy, r) in &clumps {
                let (dx, dy) = (px - cx, py - cy);
                let d = ((dx * dx + dy * dy) as f32).sqrt() as i32;
                if r - d > best {
                    best = r - d;
                    // Light from the north west: bright on the upper left of each clump.
                    shade[y as usize][x as usize] = -(dx + dy) * 14 / r;
                }
            }
            depth[y as usize][x as usize] = best;
        }
    }
    let inside = |x: i32, y: i32| {
        (0..TILE as i32).contains(&x)
            && (0..TILE as i32).contains(&y)
            && depth[y as usize][x as usize] > 0
    };
    for y in 0..TILE as i32 {
        for x in 0..TILE as i32 {
            if !inside(x, y) {
                continue;
            }
            let edge =
                !(inside(x - 1, y) && inside(x + 1, y) && inside(x, y - 1) && inside(x, y + 1));
            let (d, sh) = (depth[y as usize][x as usize], shade[y as usize][x as usize]);
            let mut v = BASE_LEVEL + 6 + sh;
            // A dark seam where two clumps meet.
            if d <= 2 {
                v -= 34;
            }
            // Sparse light leaf dots on the lit side.
            if sh > 6 && rng.below(5) == 0 {
                v += 18;
            }
            if edge {
                v = TREE_OUTLINE + if sh > 4 { 14 } else { 0 };
            }
            sheet.put(cell, x, y, v, 255);
        }
    }
}

fn tree_shadow(sheet: &mut Sheet, cell: usize) {
    // An ellipse under the canopy, pushed to the south east (the light comes from the north west).
    for y in 0..TILE as i32 {
        for x in 0..TILE as i32 {
            let (dx, dy) = (2 * x - 19, 2 * y - 21);
            if dx * dx + dy * dy * 5 / 4 <= 11 * 11 {
                sheet.put(cell, x, y, 0, 80);
            }
        }
    }
}

fn edges(sheet: &mut Sheet, first: usize, seed: u64) {
    for dir in 0..4 {
        let cell = first + dir;
        let mut rng = rng_for(seed, cell);
        for y in 0..TILE as i32 {
            for x in 0..TILE as i32 {
                let d = match dir {
                    0 => y,
                    1 => 15 - x,
                    2 => 15 - y,
                    _ => x,
                };
                let a = 210 - d * 56 + jitter(&mut rng, 28);
                if a > 0 {
                    sheet.put(cell, x, y, 255, a.min(255) as u8);
                }
            }
        }
    }
}

fn wall_face(sheet: &mut Sheet, cell: usize, seed: u64) {
    let mut rng = rng_for(seed, cell);
    for y in 9..TILE as i32 {
        for x in 0..TILE as i32 {
            let row = y - 9;
            let mut v = BASE_LEVEL - 70 - row * 6 + jitter(&mut rng, 8);
            // Block seams: a horizontal line and staggered vertical lines.
            if row == 2 {
                v -= 30;
            }
            let seam = if row < 2 { 5 } else { 11 };
            if row != 2 && x == seam {
                v -= 24;
            }
            sheet.put(cell, x, y, v, 255);
        }
    }
    // A bright lip where the top meets the face.
    for x in 0..TILE as i32 {
        sheet.put(cell, x, 9, BASE_LEVEL, 255);
    }
}

/// Build the sheet from a seed.
pub fn make_sheet(seed: u64) -> Sheet {
    let mut sheet = Sheet::new();
    for v in 0..VARIANTS {
        grass(&mut sheet, cell::GRASS + v, seed);
        soil(&mut sheet, cell::SOIL + v, seed);
        sand(&mut sheet, cell::SAND + v, seed);
        clay(&mut sheet, cell::CLAY + v, seed);
        water(&mut sheet, cell::WATER + v, seed);
        floor(&mut sheet, cell::FLOOR + v, seed);
        wall(&mut sheet, cell::WALL + v, seed);
        ore(&mut sheet, cell::ORE + v, seed);
    }
    for v in 0..2 {
        tree(&mut sheet, cell::TREE + v, seed);
        wall_face(&mut sheet, cell::WALL_FACE + v, seed);
    }
    tree_shadow(&mut sheet, cell::TREE_SHADOW);
    edges(&mut sheet, cell::EDGE, seed);
    sheet
}

// ---------------------------------------------------------------- PNG writer

/// CRC32 (IEEE) of a byte slice.
pub fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = !0u32;
    for &b in bytes {
        crc ^= u32::from(b);
        for _ in 0..8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ 0xEDB8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

/// Adler-32 of a byte slice.
pub fn adler32(bytes: &[u8]) -> u32 {
    let (mut a, mut b) = (1u32, 0u32);
    for &x in bytes {
        a = (a + u32::from(x)) % 65_521;
        b = (b + a) % 65_521;
    }
    (b << 16) | a
}

fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    let mut body = kind.to_vec();
    body.extend_from_slice(data);
    out.extend_from_slice(&body);
    out.extend_from_slice(&crc32(&body).to_be_bytes());
}

/// Encode RGBA bytes as a PNG file with stored (uncompressed) deflate blocks.
pub fn encode_png(width: usize, height: usize, rgba: &[u8]) -> Vec<u8> {
    let mut raw = Vec::with_capacity((width * 4 + 1) * height);
    for row in rgba.chunks(width * 4) {
        raw.push(0); // filter: none
        raw.extend_from_slice(row);
    }
    let mut z = vec![0x78, 0x01];
    let mut blocks = raw.chunks(65_535).peekable();
    while let Some(block) = blocks.next() {
        z.push(u8::from(blocks.peek().is_none()));
        z.extend_from_slice(&(block.len() as u16).to_le_bytes());
        z.extend_from_slice(&(!(block.len() as u16)).to_le_bytes());
        z.extend_from_slice(block);
    }
    z.extend_from_slice(&adler32(&raw).to_be_bytes());

    let mut out = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    let mut header = Vec::new();
    header.extend_from_slice(&(width as u32).to_be_bytes());
    header.extend_from_slice(&(height as u32).to_be_bytes());
    header.extend_from_slice(&[8, 6, 0, 0, 0]); // 8 bit, RGBA
    chunk(&mut out, b"IHDR", &header);
    chunk(&mut out, b"IDAT", &z);
    chunk(&mut out, b"IEND", &[]);
    out
}

/// A contact sheet for people: the tile sheet at 4 x, then the colonist sprites at 5 x
/// (shirt tinted red, west flipped) on a green ground. Not used by the game.
pub fn preview(seed: u64) -> (usize, usize, Vec<u8>) {
    let tiles = make_sheet(seed);
    let actor = actors::make_sheet();
    let (tz, az) = (4usize, 5usize);
    let width = tiles.width * tz;
    let height = tiles.height * tz + 16 * az + 8;
    let mut rgba = vec![0u8; width * height * 4];
    for px in rgba.chunks_mut(4) {
        px.copy_from_slice(&[70, 120, 60, 255]);
    }
    for y in 0..tiles.height * tz {
        for x in 0..width {
            let i = ((y / tz) * tiles.width + x / tz) * 4;
            let a = u32::from(tiles.rgba[i + 3]);
            let o = (y * width + x) * 4;
            for c in 0..3 {
                let v = u32::from(tiles.rgba[i + c]);
                rgba[o + c] = ((u32::from(rgba[o + c]) * (255 - a) + v * a) / 255) as u8;
            }
        }
    }
    let top = tiles.height * tz + 8;
    // Nine sprites, then the flipped east sprites (the west facing).
    for n in 0..actors::COUNT + 3 {
        let (cell, flip) = if n < actors::COUNT {
            (actors::SOUTH + n, false)
        } else {
            (actors::EAST + n - actors::COUNT, true)
        };
        for y in 0..TILE * az {
            for x in 0..TILE * az {
                let sx = if flip { TILE - 1 - x / az } else { x / az };
                let i = ((y / az) * actor.width + cell * TILE + sx) * 4;
                let p = &actor.rgba[i..i + 4];
                if p[3] == 0 {
                    continue;
                }
                let gray = p[0] == p[1] && p[1] == p[2] && p[3] == 255;
                let a = u32::from(p[3]);
                let o = ((top + y) * width + n * TILE * az + x) * 4;
                if o + 4 > rgba.len() {
                    continue;
                }
                let tint = [220u32, 70, 60];
                for c in 0..3 {
                    let v = if gray {
                        tint[c] * u32::from(p[c]) / 224
                    } else {
                        u32::from(p[c])
                    };
                    rgba[o + c] =
                        ((u32::from(rgba[o + c]) * (255 - a) + v.min(255) * a) / 255) as u8;
                }
            }
        }
    }
    (width, height, rgba)
}

/// Run `assetgen [--seed <n>] [--out <dir>]`. Returns a message.
pub fn run(args: &[String]) -> Result<String, String> {
    let mut seed = DEFAULT_SEED;
    let mut dir = "assets/sprites".to_owned();
    let mut it = args.iter();
    while let Some(flag) = it.next() {
        let value = it.next().ok_or_else(|| format!("{flag} needs a value"))?;
        match flag.as_str() {
            "--seed" => {
                seed = value
                    .parse()
                    .map_err(|e| format!("bad seed {value:?}: {e}"))?
            }
            "--out" => dir = value.clone(),
            _ => return Err(format!("unknown flag {flag:?}")),
        }
    }
    let sheet = make_sheet(seed);
    let png = encode_png(sheet.width, sheet.height, &sheet.rgba);
    std::fs::create_dir_all(&dir).map_err(|e| format!("cannot create {dir}: {e}"))?;
    let write = |name: &str, bytes: &[u8]| {
        let path = std::path::Path::new(&dir).join(name);
        std::fs::write(&path, bytes).map_err(|e| format!("cannot write {}: {e}", path.display()))
    };
    write("tiles.png", &png)?;
    let actor = actors::make_sheet();
    write(
        "actors.png",
        &encode_png(actor.width, actor.height, &actor.rgba),
    )?;
    let (pw, ph, prgba) = preview(seed);
    write("preview.png", &encode_png(pw, ph, &prgba))?;
    let path = std::path::Path::new(&dir).join("tiles.png");
    Ok(format!("{} ({} bytes)", path.display(), png.len()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crc32_known_answer() {
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
    }

    #[test]
    fn adler32_known_answer() {
        assert_eq!(adler32(b"Wikipedia"), 0x11E6_0398);
    }

    #[test]
    fn png_has_signature_and_ends_with_iend() {
        let png = encode_png(2, 1, &[1, 2, 3, 4, 5, 6, 7, 8]);
        assert_eq!(&png[..8], &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]);
        assert_eq!(&png[png.len() - 8..png.len() - 4], b"IEND");
    }

    #[test]
    fn sheet_is_repeatable_and_seed_dependent() {
        assert_eq!(make_sheet(1).rgba, make_sheet(1).rgba);
        assert_ne!(make_sheet(1).rgba, make_sheet(2).rgba);
    }

    #[test]
    fn shipped_file_matches_generator() {
        let sheet = make_sheet(DEFAULT_SEED);
        let png = encode_png(sheet.width, sheet.height, &sheet.rgba);
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../assets/sprites/tiles.png"
        );
        assert_eq!(std::fs::read(path).ok(), Some(png));
    }

    #[test]
    fn sheet_png_known_answer() {
        let sheet = make_sheet(DEFAULT_SEED);
        let png = encode_png(sheet.width, sheet.height, &sheet.rgba);
        assert_eq!(base::hash::fnv1a(&png), 0xc0e5a88bd8b736f9);
    }

    #[test]
    fn shipped_actor_file_matches_generator() {
        let sheet = actors::make_sheet();
        let png = encode_png(sheet.width, sheet.height, &sheet.rgba);
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../assets/sprites/actors.png"
        );
        assert_eq!(std::fs::read(path).ok(), Some(png));
    }

    #[test]
    fn shipped_preview_matches_generator() {
        let (w, h, rgba) = preview(DEFAULT_SEED);
        let png = encode_png(w, h, &rgba);
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../assets/sprites/preview.png"
        );
        assert_eq!(std::fs::read(path).ok(), Some(png));
    }
}
