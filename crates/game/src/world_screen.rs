//! The world map screen (Sprint 9 and 11): the world with its layers, a seed box, a cell panel,
//! and the "Embark here" button. The pure parts are in `world_map.rs`. This file draws and keeps
//! the state of the screen.

use crate::camera::View;
use crate::input::Typed;
use crate::overlay::SELECT;
use crate::palette::Rgb;
use crate::render::color as rgb_color;
use crate::ui::{self, BUTTON_H, GAP, LINE_H, PAD, Rect, Ui, draw_frame};
use crate::world_map::{
    self as map, Layer, MapRect, ROAD_COLOR, biome_color, edit_seed_text, parse_seed, site_color,
};
use macroquad::prelude::{
    DrawTextureParams, FilterMode, Image, Texture2D, WHITE, draw_rectangle, draw_rectangle_lines,
    draw_texture_ex, mouse_position, vec2,
};
use worldgen::history::SiteKind;
use worldgen::{
    EmbarkSummary, History, LocalMapData, WORLD_SIZE, WorldBiome, WorldData, WorldParams,
    generate_history, generate_world,
};

/// Height of the bar at the top.
const TOP_H: f32 = 44.0;
/// Width of the layer panel.
const LEFT_W: f32 = 190.0;
/// Width of the cell panel.
const RIGHT_W: f32 = 300.0;
/// Width of the seed box.
const SEED_W: f32 = 220.0;
/// Width of the small buttons next to the seed box.
const STEP_W: f32 = 40.0;

/// What the player chose on this screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorldAction {
    /// Go back to the colony.
    Back,
    /// Start a colony in this world cell.
    Embark(i32, i32),
}

/// The result of an embark: the local map and the values the sim needs.
pub struct Embarked {
    /// The embark summary of the cell.
    pub summary: EmbarkSummary,
    /// The local map with the stamped sites.
    pub map: LocalMapData,
    /// Half of the yearly temperature range, in tenths of a degree.
    pub amplitude: i16,
    /// Moisture 0 to 255.
    pub moisture: u8,
    /// Short name for the status line.
    pub name: String,
}

/// The rectangles of the screen.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Layout {
    /// The bar at the top.
    pub top: Rect,
    /// The layer panel.
    pub left: Rect,
    /// The cell panel.
    pub right: Rect,
    /// The world map.
    pub map: MapRect,
}

impl Layout {
    /// Place the panels and the map for a window size.
    pub fn compute(view: View) -> Self {
        let top = Rect::new(0.0, 0.0, view.0, TOP_H);
        let body_y = TOP_H + PAD;
        let body_h = (view.1 - body_y - PAD).max(0.0);
        let left = Rect::new(PAD, body_y, LEFT_W, body_h);
        let right = Rect::new((view.0 - PAD - RIGHT_W).max(0.0), body_y, RIGHT_W, body_h);
        let avail = Rect::new(
            left.right() + PAD,
            body_y,
            (right.x - PAD - left.right() - PAD).max(0.0),
            body_h,
        );
        Self {
            top,
            left,
            right,
            map: MapRect::fit(avail),
        }
    }
}

/// One line of a legend: a color (none for a plain line) and a text.
pub type LegendLine = (Option<Rgb>, String);

/// The legend of a layer.
pub fn legend(layer: Layer) -> Vec<LegendLine> {
    let swatch = |c: Rgb, t: &str| (Some(c), t.to_owned());
    match layer {
        Layer::Biome => {
            let mut lines: Vec<LegendLine> = WorldBiome::ALL
                .iter()
                .map(|&b| (Some(biome_color(b)), capitalize(b.name())))
                .collect();
            lines.push((None, "Rivers are blue lines".to_owned()));
            lines
        }
        Layer::Height => vec![
            swatch([10, 60, 220], "Deep sea"),
            swatch([10, 90, 190], "Shallow sea"),
            swatch([60, 120, 50], "Low land"),
            swatch([155, 170, 150], "Hills"),
            swatch([250, 220, 250], "High mountains"),
        ],
        Layer::Temperature => vec![
            swatch([0, 60, 255], "Cold"),
            swatch([128, 60, 127], "Mild"),
            swatch([255, 60, 0], "Hot"),
        ],
        Layer::Moisture => vec![
            swatch([190, 150, 90], "Dry"),
            swatch([115, 125, 145], "Medium"),
            swatch([40, 100, 200], "Wet"),
        ],
        Layer::Region => vec![(None, "Each region has its own color".to_owned())],
        Layer::History => {
            let mut lines: Vec<LegendLine> = SiteKind::ALL
                .iter()
                .map(|&k| (Some(site_color(k)), map::site_name(k).to_owned()))
                .collect();
            lines.push(swatch(ROAD_COLOR, "Road"));
            lines.push((None, "Dark tint: territory".to_owned()));
            lines
        }
    }
}

