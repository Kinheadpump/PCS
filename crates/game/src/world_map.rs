//! The world map without a window: layer colors, the map rectangle, seed text, the facts of a
//! world cell, and the embark of a cell. `world_screen.rs` draws it. Everything here has tests.

use crate::palette::Rgb;
use crate::ui::Rect;
use worldgen::history::SiteKind;
use worldgen::{
    EmbarkSummary, History, LocalMapData, WORLD_SIZE, WorldBiome, WorldData, embark_from_world,
    generate_local_with_sites, is_habitable,
};

/// Side length of the world in cells.
const SIDE: i32 = WORLD_SIZE as i32;
/// Largest number of digits of a seed that the text box takes.
pub const SEED_DIGITS: usize = 19;
/// Color of water that a river adds.
const RIVER: Rgb = [70, 130, 220];
/// Color of a road on the history layer.
const ROAD: Rgb = [214, 190, 140];

/// What the map colors show.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layer {
    /// Biome of each cell, with rivers.
    Biome,
    /// Height above the sea.
    Height,
    /// Mean temperature.
    Temperature,
    /// Moisture.
    Moisture,
    /// The regions.
    Region,
    /// Civilization territory, roads, and sites.
    History,
}

impl Layer {
    /// All layers, in the order of the buttons.
    pub const ALL: [Layer; 6] = [
        Layer::Biome,
        Layer::Height,
        Layer::Temperature,
        Layer::Moisture,
        Layer::Region,
        Layer::History,
    ];

    /// Name for the button.
    pub fn name(self) -> &'static str {
        match self {
            Layer::Biome => "Biomes",
            Layer::Height => "Height",
            Layer::Temperature => "Temperature",
            Layer::Moisture => "Moisture",
            Layer::Region => "Regions",
            Layer::History => "History",
        }
    }

    /// The layer for a word of `PCS_LAYER`.
    pub fn from_word(word: &str) -> Option<Layer> {
        Layer::ALL
            .into_iter()
            .find(|l| l.name().eq_ignore_ascii_case(word.trim()))
    }
}

/// Color of a biome.
pub fn biome_color(b: WorldBiome) -> Rgb {
    match b {
        WorldBiome::Ocean => [30, 60, 140],
        WorldBiome::Lake => [70, 130, 220],
        WorldBiome::Glacier => [235, 245, 250],
        WorldBiome::HighMountain => [140, 135, 135],
        WorldBiome::Volcanic => [110, 40, 40],
        WorldBiome::Coast => [226, 210, 150],
        WorldBiome::Floodplain => [120, 170, 90],
        WorldBiome::Tundra => [170, 180, 160],
        WorldBiome::ColdDesert => [190, 185, 170],
        WorldBiome::Taiga => [50, 100, 80],
        WorldBiome::Steppe => [190, 180, 100],
        WorldBiome::Shrubland => [160, 150, 90],
        WorldBiome::Grassland => [120, 180, 80],
        WorldBiome::TemperateForest => [40, 130, 50],
        WorldBiome::Marsh => [80, 120, 100],
        WorldBiome::Desert => [225, 200, 120],
        WorldBiome::Savanna => [200, 170, 70],
        WorldBiome::TropicalForest => [30, 150, 70],
        WorldBiome::Rainforest => [10, 110, 60],
    }
}

/// Color of a site kind on the history layer. The same colors are in the legend.
pub fn site_color(kind: SiteKind) -> Rgb {
    match kind {
        SiteKind::Settlement => [255, 255, 255],
        SiteKind::Mine => [255, 150, 60],
        SiteKind::Battlefield => [235, 60, 60],
        SiteKind::Ruin => [190, 170, 220],
        SiteKind::Sealed => [150, 70, 200],
    }
}

/// Color of a road in the legend.
pub const ROAD_COLOR: Rgb = ROAD;

/// Name of a site kind for text.
pub fn site_name(kind: SiteKind) -> &'static str {
    match kind {
        SiteKind::Settlement => "Settlement",
        SiteKind::Mine => "Mine",
        SiteKind::Battlefield => "Battlefield",
        SiteKind::Ruin => "Ruin",
        SiteKind::Sealed => "Sealed site",
    }
}

/// A bright color for an ID (a region or a civilization). The same ID gives the same color.
pub fn id_color(id: u32) -> Rgb {
    let m = u64::from(id)
        .wrapping_add(1)
        .wrapping_mul(0x9E37_79B9_7F4A_7C15);
    let channel = |shift: u32| 70 + ((m >> shift) & 0xff) as u8 % 170;
    [channel(40), channel(24), channel(8)]
}

