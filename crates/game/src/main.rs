//! The game: window, rendering, input, and interface.

mod actors;
mod bake;
mod build_ui;
mod camera;
mod cells;
mod clock;
mod font;
mod hud;
mod icons;
mod info_ui;
mod input;
mod inspector;
mod layout;
mod lifemark;
mod overlay;
mod palette;
mod plants;
mod render;
mod site_layer;
mod sound;
mod sprites;
mod structures;
mod tools;
mod ui;
mod weather;
mod world_map;
mod world_screen;

use bake::Palette;
use build_ui::BuildState;
use camera::{Camera, View};
use clock::Clock;
use input::{Action, InputState, Typed};
use macroquad::prelude::{Conf, get_frame_time, get_screen_data, mouse_position, next_frame};
use render::MapView;
use sim::{ColonistId, Command, Event, JobEvent, JobKind, LifeEvent, Sim};
use sprites::Sprites;
use std::collections::BTreeSet;
use structures::Structures;
use tools::{Area, Tool};
use ui::{Ui, UiAction};
use world_screen::{WorldAction, WorldScreen};
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

/// The toast text of a life event: who left or died, with the cause.
fn life_event_text(event: LifeEvent) -> String {
    match event {
        LifeEvent::Died { x, y, cause } => {
            format!(
                "A colonist died at ({x}, {y}): {}",
                cause.name().to_lowercase()
            )
        }
        LifeEvent::Left { x, y } => format!("A colonist left the colony at ({x}, {y})"),
    }
}

/// The window size. Developer aid: `PCS_WINDOW=<w>x<h>` sets it, for screenshots.
fn window_size() -> (i32, i32) {
    std::env::var("PCS_WINDOW")
        .ok()
        .and_then(|v| {
            let (w, h) = v.split_once('x')?;
            Some((w.parse().ok()?, h.parse().ok()?))
        })
        .unwrap_or((1280, 720))
}

fn window_conf() -> Conf {
    let (window_width, window_height) = window_size();
    Conf {
        window_title: "Ruinstead".to_owned(),
        window_width,
        window_height,
        ..Default::default()
    }
}

/// The building, the material, and the tiles of a build ghost.
type Ghost = (u16, u16, Vec<(u16, u16)>);

/// Read `<building key>,<material key>,<numbers...>` of a debug setting.
fn parse_build(rules: &sim::SimRules, text: &str) -> Option<(u16, u16, Vec<u16>)> {
    let mut parts = text.split(',').map(str::trim);
    let building = rules.building_index(parts.next()?)?;
    let material = rules.material_index(parts.next()?)?;
    let n: Vec<u16> = parts.filter_map(|p| p.parse().ok()).collect();
    Some((building, material, n))
}

/// A loaded map with its textures.
struct Loaded {
    map: LocalMapData,
    view: MapView,
}

/// One build order of the developer hook: finished or blueprint, building, material, tiles.
type BuildJob = (bool, u16, u16, Vec<(u16, u16)>);

struct App {
    palette: Palette,
    sprites: Sprites,
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
    /// The folds that are open and the debug switch of the display.
    ui_state: hud::UiState,
    /// True if the work panel is open.
    show_work: bool,
    /// True if the rooms overlay is on.
    show_rooms: bool,
    /// True if the temperature overlay is on.
    show_temp: bool,
    /// The building and the material of the build tool.
    build: BuildState,
    /// The textures of the buildings.
    structures: Structures,
    /// The panels of the last frame, so the mouse does not act through them.
    panels: Vec<ui::Rect>,
    /// Chunks of the baked map that must be baked again.
    dirty: BTreeSet<(u32, u32)>,
    /// A drag preview from `PCS_PREVIEW`, for screenshots.
    preview_override: Option<(Tool, Area)>,
    /// True if `PCS_HUD=0` is set: the screenshot shows the map only.
    hide_hud: bool,
    /// The world map screen. It is made when the player opens it for the first time.
    world: Option<Box<WorldScreen>>,
    /// True while the world map screen is on. The sim does not run then.
    show_world: bool,
    /// The name of the map for the status line.
    map_name: String,
    /// The text keys of this frame.
    typed: Typed,
    /// The sound engine. It is made after the window opens. `None` means no sound.
    sound: Option<sound::Engine>,
    /// The alerts of the last tick, to find the new ones for the alert sounds.
    alert_keys: Vec<sound::AlertKey>,
}

