//! Rooms: closed, roofed areas (R-020), room insulation (R-021), and room beauty.
//!
//! The rooms are a derived cache. They are not part of the state or the hash. `Sim`
//! detects them again in the tick after a structural change (a wall, a door, a roof, a
//! floor, or the rock changed) and not at any other time. `Sim::room_detections` counts the
//! runs.
//!
//! A room is a group of tiles that are connected by their four sides. The group stops at
//! border tiles: wall buildings, door buildings, wall tiles of the map, and natural rock.
//! The group is a room only if every tile has a supported roof, the group does not touch
//! the map edge, and it has at most `max_tiles` tiles. A door is a border tile, so two
//! rooms next to a door are two rooms.

use crate::Sim;
use base::content::BuildingKind;
use std::collections::BTreeMap;

/// What a border tile of a room is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum BorderKind {
    /// A wall building.
    Wall,
    /// A door building.
    Door,
    /// Natural rock.
    NaturalRock,
    /// A wall tile of the map that has no building (scenario walls).
    MapWall,
}

impl BorderKind {
    /// Display name.
    pub fn name(self) -> &'static str {
        match self {
            BorderKind::Wall => "Wall",
            BorderKind::Door => "Door",
            BorderKind::NaturalRock => "Natural rock",
            BorderKind::MapWall => "Wall tile",
        }
    }
}

/// A tile on the border of a room.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BorderTile {
    /// Tile column.
    pub x: u16,
    /// Tile row.
    pub y: u16,
    /// What the tile is.
    pub kind: BorderKind,
    /// Material index of the tile.
    pub material: u16,
}

/// One named contribution to the insulation of a room: all border tiles of one kind and
/// material (R-021).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InsulationPart {
    /// Kind of the border tiles.
    pub kind: BorderKind,
    /// Their material index.
    pub material: u16,
    /// Number of border tiles.
    pub tiles: u32,
    /// Insulation of each tile, 0 to 100.
    pub each: u32,
}

/// The insulation of a room, 0 to 100, with its parts (R-021).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RoomInsulation {
    /// Named contributions, in a fixed order (kind, then material).
    pub parts: Vec<InsulationPart>,
    /// The integer mean over all border tiles, rounded down. 0 if there is no border tile.
    pub total: u32,
}

impl RoomInsulation {
    /// Make the insulation from its parts. The total is the integer mean over the border
    /// tiles, rounded down: 10 tiles at 60 and 2 tiles at 30 give 55.
    pub fn from_parts(parts: Vec<InsulationPart>) -> Self {
        let tiles: u64 = parts.iter().map(|p| u64::from(p.tiles)).sum();
        let sum: u64 = parts
            .iter()
            .map(|p| u64::from(p.tiles) * u64::from(p.each))
            .sum();
        let total = sum.checked_div(tiles).unwrap_or(0);
        Self {
            parts,
            total: u32::try_from(total).unwrap_or(0),
        }
    }
}

/// Insulation of one tile (0 to 100) from the per-mille value of its material.
pub fn insulation_from_permille(permille: u32) -> u32 {
    (permille / 10).min(100)
}

/// Where a part of the room beauty comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum BeautyCause {
    /// A floor building inside the room.
    Floor,
    /// A bed, hearth, or workshop inside the room.
    Furniture,
    /// A wall, door, or rock tile on the border.
    Border,
}

impl BeautyCause {
    /// Display name.
    pub fn name(self) -> &'static str {
        match self {
            BeautyCause::Floor => "Floor",
            BeautyCause::Furniture => "Furniture",
            BeautyCause::Border => "Border",
        }
    }
}

/// One named contribution to the beauty of a room.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BeautyPart {
    /// The cause.
    pub cause: BeautyCause,
    /// Building type index. `None` for natural rock and wall tiles of the map.
    pub building: Option<u16>,
    /// Material index.
    pub material: u16,
    /// Number of tiles.
    pub tiles: u32,
    /// Beauty of each tile: the beauty of the building plus the beauty of the material.
    pub each: i32,
}