/// Mix `a` toward `b` by `percent` (0 to 100).
fn blend(a: Rgb, b: Rgb, percent: u32) -> Rgb {
    let mix =
        |x: u8, y: u8| ((u32::from(x) * (100 - percent) + u32::from(y) * percent) / 100) as u8;
    [mix(a[0], b[0]), mix(a[1], b[1]), mix(a[2], b[2])]
}

fn height_color(rel: i32) -> Rgb {
    if rel < 0 {
        let depth = (-rel).min(2000) as u32;
        [
            10,
            (30 + 60 - depth * 60 / 2000) as u8,
            (120 + 100 - depth * 100 / 2000) as u8,
        ]
    } else {
        let t = rel.min(3000) as u32 * 255 / 3000;
        [
            (60 + t * 190 / 255) as u8,
            (120 + t * 100 / 255) as u8,
            (50 + t * 200 / 255) as u8,
        ]
    }
}

fn temperature_color(t: i8) -> Rgb {
    let v = ((i32::from(t) + 30).clamp(0, 60) * 255 / 60) as u8;
    [v, 60, 255 - v]
}

fn moisture_color(m: u8) -> Rgb {
    blend([190, 150, 90], [40, 100, 200], u32::from(m) * 100 / 255)
}

/// The history layer colors of a cell: dim biome, owner tint, road, and site marker.
fn history_color(world: &WorldData, history: &History, x: i32, y: i32) -> Rgb {
    let biome = world.biome.get(x, y).copied().unwrap_or(WorldBiome::Ocean);
    let mut color = blend(biome_color(biome), [0, 0, 0], 55);
    let owner = history.owner.get(x, y).copied().unwrap_or(0);
    if owner > 0 {
        color = blend(color, id_color(u32::from(owner)), 55);
    }
    if history.road.get(x, y).copied().unwrap_or(0) > 0 {
        color = ROAD;
    }
    color
}

/// The color of one cell on a layer.
pub fn cell_color(world: &WorldData, history: &History, layer: Layer, x: i32, y: i32) -> Rgb {
    let biome = world.biome.get(x, y).copied().unwrap_or(WorldBiome::Ocean);
    match layer {
        Layer::Biome => {
            let river = u32::from(world.river.get(x, y).copied().unwrap_or(0));
            let land = !matches!(biome, WorldBiome::Ocean | WorldBiome::Lake);
            let base = biome_color(biome);
            if land && river > 0 {
                blend(base, RIVER, 30 + 20 * river)
            } else {
                base
            }
        }
        Layer::Height => height_color(world.height_above_sea(x, y)),
        Layer::Temperature => temperature_color(world.temperature.get(x, y).copied().unwrap_or(0)),
        Layer::Moisture => moisture_color(world.moisture.get(x, y).copied().unwrap_or(0)),
        Layer::Region => match world.regions.at(x, y) {
            0 => biome_color(WorldBiome::Ocean),
            id => id_color(u32::from(id)),
        },
        Layer::History => history_color(world, history, x, y),
    }
}

/// RGBA bytes of the whole map on a layer, row by row. On the history layer a site is one cell
/// in the color of its kind.
pub fn layer_pixels(world: &WorldData, history: &History, layer: Layer) -> Vec<u8> {
    let mut sites: Vec<(usize, Rgb)> = Vec::new();
    if layer == Layer::History {
        // Later sites in the list draw over earlier ones. A living settlement draws last.
        let mut ordered: Vec<_> = history.sites.iter().collect();
        ordered.sort_by_key(|s| (s.kind == SiteKind::Settlement, s.id));
        for s in ordered {
            let (x, y) = (i32::from(s.x), i32::from(s.y));
            if (0..SIDE).contains(&x) && (0..SIDE).contains(&y) {
                sites.push(((y * SIDE + x) as usize, site_color(s.kind)));
            }
        }
    }
    let mut bytes = Vec::with_capacity((SIDE * SIDE * 4) as usize);
    for y in 0..SIDE {
        for x in 0..SIDE {
            let c = cell_color(world, history, layer, x, y);
            bytes.extend_from_slice(&[c[0], c[1], c[2], 255]);
        }
    }
    for (i, c) in sites {
        bytes[i * 4..i * 4 + 3].copy_from_slice(&c);
    }
    bytes
}

/// Where the world map is on the screen: the corner and the screen pixels per cell.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MapRect {
    /// Left edge.
    pub x: f32,
    /// Top edge.
    pub y: f32,
    /// Screen pixels per cell. A whole number, so the pixels stay square.
    pub scale: f32,
}

