# Technical Design

Status: Draft 0.1. The lead approved it. Open questions remain (see `DECISIONS.md` §1). No code exists yet. Sprint 0 makes the workspace.

This document is the canonical source for the architecture, module boundaries, the dependency policy, the required checks, the performance targets (`P-nn`), and the test strategy. Game rules are in `GAME_DESIGN.md`. Decision records are in `DECISIONS.md`.

Writing rule: This document uses ASD-STE 100 Simplified Technical English. Code names are written as `code`.

## 1. Technical goals

1. **Deterministic.** The same seed, the same content, and the same command log give the same state. This is true on Windows and Linux.
2. **Headless.** The simulation runs and is tested without a window.
3. **Lightweight.** Few dependencies. Single-threaded. No background work.
4. **Reliable saves.** Save, then load, gives an identical state.
5. **Measurable.** Each performance target has a benchmark.
6. **Easy to extend.** Clear crate boundaries. Plain data and plain functions. No framework.

## 2. Toolchain

- Rust stable, edition 2024. `rust-toolchain.toml` sets the version. Sprint 0 selects the current stable version.
- Tools: `cargo`, `rustfmt`, `clippy`.
- Development platform: Windows 11. Check platform: Linux in CI.
- Repository: `https://github.com/Kinheadpump/PCS` (D-028). CI: GitHub Actions on Windows and Linux.

## 3. Workspace structure

```
Cargo.toml              Workspace. [workspace.dependencies] and [workspace.lints].
rust-toolchain.toml
clippy.toml             Disallowed types (see §7).
crates/
  base/                 Shared, deterministic building blocks.
  worldgen/             World map, history, and local map generation.
  sim/                  Colony simulation, commands, events, save and load.
  game/                 Binary. Window, rendering, input, interface.
  tools/                Binary. Headless command-line tools.
assets/
  content/              Content data in RON (materials, items, buildings, recipes, crops, creatures).
  sprites/              PNG sprite sheets.
  fonts/                Bitmap font.
docs/
```

### 3.1 Crate responsibilities

| Crate | Owns | Depends on | Must not |
|---|---|---|---|
| `base` | `Rng`, stable hash, fixed-point type, `Grid<T>`, generational `Arena<T>`, typed IDs, content definitions, content loader and validator. | `serde`, `ron` | Do file I/O, use time, use threads, use floats in state. |
| `worldgen` | `WorldData`, `History`, `EmbarkSummary`, `LocalMapData`. All generation steps. | `base` | Know about colonists, jobs, or rendering. |
| `sim` | `Sim` state, tick, systems, `Command`, `Event`, chronicle, save format. | `base`, `worldgen` (types and local map input), `serde`, `postcard` | Do file I/O, use time, use threads, depend on `macroquad`. |
| `game` | Window, camera, rendering, input mapping, interface, speed control, file I/O for saves. | All crates, `macroquad` | Change `Sim` state directly. |
| `tools` | Command-line tools: `gen-world`, `dump-layers`, `run`, `bench`, `soak`, `hash`, `replay`, `validate-content`, `dump-save`, `assetgen`. | All crates except `game` | Contain game rules. |

Inside `sim`, use modules, not more crates: `time`, `map`, `colonist`, `item`, `job`, `path`, `region`, `build`, `room`, `climate`, `need`, `mood`, `health`, `farm`, `production`, `knowledge`, `site`, `notice`, `creature`, `combat`, `weather`, `fire`, `disease`, `rival`, `migrant`, `alert`, `ledger`, `chronicle`, `command`, `event`, `save`.

The module `chronicle::text` makes chronicle text from `chronicle_templates.ron`. The `game` crate only shows the text.

Inside `worldgen`, use modules: `world` (stage seeds, `WorldParams`, step order), `plates`, `height`, `hydro`, `climate`, `biome`, `geology`, `deposit`, `region`, `history`, `embark`, `local`.

Inside `sim`, Sprint 3 added modules `item`, `job`, `skill`, `stockpile`, and `rules`. Job numbers are in `assets/content/jobs.ron`. The save `FORMAT_VERSION` is 2. `Sim::drain_job_events()` returns `JobEvent` values. The `game` crate has the modules `ui`, `hud`, `overlay`, `tools`, and `inspector`.

