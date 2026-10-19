//! The game: window, rendering, input, and interface.

mod camera;
mod input;
mod render;
mod sprites;

use camera::{Camera, View};
use input::{Action, InputState};
use macroquad::prelude::{Conf, get_frame_time, get_screen_data, mouse_position, next_frame};
use render::{MapView, Palette};
use sprites::Sheet;
use worldgen::{EmbarkSummary, LocalMapData, generate_local};

/// Embark presets on keys 1, 2, 3.
type PresetFn = fn() -> EmbarkSummary;
const PRESETS: [(&str, PresetFn); 3] = [
    ("temperate forest", EmbarkSummary::temperate_forest),
    ("cold hills", EmbarkSummary::cold_hills),
    ("dry plain", EmbarkSummary::dry_plain),
];
/// Seed for the preset maps.
const SEED: u64 = 1;
/// Keyboard pan speed in screen pixels per second.
const PAN_PIXELS_PER_SECOND: f32 = 700.0;
/// Zoom factor of one zoom step.
const ZOOM_STEP: f32 = 1.25;
/// Zoom at start, in screen pixels per tile.
const START_ZOOM: f32 = 16.0;

fn window_conf() -> Conf {
    Conf {
        window_title: "Ruinstead".to_owned(),
        window_width: 1280,
        window_height: 720,
        ..Default::default()
    }
}

/// A loaded map with its textures.
struct Loaded {
    map: LocalMapData,
    view: MapView,
}

struct App {
    palette: Palette,
    sheet: Sheet,
    grid: bool,
    loaded: Loaded,
    preset: usize,
    camera: Camera,
}

impl App {
    fn load_preset(palette: &Palette, sheet: &Sheet, preset: usize) -> Loaded {
        let embark = PRESETS[preset % PRESETS.len()].1().with_seed(SEED);
        let map = generate_local(&embark);
        let view = MapView::new(&map, palette, sheet);
        Loaded { map, view }
    }

    fn new(palette: Palette, sheet: Sheet) -> Self {
        let loaded = Self::load_preset(&palette, &sheet, 0);
        let size = (loaded.map.ground.width(), loaded.map.ground.height());
        Self {
            palette,
            sheet,
            grid: false,
            loaded,
            preset: 0,
            camera: Camera::new(size, START_ZOOM),
        }
    }

    fn map_size(&self) -> (u32, u32) {
        (
            self.loaded.map.ground.width(),
            self.loaded.map.ground.height(),
        )
    }

    /// Apply the actions of one frame. Returns `false` when the game must close.
    fn apply(&mut self, actions: &[Action], view: View, dt: f32) -> bool {
        let size = self.map_size();
        let center = (view.0 / 2.0, view.1 / 2.0);
        let step = PAN_PIXELS_PER_SECOND * dt / self.camera.zoom;
        for action in actions {
            match *action {
                Action::PanUp => self.camera.pan_tiles(0.0, -step, size),
                Action::PanDown => self.camera.pan_tiles(0.0, step, size),
                Action::PanLeft => self.camera.pan_tiles(-step, 0.0, size),
                Action::PanRight => self.camera.pan_tiles(step, 0.0, size),
                Action::DragPan(dx, dy) => self.camera.pan_pixels(dx, dy, size),
                Action::ZoomIn => self.camera.zoom_at(ZOOM_STEP, center, view, size),
                Action::ZoomOut => self.camera.zoom_at(1.0 / ZOOM_STEP, center, view, size),
                Action::WheelZoom { steps, x, y } => {
                    self.camera
                        .zoom_at(ZOOM_STEP.powf(steps), (x, y), view, size);
                }
                Action::Preset(n) if n < PRESETS.len() && n != self.preset => {
                    self.preset = n;
                    self.loaded = Self::load_preset(&self.palette, &self.sheet, n);
                }
                Action::ToggleGrid => self.grid = !self.grid,
                Action::Preset(_) => {}
                Action::Close => return false,
            }
        }
        true
    }

    fn draw(&self, view: View) {
        render::clear();
        let size = self.map_size();
        self.loaded.view.draw(&self.camera, size);
        if self.grid {
            MapView::draw_grid(&self.camera, size);
        }
        let hover = match self.camera.tile_at(mouse_position(), view, size) {
            Some((x, y)) => {
                let map = &self.loaded.map;
                format!(
                    "tile ({x}, {y}) ground {:?} rock {:?} ore {:?} tree {}",
                    map.ground.get(x, y),
                    map.rock.get(x, y),
                    map.ore.get(x, y),
                    map.tree.get(x, y).copied().unwrap_or(false)
                )
            }
            None => "tile (-, -)".to_owned(),
        };
        render::draw_overlay(&[
            format!(
                "FPS {}   zoom {:.1} px/tile   preset {}",
                render::fps(),
                self.camera.zoom,
                PRESETS[self.preset].0
            ),
            hover,
            "WASD/arrows, middle/right drag: pan. Wheel, +/-: zoom. 1-3: preset. G: grid. Esc: quit."
                .to_owned(),
        ]);
    }
}

/// Draw an error message instead of the map.
fn draw_error(message: &str) {
    render::clear();
    let mut lines = vec!["Cannot start: content error".to_owned()];
    lines.extend(message.lines().map(str::to_owned));
    render::draw_overlay(&lines);
}

#[macroquad::main(window_conf)]
async fn main() {
    let text = include_str!("../../../assets/content/materials.ron");
    let setup = base::content::load_materials("materials.ron", text)
        .map_err(|e| e.to_string())
        .and_then(|materials| Palette::from_materials(&materials))
        .and_then(|palette| Sheet::shipped().map(|sheet| (palette, sheet)));
    let mut state = setup.map(|(palette, sheet)| App::new(palette, sheet));
    // Developer aid: PCS_SHOT=<file.png> saves one screenshot and quits.
    // PCS_PRESET=<0-2> and PCS_ZOOM=<n> choose the view.
    let shot = std::env::var("PCS_SHOT").ok();
    if let Ok(app) = &mut state {
        if let Some(n) = std::env::var("PCS_PRESET")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
        {
            app.apply(&[Action::Preset(n)], render::view_size(), 0.0);
        }
        let env_f = |k: &str| std::env::var(k).ok().and_then(|v| v.parse::<f32>().ok());
        if let (Some(x), Some(y)) = (env_f("PCS_CX"), env_f("PCS_CY")) {
            app.camera.center = (x, y);
        }
        if let Some(z) = std::env::var("PCS_ZOOM")
            .ok()
            .and_then(|v| v.parse::<f32>().ok())
        {
            app.camera.zoom = camera::clamp_zoom(z);
        }
    }
    let mut frames = 0;
    let mut input = InputState::default();
    loop {
        let actions = input.poll();
        match &mut state {
            Ok(app) => {
                if !app.apply(&actions, render::view_size(), get_frame_time()) {
                    break;
                }
                app.draw(render::view_size());
            }
            Err(message) => {
                if actions.contains(&Action::Close) {
                    break;
                }
                draw_error(message);
            }
        }
        frames += 1;
        if let (Some(path), true) = (&shot, frames == 4) {
            get_screen_data().export_png(path);
            break;
        }
        next_frame().await;
    }
}
