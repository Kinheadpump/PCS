//! The game: window, rendering, input, and interface.

mod actors;
mod camera;
mod clock;
mod font;
mod input;
mod render;
mod sprites;
mod ui;

use camera::{Camera, View};
use clock::Clock;
use input::{Action, InputState};
use macroquad::prelude::{Conf, get_frame_time, get_screen_data, mouse_position, next_frame};
use render::{MapView, Palette};
use sim::{ColonistId, Command, Event, Sim};
use sprites::Sheet;
use ui::{Ui, UiAction};
use worldgen::{EmbarkSummary, LocalMapData, generate_local};

/// Embark presets on keys F1, F2, F3.
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
    /// Hash of the content files; part of the sim identity (TECHNICAL_DESIGN §6).
    content_hash: u64,
    sim: Sim,
    clock: Clock,
    selected: Option<ColonistId>,
    /// Commands that wait for the next tick.
    pending: Vec<Command>,
    /// Text of the last sim event, for the status panel.
    last_event: String,
}

impl App {
    fn load_preset(palette: &Palette, sheet: &Sheet, preset: usize) -> Loaded {
        let embark = PRESETS[preset % PRESETS.len()].1().with_seed(SEED);
        let map = generate_local(&embark);
        let view = MapView::new(&map, palette, sheet);
        Loaded { map, view }
    }

    fn new(palette: Palette, sheet: Sheet, content_hash: u64) -> Self {
        let loaded = Self::load_preset(&palette, &sheet, 0);
        let size = (loaded.map.ground.width(), loaded.map.ground.height());
        let sim = Sim::new(&loaded.map, SEED, content_hash);
        Self {
            content_hash,
            sim,
            clock: Clock::default(),
            selected: None,
            pending: Vec::new(),
            last_event: String::new(),
            palette,
            sheet,
            grid: false,
            loaded,
            preset: 0,
            camera: Camera::new(size, START_ZOOM),
        }
    }

