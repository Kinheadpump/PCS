//! `assetgen`: makes all sprite sheets with code (D-034).
//!
//! The ground sheet and the item sheet are gray. The game turns a gray level into a color with
//! `palette::ramp` and the material color. Level 224 is the plain material color. The same seed
//! gives the same bytes. The PNG writer is our own: stored deflate, CRC32, Adler-32.

use base::rng::Pcg32;

#[path = "../../game/src/cells.rs"]
pub mod cells;
#[path = "../../game/src/palette.rs"]
pub mod palette;

#[path = "assetgen_actors.rs"]
pub mod actors;
#[path = "assetgen_buildings.rs"]
pub mod buildings;
#[path = "assetgen_items.rs"]
pub mod items;
#[path = "assetgen_objects.rs"]
pub mod objects;
#[path = "assetgen_preview.rs"]
pub mod preview;
#[path = "assetgen_terrain.rs"]
pub mod terrain;

pub use cells::{COLUMNS, TILE};

/// Default seed of the shipped sheets.
pub const DEFAULT_SEED: u64 = 1;

/// An RGBA image.
pub struct Image {
    /// Width in pixels.
    pub width: usize,
    /// Height in pixels.
    pub height: usize,
    /// RGBA bytes, row-major.
    pub rgba: Vec<u8>,
}

impl Image {
    /// A transparent image.
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            rgba: vec![0; width * height * 4],
        }
    }

    /// Set a pixel. Pixels outside the image are ignored.
    pub fn set(&mut self, x: i32, y: i32, px: [u8; 4]) {
        if x < 0 || y < 0 || x as usize >= self.width || y as usize >= self.height {
            return;
        }
        let i = (y as usize * self.width + x as usize) * 4;
        self.rgba[i..i + 4].copy_from_slice(&px);
    }

    /// Read a pixel. Pixels outside the image are transparent.
    pub fn get(&self, x: i32, y: i32) -> [u8; 4] {
        if x < 0 || y < 0 || x as usize >= self.width || y as usize >= self.height {
            return [0; 4];
        }
        let i = (y as usize * self.width + x as usize) * 4;
        [
            self.rgba[i],
            self.rgba[i + 1],
            self.rgba[i + 2],
            self.rgba[i + 3],
        ]
    }

    /// Set a gray pixel.
    pub fn set_gray(&mut self, x: i32, y: i32, level: i32, alpha: u8) {
        let v = level.clamp(0, 255) as u8;
        self.set(x, y, [v, v, v, alpha]);
    }

    /// Set a pixel inside a 16 x 16 cell of a sheet with 16 columns.
    pub fn set_cell(&mut self, cell: usize, x: i32, y: i32, px: [u8; 4]) {
        if !(0..TILE as i32).contains(&x) || !(0..TILE as i32).contains(&y) {
            return;
        }
        let (ox, oy) = ((cell % COLUMNS) * TILE, (cell / COLUMNS) * TILE);
        self.set(ox as i32 + x, oy as i32 + y, px);
    }

    /// Read a pixel inside a 16 x 16 cell.
    pub fn get_cell(&self, cell: usize, x: i32, y: i32) -> [u8; 4] {
        if !(0..TILE as i32).contains(&x) || !(0..TILE as i32).contains(&y) {
            return [0; 4];
        }
        let (ox, oy) = ((cell % COLUMNS) * TILE, (cell / COLUMNS) * TILE);
        self.get(ox as i32 + x, oy as i32 + y)
    }

    /// A sheet of 16 columns with the given number of rows.
    pub fn cell_sheet(rows: usize) -> Self {
        Self::new(COLUMNS * TILE, rows * TILE)
    }

    /// Encode as PNG.
    pub fn png(&self) -> Vec<u8> {
        encode_png(self.width, self.height, &self.rgba)
    }
}

/// A random stream for one named part of a sheet.
pub fn rng_for(seed: u64, salt: u64) -> Pcg32 {
    Pcg32::from_seed(seed ^ (salt + 1).wrapping_mul(0x9E37_79B9_7F4A_7C15))
}

/// A hash of three numbers: stable, for placing details.
pub fn hash3(seed: u64, salt: u64, x: i32, y: i32) -> u32 {
    let v = seed
        ^ salt.wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ (u64::from(x as u32) << 32 | u64::from(y as u32));
    (base::rng::mix(v) >> 16) as u32
}

/// A named sheet file: its name and its image.
pub struct Output {
    /// File name.
    pub name: &'static str,
    /// The image.
    pub image: Image,
}