impl App {
    fn load_preset(palette: &Palette, sprites: &Sprites, preset: usize) -> Loaded {
        let embark = PRESETS[preset % PRESETS.len()].1().with_seed(SEED);
        let map = generate_local(&embark);
        let view = MapView::new(&map, palette, sprites);
        Loaded { map, view }
    }

    /// A new sim for a preset. The mean temperature comes from the embark. The embark has no
    /// seasonal amplitude and no moisture yet, so the sim keeps the defaults of `climate.ron`.
    fn start_sim(map: &LocalMapData, preset: usize, content_hash: u64) -> Sim {
        let mean = PRESETS[preset % PRESETS.len()].1().mean_temperature;
        let mut sim = Sim::new(map, SEED, content_hash);
        let params = sim.climate_params();
        sim.set_climate(mean, params.amplitude, params.moisture);
        sim
    }

    fn new(palette: Palette, sprites: Sprites, content_hash: u64) -> Self {
        let loaded = Self::load_preset(&palette, &sprites, 0);
        let size = (loaded.map.ground.width(), loaded.map.ground.height());
        let sim = Self::start_sim(&loaded.map, 0, content_hash);
        let build = BuildState::new(sim.rules());
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
            ui_state: hud::UiState::default(),
            show_work: false,
            show_rooms: false,
            show_temp: false,
            build,
            structures: Structures::new(),
            panels: Vec::new(),
            dirty: BTreeSet::new(),
            preview_override: None,
            hide_hud: std::env::var("PCS_HUD").is_ok_and(|v| v == "0"),
            world: None,
            show_world: false,
            map_name: PRESETS[0].0.to_owned(),
            typed: Typed::default(),
            sound: None,
            alert_keys: Vec::new(),
            palette,
            sprites,
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
    /// `PCS_ITEM=<material key>,<count>,<x>,<y>` (several, separated by `;`),
    /// `PCS_TEMP=1` (temperature overlay), `PCS_CLIMATE=<mean>,<amplitude>,<moisture>` (tenths of a
    /// degree, moisture 0 to 255), and `PCS_FUEL=<x>,<y>,<hours>` (hearth fuel, several separated
    /// by `;`; these two run before the ticks). `PCS_DAYS=<n>` runs n days of ticks.
    /// `PCS_FOLDS=<words>` opens folds (1, temp, mood, health, comfort, skills, speed, all),
    /// `PCS_NEEDS=<slot>,<hunger>,<rest>,<health>;...` sets needs, and
    /// `PCS_LIFE=<slot>:<wound|kill|memory=<key>|traits=<i>/<j>|clothes=<key|none>>;...`
    /// changes a colonist (both run before the ticks).
    fn debug_setup(&mut self) {
        let var = |k: &str| std::env::var(k).ok();
        let numbers = |v: &str| -> Vec<u16> {
            v.split(',')
                .filter_map(|p| p.trim().parse::<u16>().ok())
                .collect()
        };
        let numbers_signed = |v: &str| -> Vec<i32> {
            v.split(',')
                .filter_map(|p| p.trim().parse::<i32>().ok())
                .collect()
        };
        if let Some(tool) = var("PCS_TOOL").and_then(|v| Tool::from_word(&v)) {
            self.tool = tool;
        }
        self.show_work = var("PCS_WORK").is_some_and(|v| v == "1");
        self.show_rooms = var("PCS_ROOMS").is_some_and(|v| v == "1");
        for word in var("PCS_FOLDS").iter().flat_map(|v| v.split(',')) {
            let all = word == "all";
            for (name, fold) in [
                ("1", ui::Fold::TempChain),
                ("temp", ui::Fold::TempChain),
                ("mood", ui::Fold::Mood),
                ("health", ui::Fold::Health),
                ("comfort", ui::Fold::Comfort),
                ("skills", ui::Fold::Skills),
                ("speed", ui::Fold::WorkSpeed),
                ("food", ui::Fold::FoodParts),
                ("fuel", ui::Fold::FuelParts),
            ] {
                if all || word == name {
                    self.ui_state.open.insert(fold);
                }
            }
        }
        self.show_temp = var("PCS_TEMP").is_some_and(|v| v == "1");
        self.ui_state.colony = var("PCS_COLONY").is_some_and(|v| v == "1");
        self.ui_state.fertility = var("PCS_FERTILITY").is_some_and(|v| v == "1");
        if let Some(n) = var("PCS_CROP").and_then(|v| v.parse().ok()) {
            self.ui_state.crop = n;
        }
        if let Some(v) = var("PCS_CLIMATE")
            && let [mean, amplitude, moisture] = numbers_signed(&v)[..]
        {
            self.sim
                .set_climate(mean as i16, amplitude as i16, moisture.clamp(0, 255) as u8);
        }
        self.debug_build(&var);
        for entry in var("PCS_FUEL").iter().flat_map(|v| v.split(';')) {
            if let [x, y, hours] = numbers(entry)[..] {
                self.sim.debug_set_hearth_fuel(x, y, u32::from(hours));
            }
        }
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
        self.debug_life(&var);
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
                if tool == Tool::Farm {
                    self.pending.push(Command::SetFarmZone {
                        x0: area.x0,
                        y0: area.y0,
                        x1: area.x1,
                        y1: area.y1,
                        crop: self.ui_state.crop,
                    });
                    continue;
                }
                self.pending
                    .extend(tools::commands_for_drag(tool, area, |t, x, y| {
                        tools::applies(sim, t, x, y)
                    }));
            }
        }
        self.preview_override = var("PCS_PREVIEW").and_then(|v| tools::parse_order(&v, self.tool));
        let days = var("PCS_DAYS")
            .and_then(|v| v.parse::<u32>().ok())
            .unwrap_or(0);
        let ticks = var("PCS_TICKS")
            .and_then(|v| v.parse::<u32>().ok())
            .unwrap_or(0)
            .saturating_add(days.saturating_mul(sim::time::TICKS_PER_DAY as u32))
            .max(if var("PCS_BUILD").is_some() { 2 } else { 0 })
            .max(u32::from(!self.pending.is_empty()));
        for _ in 0..ticks {
            let commands = std::mem::take(&mut self.pending);
            self.sim.step(&commands);
            self.note_events();
        }
        self.note_events();
    }

    /// Developer aid for the needs: `PCS_NEEDS=<slot>,<hunger>,<rest>,<health>` (whole units; `-`
    /// keeps a value; several entries separated by `;`) and
    /// `PCS_LIFE=<slot>:<action>[=<value>]` with the actions `wound`, `kill`, `memory=<key>`,
    /// `traits=<index>/<index>`, and `clothes=<material key>|none` (several, separated by `;`).
    fn debug_life(&mut self, var: &dyn Fn(&str) -> Option<String>) {
        let nth = |sim: &Sim, slot: &str| {
            let slot = slot.trim().parse::<usize>().ok()?;
            sim.colonists().iter().nth(slot).map(|(id, _)| id)
        };
        for entry in var("PCS_NEEDS").iter().flat_map(|v| v.split(';')) {
            let parts: Vec<&str> = entry.split(',').map(str::trim).collect();
            let [slot, hunger, rest, health] = parts[..] else {
                continue;
            };
            let Some(id) = nth(&self.sim, slot) else {
                continue;
            };
            if let Ok(v) = hunger.parse() {
                self.sim.debug_set_hunger(id, v);
            }
            if let Ok(v) = rest.parse() {
                self.sim.debug_set_rest(id, v);
            }
            if let Ok(v) = health.parse() {
                self.sim.debug_set_health(id, v);
            }
        }
        for entry in var("PCS_LIFE").iter().flat_map(|v| v.split(';')) {
            let Some((slot, action)) = entry.split_once(':') else {
                continue;
            };
            let Some(id) = nth(&self.sim, slot) else {
                continue;
            };
            let (name, value) = action.split_once('=').unwrap_or((action, ""));
            match name {
                "wound" => self.sim.debug_wound(id),
                "kill" => self.sim.debug_kill(id),
                "memory" => {
                    self.sim.debug_add_memory(id, value);
                }
                "traits" => {
                    let list: Vec<u8> = value.split('/').filter_map(|t| t.parse().ok()).collect();
                    self.sim.debug_set_traits(id, &list);
                }
                "clothes" => {
                    let material = self
                        .sim
                        .rules()
                        .materials
                        .iter()
                        .position(|m| m.key == value)
                        .map(|m| m as u16);
                    self.sim.debug_set_clothes(id, material);
                }
                _ => {}
            }
        }
    }

    /// Developer aid for the build tool. `PCS_BUILD=<building>,<material>,<x0>,<y0>,<x1>,<y1>;...`
    /// builds finished buildings (like `Sim::debug_build`) on the tiles of the rectangle.
    /// `PCS_BLUEPRINT` has the same form and places blueprints. `PCS_CHOICE=<building>,<material>`
    /// sets the building and the material of the build tool.
    fn debug_build(&mut self, var: &dyn Fn(&str) -> Option<String>) {
        let parse = |text: &str| parse_build(self.sim.rules(), text);
        let mut jobs: Vec<BuildJob> = Vec::new();
        for (name, finished) in [("PCS_BUILD", true), ("PCS_BLUEPRINT", false)] {
            for entry in var(name).iter().flat_map(|v| v.split(';')) {
                if let Some((b, m, n)) = parse(entry)
                    && let [x0, y0, x1, y1] = n[..]
                {
                    let tiles = build_ui::drag_tiles(build_ui::Shape::Rect, (x0, y0), (x1, y1));
                    jobs.push((finished, b, m, tiles));
                }
            }
        }
        for (finished, b, m, tiles) in jobs {
            for (x, y) in tiles {
                if finished {
                    self.sim.debug_build(b, x, y, m);
                } else {
                    self.pending.push(Command::PlaceBlueprint {
                        building: b,
                        material: m,
                        x,
                        y,
                    });
                }
            }
        }
        if let Some(v) = var("PCS_CHOICE")
            && let Some((b, m, _)) = parse_build(self.sim.rules(), &v)
        {
            self.build.pick_building(self.sim.rules(), usize::from(b));
            self.build.pick_material(self.sim.rules(), m);
        }
    }

    /// Open the world map screen. The world of the first seed is made on the first call.
    fn open_world(&mut self) {
        if self.world.is_none() {
            match WorldScreen::new(world_seed()) {
                Ok(screen) => self.world = Some(Box::new(screen)),
                Err(message) => {
                    self.last_event = format!("World error: {message}");
                    return;
                }
            }
        }
        self.show_world = true;
    }

    /// Start a colony in a world cell: the local map with its stamped sites, a new sim, and a new
    /// camera. Does nothing if the player cannot embark on the cell.
    fn embark(&mut self, x: i32, y: i32) {
        let Some(embarked) = self.world.as_ref().and_then(|w| w.embark(x, y)) else {
            return;
        };
        let view = MapView::new(&embarked.map, &self.palette, &self.sprites);
        self.loaded = Loaded {
            map: embarked.map,
            view,
        };
        let mut sim = Sim::new(&self.loaded.map, embarked.summary.seed, self.content_hash);
        sim.set_climate(
            embarked.summary.mean_temperature,
            embarked.amplitude,
            embarked.moisture,
        );
        self.sim = sim;
        self.map_name = embarked.name;
        self.reset_colony_state();
        self.camera = Camera::new(self.map_size(), START_ZOOM);
        self.show_world = false;
    }

    /// Developer aid: `PCS_WORLD=1` opens the world map. `PCS_LAYER=<name>` chooses the layer,
    /// `PCS_WORLD_CELL=<x>,<y>` selects a cell, and `PCS_EMBARK=<x>,<y>` starts a colony there.
    /// `PCS_WORLD_SEED=<n>` is the seed of the world.
    fn debug_world(&mut self) {
        let var = |k: &str| std::env::var(k).ok();
        let cell = |k: &str| -> Option<(i32, i32)> {
            let v = var(k)?;
            let (x, y) = v.split_once(',')?;
            Some((x.trim().parse().ok()?, y.trim().parse().ok()?))
        };
        if var("PCS_WORLD").is_none_or(|v| v != "1") && cell("PCS_EMBARK").is_none() {
            return;
        }
        self.open_world();
        if let Some(world) = self.world.as_mut() {
            if let Some(layer) = var("PCS_LAYER").and_then(|v| world_map::Layer::from_word(&v)) {
                world.set_layer(layer);
            }
            if let Some(c) = cell("PCS_WORLD_CELL") {
                world.select(Some(c));
            }
        }
        if let Some((x, y)) = cell("PCS_EMBARK") {
            self.embark(x, y);
        }
    }

    /// Clear the selection, the orders, and the build choice after a new map.
    fn reset_colony_state(&mut self) {
        self.selected = None;
        self.selected_tile = None;
        self.drag_start = None;
        self.build = BuildState::new(self.sim.rules());
        self.dirty.clear();
        self.pending.clear();
        self.last_event.clear();
    }

    /// Apply the actions of one frame on the world map screen.
    fn apply_world(&mut self, actions: &[Action]) -> bool {
        for action in actions {
            match action {
                Action::ToggleWorld => self.show_world = false,
                Action::Close if self.world.as_mut().is_none_or(|w| w.on_close()) => {
                    self.show_world = false;
                }
                _ => {}
            }
        }
        true
    }

    fn map_size(&self) -> (u32, u32) {
        (
            self.loaded.map.ground.width(),
            self.loaded.map.ground.height(),
        )
    }

    /// Apply the actions of one frame. Returns `false` when the game must close.
    fn apply(&mut self, actions: &[Action], view: View, dt: f32) -> bool {
        if self.show_world {
            return self.apply_world(actions);
        }
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
                    self.loaded = Self::load_preset(&self.palette, &self.sprites, n);
                    self.sim = Self::start_sim(&self.loaded.map, n, self.content_hash);
                    self.map_name = PRESETS[n].0.to_owned();
                    self.reset_colony_state();
                }
                Action::SelectTool(tool) => self.set_tool(tool),
                Action::ToggleWork => self.show_work = !self.show_work,
                Action::ToggleRooms => self.show_rooms = !self.show_rooms,
                Action::ToggleTemp => self.show_temp = !self.show_temp,
                Action::ToggleColony => self.ui_state.colony = !self.ui_state.colony,
                Action::ToggleFertility => self.ui_state.fertility = !self.ui_state.fertility,
                Action::PickBuilding(n) => {
                    self.set_tool(Tool::Build);
                    self.build.pick_building(self.sim.rules(), n);
                }
                Action::TogglePause => self.clock.toggle_pause(),
                Action::SetSpeed(speed) => self.clock.set_speed(speed),
                Action::LeftClick(..) | Action::LeftRelease(..) | Action::RightClick(..) => {}
                Action::ToggleGrid => self.grid = !self.grid,
                Action::ToggleDebug => self.ui_state.debug = !self.ui_state.debug,
                Action::ToggleWorld => self.open_world(),
                Action::Preset(_) => {}
                Action::Close if self.ui_state.menu => self.close_menu(),
                Action::Close => {
                    self.ui_state.menu = true;
                    if self.clock.speed() != sim::Speed::Paused {
                        self.clock.toggle_pause();
                        self.ui_state.menu_paused = true;
                    }
                }
            }
        }
        true
    }

    /// Close the pause menu. A pause that the menu made ends with it.
    fn close_menu(&mut self) {
        self.ui_state.menu = false;
        if std::mem::take(&mut self.ui_state.menu_paused) {
            self.clock.toggle_pause();
        }
    }

    /// Run the ticks of one frame. Pending commands go into the first tick.
    fn update(&mut self, frame_micros: u64) {
        if self.show_world {
            return;
        }
        let ticks = self.clock.advance(frame_micros);
        if self.sim.run_state() != sim::RunState::Running {
            // The run has ended: the sim stops. The end panel offers a new colony.
            return;
        }
        for _ in 0..ticks {
            let commands = std::mem::take(&mut self.pending);
            self.sim.step(&commands);
            self.note_events();
        }
        self.sound_frame();
        self.rebake();
    }

    /// Choose a tool. A drag in progress ends.
    fn set_tool(&mut self, tool: Tool) {
        self.tool = tool;
        self.drag_start = None;
        if matches!(tool, Tool::Build | Tool::Deconstruct) {
            self.selected_tile = None;
        }
    }

    /// Play a sound. Nothing happens if the sound engine is not on.
    fn play(&mut self, id: sound::SoundId) {
        if let Some(engine) = &mut self.sound {
            engine.play(id);
        }
    }

    /// Once for each frame: the sound cool-downs, and the sound of the most urgent new alert.
    fn sound_frame(&mut self) {
        let Some(engine) = &mut self.sound else {
            return;
        };
        engine.next_frame();
        let alerts = self.sim.alerts();
        let new = sound::new_alert_sound(&self.alert_keys, &alerts);
        self.alert_keys = sound::alert_keys(&alerts);
        if let Some(id) = new {
            engine.play(id);
        }
    }

    fn note_events(&mut self) {
        for event in self.sim.drain_events() {
            match event {
                Event::ColonistArrived { .. } => self.play(sound::SoundId::Footstep),
                Event::Crafted { .. } => self.play(sound::SoundId::CraftDone),
                Event::PathFailed { .. } => {}
            }
            self.last_event = match event {
                Event::PathFailed { colonist, x, y } => {
                    format!("Colonist {}: no path to ({x}, {y})", colonist.index())
                }
                Event::ColonistArrived { colonist, x, y } => {
                    format!("Colonist {} arrived at ({x}, {y})", colonist.index())
                }
                Event::Crafted { x, y, .. } => format!("Crafted at ({x}, {y})"),
            };
        }
        for event in self.sim.drain_life_events() {
            self.last_event = life_event_text(event);
        }
        if self
            .selected
            .is_some_and(|id| self.sim.colonist(id).is_none())
        {
            self.selected = None;
        }
        for event in self.sim.drain_job_events() {
            if let JobEvent::Completed {
                kind: kind @ (JobKind::Cut | JobKind::Mine),
                x,
                y,
                ..
            } = event
            {
                self.play(if kind == JobKind::Cut {
                    sound::SoundId::ToolChop
                } else {
                    sound::SoundId::ToolMine
                });
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
            .extend(bake::chunks_touched_by(u32::from(x), u32::from(y), size));
    }

    /// Bake the changed chunks again.
    fn rebake(&mut self) {
        if self.dirty.is_empty() {
            return;
        }
        let dirty = std::mem::take(&mut self.dirty);
        self.loaded
            .view
            .rebake_chunks(&self.loaded.map, &self.palette, &self.sprites, &dirty);
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
                        let commands = match self.tool {
                            Tool::Build => self.build_commands(start, end),
                            Tool::Deconstruct => build_ui::remove_commands(sim, area),
                            Tool::Farm => vec![Command::SetFarmZone {
                                x0: area.x0,
                                y0: area.y0,
                                x1: area.x1,
                                y1: area.y1,
                                crop: self.ui_state.crop,
                            }],
                            _ => tools::commands_for_drag(self.tool, area, |t, tx, ty| {
                                tools::applies(sim, t, tx, ty)
                            }),
                        };
                        self.pending.extend(commands);
                        if self.tool == Tool::Cancel {
                            self.pending.push(Command::ClearFarmZone {
                                x0: area.x0,
                                y0: area.y0,
                                x1: area.x1,
                                y1: area.y1,
                            });
                        }
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

    /// The tiles of the build drag from `start` to `end`.
    fn build_tiles(&self, start: (u16, u16), end: (u16, u16)) -> Vec<(u16, u16)> {
        let kind = self
            .sim
            .rules()
            .building(self.build.building())
            .map(|d| d.kind);
        let shape = kind.map_or(build_ui::Shape::Point, build_ui::shape_of);
        build_ui::drag_tiles(shape, start, end)
    }

    /// The blueprint commands of a finished build drag.
    fn build_commands(&self, start: (u16, u16), end: (u16, u16)) -> Vec<Command> {
        let Some(material) = self.build.material(&self.sim) else {
            return Vec::new();
        };
        let tiles = self.build_tiles(start, end);
        build_ui::place_commands(&self.sim, self.build.building(), material, &tiles)
    }

    /// The area and tool of the drag preview, if there is one. The build tool has its own preview.
    fn preview(&self, view: View) -> Option<(Tool, Area)> {
        if self.tool == Tool::Build {
            return None;
        }
        if let (Some(start), true) = (self.drag_start, self.tool.drags()) {
            let end = self.drag_tile(mouse_position(), view)?;
            return Some((self.tool, Area::from_corners(start, end)));
        }
        self.preview_override
    }

    /// The ghost of the build tool: the building, the material, and the tiles of the drag, or
    /// the tile under the mouse.
    fn build_ghost(&self, view: View, hover: Option<(u16, u16)>) -> Option<Ghost> {
        let material = self.build.material(&self.sim)?;
        let tiles = if let Some((Tool::Build, a)) = self.preview_override {
            self.build_tiles((a.x0, a.y0), (a.x1, a.y1))
        } else if let Some(start) = self.drag_start {
            let end = self.drag_tile(mouse_position(), view)?;
            self.build_tiles(start, end)
        } else {
            vec![hover?]
        };
        Some((self.build.building(), material, tiles))
    }

    /// Draw one frame, then apply the UI actions and the clicks.
    fn draw(&mut self, actions: &[Action], view: View) {
        if self.show_world {
            self.draw_world(actions, view);
            return;
        }
        render::clear();
        let size = self.map_size();
        self.loaded.view.draw(&self.camera, size);
        site_layer::draw(&self.loaded.map, &self.camera, view);
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
        weather::draw_ice(&frame);
        overlay::draw_zones(&frame, selected_zone, self.tool == Tool::Stockpile);
        plants::draw_zones(&frame);
        if self.ui_state.fertility || self.tool == Tool::Farm {
            plants::draw_fertility(&frame);
        }
        self.structures.draw_floors(&frame, &self.sprites);
        plants::draw_plants(&frame, &self.sprites);
        overlay::draw_items(&frame);
        self.structures.draw_objects(&frame, &self.sprites);
        overlay::draw_designations(&frame);
        lifemark::draw_graves(&frame);
        actors::draw(&self.sim, &self.camera, view, self.selected);
        let sprite_size = self.camera.zoom.max(12.0);
        for (id, c) in self.sim.colonists().iter() {
            if c.carrying.is_some() {
                let p = self.camera.tile_to_screen(actors::position_tiles(c), view);
                overlay::draw_carried(&frame, p, sprite_size, id);
            }
        }
        lifemark::draw_marks(&self.sim, &self.camera, view);
        self.structures.draw_roofs(&frame, &self.sprites);
        self.structures.draw_blueprints(&frame, &self.sprites);
        if self.show_rooms {
            structures::draw_rooms(&frame);
        }
        if self.show_temp {
            weather::draw_temperature(&frame);
        }
        let mouse = mouse_position();
        let hover_tile = if self.panels.iter().any(|r| r.contains(mouse)) {
            None
        } else {
            self.tile_under(mouse, view)
        };
        if let Some((tool, area)) = self.preview(view) {
            overlay::draw_preview(&frame, tool, area);
        }
        if self.tool == Tool::Build
            && let Some((building, material, tiles)) = self.build_ghost(view, hover_tile)
        {
            self.structures
                .draw_build_preview(&frame, &self.sprites, building, material, &tiles);
            let ok = build_ui::place_commands(&self.sim, building, material, &tiles).len();
            if tiles.len() > 1
                && let Some(first) = tiles.iter().copied().min_by_key(|&(x, y)| (y, x))
            {
                let text = format!("{ok} of {} tiles", tiles.len());
                structures::draw_build_label(&frame, first, &text);
            }
        }
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
            build: &self.build,
            show_work: self.show_work,
            show_rooms: self.show_rooms,
            show_temp: self.show_temp,
            selected: self.selected,
            selected_tile: self.selected_tile,
            hover_tile,
            state: &self.ui_state,
            speed: self.clock.speed(),
            fps: render::fps(),
            zoom: self.camera.zoom,
            map_name: &self.map_name,
            last_event: &self.last_event,
        };
        // Developer aid: PCS_HUD=0 hides the panels, for clean screenshots of the map.
        let ui_actions = if self.hide_hud {
            Vec::new()
        } else {
            hud::draw(&mut ui, &input)
        };
        if !ui_actions.is_empty() {
            self.play(sound::SoundId::UiClick);
        }
        for action in ui_actions {
            match action {
                UiAction::SetSpeed(speed) => self.clock.set_speed(speed),
                UiAction::SetTool(tool) => self.set_tool(tool),
                UiAction::ToggleWork => self.show_work = !self.show_work,
                UiAction::ToggleRooms => self.show_rooms = !self.show_rooms,
                UiAction::ToggleTemp => self.show_temp = !self.show_temp,
                UiAction::ToggleColony => self.ui_state.colony = !self.ui_state.colony,
                UiAction::PickCrop(n) => self.ui_state.crop = n,
                UiAction::NewColony => self.open_world(),
                UiAction::Resume => self.close_menu(),
                UiAction::Quit => self.ui_state.quit = true,
                UiAction::JumpTo(x, y) => {
                    self.camera.center = (f32::from(x) + 0.5, f32::from(y) + 0.5);
                    self.camera.clamp_center(size);
                }
                UiAction::PickBuilding(n) => self.build.pick_building(self.sim.rules(), n),
                UiAction::PickMaterial(m) => self.build.pick_material(self.sim.rules(), m),
                UiAction::SelectColonist(id) => {
                    self.selected = Some(id);
                    self.selected_tile = None;
                }
                UiAction::ToggleFold(fold) => self.ui_state.toggle(fold),
                UiAction::CloseColonist => self.selected = None,
                UiAction::CloseTile => self.selected_tile = None,
                UiAction::Command(command) => self.pending.push(command),
            }
        }
        self.handle_clicks(actions, &ui, view);
        self.panels = ui.panels().to_vec();
    }
}

impl App {
    /// Draw the world map screen and act on what the player chose.
    fn draw_world(&mut self, actions: &[Action], view: View) {
        let click = actions.iter().find_map(|a| match a {
            Action::LeftClick(x, y) => Some((*x, *y)),
            _ => None,
        });
        let typed = std::mem::take(&mut self.typed);
        let Some(world) = self.world.as_mut() else {
            self.show_world = false;
            return;
        };
        match world.draw(view, click, &typed) {
            Some(WorldAction::Back) => self.show_world = false,
            Some(WorldAction::Embark(x, y)) => self.embark(x, y),
            None => {}
        }
    }
}

/// The seed of the first world: `PCS_WORLD_SEED`, or 1.
fn world_seed() -> u64 {
    std::env::var("PCS_WORLD_SEED")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(1)
}

/// Draw an error message instead of the map.
fn draw_error(message: &str) {
    render::clear();
    let mut lines = vec!["Cannot start: content error".to_owned()];
    lines.extend(message.lines().map(str::to_owned));
    let mut ui = Ui::new(mouse_position(), None);
    render::draw_overlay(&mut ui, &lines);
}

#[macroquad::main(window_conf)]
async fn main() {
    let text = include_str!("../../../assets/content/materials.ron");
    let setup = base::content::load_materials("materials.ron", text)
        .map_err(|e| e.to_string())
        .and_then(|materials| Palette::from_materials(&materials))
        .and_then(|palette| Sprites::shipped().map(|sprites| (palette, sprites)));
    let content_hash = base::hash::fnv1a(text.as_bytes());
    let mut state = setup.map(|(palette, sprites)| App::new(palette, sprites, content_hash));
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
        app.debug_world();
        app.debug_setup();
    }
    if let Ok(app) = &mut state {
        app.sound = Some(sound::Engine::load().await);
    }
    let mut frames = 0;
    let mut input = InputState::default();
    loop {
        let actions = input.poll();
        let typed = input::typed();
        match &mut state {
            Ok(app) => {
                app.typed = typed;
                if !app.apply(&actions, render::view_size(), get_frame_time()) || app.ui_state.quit
                {
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

#[cfg(test)]
mod tests {
    use super::*;
    use sim::scenario::sim_from_ascii;

    #[test]
    fn r016_a_death_toast_names_the_place_and_the_cause() {
        let mut sim = sim_from_ascii("c..", 1).unwrap();
        let id = sim.colonists().iter().next().map(|(id, _)| id).unwrap();
        sim.debug_kill(id);
        sim.step(&[]);
        let events = sim.drain_life_events();
        assert_eq!(events.len(), 1);
        let text = life_event_text(events[0]);
        assert!(text.starts_with("A colonist died at ("), "{text}");
        assert!(text.contains("): "), "{text}");
        assert_eq!(
            life_event_text(LifeEvent::Left { x: 3, y: 4 }),
            "A colonist left the colony at (3, 4)"
        );
    }
}