### 3.2 Data ownership

- `Sim` owns all colony state. Examples: `map: LocalMap`, `colonists: Arena<Colonist>`, `creatures: Arena<Creature>`, `items: Arena<Item>`, `jobs: JobBoard`, `rng: Rng`, `tick: u64`, `notice`, `chronicle`.
- `Content` is immutable after load. `Sim` holds it in an `Arc<Content>`.
- `WorldData` and `History` are immutable after generation. The save file contains them (§9).
- Derived caches (connectivity regions, rooms, path caches) belong to `Sim`. They are not saved. Load rebuilds them.

### 3.3 Communication between modules

- **Commands in.** The interface sends `Command` values. `Sim::step(&mut self, commands: &[Command])` applies them at the start of the tick. In debug builds and in `tools`, a replay file records each command with its tick. Normal saves do not contain the command log.
- **Events out.** Systems push `Event` values. `Sim::drain_events() -> Vec<Event>` gives them to the game for alerts and sound. The chronicle system reads events inside the sim and keeps chronicle entries in the state.
- **Read access.** Rendering and the interface read `&Sim`. They never change it.
- **Systems** are plain functions, for example `fn update_needs(sim: &mut Sim)`. `Sim::step` calls them in a fixed, documented order.
- **No global state.** No `static mut`, no thread locals, no singletons in `base`, `worldgen`, or `sim`.

### 3.4 Why chains (D-021)

Each value that the player can inspect (mood, Notice, room temperature, work speed, crop growth) is the sum of named contributions. Store the contributions, for example `Vec<(Cause, i32)>`, or compute them with one function that returns the contributions. The interface shows this list. Do not compute a player-visible value in two different places.

## 4. Rendering, window, and input (D-007)

### 4.1 Options evaluated

| Option | Result |
|---|---|
| `macroquad` | **Selected.** Light. No C toolchain on Windows. Batched textures, 2D camera, input, PNG loading. Risk: mainly one maintainer. |
| `winit` + `wgpu` | **Fallback.** Strong support. Heavy dependencies and frequent breaking releases. |
| `miniquad` | Lighter, but we must write the sprite batcher. |
| SDL2 or SDL3 bindings | Stable C library. DLL and build management on Windows. |
| raylib bindings | Needs CMake and a C compiler. Bindings lag behind. |
| `pixels` + `winit` | CPU framebuffer. Uses `wgpu`, so it is heavy. Slow zoom at high resolution. |
| Bevy | Very heavy. Slow compile. Frequent breaking changes. Its ECS conflicts with D-009. |
| `ggez`, `notan` | Low maintenance activity. |

### 4.2 Rules

- Only the `game` crate depends on `macroquad`. Use `default-features = false`. Enable audio only when needed.
- Keep all `macroquad` calls in the `game` modules `render`, `input`, and `ui_draw`. A change of backend must change only these modules.
- Do not use the async or coroutine features of `macroquad` for game logic.
- Sprites are 16 × 16 pixels. Use nearest-neighbor filtering. Tint sprites with the material color.
- Draw only visible tiles. Batch static tile layers by chunk (32 × 32 tiles).
- Art and sound come from `tools assetgen` (D-034). It writes PNG files (our own writer with stored deflate, CRC32, and Adler-32) and 16-bit mono WAV files. Generated files are committed in `assets/`. Sound playback uses the `macroquad` audio feature. Enable it when the first sound exists.

### 4.3 User interface (D-008)

- Use a small immediate-mode UI that we write. It draws with the same sprite batch and a bitmap pixel font.
- Widgets: panel, label, button, toggle, scroll list, table with sortable columns, tooltip, tabs.
- The UI reads `&Sim` and returns `Vec<Command>`. It never changes `Sim`.
- Text input in the MVP is only a numeric seed widget. Save names are automatic (D-027).
- If the UI code becomes larger than approximately 3,000 lines, or free text input becomes necessary, evaluate `egui` for debug panels only. Record the decision in `DECISIONS.md`.

### 4.4 Input

- The `input` module maps keys and mouse actions to `Action` values. A table holds the key bindings.
- Interface code converts `Action` values to `Command` values.

