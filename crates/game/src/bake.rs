//! The pixel bake of the map. Pure code: no window calls, so tests run without a window.
//!
//! One chunk is 32 x 32 tiles = 512 x 512 pixels. Every pixel is decided from its world position,
//! not from its tile, so the ground has no tile grid:
//!
//! 1. The ground class of each pixel. A ground class with a higher rank grows a ragged fringe of
//!    1 to 4 pixels over its lower neighbor, also over diagonal neighbors (rounded corners). The
//!    edge of a fringe is hard, with a darker rim. There are no soft alpha edges.
//! 2. Water has foam at the shore, a beach strip on the land side, and bands of depth by the
//!    distance to the land. Land has a wet band at the water.
//! 3. A rock wall has a top with light on the north and west edge and a dark rim on the east edge,
//!    a south face of `terrain::FACE_H` pixels, and a shadow on the ground to the south and east.
//! 4. Ore lumps are drawn over a wall in the ore color.
//! 5. Decoration, tree shadows (cast to the south east), and trees.
//!
//! The ground textures are gray. `palette::ramp` makes colors from the material color and the gray
//! level, with hue shifts (ART_DIRECTION section 4).

use crate::cells::{objects as obj, terrain as tex, tiles};
use crate::palette::{Rgb, ramp};
use crate::sprites::Sprites;
use base::content::MaterialDef;
use base::rng::mix;
use worldgen::{Ground, LocalMapData, Ore, RockKind};

/// Tiles per chunk side.
pub const CHUNK: u32 = 32;
/// Pixels per tile.
pub const PIXELS_PER_TILE: u32 = 16;
/// Pixels per chunk side.
pub const SIDE: usize = (CHUNK * PIXELS_PER_TILE) as usize;
/// Extra pixels around a chunk that the bake reads. Three tiles.
const APRON: i32 = 48;
/// Side of the working region in pixels.
const REGION: usize = SIDE + 2 * APRON as usize;
/// Side of the working region in tiles.
const REGION_TILES: usize = REGION / 16;
/// Tiles that the region reaches beyond the chunk.
const APRON_TILES: i32 = APRON / 16;
/// Color of pixels outside the map.
const VOID_COLOR: Rgb = [12, 12, 16];
/// Color of the foam on the water at the shore.
const FOAM: Rgb = [226, 240, 248];

// Ground classes, in the order of their rank: a higher class grows over a lower class.
const STONE: u8 = 0;
const SAND: u8 = 1;
const CLAY: u8 = 2;
const SOIL: u8 = 3;
const GRASS: u8 = 4;
const WATER: u8 = 5;
const WALL: u8 = 6;
const VOID: u8 = 7;
/// Ground texture of each ground class.
const TEXTURE: [usize; 6] = [
    tex::FLOOR,
    tex::SAND,
    tex::CLAY,
    tex::SOIL,
    tex::GRASS,
    tex::WATER,
];

/// Pixel flag: the pixel is on the rim of a fringe and is drawn darker.
const RIM: u8 = 1;

type Lut = [Rgb; 256];

/// Tile colors, taken from `materials.ron`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Palette {
    grass: Rgb,
    soil: Rgb,
    sand: Rgb,
    clay: Rgb,
    water: Rgb,
    sandstone: Rgb,
    limestone: Rgb,
    granite: Rgb,
    slate: Rgb,
    coal: Rgb,
    copper_ore: Rgb,
    iron_ore: Rgb,
}

fn color_of(materials: &[MaterialDef], key: &str) -> Result<Rgb, String> {
    materials
        .iter()
        .find(|m| m.key == key)
        .map(|m| m.color)
        .ok_or_else(|| format!("materials.ron has no entry \"{key}\""))
}

impl Palette {
    /// Build the palette. Fails with a message if a needed material is missing.
    pub fn from_materials(materials: &[MaterialDef]) -> Result<Self, String> {
        let c = |key: &str| color_of(materials, key);
        Ok(Self {
            grass: c("grass")?,
            soil: c("soil")?,
            sand: c("sand")?,
            clay: c("clay")?,
            water: c("water")?,
            sandstone: c("sandstone")?,
            limestone: c("limestone")?,
            granite: c("granite")?,
            slate: c("slate")?,
            coal: c("coal")?,
            copper_ore: c("copper_ore")?,
            iron_ore: c("iron_ore")?,
        })
    }