fn capitalize(text: &str) -> String {
    let mut chars = text.chars();
    chars
        .next()
        .map(|c| c.to_uppercase().chain(chars).collect())
        .unwrap_or_default()
}

fn texture_for(world: &WorldData, history: &History, layer: Layer) -> Texture2D {
    let image = Image {
        bytes: map::layer_pixels(world, history, layer),
        width: WORLD_SIZE as u16,
        height: WORLD_SIZE as u16,
    };
    let texture = Texture2D::from_image(&image);
    texture.set_filter(FilterMode::Nearest);
    texture
}

/// The state of the world map screen.
pub struct WorldScreen {
    seed: u64,
    seed_text: String,
    editing: bool,
    world: WorldData,
    history: History,
    layer: Layer,
    texture: Texture2D,
    selected: Option<(i32, i32)>,
    lines: Vec<String>,
}

impl WorldScreen {
    /// Make the world of a seed. Needs a running window. The error is the content error text.
    pub fn new(seed: u64) -> Result<Self, String> {
        let (world, history) = Self::generate(seed)?;
        let texture = texture_for(&world, &history, Layer::Biome);
        Ok(Self {
            seed,
            seed_text: seed.to_string(),
            editing: false,
            world,
            history,
            layer: Layer::Biome,
            texture,
            selected: None,
            lines: Vec::new(),
        })
    }

    fn generate(seed: u64) -> Result<(WorldData, History), String> {
        let world = generate_world(seed, &WorldParams::default());
        let history = generate_history(&world, None).map_err(|e| e.to_string())?;
        Ok((world, history))
    }

    /// Show another layer.
    pub fn set_layer(&mut self, layer: Layer) {
        if layer != self.layer {
            self.layer = layer;
            self.texture = texture_for(&self.world, &self.history, layer);
        }
    }

    /// Select a cell and fill the cell panel.
    pub fn select(&mut self, cell: Option<(i32, i32)>) {
        self.selected = cell;
        self.lines = cell.map_or_else(Vec::new, |(x, y)| {
            map::cell_lines(&self.world, &self.history, x, y)
        });
    }

    /// Make the world of another seed. The selection ends. A content error keeps the old world.
    pub fn set_seed(&mut self, seed: u64) {
        self.seed_text = seed.to_string();
        if seed == self.seed {
            return;
        }
        if let Ok((world, history)) = Self::generate(seed) {
            self.seed = seed;
            self.world = world;
            self.history = history;
            self.texture = texture_for(&self.world, &self.history, self.layer);
            self.select(None);
        }
    }

    /// Take the text of the seed box. Text that is not a seed returns to the current seed.
    fn apply_seed_text(&mut self) {
        match parse_seed(&self.seed_text) {
            Some(seed) => self.set_seed(seed),
            None => self.seed_text = self.seed.to_string(),
        }
    }

    /// The Escape key: stop editing, or leave the screen. Returns true to leave.
    pub fn on_close(&mut self) -> bool {
        if self.editing {
            self.editing = false;
            self.seed_text = self.seed.to_string();
            false
        } else {
            true
        }
    }

    /// The local map and the climate values of a cell, or `None` if the player cannot embark.
    pub fn embark(&self, x: i32, y: i32) -> Option<Embarked> {
        let (summary, map) = map::embark_map(&self.world, &self.history, x, y)?;
        let (amplitude, moisture) = map::cell_climate(&self.world, x, y);
        let biome = self.world.biome.get(x, y).map_or("land", |b| b.name());
        Some(Embarked {
            summary,
            map,
            amplitude,
            moisture,
            name: format!("world {} cell {x},{y} {biome}", self.seed),
        })
    }

