//! The tile sprite sheet made by `tools assetgen` (16 x 16 pixels per sprite).
//! Decoding is pure: it needs no window.

use macroquad::prelude::Image;

/// Sprite side in pixels.
pub const TILE: usize = 16;
/// Sprites per sheet row.
pub const COLUMNS: usize = 16;
/// Value that means "the plain material color" in the grayscale sprites.
pub const BASE_LEVEL: u32 = 224;

/// Sheet cell numbers. They match `tools/src/assetgen.rs`.
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
    /// First ore speckle variant.
    pub const ORE: usize = 28;
    /// First tree canopy variant (2 variants).
    pub const TREE: usize = 32;
    /// Tree shadow.
    pub const TREE_SHADOW: usize = 34;
    /// Edge masks: north, east, south, west.
    pub const EDGE: usize = 36;
    /// First wall face variant (2 variants).
    pub const WALL_FACE: usize = 40;
}

/// The decoded sheet.
pub struct Sheet {
    width: usize,
    rgba: Vec<u8>,
}

impl Sheet {
    /// Decode a PNG file. Returns a message if the data is bad.
    pub fn from_png(bytes: &[u8]) -> Result<Self, String> {
        let image =
            Image::from_file_with_format(bytes, None).map_err(|e| format!("tiles.png: {e:?}"))?;
        let width = usize::from(image.width);
        if width != COLUMNS * TILE || usize::from(image.height) % TILE != 0 {
            return Err(format!(
                "tiles.png has size {}x{}",
                image.width, image.height
            ));
        }
        Ok(Self {
            width,
            rgba: image.bytes,
        })
    }

    /// The shipped sheet.
    pub fn shipped() -> Result<Self, String> {
        Self::from_png(include_bytes!("../../../assets/sprites/tiles.png"))
    }

    /// One pixel of a sprite as `[value, alpha]`. Outside the sheet: transparent.
    pub fn pixel(&self, cell: usize, x: usize, y: usize) -> (u32, u32) {
        let px = (cell % COLUMNS) * TILE + x;
        let py = (cell / COLUMNS) * TILE + y;
        let i = (py * self.width + px) * 4;
        match self.rgba.get(i..i + 4) {
            Some(p) => (u32::from(p[0]), u32::from(p[3])),
            None => (0, 0),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shipped_sheet_decodes_with_expected_size() {
        let sheet = Sheet::shipped();
        assert!(sheet.is_ok());
        if let Ok(sheet) = sheet {
            assert_eq!(sheet.width, 256);
            assert_eq!(sheet.rgba.len(), 256 * 48 * 4);
        }
    }

    #[test]
    fn ground_sprites_are_opaque_and_tree_has_transparent_corners() {
        let Ok(sheet) = Sheet::shipped() else { return };
        for c in [cell::GRASS, cell::SOIL, cell::SAND, cell::CLAY, cell::WATER] {
            assert!((0..TILE).all(|y| (0..TILE).all(|x| sheet.pixel(c, x, y).1 == 255)));
        }
        assert_eq!(sheet.pixel(cell::TREE, 0, 0).1, 0);
        assert_eq!(sheet.pixel(cell::TREE, 8, 8).1, 255);
    }

    #[test]
    fn bad_png_is_an_error() {
        assert!(Sheet::from_png(b"nope").is_err());
    }
}