## 5. Simulation data model (D-009)

- Do not use an ECS crate.
- Use plain structs in an `Arena<T>`. The arena is a generational arena (`Vec` of slots with a generation counter). Typed IDs, for example `ColonistId`, prevent mix-ups.
- Iterate arenas in slot order. This gives a deterministic order.
- Use struct-of-arrays layouts only where a benchmark shows a need. Tile layers are separate `Vec`s from the start: terrain, wall, floor, roof, item index, fire intensity, fire fuel, irrigation, flags.
- The local map stores layers as `Vec<Layer>` with one layer in the MVP. This makes a later underground layer possible without a rewrite (Q-002).
- Jobs are small state machines: `GoTo`, `PickUp`, `Work(ticks)`, `Drop`. Do not use behavior trees.

## 6. Simulation update model (D-016)

### 6.1 Time

| Unit | Value |
|---|---|
| 1 game hour | 100 ticks |
| 1 game day | 2,400 ticks |
| 1 year | 40 game days = 96,000 ticks (Q-004) |
| Speed 1× | 40 ticks per real second |
| Speed 3× | 120 ticks per real second |
| Speed 6× | 240 ticks per real second |
| Benchmark speed | No limit (tools only) |

- The game uses a time accumulator. It runs a maximum of 60 ticks per frame. If the computer is too slow, the game speed decreases. The game does not freeze.
- Pause and speed are game state, not `Sim` state.

### 6.2 System schedule

All schedules depend only on the tick number. This keeps them deterministic.

| System | Frequency |
|---|---|
| Commands, movement, job steps | Each tick |
| Path requests | Each tick, with a budget (§6.3) |
| Needs and mood | Each 10 ticks, staggered by `(id + tick) % 10` |
| Job assignment for idle colonists | Each 10 ticks |
| Weather for day + 3 | Each 2,400 ticks (1 game day) |
| Outdoor temperature, room temperature | Each 100 ticks (1 game hour) |
| Fire spread | Each 10 ticks |
| Crop growth, spoilage | Each 100 ticks |
| Creature behavior | Each 10 ticks, staggered like needs |
| Notice, beast spawn checks, migrant checks | Each 2,400 ticks (1 game day) |
| Room detection | Only when walls, doors, or roofs change |
| Connectivity regions | Only when passability changes |

### 6.3 Pathfinding

- Grid A* with 8 directions. Integer costs 10 and 14. Octile heuristic.
- Tie-break by f, then h, then tile index. This makes paths deterministic.
- Reuse search buffers. Use generation stamps, so the buffers are not cleared for each search.
- Connectivity regions: a flood-fill region ID for each tile. A job in a different region is rejected immediately.
- A path request queue with a budget for each tick: a maximum of 20 searches or 50,000 expanded nodes. Colonists and creatures use the same queue and budget.
- Each colonist keeps its path. When a tile changes, a path is checked again only if the path contains that tile.
- Hierarchical pathfinding is a later option. Add it only if a benchmark shows a need.

### 6.4 Job assignment

- The `JobBoard` holds jobs with a type, a priority, a location, and required items.
- An idle colonist selects a job by: player priority, skill, estimated distance in the same connectivity region. The job ID breaks ties.
- Reservations on items and tiles prevent two colonists from taking the same item or tile.

## 7. Determinism (D-010, D-011, D-012)

- **No floats** in the state or logic of `base`, `worldgen`, or `sim`.
  - Temperature: `i16` in tenths of a degree Celsius.
  - Positions: tile coordinates plus sub-tile progress as `u16`.
  - Quantities: `u32`.
  - Fractions: a fixed-point type in `base` (`i32`, 16.16) only where needed.
  - Curves (latitude, seasons): lookup tables. Do not use `sin`, `cos`, `exp`, `powf`, or `ln`.
  - The `game` crate can use floats for rendering.
