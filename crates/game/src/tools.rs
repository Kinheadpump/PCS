//! Designation tools: pure logic for drag rectangles, tool commands, stockpile filters,
//! and priority clicks. No `macroquad` calls, so tests run without a window.

use sim::rules::NO_MATERIAL;
use sim::{Command, ItemFilter, ItemLoc, JobKind, Sim, SimRules};

/// The tool that the left mouse button uses on the map.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tool {
    /// Select a colonist or a tile. The right button gives the debug move order.
    Select,
    /// Drag a rectangle to designate trees for cutting.
    Cut,
    /// Drag a rectangle to designate rock walls for mining.
    Mine,
    /// Drag a rectangle to order the items on it to a stockpile.
    Haul,
    /// Drag a rectangle to make a stockpile zone.
    Stockpile,
    /// Drag a rectangle to remove designations and haul orders.
    Cancel,
    /// Drag a line or a rectangle to place blueprints of the chosen building.
    Build,
    /// Drag a rectangle to take buildings down and to remove blueprints.
    Deconstruct,
}

impl Tool {
    /// All tools, in toolbar order.
    pub const ALL: [Tool; 8] = [
        Tool::Select,
        Tool::Cut,
        Tool::Mine,
        Tool::Haul,
        Tool::Stockpile,
        Tool::Cancel,
        Tool::Build,
        Tool::Deconstruct,
    ];

    /// Name for the toolbar and the status text.
    pub fn name(self) -> &'static str {
        match self {
            Tool::Select => "Select",
            Tool::Cut => "Cut",
            Tool::Mine => "Mine",
            Tool::Haul => "Haul",
            Tool::Stockpile => "Zone",
            Tool::Cancel => "Cancel",
            Tool::Build => "Build",
            Tool::Deconstruct => "Remove",
        }
    }

    /// The key that selects the tool (shown in the toolbar). The key binding table in
    /// `input.rs` must use the same keys; a test checks this.
    pub fn key_label(self) -> char {
        match self {
            Tool::Select => 'V',
            Tool::Cut => 'T',
            Tool::Mine => 'M',
            Tool::Haul => 'H',
            Tool::Stockpile => 'Z',
            Tool::Cancel => 'C',
            Tool::Build => 'B',
            Tool::Deconstruct => 'X',
        }
    }

    /// The designation type of the tool, for the three designation tools.
    pub fn job_kind(self) -> Option<JobKind> {
        match self {
            Tool::Cut => Some(JobKind::Cut),
            Tool::Mine => Some(JobKind::Mine),
            Tool::Haul => Some(JobKind::Haul),
            _ => None,
        }
    }

    /// True if the tool drags a rectangle.
    pub fn drags(self) -> bool {
        self != Tool::Select
    }

    /// The tool for a word in a debug setting (`PCS_TOOL`).
    pub fn from_word(word: &str) -> Option<Tool> {
        match word.trim().to_ascii_lowercase().as_str() {
            "select" => Some(Tool::Select),
            "cut" => Some(Tool::Cut),
            "mine" => Some(Tool::Mine),
            "haul" => Some(Tool::Haul),
            "zone" | "stockpile" => Some(Tool::Stockpile),
            "cancel" => Some(Tool::Cancel),
            "build" => Some(Tool::Build),
            "remove" | "deconstruct" => Some(Tool::Deconstruct),
            _ => None,
        }
    }
}

/// A rectangle of tiles. The corners are inclusive and `x0 <= x1`, `y0 <= y1`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Area {
    /// Left column.
    pub x0: u16,
    /// Top row.
    pub y0: u16,
    /// Right column.
    pub x1: u16,
    /// Bottom row.
    pub y1: u16,
}

impl Area {
    /// The rectangle between two corner tiles, in any order.
    pub fn from_corners(a: (u16, u16), b: (u16, u16)) -> Self {
        Self {
            x0: a.0.min(b.0),
            y0: a.1.min(b.1),
            x1: a.0.max(b.0),
            y1: a.1.max(b.1),
        }
    }

    /// Width in tiles.
    pub fn width(&self) -> u32 {
        u32::from(self.x1 - self.x0) + 1
    }