    /// Draw the screen and read its widgets. `click` is the position of a left click.
    pub fn draw(
        &mut self,
        view: View,
        click: Option<(f32, f32)>,
        typed: &Typed,
    ) -> Option<WorldAction> {
        let mouse = mouse_position();
        let layout = Layout::compute(view);
        let mut action = None;
        if self.editing {
            edit_seed_text(&mut self.seed_text, &typed.chars, typed.backspace);
            if typed.enter {
                self.editing = false;
                self.apply_seed_text();
            }
        }
        crate::render::clear();
        self.draw_map(&layout, mouse);
        let mut ui = Ui::new(mouse, click);
        let clicked_box = self.draw_top(&mut ui, &layout, click, &mut action);
        self.draw_layers(&mut ui, &layout);
        self.draw_cell_panel(&mut ui, &layout, &mut action);
        ui.finish(view);
        if let Some(c) = click {
            if clicked_box {
                self.editing = true;
            } else if !ui.captures(c) {
                if self.editing {
                    self.editing = false;
                    self.apply_seed_text();
                }
                if let Some(cell) = layout.map.cell_at(c) {
                    self.select(Some(cell));
                }
            } else if self.editing {
                self.editing = false;
                self.apply_seed_text();
            }
        }
        action
    }

    fn draw_map(&self, layout: &Layout, mouse: (f32, f32)) {
        let m = layout.map;
        draw_texture_ex(
            &self.texture,
            m.x,
            m.y,
            WHITE,
            DrawTextureParams {
                dest_size: Some(vec2(m.side(), m.side())),
                ..Default::default()
            },
        );
        if let Some(cell) = m.cell_at(mouse) {
            let r = m.cell_rect(cell);
            draw_rectangle(
                r.x,
                r.y,
                r.w,
                r.h,
                macroquad::prelude::Color::new(1.0, 1.0, 1.0, 0.3),
            );
        }
        if let Some(cell) = self.selected {
            let r = m.cell_rect(cell);
            draw_rectangle_lines(r.x - 2.0, r.y - 2.0, r.w + 4.0, r.h + 4.0, 2.0, SELECT);
        }
    }

    /// Draw the top bar. Returns true if the click hit the seed box.
    fn draw_top(
        &mut self,
        ui: &mut Ui,
        layout: &Layout,
        click: Option<(f32, f32)>,
        action: &mut Option<WorldAction>,
    ) -> bool {
        ui.bar(layout.top);
        let y = (TOP_H - BUTTON_H) / 2.0 - 2.0;
        let back = Rect::new(PAD, y, ui::text_px("Back to colony") + 2.0 * PAD, BUTTON_H);
        if ui.button(back, "Back to colony", false) {
            *action = Some(WorldAction::Back);
        }
        let mut x = back.right() + 2.0 * PAD;
        ui.label_color((x, y + 8.0), "World map", ui::accent());
        x += ui::text_px("World map") + 2.0 * PAD;
        ui.label((x, y + 8.0), "Seed");
        x += ui::text_px("Seed") + PAD;
        let boxr = Rect::new(x, y, SEED_W, BUTTON_H);
        draw_frame(
            boxr,
            rgb_color(crate::palette::ui::INK, 255),
            rgb_color(crate::palette::ui::PANEL_LIGHT, 255),
            rgb_color(crate::palette::ui::PANEL_DARK, 255),
            rgb_color(crate::palette::ui::INK, 255),
            true,
        );
        let blink = ((macroquad::prelude::get_time() * 2.0) as u32).is_multiple_of(2);
        let text = if self.editing && blink {
            format!("{}_", self.seed_text)
        } else {
            self.seed_text.clone()
        };
        ui.label((boxr.x + 8.0, boxr.y + 8.0), &text);
        ui.tip(boxr, "Seed of the world\nClick, type digits, press Enter");
        let hit_box = click.is_some_and(|c| boxr.contains(c));
        x = boxr.right() + GAP;
        let prev = Rect::new(x, y, STEP_W, BUTTON_H);
        let next = Rect::new(x + STEP_W + GAP, y, STEP_W, BUTTON_H);
        if ui.button(prev, "-", false) {
            self.set_seed(self.seed.saturating_sub(1));
            self.editing = false;
        }
        if ui.button(next, "+", false) {
            self.set_seed(self.seed.saturating_add(1));
            self.editing = false;
        }
        hit_box
    }