impl MapRect {
    /// The largest whole-number scale that fits in `avail`, centered in it.
    pub fn fit(avail: Rect) -> Self {
        let scale = (avail.w.min(avail.h) / WORLD_SIZE as f32).floor().max(1.0);
        let side = scale * WORLD_SIZE as f32;
        Self {
            x: (avail.x + (avail.w - side) / 2.0).round(),
            y: (avail.y + (avail.h - side) / 2.0).round(),
            scale,
        }
    }

    /// Side length on the screen.
    pub fn side(&self) -> f32 {
        self.scale * WORLD_SIZE as f32
    }

    /// The cell under a screen position, or `None` outside the map.
    pub fn cell_at(&self, p: (f32, f32)) -> Option<(i32, i32)> {
        let cx = ((p.0 - self.x) / self.scale).floor();
        let cy = ((p.1 - self.y) / self.scale).floor();
        let inside = |v: f32| (0.0..WORLD_SIZE as f32).contains(&v);
        (inside(cx) && inside(cy)).then_some((cx as i32, cy as i32))
    }

    /// Screen rectangle of a cell.
    pub fn cell_rect(&self, cell: (i32, i32)) -> Rect {
        Rect::new(
            self.x + cell.0 as f32 * self.scale,
            self.y + cell.1 as f32 * self.scale,
            self.scale,
            self.scale,
        )
    }
}

/// Read a seed from the text of the seed box. Empty or too large text gives `None`.
pub fn parse_seed(text: &str) -> Option<u64> {
    let text = text.trim();
    if text.is_empty() || text.len() > SEED_DIGITS || !text.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    text.parse().ok()
}

/// Apply typed characters to the text of the seed box. Only digits count.
pub fn edit_seed_text(text: &mut String, typed: &[char], backspace: bool) {
    if backspace {
        text.pop();
    }
    for c in typed.iter().filter(|c| c.is_ascii_digit()) {
        if text.len() < SEED_DIGITS {
            text.push(*c);
        }
    }
}

/// The lines of the cell panel: what the world has in the cell.
pub fn cell_lines(world: &WorldData, history: &History, x: i32, y: i32) -> Vec<String> {
    let Some(&biome) = world.biome.get(x, y) else {
        return Vec::new();
    };
    let river = match world.river.get(x, y).copied().unwrap_or(0) {
        1 => "small river",
        2 => "river",
        3 => "large river",
        _ => "none",
    };
    let mut lines = vec![
        format!("Cell {x}, {y}"),
        format!("Biome: {}", biome.name()),
        format!("Height: {} m", world.height_above_sea(x, y)),
        format!(
            "Temperature: {}{}C",
            world.temperature.get(x, y).copied().unwrap_or(0),
            crate::font::DEGREE_SIGN
        ),
        format!(
            "Moisture: {}",
            world.moisture.get(x, y).copied().unwrap_or(0)
        ),
        format!("Water: {river}"),
    ];
    match world.regions.at(x, y) {
        0 => {}
        id => lines.push(format!("Region: {id}")),
    }
    match history.owner.get(x, y).copied().unwrap_or(0) {
        0 => {}
        id => lines.push(format!("Territory of civilization {id}")),
    }
    match embark_from_world(world, history, x, y) {
        None => lines.push("Cannot embark here".to_owned()),
        Some((_, sites)) => {
            if sites.has_history() {
                lines.push(format!("History in the area: {} sites", sites.site_count));
            } else {
                lines.push("No history in the area".to_owned());
            }
            for s in sites.sites.iter().take(4) {
                lines.push(format!("  {}, {} years", site_name(s.kind), s.age));
            }
            if sites.road_near {
                lines.push("  Old road".to_owned());
            }
        }
    }
    lines
}

/// Make the local map of a world cell: the summary, then the map with the stamped sites. `None`
/// when the player cannot embark on the cell.
pub fn embark_map(
    world: &WorldData,
    history: &History,
    x: i32,
    y: i32,
) -> Option<(EmbarkSummary, LocalMapData)> {
    let (summary, sites) = embark_from_world(world, history, x, y)?;
    let map = generate_local_with_sites(&summary, &sites);
    Some((summary, map))
}

/// The yearly temperature range (tenths of a degree) and moisture (0 to 255) of a cell, for the
/// climate of the sim.
pub fn cell_climate(world: &WorldData, x: i32, y: i32) -> (i16, u8) {
    let amplitude = i16::from(world.temp_amp.get(x, y).copied().unwrap_or(0)) * 10;
    (amplitude, world.moisture.get(x, y).copied().unwrap_or(0))
}