    /// Height in tiles.
    pub fn height(&self) -> u32 {
        u32::from(self.y1 - self.y0) + 1
    }

    /// All tiles in row-major order.
    pub fn tiles(&self) -> impl Iterator<Item = (u16, u16)> + use<> {
        let a = *self;
        (a.y0..=a.y1).flat_map(move |y| (a.x0..=a.x1).map(move |x| (x, y)))
    }
}

/// The tile for a map position (tiles), clamped into a map of `size`. `None` for an empty map.
/// A drag that ends outside the map ends at the nearest edge tile.
pub fn clamp_tile(point: (f32, f32), size: (u32, u32)) -> Option<(u16, u16)> {
    if size.0 == 0 || size.1 == 0 {
        return None;
    }
    let clamp = |v: f32, n: u32| -> u16 {
        let max = n.saturating_sub(1).min(u32::from(u16::MAX));
        (v.floor().max(0.0) as u32).min(max) as u16
    };
    Some((clamp(point.0, size.0), clamp(point.1, size.1)))
}

/// True if the tool does something on the tile now. A designation tool skips tiles where
/// the order makes no sense, so the sim does not reject it.
pub fn applies(sim: &Sim, tool: Tool, x: u16, y: u16) -> bool {
    let (ix, iy) = (i32::from(x), i32::from(y));
    let free = sim.designation(x, y).is_none();
    match tool {
        Tool::Cut => free && sim.map().tree(ix, iy),
        Tool::Mine => {
            free && sim
                .map()
                .rock(ix, iy)
                .is_some_and(|r| r != worldgen::RockKind::None)
        }
        Tool::Haul => sim.items().at_tile(x, y).iter().any(|&id| {
            sim.items()
                .get(id)
                .is_some_and(|i| !i.to_haul && matches!(i.loc, ItemLoc::Tile(..)))
        }),
        Tool::Cancel => {
            !free
                || sim.items().at_tile(x, y).iter().any(|&id| {
                    sim.items()
                        .get(id)
                        .is_some_and(|i| i.to_haul && matches!(i.loc, ItemLoc::Tile(..)))
                })
        }
        Tool::Stockpile | Tool::Select | Tool::Build | Tool::Deconstruct => false,
    }
}

/// Number of tiles of the area where the tool does something. A zone counts all its tiles.
pub fn count_applicable(sim: &Sim, tool: Tool, area: Area) -> u32 {
    match tool {
        Tool::Select | Tool::Build | Tool::Deconstruct => 0,
        Tool::Stockpile => area.width() * area.height(),
        _ => area
            .tiles()
            .filter(|&(x, y)| applies(sim, tool, x, y))
            .count() as u32,
    }
}

/// The commands for a finished drag. The tool is not `Select` for any command.
/// `accepts` says if a designation tool works on a tile (see [`applies`]).
pub fn commands_for_drag(
    tool: Tool,
    area: Area,
    accepts: impl Fn(Tool, u16, u16) -> bool,
) -> Vec<Command> {
    match tool {
        Tool::Select | Tool::Build | Tool::Deconstruct => Vec::new(),
        Tool::Stockpile => vec![Command::CreateStockpile {
            x0: area.x0,
            y0: area.y0,
            x1: area.x1,
            y1: area.y1,
            filter: ItemFilter::All,
        }],
        Tool::Cancel => area
            .tiles()
            .filter(|&(x, y)| accepts(tool, x, y))
            .map(|(x, y)| Command::CancelDesignation { x, y })
            .collect(),
        Tool::Cut | Tool::Mine | Tool::Haul => {
            let Some(kind) = tool.job_kind() else {
                return Vec::new();
            };
            area.tiles()
                .filter(|&(x, y)| accepts(tool, x, y))
                .map(|(x, y)| Command::Designate { kind, x, y })
                .collect()
        }
    }
}

/// Read a debug order `tool:x0,y0,x1,y1` (the tool word is optional when `default` is set).
pub fn parse_order(text: &str, default: Tool) -> Option<(Tool, Area)> {
    let (tool, numbers) = match text.split_once(':') {
        Some((word, rest)) => (Tool::from_word(word)?, rest),
        None => (default, text),
    };
    let mut it = numbers.split(',').map(|p| p.trim().parse::<u16>().ok());
    let a = (it.next()??, it.next()??);
    let b = (it.next()??, it.next()??);
    Some((tool, Area::from_corners(a, b)))
}