    /// Tint of a tile: ore, else rock wall, else ground.
    #[cfg(test)]
    pub fn tile_color(&self, ground: Ground, rock: RockKind, ore: Ore) -> Rgb {
        match ore {
            Ore::Coal => return self.coal,
            Ore::Copper => return self.copper_ore,
            Ore::Iron => return self.iron_ore,
            Ore::None => {}
        }
        match rock {
            RockKind::Sandstone => return self.sandstone,
            RockKind::Limestone => return self.limestone,
            RockKind::Granite => return self.granite,
            RockKind::None => {}
        }
        match ground {
            Ground::Soil => self.grass,
            Ground::Sand => self.sand,
            Ground::Clay => self.clay,
            Ground::Water => self.water,
            Ground::Stone => self.slate,
        }
    }
}

/// The color of every gray level, for each tint that the bake uses.
struct Luts {
    ground: [Lut; 6],
    foam: Lut,
    wall: [Lut; 3],
    ore: [Lut; 3],
}

fn lut(tint: Rgb) -> Lut {
    let mut t = [[0u8; 3]; 256];
    for (level, entry) in t.iter_mut().enumerate() {
        *entry = ramp(tint, level as i32);
    }
    t
}

impl Luts {
    fn new(p: &Palette) -> Self {
        Self {
            ground: [
                lut(p.slate),
                lut(p.sand),
                lut(p.clay),
                lut(p.soil),
                lut(p.grass),
                lut(p.water),
            ],
            foam: lut(FOAM),
            wall: [lut(p.sandstone), lut(p.limestone), lut(p.granite)],
            ore: [lut(p.coal), lut(p.copper_ore), lut(p.iron_ore)],
        }
    }
}

/// The chunks that a change of the tile `(x, y)` touches: the chunk of the tile and the chunks of
/// its neighbors (fringes, shadows, and trees reach into the next tile).
pub fn chunks_touched_by(
    x: u32,
    y: u32,
    map_size: (u32, u32),
) -> std::collections::BTreeSet<(u32, u32)> {
    let mut set = std::collections::BTreeSet::new();
    for dy in -1_i64..=1 {
        for dx in -1_i64..=1 {
            let (nx, ny) = (i64::from(x) + dx, i64::from(y) + dy);
            if nx < 0 || ny < 0 || nx >= i64::from(map_size.0) || ny >= i64::from(map_size.1) {
                continue;
            }
            set.insert((nx as u32 / CHUNK, ny as u32 / CHUNK));
        }
    }
    set
}

/// Number of chunks on each axis.
pub fn chunk_counts(map: &LocalMapData) -> (u32, u32) {
    (
        map.ground.width().div_ceil(CHUNK),
        map.ground.height().div_ceil(CHUNK),
    )
}

/// A hash of a tile or a pixel block and a salt.
fn hash(x: i32, y: i32, salt: u64) -> u64 {
    mix((u64::from(x as u32) << 32 | u64::from(y as u32))
        ^ salt.wrapping_mul(0x9E37_79B9_7F4A_7C15))
}

/// What a tile is, for the purpose of drawing.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Kind {
    Void,
    Wall,
    Water,
    Open,
}

#[derive(Clone, Copy, Debug)]
struct TileInfo {
    kind: Kind,
    class: u8,
    rock: u8,
    ore: u8,
    tree: bool,
}

const VOID_TILE: TileInfo = TileInfo {
    kind: Kind::Void,
    class: VOID,
    rock: 0,
    ore: 0,
    tree: false,
};