- **Own random number generator** in `base`: SplitMix64 for seeds and hashing. PCG32 for streams. Known-answer tests freeze the output. Do not use the `rand` crate.
- **Seed derivation:** each worldgen stage seed is `mix(world_seed ^ STAGE_TAG)`. Each stage has a fixed tag constant. Values for a location use stateless hashing of the stage seed and the coordinates. A change in one stage does not change the random values of another stage.
- **Gameplay random values** use the `Sim` RNG stream. The save file contains its state.
- **Collections:** `HashMap` and `HashSet` are not permitted in `base`, `worldgen`, and `sim`. `clippy.toml` `disallowed-types` enforces this. Use `Vec`, sorted `Vec`, or `BTreeMap`. Always sort with an explicit tie-break.
- **No time, threads, or environment reads** in `base`, `worldgen`, or `sim`.
- **State hash:** a stable 64-bit hash (FNV-1a over the `postcard` bytes of the state). Do not use `std::hash::DefaultHasher`. The debug overlay shows the hash.
- **Quantization rule for worldgen:** each stage writes integer outputs. The next stage reads only these outputs.

## 8. World generation (D-002, D-019)

All steps are single-threaded and use integer math. Each step has a stage hash test (§11).

| Step | Method | Output |
|---|---|---|
| 1. Seeds | Stage seeds from the world seed. | Stage seeds |
| 2. Plates (stretch task) | 12 to 20 jittered Voronoi plate seeds. Continental or oceanic. Drift vector. Uplift at converging borders. Fallback: ridged noise for mountain ranges. S8 uses the fallback (D-044); `plates.rs` holds the ridged noise. | Plate ID, uplift, volcanic flag |
| 3. Height | Plate base + uplift + fBm (5 to 6 octaves of integer value or gradient noise) + one domain warp. Ocean at the map edges. | `i16` height |
| 4. Sea level | Percentile of heights. The default land fraction is 40%. | Sea level |
| 5. Lakes | Priority-Flood depression filling (binary heap, ties broken by cell index). Deep depressions become lakes with an outlet. | Filled height, lake ID |
| 6. Temperature | Latitude table − lapse rate × height + noise. Seasonal amplitude from latitude and distance to ocean. | `i8` mean °C, amplitude |
| 7. Moisture | Wind bands by latitude. A row sweep carries moisture. Mountains remove moisture (rain shadow). | `u8` moisture |
| 8. Rivers | D8 flow directions follow the parent pointers of the Priority-Flood fill (D-045). All tunables are in the `WorldParams` struct, not in `.ron` files. Flow accumulation in descending height order. River above a threshold. 3 size classes. | River flag and size |
| 9. Biomes | Lookup table: 6 temperature bands × 6 moisture bands. Overrides: ocean, lake, glacier, high mountain, coast, floodplain, volcanic. | Biome ID |
| 10. Geology | Provinces from a jittered lattice (cell size 16). Each province has a setting (plain, mountain, volcanic, coast) and 2 or 3 distinct rock layers. Each cell has a soil depth in decimeters. Step 10 has its own seed and hash (`LayerStage`, D-047). | Province grid, setting, rock layers, soil depth |
| 11. Deposits | A stateless hash roll `roll(deposit_seed, cell, mineral)` for each land cell and mineral. Weighted by host rock affinity. A cell holds one deposit: the mineral with the largest margin, ties to the lowest mineral index (D-048). Richness 1 to 3. | Deposit mask, mineral, richness, host layer |
| 12. Regions | One seed for each 8 × 8 block that has land. A cost-ordered flood fill (binary heap, ties by cell index) with ridge, river, and slope costs. Ocean and lake are walls, so isolated islands become their own regions. Pieces that are not connected merge into a neighbor. Regions under 24 cells merge into the neighbor with the longest border. The adjacency graph has sorted edges with border length and travel cost. The repair step gives each region with no deposit one richness-1 deposit on its best host rock. The repair result is a separate overlay `Regions.deposits`. It does not change the step 11 output (D-047). | Region ID, region graph, repaired deposit overlay |
| 13. History | §8.1 | `History`, cell overlays |
| 14. Embark | Read the selected cell, its 8 neighbors, and the site records. | `EmbarkSummary` |
| 15. Local map | §8.2 | `LocalMapData` |

### 8.1 History simulation