/// Materials that a stockpile can hold: logs, stone, and ore. Sorted, no duplicates.
pub fn haulable_materials(rules: &SimRules) -> Vec<u16> {
    let mut list: Vec<u16> = std::iter::once(rules.log)
        .chain(rules.rock_item[1..].iter().copied())
        .chain(rules.ore_item[1..].iter().copied())
        .filter(|&m| m != NO_MATERIAL)
        .collect();
    list.sort_unstable();
    list.dedup();
    list
}

/// The filter after the player toggles one material. A list with all `materials` is `All`.
pub fn toggle_material(filter: &ItemFilter, material: u16, materials: &[u16]) -> ItemFilter {
    let mut list: Vec<u16> = match filter {
        ItemFilter::All => materials.to_vec(),
        ItemFilter::Only(list) => list.clone(),
    };
    match list.iter().position(|&m| m == material) {
        Some(i) => {
            list.remove(i);
        }
        None => list.push(material),
    }
    list.sort_unstable();
    list.dedup();
    if materials.iter().all(|m| list.contains(m)) {
        ItemFilter::All
    } else {
        ItemFilter::Only(list)
    }
}

/// The priority after a click: forward goes off, 1, 2, 3, 4, off; backward is the reverse.
pub fn next_priority(priority: u8, backward: bool) -> u8 {
    let p = priority.min(4);
    if backward { (p + 4) % 5 } else { (p + 1) % 5 }
}