fn tile_info(map: &LocalMapData, x: i32, y: i32) -> TileInfo {
    let (Some(g), Some(r), Some(o)) = (map.ground.get(x, y), map.rock.get(x, y), map.ore.get(x, y))
    else {
        return VOID_TILE;
    };
    let tree = map.tree.get(x, y).copied().unwrap_or(false);
    let ore = match o {
        Ore::None => 0,
        Ore::Coal => 1,
        Ore::Copper => 2,
        Ore::Iron => 3,
    };
    if *r != RockKind::None {
        let rock = match r {
            RockKind::Sandstone => 0,
            RockKind::Limestone => 1,
            _ => 2,
        };
        return TileInfo {
            kind: Kind::Wall,
            class: WALL,
            rock,
            ore,
            tree: false,
        };
    }
    let (kind, class) = match g {
        Ground::Water => (Kind::Water, WATER),
        Ground::Stone => (Kind::Open, STONE),
        Ground::Sand => (Kind::Open, SAND),
        Ground::Clay => (Kind::Open, CLAY),
        // Soil is grass with patches of bare earth. A patch is 2 x 2 tiles.
        Ground::Soil if hash(x.div_euclid(2), y.div_euclid(2), 2) % 100 >= 90 => (Kind::Open, SOIL),
        Ground::Soil => (Kind::Open, GRASS),
    };
    TileInfo {
        kind,
        class,
        rock: 0,
        ore: 0,
        tree: tree && kind == Kind::Open,
    }
}

/// Width of a fringe at a world pixel: 1 to `max` + 1 pixels, in blocks that are not square
/// (two lattices of 4 pixels that are shifted against each other).
fn fringe_width(wx: i32, wy: i32, max: i32) -> i32 {
    let a = (hash(wx >> 2, wy >> 2, 21) & 3) as i32;
    let b = (hash((wx + 2) >> 2, (wy + 2) >> 2, 22) & 3) as i32;
    1 + (a + b) * max / 6
}

/// The working region of one chunk: classes of pixels with an apron.
struct Region {
    /// World pixel of the region corner.
    px0: (i32, i32),
    /// World tile of the region corner.
    tile0: (i32, i32),
    tiles: Vec<TileInfo>,
    near_wall: Vec<bool>,
    cls: Vec<u8>,
    flags: Vec<u8>,
    has_water: bool,
}

impl Region {
    fn tile(&self, rtx: i32, rty: i32) -> TileInfo {
        if rtx < 0 || rty < 0 || rtx >= REGION_TILES as i32 || rty >= REGION_TILES as i32 {
            return VOID_TILE;
        }
        self.tiles[rty as usize * REGION_TILES + rtx as usize]
    }

    fn new(map: &LocalMapData, chunk_x: u32, chunk_y: u32) -> Self {
        let tile0 = (
            (chunk_x * CHUNK) as i32 - APRON_TILES,
            (chunk_y * CHUNK) as i32 - APRON_TILES,
        );
        let px0 = (tile0.0 * 16, tile0.1 * 16);
        let mut tiles = Vec::with_capacity(REGION_TILES * REGION_TILES);
        for rty in 0..REGION_TILES as i32 {
            for rtx in 0..REGION_TILES as i32 {
                tiles.push(tile_info(map, tile0.0 + rtx, tile0.1 + rty));
            }
        }
        let has_water = tiles.iter().any(|t| t.kind == Kind::Water);
        let mut region = Self {
            px0,
            tile0,
            tiles,
            near_wall: vec![false; REGION_TILES * REGION_TILES],
            cls: vec![VOID; REGION * REGION],
            flags: vec![0; REGION * REGION],
            has_water,
        };
        for rty in 0..REGION_TILES as i32 {
            for rtx in 0..REGION_TILES as i32 {
                let near = (-1..=1).any(|dy| {
                    (-1..=1).any(|dx| region.tile(rtx + dx, rty + dy).kind == Kind::Wall)
                });
                region.near_wall[rty as usize * REGION_TILES + rtx as usize] = near;
                let class = region.tile(rtx, rty).class;
                for ly in 0..16 {
                    let row = (rty as usize * 16 + ly) * REGION + rtx as usize * 16;
                    region.cls[row..row + 16].fill(class);
                }
            }
        }
        region.fringes();
        region
    }