- Runs on the region graph. 1-year steps for event rolls. Heavy updates (population, territory, relations) each 10 years.
- Entities in `Vec`s with `u32` IDs: `Civilization`, `Settlement`, `Figure`, `Artifact`, `Site`, `Beast`, `Event`.
- `Event` is a compact struct: year, kind, up to 3 actor IDs, region or site, extra value. Events contain no strings. Text comes from templates when the player reads it.
- MVP event types (10). Each one writes a map result (R-040):

| Event type | Map result |
|---|---|
| FoundCiv | Civilization state |
| FoundSettlement | Site (settlement), owner overlay |
| OpenMine | Site (mine), old tunnels |
| MineDepleted | Depletion overlay |
| BuildRoad | Road overlay |
| Battle | Site (battlefield) |
| Raze | The settlement becomes a ruin site |
| CivCollapse | Civilization state. Its settlements become ruins. |
| ArtifactLost | Artifact location in a site |
| Seal | Sealed site with a dormant threat |

- Add more event types only with a map result and a test.
- Each civilization has scalar drives: expansion, aggression, prosperity. Rolls use `hash(history_seed, year, civ_id, roll_kind)`.
- Homeostasis: keep 2 to 8 civilizations alive. Spawn migrants if there are too few. Increase collapse pressure if there are too many.
- Hard limits: civilizations ≤ 32 (≤ 8 alive), settlements ≤ 600, figures ≤ 2,000, artifacts ≤ 300, sites ≤ 1,500, events ≤ 50,000. At a limit, the history skips rolls of that kind in a deterministic way.
- Cell overlays at the end: owner, road, depletion (`u8`), sanctity (`i8`).
- Implementation (Sprint 10): `generate_history(&WorldData, years)` returns `History`. It has its own seed (`mix(world seed ^ tag)`) and two hashes (`HistoryStage`: records, overlays). It does not change `WorldData`. A short history is a prefix of a long history, because the loop does not read the number of years. Regions with fewer than 24 cells are ignored (D-050). Each region holds at most one settlement. A civilization loses its last settlement only if more than 2 civilizations are alive. This keeps the alive count in the range 2 to 8 by construction. Figures are not in Sprint 10 (D-052).

### 8.2 Local map generation

- Input: `EmbarkSummary`. In Phase 1, test presets create the `EmbarkSummary`. In Phase 2, the world creates it. Local generation does not read `WorldData` directly.
- Terrain: local detail noise + biome features (tree groups, boulders, ponds). Mountain areas are solid rock wall tiles. Rock material depends on the depth into the mountain and the rock layers.
- Deposits: veins in rock. Richness is reduced by depletion.
- Roads: paved tiles between the edges given by the summary.
- Legends: local generation makes the legends with the local RNG stream (R-044).
- Sites: small prefab templates stamped at the site location. The culture selects the materials. The age controls decay: rubble, overgrowth, rotted items. Sites older than 1,000 years are partly buried.

## 9. Save and load (D-013)

- Format: `serde` + `postcard`, with our own header:
  - magic bytes `COLSAVE1` (neutral name, see Q-001)
  - `format_version: u32`
  - `content_hash: u64`
  - `state_hash: u64`
  - payload: `WorldData`, `History`, `Sim` state.
- Before version 1.0: if `format_version` is different, refuse the save with a clear message. After version 1.0: keep frozen structs for each old version and write explicit migration functions.
- Do not use `#[serde(skip)]` for data that affects the simulation. Derived caches are rebuilt on load.
- The `game` crate writes to a temporary file and then renames it. It keeps one backup.
- `tools dump-save` writes RON for comparison.
- Ironman and free-save modes use the same format (Q-003). Ironman writes a save at 00:00 of each game day and at exit.

## 10. Content data (D-014)

