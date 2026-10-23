//! Cell numbers of all sprite sheets. The game (`sprites.rs`) and `tools assetgen` include this
//! one file, so both sides always agree (docs/ART_DIRECTION.md section 10).
#![allow(dead_code)] // The game and the tools use different parts of this file.

/// Sprite side in pixels.
pub const TILE: usize = 16;
/// Sprites per sheet row (all sheets except `terrain.png`).
pub const COLUMNS: usize = 16;

/// `terrain.png`: seamless gray textures of 128 x 128 pixels, 4 per row. The game reads them
/// by world pixel position, so the ground has no visible tile grid.
pub mod terrain {
    /// Side of one texture in pixels. A texture repeats every 8 tiles.
    pub const SIZE: usize = 128;
    /// Textures per row.
    pub const COLS: usize = 4;
    /// Rows of the sheet.
    pub const ROWS: usize = 2;
    /// Grass.
    pub const GRASS: usize = 0;
    /// Bare soil.
    pub const SOIL: usize = 1;
    /// Sand.
    pub const SAND: usize = 2;
    /// Clay.
    pub const CLAY: usize = 3;
    /// Water.
    pub const WATER: usize = 4;
    /// Stone floor.
    pub const FLOOR: usize = 5;
    /// Top of a rock wall.
    pub const ROCK: usize = 6;
    /// Front face of a rock wall (the first rows of the texture).
    pub const FACE: usize = 7;
    /// Number of textures.
    pub const COUNT: usize = 8;
    /// Height of the wall face in pixels.
    pub const FACE_H: usize = 6;

    /// Top left pixel of a texture in the sheet.
    pub fn origin(index: usize) -> (usize, usize) {
        ((index % COLS) * SIZE, (index / COLS) * SIZE)
    }
}

/// `tiles.png`: gray overlay sprites on rock tiles (ore).
pub mod tiles {
    /// First ore variant.
    pub const ORE: usize = 0;
    /// Number of ore variants.
    pub const ORE_COUNT: usize = 8;
    /// Rows of the sheet.
    pub const ROWS: usize = 1;
}

/// `objects.png`: full color sprites that stand on the ground.
pub mod objects {
    /// First tree variant.
    pub const TREE: usize = 0;
    /// Number of tree variants: 3 oaks, a small oak, 2 pines, 2 birches.
    pub const TREE_COUNT: usize = 8;
    /// First flower variant (red, yellow, white, blue).
    pub const FLOWER: usize = 8;
    /// Number of flower variants.
    pub const FLOWER_COUNT: usize = 4;
    /// First pebble variant.
    pub const PEBBLE: usize = 12;
    /// Number of pebble variants.
    pub const PEBBLE_COUNT: usize = 3;
    /// First tall grass variant.
    pub const TUFT: usize = 15;
    /// Number of tall grass variants.
    pub const TUFT_COUNT: usize = 2;
    /// Rows of the sheet.
    pub const ROWS: usize = 2;
}

/// `items.png`: gray sprites with alpha. Each shape has 3 sizes of pile.
pub mod items {
    /// Sizes of a pile: 1 unit, 2 to 9 units, 10 or more units.
    pub const TIERS: usize = 3;
    /// Logs.
    pub const LOGS: usize = 0;
    /// Pieces of stone.
    pub const ROCKS: usize = 1;
    /// Chunks of ore.
    pub const ORE: usize = 2;
    /// Ingots of metal.
    pub const INGOTS: usize = 3;
    /// Lumps of fuel (coal).
    pub const LUMPS: usize = 4;
    /// Crates and anything else.
    pub const CRATE: usize = 5;
    /// Number of shapes.
    pub const SHAPES: usize = 6;
    /// Rows of the sheet.
    pub const ROWS: usize = 2;

    /// Cell of a shape in a tier (0 to 2).
    pub fn cell(shape: usize, tier: usize) -> usize {
        shape * TIERS + tier.min(TIERS - 1)
    }

    /// Tier of a stack with `count` units.
    pub fn tier_for(count: u32) -> usize {
        match count {
            0 | 1 => 0,
            2..=9 => 1,
            _ => 2,
        }
    }
}