/// Text of a priority: `1` to `4`, or `off`.
pub fn priority_text(priority: u8) -> &'static str {
    match priority {
        1 => "1",
        2 => "2",
        3 => "3",
        4 => "4",
        _ => "off",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sim::scenario::sim_from_ascii;

    #[test]
    fn rectangle_corners_are_normalised_in_any_order() {
        let a = Area::from_corners((5, 2), (1, 9));
        assert_eq!(
            a,
            Area {
                x0: 1,
                y0: 2,
                x1: 5,
                y1: 9
            }
        );
        assert_eq!(a, Area::from_corners((1, 9), (5, 2)));
        assert_eq!((a.width(), a.height()), (5, 8));
        assert_eq!(Area::from_corners((3, 3), (3, 3)).tiles().count(), 1);
    }

    #[test]
    fn area_tiles_are_row_major() {
        let tiles: Vec<_> = Area::from_corners((1, 1), (2, 2)).tiles().collect();
        assert_eq!(tiles, vec![(1, 1), (2, 1), (1, 2), (2, 2)]);
    }

    #[test]
    fn a_drag_end_outside_the_map_is_clamped_to_the_edge() {
        assert_eq!(clamp_tile((3.7, 4.2), (10, 10)), Some((3, 4)));
        assert_eq!(clamp_tile((-5.0, 99.0), (10, 10)), Some((0, 9)));
        assert_eq!(clamp_tile((1.0, 1.0), (0, 0)), None);
    }

    #[test]
    fn cut_and_mine_drags_designate_only_matching_tiles() {
        let sim = sim_from_ascii("cT.R\n.TR.", 1).unwrap();
        let area = Area::from_corners((0, 0), (3, 1));
        let cut = commands_for_drag(Tool::Cut, area, |t, x, y| applies(&sim, t, x, y));
        assert_eq!(
            cut,
            vec![
                Command::Designate {
                    kind: JobKind::Cut,
                    x: 1,
                    y: 0
                },
                Command::Designate {
                    kind: JobKind::Cut,
                    x: 1,
                    y: 1
                },
            ]
        );
        let mine = commands_for_drag(Tool::Mine, area, |t, x, y| applies(&sim, t, x, y));
        assert_eq!(mine.len(), 2);
        assert_eq!(count_applicable(&sim, Tool::Mine, area), 2);
    }

    #[test]
    fn a_zone_drag_makes_one_stockpile_command_with_all_materials() {
        let area = Area::from_corners((4, 4), (2, 6));
        let cmds = commands_for_drag(Tool::Stockpile, area, |_, _, _| false);
        assert_eq!(
            cmds,
            vec![Command::CreateStockpile {
                x0: 2,
                y0: 4,
                x1: 4,
                y1: 6,
                filter: ItemFilter::All
            }]
        );
    }

    #[test]
    fn select_makes_no_command_and_cancel_makes_one_per_tile() {
        let area = Area::from_corners((0, 0), (1, 1));
        assert!(commands_for_drag(Tool::Select, area, |_, _, _| true).is_empty());
        let cmds = commands_for_drag(Tool::Cancel, area, |_, _, _| true);
        assert_eq!(cmds.len(), 4);
        assert_eq!(cmds[0], Command::CancelDesignation { x: 0, y: 0 });
    }

    #[test]
    fn cancel_applies_to_designated_tiles_and_haul_to_loose_items() {
        let mut sim = sim_from_ascii("cT.", 1).unwrap();
        assert!(!applies(&sim, Tool::Cancel, 1, 0));
        sim.step(&[Command::Designate {
            kind: JobKind::Cut,
            x: 1,
            y: 0,
        }]);
        assert!(applies(&sim, Tool::Cancel, 1, 0));
        assert!(!applies(&sim, Tool::Cut, 1, 0), "already designated");
        assert!(!applies(&sim, Tool::Haul, 2, 0));
        let log = sim.rules().log;
        sim.debug_spawn_item(log, 3, 2, 0);
        assert!(applies(&sim, Tool::Haul, 2, 0));
    }

    #[test]
    fn debug_orders_parse_with_and_without_a_tool_word() {
        assert_eq!(
            parse_order("mine:5,6,2,1", Tool::Select),
            Some((Tool::Mine, Area::from_corners((2, 1), (5, 6))))
        );
        assert_eq!(
            parse_order("1,2,3,4", Tool::Cut),
            Some((Tool::Cut, Area::from_corners((1, 2), (3, 4))))
        );
        assert_eq!(parse_order("dig:1,2,3,4", Tool::Cut), None);
        assert_eq!(parse_order("cut:1,2,3", Tool::Cut), None);
    }

    #[test]
    fn toggling_a_material_off_and_on_returns_to_all() {
        let all = [3, 5, 8];
        let off = toggle_material(&ItemFilter::All, 5, &all);
        assert_eq!(off, ItemFilter::Only(vec![3, 8]));
        assert!(!off.accepts(5));
        assert_eq!(toggle_material(&off, 5, &all), ItemFilter::All);
        let one = toggle_material(&ItemFilter::Only(vec![3]), 8, &all);
        assert_eq!(one, ItemFilter::Only(vec![3, 8]));
        assert_eq!(
            toggle_material(&ItemFilter::Only(vec![3]), 3, &all),
            ItemFilter::Only(vec![])
        );
    }

    #[test]
    fn haulable_materials_are_log_stone_and_ore() {
        let rules = SimRules::builtin();
        let list = haulable_materials(&rules);
        assert!(list.contains(&rules.log));
        assert_eq!(list.len(), 7);
        assert!(list.windows(2).all(|w| w[0] < w[1]));
    }

    #[test]
    fn priority_clicks_cycle_through_off_and_1_to_4() {
        let forward: Vec<u8> = (0..6)
            .scan(0, |p, _| {
                let v = *p;
                *p = next_priority(*p, false);
                Some(v)
            })
            .collect();
        assert_eq!(forward, vec![0, 1, 2, 3, 4, 0]);
        assert_eq!(next_priority(1, true), 0);
        assert_eq!(next_priority(0, true), 4);
        assert_eq!(next_priority(9, false), 0);
        assert_eq!(priority_text(0), "off");
        assert_eq!(priority_text(3), "3");
    }

    #[test]
    fn tool_keys_are_unique_and_every_tool_has_a_name() {
        for (i, a) in Tool::ALL.iter().enumerate() {
            assert!(!a.name().is_empty());
            for b in &Tool::ALL[i + 1..] {
                assert_ne!(a.key_label(), b.key_label());
            }
        }
        assert_eq!(Tool::from_word("ZONE"), Some(Tool::Stockpile));
        assert!(!Tool::Select.drags() && Tool::Cut.drags());
    }
}