/// True if a cell can be the embark cell.
pub fn can_embark(world: &WorldData, x: i32, y: i32) -> bool {
    is_habitable(world, x, y)
}

#[cfg(test)]
mod tests {
    use super::*;
    use worldgen::{LOCAL_SIZE, WorldParams, generate_history, generate_world};

    fn world(seed: u64) -> (WorldData, History) {
        let w = generate_world(seed, &WorldParams::default());
        let h = generate_history(&w, None).expect("tables");
        (w, h)
    }

    #[test]
    fn layer_pixels_cover_the_whole_world_on_every_layer() {
        let (w, h) = world(1);
        for layer in Layer::ALL {
            let bytes = layer_pixels(&w, &h, layer);
            assert_eq!(bytes.len(), (WORLD_SIZE * WORLD_SIZE * 4) as usize);
            assert!(bytes.chunks(4).all(|p| p[3] == 255));
        }
    }

    #[test]
    fn the_biome_layer_shows_the_biome_colors_and_rivers() {
        let (w, h) = world(1);
        let mut rivers = 0;
        for y in 0..SIDE {
            for x in 0..SIDE {
                let biome = *w.biome.get(x, y).expect("cell");
                let c = cell_color(&w, &h, Layer::Biome, x, y);
                let river = *w.river.get(x, y).expect("cell") > 0;
                let land = !matches!(biome, WorldBiome::Ocean | WorldBiome::Lake);
                if land && river {
                    rivers += 1;
                    assert_ne!(c, biome_color(biome));
                } else {
                    assert_eq!(c, biome_color(biome));
                }
            }
        }
        assert!(rivers > 50, "{rivers} river cells");
    }

    #[test]
    fn the_biome_colors_are_all_different() {
        for (i, a) in WorldBiome::ALL.iter().enumerate() {
            for b in &WorldBiome::ALL[i + 1..] {
                assert_ne!(
                    biome_color(*a),
                    biome_color(*b),
                    "{} {}",
                    a.name(),
                    b.name()
                );
            }
        }
    }

    #[test]
    fn the_history_layer_marks_each_site_in_the_color_of_its_kind() {
        let (w, h) = world(2);
        assert!(!h.sites.is_empty());
        let bytes = layer_pixels(&w, &h, Layer::History);
        let at = |x: u16, y: u16| {
            let i = (usize::from(y) * WORLD_SIZE as usize + usize::from(x)) * 4;
            [bytes[i], bytes[i + 1], bytes[i + 2]]
        };
        for s in &h.sites {
            let here: Vec<_> = h
                .sites
                .iter()
                .filter(|o| o.x == s.x && o.y == s.y)
                .collect();
            let top = here
                .iter()
                .max_by_key(|o| (o.kind == SiteKind::Settlement, o.id))
                .expect("site");
            assert_eq!(at(s.x, s.y), site_color(top.kind));
        }
    }

    #[test]
    fn the_history_layer_shows_roads_and_territory() {
        let (w, h) = world(1);
        let mut road = 0;
        let mut owned = 0;
        for y in 0..SIDE {
            for x in 0..SIDE {
                let c = cell_color(&w, &h, Layer::History, x, y);
                let base = blend(
                    biome_color(*w.biome.get(x, y).expect("cell")),
                    [0, 0, 0],
                    55,
                );
                if *h.road.get(x, y).expect("cell") > 0 {
                    road += 1;
                    assert_eq!(c, ROAD);
                } else if *h.owner.get(x, y).expect("cell") > 0 {
                    owned += 1;
                    assert_ne!(c, base);
                } else {
                    assert_eq!(c, base);
                }
            }
        }
        assert!(road > 0 && owned > 0, "road {road} owned {owned}");
    }

    #[test]
    fn ids_get_stable_and_different_colors() {
        assert_eq!(id_color(5), id_color(5));
        let colors: std::collections::BTreeSet<Rgb> = (1..60).map(id_color).collect();
        assert!(colors.len() >= 55);
        assert!(colors.iter().all(|c| c.iter().all(|&v| v >= 70)));
    }

