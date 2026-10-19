//! The game: window, rendering, input, and interface.

mod actors;
mod camera;
mod clock;
mod font;
mod hud;
mod input;
mod inspector;
mod overlay;
mod render;
mod sprites;
mod tools;
mod ui;

use camera::{Camera, View};
use clock::Clock;
use input::{Action, InputState};
use macroquad::prelude::{Conf, get_frame_time, get_screen_data, mouse_position, next_frame};
use render::{MapView, Palette};
use sim::{ColonistId, Command, Event, JobEvent, JobKind, Sim};
use sprites::Sheet;
use std::collections::BTreeSet;
use tools::{Area, Tool};
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
    /// The tool of the left mouse button on the map.
    tool: Tool,
    /// The tile where a drag started.
    drag_start: Option<(u16, u16)>,
    /// The selected tile, shown in the tile panel.
    selected_tile: Option<(u16, u16)>,
    /// True if the work panel is open.
    show_work: bool,
    /// The panels of the last frame, so the mouse does not act through them.
    panels: Vec<ui::Rect>,
    /// Chunks of the baked map that must be baked again.
    dirty: BTreeSet<(u32, u32)>,
    /// A drag preview from `PCS_PREVIEW`, for screenshots.
    preview_override: Option<(Tool, Area)>,
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
            tool: Tool::Select,
            drag_start: None,
            selected_tile: None,
            show_work: false,
            panels: Vec::new(),
            dirty: BTreeSet::new(),
            preview_override: None,
            palette,
            sheet,
            grid: false,
            loaded,
            preset: 0,
            camera: Camera::new(size, START_ZOOM),
        }
    }

    /// Developer aid for `PCS_SHOT`: select, order, and run ticks from environment values.
    /// `PCS_TOOL=<word>`, `PCS_DESIGNATE=<tool:x0,y0,x1,y1;...>` (the orders a drag would give),
    /// `PCS_PREVIEW=<tool:x0,y0,x1,y1>` (draw a drag preview), `PCS_TILE=<x>,<y>`,
    /// `PCS_WORK=1`, `PCS_PRIORITY=<slot>,<job 0-2>,<priority>`, and
    /// `PCS_ITEM=<material key>,<count>,<x>,<y>` (several, separated by `;`).
    fn debug_setup(&mut self) {
        let var = |k: &str| std::env::var(k).ok();
        let numbers = |v: &str| -> Vec<u16> {
            v.split(',')
                .filter_map(|p| p.trim().parse::<u16>().ok())
                .collect()
        };
        if let Some(tool) = var("PCS_TOOL").and_then(|v| Tool::from_word(&v)) {
            self.tool = tool;
        }
        self.show_work = var("PCS_WORK").is_some_and(|v| v == "1");
        if let Some(slot) = var("PCS_SELECT").and_then(|v| v.parse::<usize>().ok()) {
            self.selected = self.sim.colonists().iter().nth(slot).map(|(id, _)| id);
        }
        if let (Some(id), Some(v)) = (self.selected, var("PCS_MOVE"))
            && let [x, y] = numbers(&v)[..]
        {
            self.pending.push(Command::MoveColonist { id, x, y });
        }
        if let Some(v) = var("PCS_TILE")
            && let [x, y] = numbers(&v)[..]
        {
            self.selected_tile = Some((x, y));
        }
        for entry in var("PCS_ITEM").iter().flat_map(|v| v.split(';')) {
            let mut parts = entry.split(',').map(str::trim);
            let key = parts.next().unwrap_or_default();
            let rest: Vec<u16> = parts.filter_map(|p| p.parse().ok()).collect();
            let material = self.sim.rules().materials.iter().position(|m| m.key == key);
            if let (Some(material), [count, x, y]) = (material, &rest[..]) {
                self.sim
                    .debug_spawn_item(material as u16, u32::from(*count), *x, *y);
            }
        }
        if let Some(v) = var("PCS_PRIORITY") {
            let values: Vec<usize> = v.split(',').filter_map(|p| p.trim().parse().ok()).collect();
            if let [slot, job, priority] = values[..] {
                let colonist = self.sim.colonists().iter().nth(slot).map(|(id, _)| id);
                if let (Some(colonist), Some(job)) = (colonist, JobKind::ALL.get(job)) {
                    self.pending.push(Command::SetPriority {
                        colonist,
                        job: *job,
                        priority: priority as u8,
                    });
                }
            }
        }
        for order in var("PCS_DESIGNATE").iter().flat_map(|v| v.split(';')) {
            if let Some((tool, area)) = tools::parse_order(order, self.tool) {
                let sim = &self.sim;
                self.pending
                    .extend(tools::commands_for_drag(tool, area, |t, x, y| {
                        tools::applies(sim, t, x, y)
                    }));
            }
        }
        self.preview_override = var("PCS_PREVIEW").and_then(|v| tools::parse_order(&v, self.tool));
        let ticks = var("PCS_TICKS")
            .and_then(|v| v.parse::<u32>().ok())
            .unwrap_or(0)
            .max(u32::from(!self.pending.is_empty()));
        for _ in 0..ticks {
            let commands = std::mem::take(&mut self.pending);
            self.sim.step(&commands);
            self.note_events();
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
                    self.selected_tile = None;
                    self.drag_start = None;
                    self.dirty.clear();
                    self.pending.clear();
                    self.last_event.clear();
                }
                Action::SelectTool(tool) => self.set_tool(tool),
                Action::ToggleWork => self.show_work = !self.show_work,
                Action::TogglePause => self.clock.toggle_pause(),
                Action::SetSpeed(speed) => self.clock.set_speed(speed),
                Action::LeftClick(..) | Action::LeftRelease(..) | Action::RightClick(..) => {}
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
        self.rebake();
    }

    /// Choose a tool. A drag in progress ends.
    fn set_tool(&mut self, tool: Tool) {
        self.tool = tool;
        self.drag_start = None;
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
        for event in self.sim.drain_job_events() {
            if let JobEvent::Completed {
                kind: JobKind::Cut | JobKind::Mine,
                x,
                y,
                ..
            } = event
            {
                self.sync_tile(x, y);
            }
            if let Some(text) = inspector::job_event_text(&event) {
                self.last_event = text;
            }
        }
    }

    /// Copy a tile from the sim map to the baked map data and mark its chunks.
    fn sync_tile(&mut self, x: u16, y: u16) {
        let (ix, iy) = (i32::from(x), i32::from(y));
        let live = self.sim.map();
        let baked = &mut self.loaded.map;
        if let Some(v) = live.ground(ix, iy) {
            baked.ground.set(ix, iy, v);
        }
        if let Some(v) = live.rock(ix, iy) {
            baked.rock.set(ix, iy, v);
        }
        if let Some(v) = live.ore(ix, iy) {
            baked.ore.set(ix, iy, v);
        }
        baked.tree.set(ix, iy, live.tree(ix, iy));
        let size = self.map_size();
        self.dirty
            .extend(render::chunks_touched_by(u32::from(x), u32::from(y), size));
    }

    /// Bake the changed chunks again.
    fn rebake(&mut self) {
        if self.dirty.is_empty() {
            return;
        }
        let dirty = std::mem::take(&mut self.dirty);
        self.loaded
            .view
            .rebake_chunks(&self.loaded.map, &self.palette, &self.sheet, &dirty);
    }

    /// The tile under a screen position, or `None` outside the map.
    fn tile_under(&self, pos: (f32, f32), view: View) -> Option<(u16, u16)> {
        self.camera
            .tile_at(pos, view, self.map_size())
            .and_then(|(x, y)| Some((u16::try_from(x).ok()?, u16::try_from(y).ok()?)))
    }

    /// The tile where a drag at a screen position ends. Outside the map it is the nearest edge tile.
    fn drag_tile(&self, pos: (f32, f32), view: View) -> Option<(u16, u16)> {
        tools::clamp_tile(self.camera.screen_to_tile(pos, view), self.map_size())
    }

    /// Handle clicks after the UI is built. With the Select tool the left button selects and the
    /// right button gives a debug move order. Other tools drag a rectangle.
    fn handle_clicks(&mut self, actions: &[Action], ui: &Ui, view: View) {
        for action in actions {
            match *action {
                Action::LeftClick(x, y) if !ui.captures((x, y)) => {
                    if self.tool.drags() {
                        self.drag_start = self.tile_under((x, y), view);
                    } else {
                        let point = self.camera.screen_to_tile((x, y), view);
                        self.selected = actors::pick_colonist(&self.sim, point);
                        self.selected_tile = if self.selected.is_none() {
                            self.tile_under((x, y), view)
                        } else {
                            None
                        };
                    }
                }
                Action::LeftRelease(x, y) => {
                    let start = self.drag_start.take();
                    if let (Some(start), Some(end)) = (start, self.drag_tile((x, y), view)) {
                        let area = Area::from_corners(start, end);
                        let sim = &self.sim;
                        let commands = tools::commands_for_drag(self.tool, area, |t, tx, ty| {
                            tools::applies(sim, t, tx, ty)
                        });
                        self.pending.extend(commands);
                    }
                }
                Action::RightClick(x, y) if !ui.captures((x, y)) => {
                    if self.drag_start.take().is_some() {
                        continue;
                    }
                    if self.tool != Tool::Select {
                        self.set_tool(Tool::Select);
                        continue;
                    }
                    if let (Some(id), Some((tx, ty))) =
                        (self.selected, self.tile_under((x, y), view))
                    {
                        self.pending
                            .push(Command::MoveColonist { id, x: tx, y: ty });
                    }
                }
                _ => {}
            }
        }
    }

    /// The area and tool of the drag preview, if there is one.
    fn preview(&self, view: View) -> Option<(Tool, Area)> {
        if let (Some(start), true) = (self.drag_start, self.tool.drags()) {
            let end = self.drag_tile(mouse_position(), view)?;
            return Some((self.tool, Area::from_corners(start, end)));
        }
        self.preview_override
    }

    /// Draw one frame, then apply the UI actions and the clicks.
    fn draw(&mut self, actions: &[Action], view: View) {
        render::clear();
        let size = self.map_size();
        self.loaded.view.draw(&self.camera, size);
        if self.grid {
            MapView::draw_grid(&self.camera, size);
        }
        let frame = overlay::Frame {
            sim: &self.sim,
            camera: &self.camera,
            view,
            map_size: size,
        };
        let selected_zone = self
            .selected_tile
            .and_then(|(x, y)| inspector::zone_at(&self.sim, x, y));
        overlay::draw_zones(&frame, selected_zone);
        overlay::draw_items(&frame);
        overlay::draw_designations(&frame);
        actors::draw(&self.sim, &self.camera, view, self.selected);
        let sprite_size = self.camera.zoom.max(12.0);
        for (id, c) in self.sim.colonists().iter() {
            if c.carrying.is_some() {
                let p = self.camera.tile_to_screen(actors::position_tiles(c), view);
                overlay::draw_carried(&frame, p, sprite_size, id);
            }
        }
        if let Some((tool, area)) = self.preview(view) {
            overlay::draw_preview(&frame, tool, area);
        }
        let mouse = mouse_position();
        let hover_tile = if self.panels.iter().any(|r| r.contains(mouse)) {
            None
        } else {
            self.tile_under(mouse, view)
        };
        if let (Some(tile), true) = (hover_tile, self.tool.drags() && self.drag_start.is_none()) {
            overlay::draw_tile_outline(&frame, tile, overlay::hover_color());
        }
        if let Some(tile) = self.selected_tile {
            overlay::draw_tile_outline(&frame, tile, overlay::SELECT);
        }

        let click = actions.iter().find_map(|a| match a {
            Action::LeftClick(x, y) => Some((*x, *y)),
            _ => None,
        });
        let right_click = actions.iter().find_map(|a| match a {
            Action::RightClick(x, y) => Some((*x, *y)),
            _ => None,
        });
        let mut ui = Ui::new(mouse, click).with_right_click(right_click);
        let input = hud::HudInput {
            sim: &self.sim,
            view,
            tool: self.tool,
            show_work: self.show_work,
            selected: self.selected,
            selected_tile: self.selected_tile,
            hover_tile,
            speed: self.clock.speed(),
            fps: render::fps(),
            zoom: self.camera.zoom,
            map_name: PRESETS[self.preset].0,
            last_event: &self.last_event,
        };
        let ui_actions = hud::draw(&mut ui, &input);
        for action in ui_actions {
            match action {
                UiAction::SetSpeed(speed) => self.clock.set_speed(speed),
                UiAction::SetTool(tool) => self.set_tool(tool),
                UiAction::ToggleWork => self.show_work = !self.show_work,
                UiAction::SelectColonist(id) => {
                    self.selected = Some(id);
                    self.selected_tile = None;
                }
                UiAction::Command(command) => self.pending.push(command),
            }
        }
        self.handle_clicks(actions, &ui, view);
        self.panels = ui.panels().to_vec();
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