    /// Developer aid for `PCS_SHOT`: select, order, and run ticks from environment values.
    fn debug_setup(&mut self) {
        let var = |k: &str| std::env::var(k).ok();
        if let Some(slot) = var("PCS_SELECT").and_then(|v| v.parse::<usize>().ok()) {
            self.selected = self.sim.colonists().iter().nth(slot).map(|(id, _)| id);
        }
        if let (Some(id), Some(v)) = (self.selected, var("PCS_MOVE")) {
            let mut it = v.split(',').filter_map(|p| p.trim().parse::<u16>().ok());
            if let (Some(x), Some(y)) = (it.next(), it.next()) {
                self.pending.push(Command::MoveColonist { id, x, y });
            }
        }
        let ticks = var("PCS_TICKS")
            .and_then(|v| v.parse::<u32>().ok())
            .unwrap_or(0);
        for _ in 0..ticks {
            let commands = std::mem::take(&mut self.pending);
            self.sim.step(&commands);
        }
        self.note_events();
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
                    self.sim = Sim::new(&self.loaded.map, SEED, self.content_hash);
                    self.selected = None;
                    self.pending.clear();
                    self.last_event.clear();
                }
                Action::TogglePause => self.clock.toggle_pause(),
                Action::SetSpeed(speed) => self.clock.set_speed(speed),
                Action::LeftClick(..) | Action::RightClick(..) => {}
                Action::ToggleGrid => self.grid = !self.grid,
                Action::Preset(_) => {}
                Action::Close => return false,
            }
        }
        true
    }

    /// Run the ticks of one frame. Pending commands go into the first tick.
    fn update(&mut self, frame_micros: u64) {
        let ticks = self.clock.advance(frame_micros);
        for _ in 0..ticks {
            let commands = std::mem::take(&mut self.pending);
            self.sim.step(&commands);
            self.note_events();
        }
    }

    fn note_events(&mut self) {
        for event in self.sim.drain_events() {
            self.last_event = match event {
                Event::PathFailed { colonist, x, y } => {
                    format!("Colonist {}: no path to ({x}, {y})", colonist.index())
                }
                Event::ColonistArrived { colonist, x, y } => {
                    format!("Colonist {} arrived at ({x}, {y})", colonist.index())
                }
            };
        }
    }

    /// Handle clicks after the UI is built: left selects, right gives a debug move order.
    fn handle_clicks(&mut self, actions: &[Action], ui: &Ui, view: View) {
        let size = self.map_size();
        for action in actions {
            let (pos, order) = match *action {
                Action::LeftClick(x, y) => ((x, y), false),
                Action::RightClick(x, y) => ((x, y), true),
                _ => continue,
            };
            if ui.captures(pos) {
                continue;
            }
            if order {
                let tile = self
                    .camera
                    .tile_at(pos, view, size)
                    .and_then(|(x, y)| Some((u16::try_from(x).ok()?, u16::try_from(y).ok()?)));
                if let (Some(id), Some((x, y))) = (self.selected, tile) {
                    self.pending.push(Command::MoveColonist { id, x, y });
                }
            } else {
                let point = self.camera.screen_to_tile(pos, view);
                self.selected = actors::pick_colonist(&self.sim, point);
            }
        }
    }

    fn hover_text(&self, view: View) -> String {
        let size = self.map_size();
        match self.camera.tile_at(mouse_position(), view, size) {
            Some((x, y)) => {
                let map = &self.loaded.map;
                format!(
                    "Tile ({x}, {y}) ground {:?} rock {:?} ore {:?} tree {}",
                    map.ground.get(x, y),
                    map.rock.get(x, y),
                    map.ore.get(x, y),
                    map.tree.get(x, y).copied().unwrap_or(false)
                )
            }
            None => "Tile (-, -)".to_owned(),
        }
    }

    fn selection_text(&self) -> String {
        let Some((id, c)) = self
            .selected
            .and_then(|id| self.sim.colonist(id).map(|c| (id, c)))
        else {
            return "No colonist selected".to_owned();
        };
        match c.target {
            Some((tx, ty)) => format!(
                "Colonist {} at ({}, {}) going to ({tx}, {ty})",
                id.index(),
                c.x,
                c.y
            ),
            None => format!("Colonist {} at ({}, {}) idle", id.index(), c.x, c.y),
        }
    }

    /// Draw one frame, then apply the UI actions and the clicks.
    fn draw(&mut self, actions: &[Action], view: View) {
        render::clear();
        let size = self.map_size();
        self.loaded.view.draw(&self.camera, size);
        if self.grid {
            MapView::draw_grid(&self.camera, size);
        }
        actors::draw(&self.sim, &self.camera, view, self.selected);

        let click = actions.iter().find_map(|a| match a {
            Action::LeftClick(x, y) => Some((*x, *y)),
            _ => None,
        });
        let mut ui = Ui::new(mouse_position(), click);
        let before = [
            ui::format_time(&self.sim.time()),
            format!(
                "Tick {}   state hash {:016x}",
                self.sim.tick(),
                self.sim.state_hash()
            ),
        ];
        let mut after = vec![
            self.hover_text(view),
            self.selection_text(),
            format!(
                "FPS {}   zoom {:.1} px/tile   map: {}",
                render::fps(),
                self.camera.zoom,
                PRESETS[self.preset].0
            ),
        ];
        if !self.last_event.is_empty() {
            after.push(self.last_event.clone());
        }
        let ui_actions = ui::status_panel(&mut ui, (8.0, 8.0), &before, self.clock.speed(), &after);
        let help = [
            "WASD/arrows, middle drag: pan   Wheel, +/-: zoom   G: grid   Esc: quit".to_owned(),
            "Left click: select   Right click: move selected   Space: pause   Q/E/R: 1x/3x/6x   F1-F3: map"
                .to_owned(),
        ];
        let help_h = ui::hud_layout((0.0, 0.0), &help, &[], &[]).panel.h;
        ui::text_panel(&mut ui, (8.0, view.1 - help_h - 8.0), &help);

        for action in ui_actions {
            match action {
                UiAction::SetSpeed(speed) => self.clock.set_speed(speed),
            }
        }
        self.handle_clicks(actions, &ui, view);
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
    let content_hash = base::hash::fnv1a(text.as_bytes());
    let mut state = setup.map(|(palette, sheet)| App::new(palette, sheet, content_hash));
    // Developer aid: PCS_SHOT=<file.png> saves one screenshot and quits.
    // PCS_PRESET=<0-2> and PCS_ZOOM=<n> choose the view. PCS_SELECT=<colonist slot>,
    // PCS_MOVE=<x>,<y> (move order for the selected colonist), and PCS_TICKS=<n> (run n
    // ticks first) set up the sim for the shot.
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
        app.debug_setup();
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
                app.update((get_frame_time() * 1_000_000.0) as u64);
                app.draw(&actions, render::view_size());
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