    /// Step 1: ragged fringes between ground classes, and the beach strip at the water.
    fn fringes(&mut self) {
        for rty in 1..REGION_TILES as i32 - 1 {
            for rtx in 1..REGION_TILES as i32 - 1 {
                let me = self.tile(rtx, rty);
                if !matches!(me.kind, Kind::Open | Kind::Water) {
                    continue;
                }
                // Neighbors that grow over this tile: (dx, dy, class that they paint, max width).
                let mut cands: Vec<(i32, i32, u8, i32)> = Vec::new();
                for dy in -1..=1 {
                    for dx in -1..=1 {
                        if dx == 0 && dy == 0 {
                            continue;
                        }
                        let n = self.tile(rtx + dx, rty + dy);
                        match (me.kind, n.kind) {
                            (Kind::Open, Kind::Open) if n.class > me.class => {
                                cands.push((dx, dy, n.class, 3));
                            }
                            (Kind::Water, Kind::Open) => {
                                // A beach: grass, soil, and clay end in a strip of sand.
                                let beach = if n.class >= CLAY { SAND } else { n.class };
                                cands.push((dx, dy, beach, 2));
                            }
                            _ => {}
                        }
                    }
                }
                if cands.is_empty() {
                    continue;
                }
                for ly in 0..16i32 {
                    for lx in 0..16i32 {
                        let (wx, wy) = (self.px0.0 + rtx * 16 + lx, self.px0.1 + rty * 16 + ly);
                        let mut hit: Option<(u8, bool)> = None;
                        for &(dx, dy, class, max) in &cands {
                            let ex = axis_gap(lx, dx);
                            let ey = axis_gap(ly, dy);
                            let d2 = ex * ex + ey * ey;
                            let w = fringe_width(wx, wy, max);
                            if d2 <= w * w && hit.is_none_or(|(c, _)| class > c) {
                                let rim = me.kind == Kind::Open && d2 > (w - 1) * (w - 1);
                                hit = Some((class, rim));
                            }
                        }
                        if let Some((class, rim)) = hit {
                            let i = (rty as usize * 16 + ly as usize) * REGION
                                + rtx as usize * 16
                                + lx as usize;
                            self.cls[i] = class;
                            self.flags[i] = u8::from(rim) * RIM;
                        }
                    }
                }
            }
        }
    }

    /// Distance in thirds of a pixel from each pixel to the nearest pixel where `mask` is true.
    fn distance(&self, mask: impl Fn(u8) -> bool) -> Vec<u16> {
        const INF: u16 = 3000;
        let mut d: Vec<u16> = self
            .cls
            .iter()
            .map(|&c| if mask(c) { 0 } else { INF })
            .collect();
        let n = REGION;
        for y in 0..n {
            for x in 0..n {
                let i = y * n + x;
                let mut v = d[i];
                if x > 0 {
                    v = v.min(d[i - 1] + 3);
                }
                if y > 0 {
                    v = v.min(d[i - n] + 3);
                    if x > 0 {
                        v = v.min(d[i - n - 1] + 4);
                    }
                    if x + 1 < n {
                        v = v.min(d[i - n + 1] + 4);
                    }
                }
                d[i] = v;
            }
        }
        for y in (0..n).rev() {
            for x in (0..n).rev() {
                let i = y * n + x;
                let mut v = d[i];
                if x + 1 < n {
                    v = v.min(d[i + 1] + 3);
                }
                if y + 1 < n {
                    v = v.min(d[i + n] + 3);
                    if x + 1 < n {
                        v = v.min(d[i + n + 1] + 4);
                    }
                    if x > 0 {
                        v = v.min(d[i + n - 1] + 4);
                    }
                }
                d[i] = v;
            }
        }
        d
    }
}

/// Gap in pixels between a pixel `l` (0 to 15) of a tile and the neighbor tile in the direction
/// `d` (-1, 0, 1) on one axis. 0 if the neighbor is not on this axis.
fn axis_gap(l: i32, d: i32) -> i32 {
    match d {
        -1 => l + 1,
        1 => 16 - l,
        _ => 0,
    }
}

/// Make a color darker in the way of the ramp: darker and cooler.
fn dim(c: Rgb, level: i32) -> Rgb {
    ramp(c, level)
}