/// The beauty of a room with its parts.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RoomBeauty {
    /// Named contributions, in a fixed order.
    pub parts: Vec<BeautyPart>,
    /// Sum of `tiles * each` over all parts.
    pub total: i32,
    /// Beauty of a room tile on average: `total` divided by the room tiles, rounded toward
    /// zero.
    pub per_tile: i32,
}

impl RoomBeauty {
    /// Make the beauty from its parts and the number of room tiles.
    pub fn from_parts(parts: Vec<BeautyPart>, room_tiles: u32) -> Self {
        let total: i32 = parts
            .iter()
            .map(|p| {
                i32::try_from(p.tiles)
                    .unwrap_or(i32::MAX)
                    .saturating_mul(p.each)
            })
            .fold(0i32, i32::saturating_add);
        let per_tile = match i32::try_from(room_tiles) {
            Ok(n) if n > 0 => total / n,
            _ => 0,
        };
        Self {
            parts,
            total,
            per_tile,
        }
    }
}

/// A room.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Room {
    /// The room tiles, in row-major order.
    pub tiles: Vec<(u16, u16)>,
    /// The border tiles, in row-major order.
    pub border: Vec<BorderTile>,
    /// Insulation (R-021).
    pub insulation: RoomInsulation,
    /// Beauty.
    pub beauty: RoomBeauty,
    /// Number of border tiles that are natural rock. R-022 uses the share of rock.
    pub rock_border: u32,
}

impl Room {
    /// Number of room tiles.
    pub fn len(&self) -> usize {
        self.tiles.len()
    }

    /// True if the room has no tile. A detected room always has tiles.
    pub fn is_empty(&self) -> bool {
        self.tiles.is_empty()
    }
}

/// All rooms of the colony. The index of a room is valid until the next detection.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Rooms {
    rooms: Vec<Room>,
    /// Room index plus one for each map tile, 0 for none.
    index: Vec<u32>,
    width: u32,
}

impl Rooms {
    /// The rooms, in the order of their first tile (row-major).
    pub fn iter(&self) -> impl Iterator<Item = &Room> {
        self.rooms.iter()
    }

    /// Number of rooms.
    pub fn len(&self) -> usize {
        self.rooms.len()
    }

    /// True if there is no room.
    pub fn is_empty(&self) -> bool {
        self.rooms.is_empty()
    }

    /// The room with this index.
    pub fn get(&self, index: usize) -> Option<&Room> {
        self.rooms.get(index)
    }

    /// The index of the room that contains a tile.
    pub fn index_at(&self, x: u16, y: u16) -> Option<usize> {
        if u32::from(x) >= self.width {
            return None;
        }
        let i = usize::from(y) * self.width as usize + usize::from(x);
        match self.index.get(i) {
            Some(&n) if n > 0 => Some(n as usize - 1),
            _ => None,
        }
    }

    /// The room that contains a tile.
    pub fn at(&self, x: u16, y: u16) -> Option<&Room> {
        self.index_at(x, y).and_then(|i| self.rooms.get(i))
    }
}

impl Sim {
    // ---- queries ----

    /// The rooms, as found by the last detection.
    pub fn rooms(&self) -> &Rooms {
        &self.rooms
    }

    /// The room that contains a tile.
    pub fn room_at(&self, x: u16, y: u16) -> Option<&Room> {
        self.rooms.at(x, y)
    }

    /// The insulation of the room that contains a tile, with its parts (R-021).
    pub fn room_insulation(&self, x: u16, y: u16) -> Option<&RoomInsulation> {
        self.rooms.at(x, y).map(|r| &r.insulation)
    }

    /// The beauty of the room that contains a tile, with its parts.
    pub fn room_beauty(&self, x: u16, y: u16) -> Option<&RoomBeauty> {
        self.rooms.at(x, y).map(|r| &r.beauty)
    }

    /// How many times the room detection ran. It runs only in the tick after a structural
    /// change.
    pub fn room_detections(&self) -> u64 {
        self.room_detections
    }

    /// True if a structural change waits for the room detection in the next tick.
    pub fn rooms_pending(&self) -> bool {
        self.rooms_dirty
    }

    // ---- detection ----