    #[test]
    fn the_map_rect_uses_a_whole_scale_and_finds_cells() {
        let avail = Rect::new(200.0, 50.0, 820.0, 652.0);
        let m = MapRect::fit(avail);
        assert_eq!(m.scale, 2.0);
        assert_eq!(m.side(), 512.0);
        assert!(m.x >= avail.x && m.x + m.side() <= avail.right());
        assert!(m.y >= avail.y && m.y + m.side() <= avail.bottom());
        assert_eq!(m.cell_at((m.x + 1.0, m.y + 1.0)), Some((0, 0)));
        assert_eq!(m.cell_at((m.x + 3.0, m.y + 5.0)), Some((1, 2)));
        assert_eq!(m.cell_at((m.x - 1.0, m.y + 1.0)), None);
        assert_eq!(m.cell_at((m.x + m.side(), m.y)), None);
        let r = m.cell_rect((3, 4));
        assert_eq!(m.cell_at((r.x + 0.5, r.y + 0.5)), Some((3, 4)));
        // A small area still gives scale 1.
        assert_eq!(MapRect::fit(Rect::new(0.0, 0.0, 100.0, 100.0)).scale, 1.0);
    }

    #[test]
    fn seed_text_takes_digits_only() {
        let mut t = String::new();
        edit_seed_text(&mut t, &['4', 'x', '2', ' ', '-', '7'], false);
        assert_eq!(t, "427");
        edit_seed_text(&mut t, &[], true);
        assert_eq!(t, "42");
        edit_seed_text(&mut t, &['9'; 40], false);
        assert_eq!(t.len(), SEED_DIGITS);
        assert_eq!(parse_seed("42"), Some(42));
        assert_eq!(parse_seed(" 7 "), Some(7));
        assert_eq!(parse_seed(""), None);
        assert_eq!(parse_seed("-3"), None);
        assert_eq!(parse_seed("1a"), None);
        // The box takes 19 digits at most, so u64::MAX (20 digits) is not a seed.
        assert_eq!(parse_seed(&"9".repeat(19)), Some(9_999_999_999_999_999_999));
        assert_eq!(parse_seed("18446744073709551615"), None);
        assert_eq!(
            parse_seed("1844674407370955161"),
            Some(1_844_674_407_370_955_161)
        );
    }

    #[test]
    fn the_cell_panel_tells_what_the_cell_has() {
        let (w, h) = world(1);
        let mut land = None;
        let mut sea = None;
        for y in 0..SIDE {
            for x in 0..SIDE {
                if can_embark(&w, x, y) && land.is_none() {
                    land = Some((x, y));
                }
                if !is_habitable(&w, x, y) && sea.is_none() {
                    sea = Some((x, y));
                }
            }
        }
        let (lx, ly) = land.expect("land");
        let lines = cell_lines(&w, &h, lx, ly);
        assert!(lines[0].starts_with("Cell "));
        assert!(lines.iter().any(|l| l.starts_with("Biome: ")));
        assert!(
            lines
                .iter()
                .any(|l| l.contains("history") || l.contains("History"))
        );
        assert!(!lines.iter().any(|l| l.contains("Cannot embark")));
        let (sx, sy) = sea.expect("water or ice");
        assert!(
            cell_lines(&w, &h, sx, sy)
                .iter()
                .any(|l| l.contains("Cannot embark"))
        );
        assert!(cell_lines(&w, &h, -1, 0).is_empty());
    }

    #[test]
    fn embarking_makes_a_stamped_local_map_with_a_site() {
        let (w, h) = world(1);
        let mut made = 0;
        for y in (0..SIDE).step_by(29) {
            for x in (0..SIDE).step_by(29) {
                match embark_map(&w, &h, x, y) {
                    Some((summary, map)) => {
                        made += 1;
                        assert_eq!(map.ground.width(), LOCAL_SIZE);
                        assert_eq!(map.ground.height(), LOCAL_SIZE);
                        assert!(!map.placed.is_empty(), "R-041 at {x},{y}");
                        assert_eq!(
                            summary.mean_temperature,
                            i16::from(*w.temperature.get(x, y).expect("cell")) * 10
                        );
                    }
                    None => assert!(!can_embark(&w, x, y)),
                }
            }
        }
        assert!(made > 10);
    }

    #[test]
    fn the_cell_climate_comes_from_the_world() {
        let (w, _) = world(1);
        let (amp, moisture) = cell_climate(&w, 100, 100);
        assert_eq!(
            amp,
            i16::from(*w.temp_amp.get(100, 100).expect("cell")) * 10
        );
        assert_eq!(moisture, *w.moisture.get(100, 100).expect("cell"));
    }

    #[test]
    fn layer_words_find_the_layer() {
        assert_eq!(Layer::from_word("history"), Some(Layer::History));
        assert_eq!(Layer::from_word(" Biomes "), Some(Layer::Biome));
        assert_eq!(Layer::from_word("nothing"), None);
    }
}