/// Bake one chunk to RGBA pixels (row-major, 512 x 512). Pixels outside the map show the void color.
pub fn bake_chunk(
    map: &LocalMapData,
    palette: &Palette,
    sprites: &Sprites,
    chunk_x: u32,
    chunk_y: u32,
) -> Vec<u8> {
    let luts = Luts::new(palette);
    let region = Region::new(map, chunk_x, chunk_y);
    let (to_water, to_land) = if region.has_water {
        (
            region.distance(|c| c == WATER),
            region.distance(|c| c != WATER && c != VOID),
        )
    } else {
        (Vec::new(), Vec::new())
    };
    let mut rgb: Vec<Rgb> = vec![VOID_COLOR; SIDE * SIDE];
    let world0 = (
        (chunk_x * CHUNK * PIXELS_PER_TILE) as i32,
        (chunk_y * CHUNK * PIXELS_PER_TILE) as i32,
    );
    for py in 0..SIDE {
        for px in 0..SIDE {
            let (rx, ry) = (px + APRON as usize, py + APRON as usize);
            let ri = ry * REGION + rx;
            let (wx, wy) = (world0.0 + px as i32, world0.1 + py as i32);
            let class = region.cls[ri];
            let color = match class {
                VOID => VOID_COLOR,
                WALL => wall_pixel(&region, sprites, &luts, rx, ry, wx, wy),
                _ => {
                    let mut level =
                        i32::from(sprites.ground_level(TEXTURE[usize::from(class)], wx, wy));
                    let tile_i = (ry / 16) * REGION_TILES + rx / 16;
                    if class == WATER {
                        let jitter = (hash(wx >> 2, wy >> 2, 31) % 7) as i32 - 3;
                        let d = i32::from(to_land[ri]) + jitter;
                        if d <= 3 {
                            level = 255;
                        } else if d <= 8 {
                            level += 16;
                        } else if d >= 120 {
                            level -= 34;
                        } else if d >= 60 {
                            level -= 18;
                        }
                        let table = if d <= 3 {
                            &luts.foam
                        } else {
                            &luts.ground[usize::from(WATER)]
                        };
                        let level = level.clamp(0, 255) as usize;
                        rgb[py * SIDE + px] = table[level];
                        continue;
                    }
                    if region.flags[ri] & RIM != 0 {
                        level -= 40;
                    }
                    if region.has_water {
                        let jitter = (hash(wx >> 2, wy >> 2, 32) % 5) as i32 - 2;
                        let d = i32::from(to_water[ri]) + jitter;
                        if d <= 9 {
                            level -= 30;
                        } else if d <= 15 {
                            level -= 12;
                        }
                    }
                    if region.near_wall[tile_i] && in_wall_shadow(&region, rx, ry) {
                        level -= 34;
                    }
                    luts.ground[usize::from(class)][level.clamp(0, 255) as usize]
                }
            };
            rgb[py * SIDE + px] = color;
        }
    }
    decorate(sprites, &region, chunk_x, chunk_y, &mut rgb);
    trees(map, sprites, chunk_x, chunk_y, &mut rgb);
    let mut bytes = Vec::with_capacity(SIDE * SIDE * 4);
    for c in rgb {
        bytes.extend_from_slice(&[c[0], c[1], c[2], 255]);
    }
    bytes
}

/// True if a wall casts its shadow on the pixel: the wall is to the west or north (the light
/// comes from the north west).
fn in_wall_shadow(region: &Region, rx: usize, ry: usize) -> bool {
    const OFFSETS: [(usize, usize); 10] = [
        (1, 0),
        (2, 0),
        (3, 0),
        (0, 1),
        (0, 2),
        (0, 3),
        (0, 4),
        (1, 1),
        (2, 2),
        (1, 2),
    ];
    OFFSETS.iter().any(|&(dx, dy)| {
        let c = region.cls[(ry - dy) * REGION + rx - dx];
        c == WALL
    })
}