    /// Run the room detection now if a structural change is pending. `step` calls this at
    /// the start of the tick.
    pub(crate) fn detect_rooms(&mut self) {
        if self.rooms_dirty {
            self.rooms_dirty = false;
            self.room_detections += 1;
            let new = self.compute_rooms();
            let old = std::mem::replace(&mut self.rooms, new);
            let old_temps = std::mem::take(&mut self.room_temps);
            self.carry_room_temps(&old, &old_temps);
        }
    }

    /// Find the rooms again without counting a detection. For a new or loaded `Sim`.
    pub(crate) fn refresh_rooms(&mut self) {
        self.rooms_dirty = false;
        self.rooms = self.compute_rooms();
        self.fill_room_temps();
    }

    /// What a tile is for the border of a room, if it is a border tile.
    fn border_at(&self, x: i32, y: i32) -> Option<(BorderKind, u16)> {
        if let Some(b) = self.buildings.object_at(x as u16, y as u16) {
            if self.rules.is_kind(b.def, BuildingKind::Wall) {
                return Some((BorderKind::Wall, b.material));
            }
            if self.rules.is_kind(b.def, BuildingKind::Door) {
                return Some((BorderKind::Door, b.material));
            }
        }
        if self.map.wall(x, y) {
            return Some((BorderKind::MapWall, self.rules.bare_wall));
        }
        match self.map.rock(x, y) {
            Some(rock) if rock != worldgen::RockKind::None => {
                Some((BorderKind::NaturalRock, self.rules.rock_material(rock)))
            }
            _ => None,
        }
    }

    /// True if the tile has a roof with a supporting wall or rock (A-008).
    fn tile_roofed(&self, x: u16, y: u16) -> bool {
        self.buildings.roof_at(x, y).is_some() && self.roof_supported(x, y)
    }

    /// Find all rooms. Each tile is visited once, so the cost is at most the number of tiles
    /// of the map.
    fn compute_rooms(&self) -> Rooms {
        let width = self.map.width();
        let len = width as usize * self.map.height() as usize;
        let mut rooms = Rooms {
            rooms: Vec::new(),
            index: vec![0; len],
            width,
        };
        let mut seeds = self.buildings.roof_tiles();
        seeds.sort_unstable_by_key(|&(x, y)| (y, x));
        let mut seen = vec![false; len];
        let max_tiles = self.rules.rooms.max_tiles as usize;
        for (sx, sy) in seeds {
            let Some(start) = self.map.index(i32::from(sx), i32::from(sy)) else {
                continue;
            };
            if seen[start] || self.border_at(i32::from(sx), i32::from(sy)).is_some() {
                continue;
            }
            seen[start] = true;
            let mut stack = vec![(sx, sy)];
            let mut tiles: Vec<(u16, u16)> = Vec::new();
            let mut border: BTreeMap<(u16, u16), (BorderKind, u16)> = BTreeMap::new();
            let mut open = false;
            while let Some((x, y)) = stack.pop() {
                if !open && tiles.len() <= max_tiles {
                    tiles.push((x, y));
                }
                if !open && !self.tile_roofed(x, y) {
                    open = true;
                }
                for (dx, dy) in crate::job::NEIGHBORS {
                    let (nx, ny) = (i32::from(x) + dx, i32::from(y) + dy);
                    let Some(ni) = self.map.index(nx, ny) else {
                        open = true;
                        continue;
                    };
                    if let Some(kind) = self.border_at(nx, ny) {
                        border.insert((nx as u16, ny as u16), kind);
                    } else if !seen[ni] {
                        seen[ni] = true;
                        stack.push((nx as u16, ny as u16));
                    }
                }
            }
            if open || tiles.len() > max_tiles || tiles.is_empty() {
                continue;
            }
            tiles.sort_unstable_by_key(|&(x, y)| (y, x));
            let room = self.make_room(tiles, border);
            let number = rooms.rooms.len() as u32 + 1;
            for &(x, y) in &room.tiles {
                if let Some(i) = self.map.index(i32::from(x), i32::from(y)) {
                    rooms.index[i] = number;
                }
            }
            rooms.rooms.push(room);
        }
        rooms
    }

