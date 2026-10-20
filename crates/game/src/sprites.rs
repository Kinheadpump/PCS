//! The sprite sheets made by `tools assetgen`. Decoding is pure: it needs no window.
//! Cell numbers live in `cells.rs`, which `tools assetgen` includes too.

use crate::cells::{COLUMNS, TILE, terrain};
use macroquad::prelude::Image;

/// A decoded PNG sheet.
pub struct Png {
    width: usize,
    height: usize,
    rgba: Vec<u8>,
}

impl Png {
    /// Decode a PNG file. `width` is the width that the sheet must have.
    pub fn from_png(name: &str, bytes: &[u8], width: usize) -> Result<Self, String> {
        let image =
            Image::from_file_with_format(bytes, None).map_err(|e| format!("{name}: {e:?}"))?;
        let (w, h) = (usize::from(image.width), usize::from(image.height));
        if w != width || h == 0 {
            return Err(format!(
                "{name} has size {w}x{h}, the width must be {width}"
            ));
        }
        Ok(Self {
            width: w,
            height: h,
            rgba: image.bytes,
        })
    }

    /// Size in pixels.
    pub fn size(&self) -> (usize, usize) {
        (self.width, self.height)
    }

    /// All RGBA bytes.
    pub fn bytes(&self) -> &[u8] {
        &self.rgba
    }

    /// One pixel. Outside the sheet: transparent.
    pub fn px(&self, x: usize, y: usize) -> [u8; 4] {
        if x >= self.width || y >= self.height {
            return [0; 4];
        }
        let i = (y * self.width + x) * 4;
        [
            self.rgba[i],
            self.rgba[i + 1],
            self.rgba[i + 2],
            self.rgba[i + 3],
        ]
    }

    /// One pixel of a 16 x 16 cell.
    pub fn cell_px(&self, cell: usize, x: usize, y: usize) -> [u8; 4] {
        self.px((cell % COLUMNS) * TILE + x, (cell / COLUMNS) * TILE + y)
    }

    /// The RGBA bytes of one 16 x 16 cell.
    pub fn cell_bytes(&self, cell: usize) -> Vec<u8> {
        let mut out = Vec::with_capacity(TILE * TILE * 4);
        for y in 0..TILE {
            for x in 0..TILE {
                out.extend_from_slice(&self.cell_px(cell, x, y));
            }
        }
        out
    }
}

/// All sheets of the game.
pub struct Sprites {
    /// Gray ground textures (`terrain.png`).
    pub terrain: Png,
    /// Gray ore overlays (`tiles.png`).
    pub tiles: Png,
    /// Full color trees and decoration (`objects.png`).
    pub objects: Png,
    /// Gray item piles (`items.png`).
    pub items: Png,
    /// Gray icons (`icons.png`).
    pub icons: Png,
    /// Full color colonists (`actors.png`).
    pub actors: Png,
    /// Gray buildings with an alpha code for the color class (`buildings.png`).
    pub buildings: Png,
}

impl Sprites {
    /// The shipped sheets. Fails with a message if a file is bad.
    pub fn shipped() -> Result<Self, String> {
        let sheet = |name: &str, bytes: &[u8], width: usize| Png::from_png(name, bytes, width);
        let wide = COLUMNS * TILE;
        Ok(Self {
            terrain: sheet(
                "terrain.png",
                include_bytes!("../../../assets/sprites/terrain.png"),
                terrain::SIZE * terrain::COLS,
            )?,
            tiles: sheet(
                "tiles.png",
                include_bytes!("../../../assets/sprites/tiles.png"),
                wide,
            )?,
            objects: sheet(
                "objects.png",
                include_bytes!("../../../assets/sprites/objects.png"),
                wide,
            )?,
            items: sheet(
                "items.png",
                include_bytes!("../../../assets/sprites/items.png"),
                wide,
            )?,
            icons: sheet(
                "icons.png",
                include_bytes!("../../../assets/sprites/icons.png"),
                wide,
            )?,
            actors: sheet(
                "actors.png",
                include_bytes!("../../../assets/sprites/actors.png"),
                wide,
            )?,
            buildings: sheet(
                "buildings.png",
                include_bytes!("../../../assets/sprites/buildings.png"),
                wide,
            )?,
        })
    }

    /// Gray level of a ground texture at a world pixel. The texture repeats.
    pub fn ground_level(&self, texture: usize, wx: i32, wy: i32) -> u8 {
        let s = terrain::SIZE as i32;
        let (ox, oy) = terrain::origin(texture);
        self.terrain.px(
            ox + wx.rem_euclid(s) as usize,
            oy + wy.rem_euclid(s) as usize,
        )[0]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cells::{actors, buildings, icons, items, objects, tiles};

    #[test]
    fn shipped_sheets_decode_with_the_sizes_in_cells_rs() {
        let Ok(s) = Sprites::shipped() else {
            panic!("the shipped sheets must decode");
        };
        let t = terrain::SIZE;
        assert_eq!(s.terrain.size(), (t * terrain::COLS, t * terrain::ROWS));
        assert_eq!(s.tiles.size().1, TILE * tiles::ROWS);
        assert_eq!(s.objects.size().1, TILE * objects::ROWS);
        assert_eq!(s.items.size().1, TILE * items::ROWS);
        assert_eq!(s.icons.size().1, TILE * icons::ROWS);
        assert_eq!(s.actors.size().1, TILE * actors::ROWS);
        assert_eq!(s.buildings.size().1, TILE * buildings::ROWS);
    }

    #[test]
    fn ground_textures_are_opaque_and_trees_have_transparent_corners() {
        let Ok(s) = Sprites::shipped() else { return };
        for index in 0..terrain::COUNT {
            let (ox, oy) = terrain::origin(index);
            assert_eq!(s.terrain.px(ox + 5, oy + 5)[3], 255);
        }
        assert_eq!(s.objects.cell_px(objects::TREE, 0, 0)[3], 0);
        assert_eq!(s.objects.cell_px(objects::TREE, 8, 8)[3], 255);
    }

    #[test]
    fn a_bad_png_is_an_error() {
        assert!(Png::from_png("x.png", b"nope", 256).is_err());
    }

    #[test]
    fn ground_level_repeats_every_128_pixels() {
        let Ok(s) = Sprites::shipped() else { return };
        assert_eq!(s.ground_level(0, 3, 4), s.ground_level(0, 3 + 128, 4 - 256));
    }
}