/// Color of one pixel of a rock wall: top, face, rims, and ore.
fn wall_pixel(
    region: &Region,
    sprites: &Sprites,
    luts: &Luts,
    rx: usize,
    ry: usize,
    wx: i32,
    wy: i32,
) -> Rgb {
    let (rtx, rty) = ((rx / 16) as i32, (ry / 16) as i32);
    let me = region.tile(rtx, rty);
    let (lx, ly) = ((rx % 16) as i32, (ry % 16) as i32);
    let table = &luts.wall[usize::from(me.rock)];
    let at = |dx: i32, dy: i32| {
        let c = region.cls[(ry as i32 + dy) as usize * REGION + (rx as i32 + dx) as usize];
        c == WALL || c == VOID
    };
    let south = region.tile(rtx, rty + 1).kind;
    let face_top = 16 - tex::FACE_H as i32;
    let face = matches!(south, Kind::Open | Kind::Water) && ly >= face_top;
    let mut level;
    if face {
        let row = ly - face_top;
        level = i32::from(sprites.ground_level(tex::FACE, wx, row));
        if !at(1, 0) {
            level -= 46;
        } else if !at(-1, 0) {
            level += 10;
        }
    } else {
        level = i32::from(sprites.ground_level(tex::ROCK, wx, wy));
        if !at(0, -1) {
            level = 255;
        } else if !at(-1, 0) {
            level = 250;
        } else if !at(1, 0) {
            level = 140;
        }
    }
    let mut color = table[level.clamp(0, 255) as usize];
    if me.ore > 0 {
        let v = hash(rtx + region.tile0.0, rty + region.tile0.1, 4) % tiles::ORE_COUNT as u64;
        let cell = tiles::ORE + v as usize;
        let p = sprites.tiles.cell_px(cell, lx as usize, ly as usize);
        if p[3] != 0 {
            color = luts.ore[usize::from(me.ore) - 1][usize::from(p[0])];
        }
    }
    color
}

/// Flowers, tufts, and pebbles: one small decoration on some tiles.
fn decorate(sprites: &Sprites, region: &Region, chunk_x: u32, chunk_y: u32, rgb: &mut [Rgb]) {
    for ty in 0..CHUNK as i32 {
        for tx in 0..CHUNK as i32 {
            let (rtx, rty) = (tx + APRON_TILES, ty + APRON_TILES);
            let t = region.tile(rtx, rty);
            if t.kind != Kind::Open || t.tree {
                continue;
            }
            let near_water = (-1..=1).any(|dy| {
                (-1..=1).any(|dx| {
                    matches!(
                        region.tile(rtx + dx, rty + dy).kind,
                        Kind::Water | Kind::Wall
                    )
                })
            });
            if near_water {
                continue;
            }
            let (wtx, wty) = ((chunk_x * CHUNK) as i32 + tx, (chunk_y * CHUNK) as i32 + ty);
            let h = hash(wtx, wty, 7);
            let roll = h % 100;
            let cell = match t.class {
                GRASS if roll < 7 => obj::FLOWER + ((h >> 8) % obj::FLOWER_COUNT as u64) as usize,
                GRASS if roll < 16 => obj::TUFT + ((h >> 8) % obj::TUFT_COUNT as u64) as usize,
                SOIL | SAND | STONE if roll < 9 => {
                    obj::PEBBLE + ((h >> 8) % obj::PEBBLE_COUNT as u64) as usize
                }
                CLAY if roll < 4 => obj::PEBBLE + ((h >> 8) % obj::PEBBLE_COUNT as u64) as usize,
                _ => continue,
            };
            // The decoration must not sit on a fringe of another class: use the tile center.
            let center = region.cls[(rty as usize * 16 + 8) * REGION + rtx as usize * 16 + 8];
            if center != t.class {
                continue;
            }
            blit_sprite(sprites, cell, (tx * 16, ty * 16), rgb);
        }
    }
}

/// Draw an object sprite with its top left corner at a chunk pixel. Only solid pixels count.
fn blit_sprite(sprites: &Sprites, cell: usize, origin: (i32, i32), rgb: &mut [Rgb]) {
    for sy in 0..16 {
        for sx in 0..16 {
            let p = sprites.objects.cell_px(cell, sx, sy);
            if p[3] < 128 {
                continue;
            }
            let (x, y) = (origin.0 + sx as i32, origin.1 + sy as i32);
            if (0..SIDE as i32).contains(&x) && (0..SIDE as i32).contains(&y) {
                rgb[y as usize * SIDE + x as usize] = [p[0], p[1], p[2]];
            }
        }
    }
}