    fn draw_layers(&mut self, ui: &mut Ui, layout: &Layout) {
        let panel = layout.left;
        ui.panel(panel);
        let mut y = panel.y + PAD;
        ui.label_color((panel.x + PAD, y), "Layers", ui::accent());
        y += LINE_H + GAP;
        for layer in Layer::ALL {
            let r = Rect::new(panel.x + PAD, y, panel.w - 2.0 * PAD, BUTTON_H);
            if ui.chip(r, layer.name(), None, layer == self.layer) {
                self.set_layer(layer);
            }
            y += BUTTON_H + GAP;
        }
        y += PAD / 2.0;
        ui.rule(panel.x + PAD, y, panel.w - 2.0 * PAD);
        y += PAD;
        for (color, text) in legend(self.layer) {
            if y + LINE_H > panel.bottom() {
                break;
            }
            let mut tx = panel.x + PAD;
            if let Some(c) = color {
                draw_rectangle(
                    tx,
                    y + 2.0,
                    12.0,
                    12.0,
                    rgb_color(crate::palette::OUTLINE, 255),
                );
                draw_rectangle(tx + 2.0, y + 4.0, 8.0, 8.0, rgb_color(c, 255));
                tx += 20.0;
            }
            ui.label((tx, y), &text);
            y += LINE_H - 2.0;
        }
    }

    fn draw_cell_panel(&mut self, ui: &mut Ui, layout: &Layout, action: &mut Option<WorldAction>) {
        let panel = layout.right;
        ui.panel(panel);
        let mut y = panel.y + PAD;
        if self.lines.is_empty() {
            ui.label_color((panel.x + PAD, y), "No cell selected", ui::dim());
            y += LINE_H;
            ui.label((panel.x + PAD, y), "Click a cell on the map.");
            return;
        }
        for (i, line) in self.lines.iter().enumerate() {
            if y + LINE_H > panel.bottom() - BUTTON_H - 2.0 * PAD {
                break;
            }
            if i == 0 {
                ui.label_color((panel.x + PAD, y), line, ui::accent());
            } else {
                ui.label((panel.x + PAD, y), line);
            }
            y += LINE_H;
        }
        let Some((x, cy)) = self.selected else {
            return;
        };
        let r = Rect::new(
            panel.x + PAD,
            panel.bottom() - BUTTON_H - PAD,
            panel.w - 2.0 * PAD,
            BUTTON_H,
        );
        if map::can_embark(&self.world, x, cy) {
            if ui.button(r, "Embark here", true) {
                *action = Some(WorldAction::Embark(x, cy));
            }
        } else {
            ui.label_color((r.x, r.y + 8.0), "Pick land to embark", ui::dim());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_layout_fits_common_window_sizes() {
        for view in [(1280.0, 720.0), (1920.0, 1080.0), (1024.0, 600.0)] {
            let l = Layout::compute(view);
            let m = l.map;
            assert!(m.x >= l.left.right(), "{view:?}: map over the layer panel");
            assert!(
                m.x + m.side() <= l.right.x,
                "{view:?}: map under the cell panel"
            );
            assert!(
                m.y >= l.top.bottom() && m.y + m.side() <= view.1,
                "{view:?}"
            );
            assert!(m.scale >= 1.0 && m.scale.fract() == 0.0);
            assert!(!l.left.intersects(&l.right));
        }
        assert_eq!(Layout::compute((1280.0, 720.0)).map.scale, 2.0);
        assert_eq!(Layout::compute((1920.0, 1080.0)).map.scale, 3.0);
    }

    #[test]
    fn each_layer_has_a_legend() {
        for layer in Layer::ALL {
            assert!(!legend(layer).is_empty());
        }
        let biomes = legend(Layer::Biome);
        assert_eq!(
            biomes.iter().filter(|(c, _)| c.is_some()).count(),
            WorldBiome::COUNT
        );
        assert_eq!(biomes[0].1, "Ocean");
        let history = legend(Layer::History);
        assert!(history.iter().any(|(_, t)| t == "Road"));
        assert_eq!(
            history.iter().filter(|(c, _)| c.is_some()).count(),
            SiteKind::COUNT + 1
        );
    }
}