/// Make every sheet of the game and every review sheet from a seed.
pub fn make_all(seed: u64) -> Vec<Output> {
    let out = |name, image| Output { name, image };
    vec![
        out("terrain.png", terrain::make_terrain(seed)),
        out("tiles.png", terrain::make_tiles(seed)),
        out("objects.png", objects::make(seed)),
        out("items.png", items::make_items(seed)),
        out("buildings.png", buildings::make_buildings()),
        out("icons.png", items::make_icons()),
        out("actors.png", actors::make_sheet()),
        out("palette.png", preview::palette_sheet()),
        out("preview.png", preview::reference_sheet(seed)),
        out("repeat.png", preview::repeat_sheet(seed)),
    ]
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
    std::fs::create_dir_all(&dir).map_err(|e| format!("cannot create {dir}: {e}"))?;
    let mut total = 0;
    let outputs = make_all(seed);
    for o in &outputs {
        let png = o.image.png();
        total += png.len();
        let path = std::path::Path::new(&dir).join(o.name);
        std::fs::write(&path, &png).map_err(|e| format!("cannot write {}: {e}", path.display()))?;
    }
    Ok(format!("{} files in {dir} ({total} bytes)", outputs.len()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shipped(name: &str) -> Option<Vec<u8>> {
        let path = format!("{}/../../assets/sprites/{name}", env!("CARGO_MANIFEST_DIR"));
        std::fs::read(path).ok()
    }

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
    fn sheets_are_repeatable_and_seed_dependent() {
        let a = make_all(1);
        let b = make_all(1);
        let c = make_all(2);
        for ((x, y), z) in a.iter().zip(&b).zip(&c) {
            assert_eq!(x.image.rgba, y.image.rgba, "{}", x.name);
            if x.name == "terrain.png" {
                assert_ne!(x.image.rgba, z.image.rgba);
            }
        }
    }

    #[test]
    fn shipped_files_match_the_generator() {
        for o in make_all(DEFAULT_SEED) {
            assert_eq!(
                shipped(o.name).as_deref(),
                Some(o.image.png().as_slice()),
                "{} differs from the generator: run `cargo run -p tools -- assetgen`",
                o.name
            );
        }
    }

    #[test]
    fn sheets_have_the_sizes_that_the_game_expects() {
        let sizes: Vec<(&str, usize, usize)> = make_all(1)
            .iter()
            .map(|o| (o.name, o.image.width, o.image.height))
            .collect();
        let get = |n: &str| sizes.iter().find(|s| s.0 == n).map(|s| (s.1, s.2));
        let t = cells::terrain::SIZE;
        assert_eq!(
            get("terrain.png"),
            Some((t * cells::terrain::COLS, t * cells::terrain::ROWS))
        );
        assert_eq!(get("tiles.png"), Some((256, 16 * cells::tiles::ROWS)));
        assert_eq!(get("objects.png"), Some((256, 16 * cells::objects::ROWS)));
        assert_eq!(get("items.png"), Some((256, 16 * cells::items::ROWS)));
        assert_eq!(
            get("buildings.png"),
            Some((256, 16 * cells::buildings::ROWS))
        );
        assert_eq!(get("icons.png"), Some((256, 16 * cells::icons::ROWS)));
        assert_eq!(get("actors.png"), Some((256, 16 * cells::actors::ROWS)));
    }

    #[test]
    fn terrain_textures_are_opaque_and_gray() {
        let img = terrain::make_terrain(DEFAULT_SEED);
        for p in img.rgba.chunks(4) {
            assert_eq!(p[3], 255);
            assert!(p[0] == p[1] && p[1] == p[2]);
        }
    }

    #[test]
    fn terrain_textures_have_no_seam_when_repeated() {
        // The step between the last and the first column must be like the largest step between two
        // columns inside the texture.
        let img = terrain::make_terrain(DEFAULT_SEED);
        let s = cells::terrain::SIZE as i32;
        for index in 0..cells::terrain::COUNT - 1 {
            let (ox, oy) = cells::terrain::origin(index);
            let (ox, oy) = (ox as i32, oy as i32);
            let step = |x0: i32, y0: i32, x1: i32, y1: i32| {
                (i32::from(img.get(ox + x0, oy + y0)[0]) - i32::from(img.get(ox + x1, oy + y1)[0]))
                    .abs()
            };
            let columns = [
                s / 8,
                s / 4,
                3 * s / 8,
                s / 2,
                5 * s / 8,
                3 * s / 4,
                7 * s / 8,
            ];
            let inside: i32 = columns
                .iter()
                .map(|&c| (0..s).map(|y| step(c, y, c + 1, y)).sum::<i32>())
                .max()
                .unwrap_or(0);
            let seam: i32 = (0..s).map(|y| step(s - 1, y, 0, y)).sum();
            assert!(
                seam <= inside + inside / 2 + 64,
                "texture {index}: seam {seam} inside {inside}"
            );
        }
    }

    #[test]
    fn world_sprites_use_only_known_gray_levels_and_alpha() {
        for img in [
            items::make_items(1),
            items::make_icons(),
            buildings::make_buildings(),
            terrain::make_tiles(1),
        ] {
            for p in img.rgba.chunks(4) {
                if p[3] != 0 {
                    assert!(p[0] == p[1] && p[1] == p[2], "a gray sprite has a color");
                }
            }
        }
    }

    #[test]
    fn objects_have_a_clear_outline_and_transparent_corners() {
        let img = objects::make(1);
        for tree in 0..cells::objects::TREE_COUNT {
            assert_eq!(img.get_cell(tree, 0, 0)[3], 0, "tree {tree}");
            let solid = (0..16)
                .flat_map(|y| (0..16).map(move |x| (x, y)))
                .filter(|&(x, y)| img.get_cell(tree, x, y)[3] == 255)
                .count();
            assert!(
                (60..=230).contains(&solid),
                "tree {tree} has {solid} pixels"
            );
        }
    }
}