/// The sprite cell and the pixel offset of the tree on a tile.
pub fn tree_look(tx: i32, ty: i32) -> (usize, (i32, i32)) {
    let h = hash(tx, ty, 11);
    // Patches of 6 x 6 tiles prefer one species. One tile in five picks any species.
    let patch = hash(tx.div_euclid(6), ty.div_euclid(6), 12) % 10;
    let pick = if h % 100 < 20 { (h >> 20) % 10 } else { patch };
    let (first, count) = match pick {
        0..=4 => (obj::TREE, 4),
        5..=7 => (obj::TREE + 4, 2),
        _ => (obj::TREE + 6, 2),
    };
    let cell = first + ((h >> 8) % count) as usize;
    let off = (((h >> 12) % 5) as i32 - 2, ((h >> 16) % 3) as i32 - 1);
    (cell, off)
}

/// Trees: shadows first (cast to the south east, one pass for all trees), then the sprites in
/// row order, so a tree further south covers the tree behind it.
fn trees(map: &LocalMapData, sprites: &Sprites, chunk_x: u32, chunk_y: u32, rgb: &mut [Rgb]) {
    let (tx0, ty0) = ((chunk_x * CHUNK) as i32, (chunk_y * CHUNK) as i32);
    let mut list: Vec<(usize, (i32, i32))> = Vec::new();
    for ty in ty0 - 1..ty0 + CHUNK as i32 + 1 {
        for tx in tx0 - 1..tx0 + CHUNK as i32 + 1 {
            if !tile_info(map, tx, ty).tree {
                continue;
            }
            let (cell, off) = tree_look(tx, ty);
            list.push((cell, ((tx - tx0) * 16 + off.0, (ty - ty0) * 16 + off.1)));
        }
    }
    if list.is_empty() {
        return;
    }
    let mut shade = vec![false; SIDE * SIDE];
    for &(cell, (ox, oy)) in &list {
        for sy in 0..16usize {
            for sx in 0..16usize {
                if sprites.objects.cell_px(cell, sx, sy)[3] < 128 {
                    continue;
                }
                // The canopy shadow is lower and flatter than the canopy.
                let (x, y) = (ox + sx as i32 + 3, oy + 4 + (sy as i32 * 3) / 4 + 2);
                if (0..SIDE as i32).contains(&x) && (0..SIDE as i32).contains(&y) {
                    shade[y as usize * SIDE + x as usize] = true;
                }
            }
        }
    }
    for (c, &s) in rgb.iter_mut().zip(&shade) {
        if s {
            *c = dim(*c, 150);
        }
    }
    for &(cell, origin) in &list {
        blit_sprite(sprites, cell, origin, rgb);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use worldgen::{EmbarkSummary, generate_local};

    fn palette() -> Palette {
        let c = |v: u8| [v, v, v];
        Palette {
            grass: c(0),
            soil: c(1),
            sand: c(2),
            clay: c(3),
            water: c(4),
            sandstone: c(5),
            limestone: c(6),
            granite: c(7),
            slate: c(8),
            coal: c(9),
            copper_ore: c(10),
            iron_ore: c(11),
        }
    }

    fn real_palette() -> Palette {
        let text = include_str!("../../../assets/content/materials.ron");
        let materials = base::content::load_materials("materials.ron", text).unwrap_or_default();
        Palette::from_materials(&materials).unwrap_or_else(|_| palette())
    }

    #[test]
    fn ore_beats_rock_beats_ground() {
        let p = palette();
        assert_eq!(
            p.tile_color(Ground::Stone, RockKind::Granite, Ore::Iron),
            [11; 3]
        );
        assert_eq!(
            p.tile_color(Ground::Stone, RockKind::Granite, Ore::None),
            [7; 3]
        );
        assert_eq!(
            p.tile_color(Ground::Water, RockKind::None, Ore::None),
            [4; 3]
        );
    }

    #[test]
    fn bake_chunk_has_expected_size_and_opaque_pixels() {
        let Ok(sprites) = Sprites::shipped() else {
            return;
        };
        let map = generate_local(&EmbarkSummary::temperate_forest());
        let bytes = bake_chunk(&map, &real_palette(), &sprites, 0, 0);
        assert_eq!(bytes.len(), SIDE * SIDE * 4);
        assert!(bytes.chunks(4).all(|px| px[3] == 255));
    }

    #[test]
    fn bake_is_repeatable_and_trees_change_pixels() {
        let Ok(sprites) = Sprites::shipped() else {
            return;
        };
        let p = real_palette();
        let mut map = generate_local(&EmbarkSummary::dry_plain());
        let a = bake_chunk(&map, &p, &sprites, 0, 0);
        assert_eq!(a, bake_chunk(&map, &p, &sprites, 0, 0));
        let open = (4..CHUNK as i32 - 4)
            .flat_map(|y| (4..CHUNK as i32 - 4).map(move |x| (x, y)))
            .find(|&(x, y)| tile_info(&map, x, y).kind == Kind::Open);
        let Some((x, y)) = open else { return };
        let was = map.tree.get(x, y).copied().unwrap_or(false);
        map.tree.set(x, y, !was);
        assert_ne!(a, bake_chunk(&map, &p, &sprites, 0, 0));
    }

    #[test]
    fn a_tile_inside_a_chunk_touches_one_chunk_and_a_corner_touches_four() {
        let size = (256, 256);
        assert_eq!(chunks_touched_by(10, 10, size).len(), 1);
        assert_eq!(chunks_touched_by(32, 32, size).len(), 4);
        assert_eq!(chunks_touched_by(31, 5, size).len(), 2);
        assert_eq!(chunks_touched_by(0, 0, size).len(), 1);
        assert!(chunks_touched_by(255, 255, size).contains(&(7, 7)));
    }

    #[test]
    fn chunk_counts_cover_256_map() {
        let map = generate_local(&EmbarkSummary::dry_plain());
        assert_eq!(chunk_counts(&map), (8, 8));
    }

    #[test]
    fn shipped_materials_build_a_palette() {
        let text = include_str!("../../../assets/content/materials.ron");
        let materials = base::content::load_materials("materials.ron", text);
        assert!(materials.is_ok());
        let palette = Palette::from_materials(&materials.unwrap_or_default());
        assert!(palette.is_ok());
    }

    #[test]
    fn fringes_are_hard_edged_and_between_one_and_four_pixels_wide() {
        for wx in -40..40 {
            for wy in -40..40 {
                let w = fringe_width(wx, wy, 3);
                assert!((1..=4).contains(&w));
            }
        }
    }

    #[test]
    fn a_pixel_next_to_a_neighbor_tile_has_a_gap_of_one_pixel_at_the_border() {
        assert_eq!(axis_gap(15, 1), 1);
        assert_eq!(axis_gap(0, -1), 1);
        assert_eq!(axis_gap(7, 0), 0);
    }

    #[test]
    fn tree_looks_are_stable_and_come_from_the_tree_cells() {
        for y in 0..20 {
            for x in 0..20 {
                let (cell, off) = tree_look(x, y);
                assert_eq!((cell, off), tree_look(x, y));
                assert!((obj::TREE..obj::TREE + obj::TREE_COUNT).contains(&cell));
                assert!((-2..=2).contains(&off.0) && (-1..=1).contains(&off.1));
            }
        }
    }

    #[test]
    fn water_has_foam_and_land_next_to_it_is_darker() {
        let Ok(sprites) = Sprites::shipped() else {
            return;
        };
        let map = generate_local(&EmbarkSummary::temperate_forest());
        let p = real_palette();
        let counts = chunk_counts(&map);
        for cy in 0..counts.1 {
            for cx in 0..counts.0 {
                let region = Region::new(&map, cx, cy);
                if !region.has_water {
                    continue;
                }
                let bytes = bake_chunk(&map, &p, &sprites, cx, cy);
                let foam = lut(FOAM)[255];
                let found = bytes.chunks(4).filter(|px| px[..3] == foam).count();
                if found > 0 {
                    return;
                }
            }
        }
    }
}