- RON files in `assets/content/`: materials, rocks, deposits, items, buildings, recipes, crops, wild plants, creatures, site templates, legend templates, chronicle templates.
- `worldgen` also embeds `history.ron`, `cultures.ron`, and `event_templates.ron` (D-051). `history.ron` holds all history numbers. `cultures.ron` holds the culture drives. `event_templates.ron` holds one template for each event type, in `EventKind` order.
- `worldgen` does no file I/O. It embeds `rocks.ron` and `deposits.ron` with `include_str!` and parses them once with `base::content` (`load_rocks`, `load_minerals`). The file order feeds the hashes and must match the `Rock`, `Setting`, and `Mineral` enums. `rocks.ron` has `rocks` and `settings` (each with a `pool`). `deposits.ron` has `minerals` (each with an `affinity` list).
- All numbers in content are integers: tenths of °C, per-mille (‰), or whole units. The loader refuses float literals.
- Load into `Raw*` structs with `serde`. Then validate and link into `Content`:
  - Convert string IDs to dense typed IDs, for example `MaterialId(u16)`.
  - Report missing references, duplicate IDs, and values out of range, with the file and entry name.
  - Collect all errors. Do not stop at the first error.
- `content_hash` is a stable hash of the loaded content. The save file contains it.
- `tools validate-content` runs in CI.

## 11. Test strategy

### 11.1 Test types

| Type | Location | Examples |
|---|---|---|
| Unit tests | In each module | RNG known-answer values, arena ID reuse, fixed-point math, A* on small maps, reservation rules. |
| Scenario tests | `crates/sim/tests/` | Small maps from ASCII text. Example: "a colonist with no food and a stockpile of meals eats within 1 game day". |
| Invariant tests | `crates/*/tests/` | Seeded loops with our RNG over 1,000 cases. Examples: no item has two reservations, no colonist is inside a wall, item counts are conserved. |
| Golden hash tests | `crates/*/tests/golden.txt` | Worldgen stage hashes for seeds 1 to 3. Sim hash after N ticks of a scenario. Regenerate only with `UPDATE_GOLDEN=1`, in a separate commit with a reason. |
| Save tests | `crates/sim/tests/` | (1) save → load → equal hash. (2) save → load → run 1,000 ticks equals run 1,000 ticks without save. (3) seed + command log reproduce the final hash. (4) damaged bytes return `Err` and never panic. |
| Distribution tests | `crates/worldgen/tests/` | Multi-seed statistics (§11.3). |
| Soak tests | `tools soak` | 1,000,000 ticks with invariant checks each 1,000 ticks. Release build. Run before each milestone. |
| Benchmarks | `tools bench` | §12. Results go to `bench/results.csv`. |

### 11.2 Required checks

Run these commands before each change is complete:

```
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

CI also runs `tools validate-content` and the golden hash tests on Windows and Linux.

### 11.3 Worldgen acceptance rules

1. The same seed gives identical stage hashes on Windows and Linux.
2. A change in stage K does not change the hashes of the stages before K.
3. Land fraction = target ± 0.5%.
4. Over 50 seeds: no non-ocean biome covers more than 35% of the land. A minimum of 8 biomes exist on 90% of seeds.
5. Each river path ends in ocean or a lake. There are no cycles.
6. The mean temperature of the equator band is higher than the polar band. Mean temperature decreases with height.
7. On average, moisture behind major mountain ranges is lower than in front of them.
8. History limits hold over 100 seeds. Alive civilizations stay in the range 2 to 8 for 90% of years. Each ruin site refers to a destruction or abandon event. Each lost artifact has a valid site. Event years do not decrease.
9. On 80% of seeds, a minimum of 30% of habitable cells have a site record or a road within their embark area.
10. Each deposit lies on a host rock with an affinity above 0 for its mineral. Richness is 1 to 3. No deposit lies on ocean or lake. Each province has 2 or 3 distinct rock layers.
11. Each land cell is in exactly 1 region. No ocean or lake cell is in a region. Each region is one 4-connected area. The land region count is 300 to 500 on 90% of seeds. Each region has a minimum of 1 deposit type in `Regions.deposits`. The region graph is symmetric and sorted. The tests use 30 seeds, because the world size is fixed at 256 × 256.

## 12. Performance targets

All values are **assumptions** (A-002). A prototype must verify them. Measure on the development machine in a release build. Record the CPU model with each result. The roadmap refers to targets by ID.

### 12.1 Targets

| ID | Measurement | Target |
|---|---|---|
| P-01 | Tick time, 100 colonists, 256² map | Average ≤ 1 ms, 99th percentile ≤ 4 ms |
| P-02 | Tick time, 300 colonists, 256² map (stress) | Average ≤ 3 ms |
| P-03 | One A* path across a 256² map | ≤ 0.5 ms |
| P-04 | Path budget for each tick | ≤ 1 ms |
| P-05 | World generation, steps 1 to 12 | ≤ 300 ms |
| P-06 | History, 3,000 years | ≤ 1 s |
| P-07 | Local map generation | ≤ 200 ms |
| P-08 | Frame rate, 1080p, full map visible, integrated GPU | Mean frame time ≤ 16.7 ms (60 FPS) |
| P-09 | Memory, 100 colonists | ≤ 300 MB |
| P-10 | Save file, 256² map, 100 colonists | ≤ 10 MB |
| P-11 | Save or load time | ≤ 500 ms |
| P-12 | Incremental debug build of `game` | ≤ 5 s |

Measurement tools:

- `tools bench`: tick time median, 95th percentile, and maximum. Generation times. Peak memory.
- In-game debug overlay: tick time, frame time, entity counts, path queue length, state hash.
- `cargo build --timings` for build time.
- A sampling profiler for deep analysis (for example `samply` or Windows Performance Analyzer).

### 12.2 Build profiles

```toml
[profile.dev]
opt-level = 1
[profile.dev.package."*"]
opt-level = 3
```

## 13. Dependency policy (D-022)

1. Use the Rust standard library first.
2. Add a crate only if our own code would cost more or have more risk.
3. Declare each dependency only in `[workspace.dependencies]`.
4. Record each dependency in the table below with its reason, before the code uses it.
5. Do not add a dependency to `base`, `worldgen`, or `sim` that uses threads, time, or random state.
6. Dev-dependencies follow the same rules.

### 13.1 Approved dependencies

| Crate | Used in | Reason |
|---|---|---|
| `macroquad` (default features off) | `game` | Window, input, GPU texture batching, camera, PNG loading. It replaces 5,000 to 10,000 lines of platform and graphics code. |
| `serde` (derive) | `base`, `worldgen`, `sim` | Standard serialization. Manual serialization for each type has high maintenance cost and risk. |
| `postcard` (alloc) | `sim`, `tools` | Compact and stable binary save format. |
| `ron` | `base`, `tools` | Human-editable content files with enums and nested data. Debug save dumps. |

Sprint 0 pinned `macroquad` 0.4.16 and `serde` 1.0.229 in the workspace. `postcard` 1.1.3 and `ron` 0.12.2 are current. Add them when the first code uses them.

### 13.2 Candidates for later (not approved)

| Crate | Possible use | Condition |
|---|---|---|
| `egui` + backend | Debug panels | UI code too large, or text input needed. |
| `lz4_flex` | Save compression | Save size above target. |
| `proptest` | Property tests with shrinking | Our invariant loops are not sufficient. |

## 14. Rules for code

These rules are short. `AGENTS.md` repeats the most important ones for agents.

- `unsafe_code = "forbid"` in `base`, `worldgen`, and `sim`.
- No `unwrap()` or `expect()` on data from files or saves. Return `Result`.
- `unwrap()` is permitted in tests and for proven invariants with a comment.
- Public items have a short doc comment.
- Do not add traits for one implementation. Do not add plugin systems or system registries.
- Prefer plain `fn` systems and plain structs.

## 15. Technical risks

| Risk | Control |
|---|---|
| Determinism drift (floats, hash map order, RNG order) | §7 rules. Golden hashes on two platforms from Sprint 0. Save-then-continue test. |
| Pathfinding and job cost at scale | Connectivity regions. Path budget. A 300-colonist benchmark from Sprint 2. |
| History scope creep | Fixed event list. Hard limits. Time limit test. |
| The UI becomes the largest cost | Small fixed widget set. Debug panels from key and value lists. |
| `macroquad` maintenance | Isolation in three modules. Fallback is `winit` + `wgpu`. |
| Save compatibility | Version header. Pre-1.0 policy. Migrations after 1.0. |
| Agents add dependencies or frameworks | Dependency table. Review rule. Workspace dependency list. |
| Slow debug builds on Windows | Profile settings in §12. Headless `tools` runner. |
| Hidden state outside `Sim` | Commands are the only input. Replay tests prove it. |