    /// Fill in the border, the insulation, and the beauty of a room.
    fn make_room(
        &self,
        tiles: Vec<(u16, u16)>,
        border: BTreeMap<(u16, u16), (BorderKind, u16)>,
    ) -> Room {
        let rules = &self.rules;
        let mut border_list: Vec<BorderTile> = border
            .iter()
            .map(|(&(x, y), &(kind, material))| BorderTile {
                x,
                y,
                kind,
                material,
            })
            .collect();
        border_list.sort_unstable_by_key(|b| (b.y, b.x));

        let mut groups: BTreeMap<(BorderKind, u16), u32> = BTreeMap::new();
        for b in &border_list {
            *groups.entry((b.kind, b.material)).or_insert(0) += 1;
        }
        let insulation = RoomInsulation::from_parts(
            groups
                .iter()
                .map(|(&(kind, material), &count)| InsulationPart {
                    kind,
                    material,
                    tiles: count,
                    each: insulation_from_permille(rules.insulation(material)),
                })
                .collect(),
        );
        let rock_border = border_list
            .iter()
            .filter(|b| b.kind == BorderKind::NaturalRock)
            .count() as u32;

        // Beauty: floors and furniture inside, walls, doors, and rock on the border.
        let mut beauty: BTreeMap<(BeautyCause, Option<u16>, u16), u32> = BTreeMap::new();
        for &(x, y) in &tiles {
            if let Some(f) = self.buildings.floor_at(x, y) {
                *beauty
                    .entry((BeautyCause::Floor, Some(f.def), f.material))
                    .or_insert(0) += 1;
            }
            if let Some(o) = self.buildings.object_at(x, y) {
                *beauty
                    .entry((BeautyCause::Furniture, Some(o.def), o.material))
                    .or_insert(0) += 1;
            }
        }
        for b in &border_list {
            let building = self
                .buildings
                .object_at(b.x, b.y)
                .filter(|_| matches!(b.kind, BorderKind::Wall | BorderKind::Door))
                .map(|o| o.def);
            *beauty
                .entry((BeautyCause::Border, building, b.material))
                .or_insert(0) += 1;
        }
        let beauty_parts: Vec<BeautyPart> = beauty
            .iter()
            .map(|(&(cause, building, material), &count)| BeautyPart {
                cause,
                building,
                material,
                tiles: count,
                each: building
                    .and_then(|d| rules.building(d))
                    .map_or(0, |d| i32::from(d.beauty))
                    + rules.beauty(material),
            })
            .collect();
        let beauty = RoomBeauty::from_parts(beauty_parts, tiles.len() as u32);
        Room {
            tiles,
            border: border_list,
            insulation,
            beauty,
            rock_border,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn part(tiles: u32, each: u32) -> InsulationPart {
        InsulationPart {
            kind: BorderKind::Wall,
            material: 0,
            tiles,
            each,
        }
    }

    #[test]
    fn r021_ten_tiles_at_60_and_two_at_30_give_55() {
        let ins = RoomInsulation::from_parts(vec![part(10, 60), part(2, 30)]);
        assert_eq!(ins.total, 55);
        // The mean is rounded down: (60 + 61) / 2 = 60.
        assert_eq!(
            RoomInsulation::from_parts(vec![part(1, 60), part(1, 61)]).total,
            60
        );
        assert_eq!(RoomInsulation::from_parts(Vec::new()).total, 0);
    }

    #[test]
    fn insulation_per_tile_is_a_tenth_of_the_per_mille_value() {
        assert_eq!(insulation_from_permille(600), 60);
        assert_eq!(insulation_from_permille(305), 30);
        assert_eq!(insulation_from_permille(5_000), 100);
    }

    #[test]
    fn beauty_total_is_the_sum_of_the_parts() {
        let parts = vec![
            BeautyPart {
                cause: BeautyCause::Floor,
                building: Some(0),
                material: 0,
                tiles: 4,
                each: 2,
            },
            BeautyPart {
                cause: BeautyCause::Border,
                building: None,
                material: 0,
                tiles: 3,
                each: -1,
            },
        ];
        let beauty = RoomBeauty::from_parts(parts, 4);
        assert_eq!(beauty.total, 5);
        assert_eq!(beauty.per_tile, 1);
    }
}