/// `icons.png`: gray sprites with alpha. The game multiplies them with a color.
pub mod icons {
    /// Select tool: a pointer arrow.
    pub const SELECT: usize = 0;
    /// Cut tool: an axe.
    pub const CUT: usize = 1;
    /// Mine tool: a pickaxe.
    pub const MINE: usize = 2;
    /// Haul tool: a box with an arrow.
    pub const HAUL: usize = 3;
    /// Zone tool: a dashed square.
    pub const ZONE: usize = 4;
    /// Cancel tool: a cross.
    pub const CANCEL: usize = 5;
    /// Work panel: two small people.
    pub const WORK: usize = 6;
    /// Pause symbol.
    pub const PAUSE: usize = 7;
    /// Play symbol.
    pub const PLAY: usize = 8;
    /// Fast forward symbol.
    pub const FAST: usize = 9;
    /// Badge of a cut order (top right of a tile, 9 x 9 pixels).
    pub const BADGE_CUT: usize = 10;
    /// Badge of a mine order.
    pub const BADGE_MINE: usize = 11;
    /// Badge of a haul order.
    pub const BADGE_HAUL: usize = 12;
    /// Four corner brackets, the border of a marked tile.
    pub const BRACKETS: usize = 13;
    /// Build tool: a hammer.
    pub const BUILD: usize = 14;
    /// Remove tool: a crowbar over a broken plank.
    pub const REMOVE: usize = 15;
    /// A person (head and shoulders): the colonist count.
    pub const PERSON: usize = 16;
    /// A cross: close a panel.
    pub const CLOSE: usize = 17;
    /// A chevron that points down: an open fold.
    pub const CHEVRON_DOWN: usize = 18;
    /// A chevron that points right: a closed fold.
    pub const CHEVRON_RIGHT: usize = 19;
    /// Three triangles: the fastest speed.
    pub const FASTEST: usize = 20;
    /// A thermometer: temperature.
    pub const THERMOMETER: usize = 21;
    /// A drop: rain.
    pub const RAIN: usize = 22;
    /// A flame: hearth fuel.
    pub const FLAME: usize = 23;
    /// A snowflake: cold period.
    pub const SNOWFLAKE: usize = 24;
    /// A heart: health.
    pub const HEART: usize = 25;
    /// A face: mood.
    pub const MOOD: usize = 26;
    /// An apple: hunger.
    pub const FOOD: usize = 27;
    /// A crescent moon: rest.
    pub const MOON: usize = 28;
    /// A skull: death.
    pub const SKULL: usize = 29;
    /// A sun: heatstroke.
    pub const SUN: usize = 30;
    /// A triangle with a bar: a warning.
    pub const WARN: usize = 31;
    /// Rows of the sheet.
    pub const ROWS: usize = 2;
}

/// `buildings.png`: gray sprites. The alpha value says how the game colors a pixel:
/// 255 takes the material color, 254 cloth, 253 fire, 252 metal. A lower alpha is a black
/// shadow. Walls and roofs have 16 cells each, one for each set of connected sides.
pub mod buildings {
    /// Alpha of a pixel that takes the material color.
    pub const MATERIAL_ALPHA: u8 = 255;
    /// Alpha of a cloth pixel (blanket).
    pub const CLOTH_ALPHA: u8 = 254;
    /// Alpha of a fire pixel (hearth).
    pub const FIRE_ALPHA: u8 = 253;
    /// Alpha of a metal pixel (tools).
    pub const METAL_ALPHA: u8 = 252;
    /// Connection bit: a neighbor to the north.
    pub const NORTH: u8 = 1;
    /// Connection bit: a neighbor to the east.
    pub const EAST: u8 = 2;
    /// Connection bit: a neighbor to the south.
    pub const SOUTH: u8 = 4;
    /// Connection bit: a neighbor to the west.
    pub const WEST: u8 = 8;
    /// First stone wall cell (16 cells, index = connection bits).
    pub const WALL_STONE: usize = 0;
    /// First wood wall cell (16 cells, index = connection bits).
    pub const WALL_WOOD: usize = 16;
    /// First roof cell (16 cells, index = connection bits).
    pub const ROOF: usize = 32;
    /// Door in a wall that runs east to west.
    pub const DOOR_H: usize = 48;
    /// Door in a wall that runs north to south.
    pub const DOOR_V: usize = 49;
    /// Stone floor.
    pub const FLOOR_STONE: usize = 50;
    /// Wood floor.
    pub const FLOOR_WOOD: usize = 51;
    /// Bed.
    pub const BED: usize = 52;
    /// Hearth, first flame frame.
    pub const HEARTH: usize = 53;
    /// Hearth, second flame frame.
    pub const HEARTH_B: usize = 54;
    /// Workshop (workbench).
    pub const WORKSHOP: usize = 55;
    /// Rows of the sheet.
    pub const ROWS: usize = 4;

    /// The wall cell for a set of connected sides.
    pub fn wall_cell(wood: bool, mask: u8) -> usize {
        let base = if wood { WALL_WOOD } else { WALL_STONE };
        base + usize::from(mask & 15)
    }

    /// The roof cell for a set of connected sides.
    pub fn roof_cell(mask: u8) -> usize {
        ROOF + usize::from(mask & 15)
    }
}

/// `actors.png`: full color colonists. The shirt is gray and the game tints it. Hair and skin
/// use the colors of index 0 in `palette.rs`; the game swaps them for each colonist.
pub mod actors {
    /// Stand, step 1, step 2 for each of south, north, east.
    pub const WALK: usize = 9;
    /// Work frames: 2 for each of south, north, east.
    pub const WORK: usize = 6;
    /// Cells of one hair style.
    pub const STYLE_CELLS: usize = WALK + WORK;
    /// First south frame (stand, step 1, step 2).
    pub const SOUTH: usize = 0;
    /// First north frame.
    pub const NORTH: usize = 3;
    /// First east frame. West is the east frame flipped.
    pub const EAST: usize = 6;
    /// First south work frame (2 frames).
    pub const WORK_SOUTH: usize = 9;
    /// First north work frame.
    pub const WORK_NORTH: usize = 11;
    /// First east work frame.
    pub const WORK_EAST: usize = 13;
    /// Rows of the sheet (4 styles of 15 cells in 16 columns need 4 rows).
    pub const ROWS: usize = 4;
}
