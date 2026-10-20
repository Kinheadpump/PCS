# Decisions

This document is the canonical record of decisions (`D-nnn`), open questions (`Q-nnn`), and assumptions (`A-nnn`). Other documents refer to these IDs. They do not repeat the full reasons.

Writing rule: This document uses ASD-STE 100 Simplified Technical English.

## How to use this document

- To change a decision, add a new decision that replaces the old one. Write "Replaced by D-nnn" on the old decision. Do not delete old decisions.
- To answer an open question, write the answer and the date. Then add a decision and update the canonical documents.
- Status values: **Confirmed** (the lead or the owner made the decision), **Proposed** (the default that work uses until the owner answers the related question), **Replaced**.
- A decision is a choice. An assumption is a fact that is not verified. Keep them separate.

## 1. Open questions [DECISION NEEDED]

Work continues with the recommended choice until the owner answers.

| ID | Question | Recommended | Alternatives | Why it matters | Blocks |
|---|---|---|---|---|---|
| Q-018 | Do floods and weather events go into the MVP, and when does the sound engine start? | Floods and storms in Phase 4 (S27), because they need a river level model. The sound engine in S6, ambience in S16, music in S15. | Floods in the MVP (adds a sprint before M3). Sound only after M3. | Floods change the world model (see Q-016). Sound helps alerts and readability early. | S27, S6 |
| Q-016 | The owner wants a technically infinite world. The world is now fixed at 256 × 256 cells. How does an infinite world fit with the zoom, the world map, history, and gameplay? | **Owner, 2026-10-20:** not in the MVP, but early, because it interacts with other mechanics. Proposed: a design spike S11a right after M2 and before S12. The spike studies a world of chunks that generate on demand from the seed, with the world map as a zoomed-out view and history made per region. | Keep the fixed world and add more worlds or expeditions. Wrap the world at the edges (excluded in `GAME_DESIGN.md` §16.3). | World steps 5 to 12 (rivers, regions, history) read the whole world. An infinite world needs steps that work on chunks. P-05 and the golden hashes change. | S11a spike |
| Q-017 | Who reviews the controls and the game feel, and when? | The `controls-reviewer` subagent (a new file in `.claude/agents/`) at M1, and again in S19. | One review only at M3. | Controls must fit the real gameplay. | M1 |
| Q-015 | The owner wants a few mystical races, with rules for who can have offspring with whom and who can fall in love with whom. Q-006 says grounded, low magic. How do they fit? | **Owner, 2026-10-20:** races should exist, a few only (3 or 4). Proposed: races are other peoples with different bodies, lifespans, and customs, and no spells, so Q-006 stays. **Answered 2026-10-20:** races without spells are enough for now. Magic abilities can come later. | Races with magic (changes Q-006). No races. | A race needs rules for age, attraction, and offspring. Spells need a magic system, which is out of scope. | L6 (S25) |
| Q-014 | Do we plan colonist life (own goals, relationships, families, children, growth from inside)? | Yes. A small MVP slice in S15 (bonds, background line). The full system in Phase 4 after M3 (`ROADMAP.md` §7.1). | Only bonds in the MVP and the rest in the backlog. The full system in the MVP. | The owner wants colonists with their own lives. The MVP scope and the S15 size change. | S15 scope |
| Q-001 | What is the working title? | **Answered 2026-10-19:** keep "Ruinstead" as the working title only. | — | — | — |
| Q-002 | Do we add an underground layer? | **Answered 2026-10-19:** no underground layer now (D-003 is Confirmed). Phase 6 can look at it again. | — | — | — |
| Q-003 | Which save mode is the default? | **Answered 2026-10-19:** ironman (D-032). | — | — | — |
| Q-004 | How long is a year? | **Answered 2026-10-19:** 40 game days as the start value. Playtests tune it (D-017 is Confirmed). | — | — | — |
| Q-005 | Do we add legacy worlds? | **Answered 2026-10-19:** yes, in Phase 4 (D-033). | — | — | — |
| Q-006 | Which tone and magic level? | **Answered 2026-10-19:** grounded, low magic. | — | — | — |
| Q-007 | Who makes the art? | **Answered 2026-10-19:** AI subagents make the art and the sound with Rust code (D-034). | — | — | — |
| Q-008 | Where is the repository hosted? | **Answered 2026-10-19:** `https://github.com/Kinheadpump/PCS` (D-028). | — | — | — |
| Q-009 | How detailed is combat in the MVP? | **Answered 2026-10-19:** melee only, health and "wounded", no body parts. | — | — | — |
| Q-010 | Which platforms do we support? | **Answered 2026-10-19:** Windows first. Linux stays in CI only as a determinism check. | — | — | — |
| Q-011 | Which proposed crises go into the MVP? | **Answered 2026-10-19:** all of them (D-037). | — | — | — |
| Q-012 | How does the colony learn new knowledge? | **Answered 2026-10-19:** lore fragments from ruins, with no story text (D-005 stays). | — | — | — |
| Q-013 | Can the player pay a raiding rival colony? | **Answered 2026-10-19:** yes, one choice between tribute and fight (R-072). | — | — | — |

## 2. Decisions

| ID | Decision | Status | Reason | Alternatives rejected |
|---|---|---|---|---|
| D-001 | Colonists are individual. The player controls them indirectly. Scale targets are in `GAME_DESIGN.md` §7.1. | Confirmed | Emergent stories need named persons. A small scale is possible for a small team. | Abstract population (loses stories, needs heavy optimization). Direct control (micromanagement). |
| D-002 | The world has two levels: a 256 × 256 world map and a local map. | Confirmed | History runs cheaply on the world. Detail exists only where the colony lives. | One large detailed map (too expensive). |
| D-003 | No z-levels in the MVP. Mountains are solid mineable tiles. Tile layers are a `Vec<Layer>`. | Confirmed (owner, Q-002) | Controls cost. Keeps a later layer possible. | Full z-levels. Fixed layers. |
| D-004 | Threats come from Notice, the world, and history. There is no storyteller and no event deck. | Confirmed | Supports emergent stories and fair failure. The player controls the risk. | AI storyteller. Random event deck. |
| D-005 | Knowledge comes from lore fragments and the start set. There is no research tree. | Confirmed | Connects progression to history (pillar 1). | Research tree. Skill-based unlocks only. |
| D-006 | Phase 1 builds the colony survival loop on test maps before world generation. | Confirmed | The survival loop is the largest design risk. The `EmbarkSummary` boundary removes the dependency on the world. | World generation first (late feedback on the loop). |
| D-007 | `macroquad` for window, rendering, and input. Fallback: `winit` + `wgpu`. | Confirmed | Lightest option with batched textures and no C toolchain. See `TECHNICAL_DESIGN.md` §4.1. | Bevy, SDL, raylib, `pixels`, `ggez`, `notan`. |
| D-008 | Our own small immediate-mode UI with a bitmap font. | Confirmed | Matches pixel art. No extra dependency. Full control. | `egui` (version coupling with backends). |
| D-009 | No ECS. Plain structs in a generational arena. | Confirmed | Small scale. Clear data flow. Simple save. Deterministic order. | `hecs`, `bevy_ecs`. |
| D-010 | No floats in `base`, `worldgen`, and `sim` state and logic. Integers and fixed point only. Content data has only integers. | Confirmed | Removes platform differences from math functions. | `f32` with care. |
| D-011 | Our own RNG: SplitMix64 and PCG32. Stage seeds and stateless coordinate hashing. | Confirmed | Stable output across versions. A change in one stage does not change other stages. | `rand` crate (output can change between versions). |
| D-012 | `HashMap` and `HashSet` are not permitted in the deterministic crates. | Confirmed | Their iteration order changes between runs. | Fixed-hasher maps. |
| D-013 | Saves use `serde` + `postcard` with our header. Before 1.0, old save versions are refused. Normal saves do not contain the command log. | Confirmed | Compact and stable. Low maintenance. | Hand-written format. `bincode`. JSON. |
| D-014 | Content data is in RON files with a validation and link pass. | Confirmed | Supports enums and nested data. One format. Easy for agents to edit. | TOML. A hand-written format. Rust constants. |
| D-015 | Commands go in, events come out. Rendering reads `&Sim`. Single-threaded. | Confirmed | Replay, tests, and a clear boundary. | Shared mutable state. A sim thread with snapshots (not needed yet). |
| D-016 | 1 game hour is 100 ticks. Speed 1× is 40 ticks per second. | Confirmed | Enough time resolution for movement. Fits the tick budget at 6×. | 1 tick per game minute. |
| D-017 | A year has 4 seasons of 10 days. Playtests tune the value. | Confirmed (owner, Q-004) | A pace of approximately 40 minutes for each year. | 60 or 24 days. |
| D-018 | One outdoor temperature for the full local map. Room temperature from insulation. No tile-to-tile heat diffusion. | Confirmed | Low cost. Easy for the player to understand. | Per-tile heat diffusion. |
| D-019 | History runs on the region graph for 3,000 years by default, with hard limits. Each event type must write a map result. | Confirmed | Bounded time and memory. History must change gameplay. | Per-person simulation. Unlimited history. |
| D-020 | MVP history features are the five features in `GAME_DESIGN.md` §9.3. Hostile descendant civilizations wait for Phase 4. Rival colony raids are in the MVP (D-037). | Confirmed | Best value for cost. Resolves a conflict between the design review (sealed sites first) and the world review (civilizations first). Raids need more combat and AI. | Hostile civilizations in the MVP. |
| D-021 | Each player-visible derived value is a sum of named contributions (why chain). | Confirmed | Pillar 3. Makes deaths and failures understandable. | Hidden formulas with tooltips added later. |
| D-022 | MVP dependencies are the crates in `TECHNICAL_DESIGN.md` §13.1. Nothing more without a record. | Confirmed | Lightweight. Each one replaces large, risky code. | `rand`, `log`, `clap`, `criterion`, ECS crates. |
| D-023 | Excluded systems are the list in `GAME_DESIGN.md` §16.3. | Confirmed | High cost, low value for the core loop. | — |
| D-024 | The local map is 256 × 256 tiles. | Confirmed | Resolves a conflict: the design review suggested 200 × 200. A power of two fits 32 × 32 chunks. The performance difference is small. | 200 × 200. 512 × 512 in the MVP. |
| D-025 | All project documents use ASD-STE 100 Simplified Technical English. | Confirmed | Owner requirement. Clear text for humans and agents. | — |
| D-026 | Our own benchmark harness and invariant test loops. No `criterion`. `proptest` only if needed. | Confirmed | Fewer dependencies. Sufficient for current needs. | `criterion`. |
| D-027 | The MVP has no free text input. The only input field is a numeric seed widget. Save names are automatic. | Confirmed | A text input widget with cursor, selection, and keyboard layouts is a large cost. | Our own text widget. `egui`. |
| D-028 | The repository is `https://github.com/Kinheadpump/PCS`. CI uses GitHub Actions on Windows and Linux. | Confirmed (owner, 2026-10-19) | The owner made the repository. GitHub Actions has Windows and Linux runners. | Local checks only. |
| D-029 | The plan has 17 sprints (S0 to S16) in `ROADMAP.md`. It replaces the first plan with 15 sprints. | Replaced by D-037 | The quality review found sprints with too much work (needs and climate together, the full Phase 3 readability work in one sprint). Each sprint now delivers its own readability work. | 15 sprints with a readability sprint. |
| D-030 | Scope cuts from the quality review: no cave-ins, fishing, water need, well, cart, salt, or chronicle export in the MVP. One metal (iron). Legends have two reliability levels. | Confirmed | Each cut item has low value for the core loop or needs a new model. The items are in the backlog (`ROADMAP.md` §9). | Keep the items in Phase 3. |
| D-031 | `AGENTS.md` is the canonical file for agent rules. `CLAUDE.md` imports it and adds only rules for Claude Code. | Confirmed (owner request, 2026-10-19) | Many AI tools read `AGENTS.md`. One canonical file prevents conflicts. | Two full copies. |
| D-032 | Ironman is the only save mode in the MVP. One save file. The game writes it automatically. | Confirmed (owner, Q-003) | Failure must have consequences. | Free saves. |
| D-033 | Legacy worlds (a dead colony becomes a site in the next run) come in Phase 4. | Confirmed (owner, Q-005) | High value. It uses the site system. | A new world for each run only. |
| D-034 | Art and sound come from Rust code. The `assetgen` tool makes 16 × 16 PNG sprite sheets and WAV sound files from seeds and parameters. The `art-generator` and `sound-designer` subagents write and tune the generators. Generated files are committed. | Confirmed (owner, Q-007) | The owner wants AI agents to make the assets with Rust. Generators make consistent palettes and styles. | Hand-made placeholders. Asset packs. |
| D-035 | The game is about the present colony. History is implied. It shapes the map, the resources, and the threats, but the game does not tell a story. Short text (legends, chronicle) is optional information, not the reward. | Confirmed (owner, 2026-10-19) | The owner wants colony management to be the center of the game. | History as the central experience. |
| D-036 | Project subagents are Markdown files in `.claude/agents/`. The `owner-interviewer` subagent prepares questions when the plan is unclear. The lead agent asks the owner. | Confirmed (owner, 2026-10-19) | The owner wants to correct the course early and often. | No interviews. |
| D-037 | All crises marked MVP in `GAME_DESIGN.md` §10.1 are in the MVP: drought and wildfire, food poisoning, wound infection, winter fever with medicine, and melee raids by rival colonies with a tribute choice. The plan has 20 sprints (S0 to S19). This replaces D-029 and the drought cut in D-030. | Confirmed (owner, Q-011, Q-013) | The owner wants many crises to manage in the present. | A smaller MVP. |
| D-038 | `docs/ART_DIRECTION.md` is the source of truth for art. The colonist shirt is gray in `actors.png` and the game tints it. Item sprites are drawn in `crates/game/src/overlay.rs` until they move to `assetgen`. |
| D-039 | Sprint 3 has no tool rule (R-008). Colonists work without tools. The work step in `job.rs` marks the place for the check. R-008 comes with crafting. |
| D-040 | Job numbers are in `assets/content/jobs.ron`: 100 experience per job, 1,000 per level, stack size 50, priorities 1 to 4 (0 is off, default 3). Job selection: lowest priority number, then octile distance, then job index. |
| D-041 | A flagged item gets a haul job only if a stockpile accepts its material. One stack per stockpile tile. A colonist carries a whole stack. |
| D-042 | Sprint 3 added colonist, item, and job state to `state_hash`. The golden hashes `sim_ascii_600_ticks` and `sim_forest_seed1_1000_ticks` changed (commit 62ebe66). The save `FORMAT_VERSION` is 2. |
| D-043 | The player moves a colonist with the right click only when the Select tool is active. This is a debug order. Pan is middle drag. |
| D-044 | S8 skips tectonic plates. Ridged noise makes the mountain ranges (the allowed fallback in `TECHNICAL_DESIGN.md` §8). Plates stay a stretch task. |
| D-045 | River flow direction comes from the parent pointers of the Priority-Flood lake fill, so rivers always reach the sea or a lake outlet. World tunables are in the `WorldParams` struct in code, not in `.ron` files, until the numbers settle. |
| D-046 | Biome balance after S8 is off: temperate forest is 1 to 3% and taiga is 22% of land. Tune `WorldParams` and the biome table before S10 (embark picks). |
| D-047 | World steps 10 to 12 have their own seed and hash arrays (`LayerStage`, `RegionStage`). The hashes of steps 1 to 9 do not change when a step is added. The region repair writes a separate overlay and does not change the step 11 output. Game and UI code reads deposits from `Regions.deposits` to get at least 1 deposit type in each region. |
| D-048 | A world cell holds at most one deposit. The mineral with the largest margin of probability over the roll wins. Ties go to the lowest mineral index. |
| D-049 | The rock and mineral tables are in `assets/content/rocks.ron` and `deposits.ron`. `worldgen` embeds them with `include_str!` and parses them at the start of world generation. It does no file I/O at run time (owner decision). |
| D-050 | Sprint 4 room rules: a door is a border tile, so rooms stop at doors. The flood fill is 4-connected. A room has at most 400 tiles and may not touch the map edge. Detection runs in the tick after a structural change. Rooms are a derived cache, recomputed on load. |
| D-051 | Construction conserves material. Deconstruction returns `floor(units × return_pct / 100)` and the rest is lost by design. A cancelled blueprint returns delivered units. Save format version is 3. Sprint 4 changed the golden hashes `sim_ascii_600_ticks` and `sim_forest_seed1_1000_ticks`. |
| D-052 | Game keys added in Sprint 4: B build, X remove, O rooms overlay, 1 to 7 pick a building. Developer env hooks: `PCS_BUILD`, `PCS_BLUEPRINT`, `PCS_CHOICE`, `PCS_ROOMS`. |
| D-053 | History ignores regions with fewer than 24 cells (`min_region_cells` in `history.ron`) for starts, settlements, mines, and roads. The history has its own seed and hash array (`HistoryStage`), so the hashes of steps 1 to 12 do not change. It reads `Regions.deposits` (the repaired deposits), not the raw deposits. |
| D-054 | The history tables are `history.ron`, `cultures.ron`, and `event_templates.ron`. `worldgen` embeds them with `include_str!` (same rule as D-049). `try_generate_world` checks all embedded tables before it generates a world. `generate_world` keeps its signature. |
| D-055 | Sprint 10 has no figures (notable persons). No event type uses them. The limit of 2,000 figures stays in `TECHNICAL_DESIGN.md` §8.1 for a later sprint. The biome balance of D-046 is not changed: it does not block the history. |
| D-056 | Weather: the offset is a clamped random walk (daily step at most ±3.0 °C, limit ±12.0 °C). It uses its own RNG from the game seed and the day number. The 3-day forecast equals the true weather. |
| D-057 | Room temperature is saved state in milli-degrees, keyed by the first room tile. A new room starts at outdoor temperature. A re-detected room takes the tile-weighted mean of the old values. Each hour a room moves toward its target by a share set by insulation (10% of the gap at insulation 60). The target is outdoor plus hearth heat. A rock room targets the ground temperature (the annual mean). Heat of several hearths adds up. Open question for the owner: the share per hour. |
| D-058 | Hearth: fuel is stored in burning hours (capacity 48). It burns 1 hour per game hour. Firewood and coal give 4 hours per unit. A hearth with 24 hours or less gets a refuel haul job. A new hearth starts with 0 fuel. Without fuel it gives no heat. |
| D-059 | Water with 3 or more tiles to land is deep and never freezes. Ice is global. 3 days with a mean below 0 °C freeze shallow water. 3 days above 0 °C thaw it. A colonist on ice at thaw moves to the nearest land tile. |
| D-060 | Alerts are computed on demand and are not saved. Low fuel: less than 12 hours left. Cold period: offset of −6.0 °C or lower today or in the forecast. Save format version is 4. Golden hashes changed: `sim_ascii_600_ticks` eff68292bc9965e5 to 55c702b6c5485075, `sim_forest_seed1_1000_ticks` 407cda27705b85dc to e88cd008435bccc4. |
| D-061 | The HUD shows no help panel, no key legend, and no hint lines. Tooltips are short and show on hover only. The explanations move to a tutorial in a later sprint. The top bar has the clock, the date, the speed buttons, and the colonist count. The layout reserves a slot for the temperature readout and a strip for alerts (Sprint 5). |
| D-062 | F4 shows a debug text (tick, state hash, FPS, zoom, map, hover tile). The developer hook `PCS_WINDOW=<w>x<h>` sets the window size for screenshots. |
| D-063 | Sprint 5 UI: key Y toggles the temperature overlay. The top bar shows the outdoor temperature with a 3-day forecast tooltip. The alerts strip shows up to 3 chips, and a click centers the camera on the hearth. Ice is drawn as an overlay in code. Only the embark mean temperature is wired into `Sim::set_climate`. Amplitude and moisture keep the sim defaults until worldgen exposes them. Developer hooks: `PCS_TEMP`, `PCS_CLIMATE`, `PCS_FUEL`, `PCS_DAYS`, `PCS_FOLDS`. |
| D-063 | Sprint 11 is split. The `worldgen` part is done first: the embark summary from a world cell and the site stamping. The game and sim part (cell selection, salvage items, legends R-042 and R-043) comes after the HUD rework and Sprint 5. `EmbarkSummary` does not change. Sites go in a second type, `EmbarkSites`, so old code and the local goldens stay valid. The embark area is the 3 by 3 cells. All its sites (at most `max_map_sites`) are on the one local map at positions from a hash of the site ID. Numbers are in `embark.ron`. |
| D-064 | Rule 9 needed more history. The S10 numbers gave 15 to 27 percent coverage of the habitable cells. `settle_pm` is now 200, `mine_pm` 70, `road_pm` 75, and `max_civ_settlements` 24. Coverage is 31 to 46 percent on 20 seeds. Rule 8 holds on 100 seeds. The history golden hashes changed in their own commit. A habitable cell is any land cell that is not ocean, lake, or glacier. |
| D-065 | Biome balance (D-046) is fixed in the biome table of `biome.rs`. The two cool-temperate bands (0 to 9 and 9 to 18 degrees) give temperate forest from moisture 95, and the 9 to 18 band gives temperate forest from moisture 60. Temperate forest is now 9 to 15 percent of the land (it was 1 to 3 percent). Taiga is 3 to 17 percent. The test `biome_balance.rs` needs at least 8 percent temperate forest and at most 30 percent for each biome on 12 seeds. The moisture limits did not change. Only the biome, deposit-region, and later world hashes changed (`golden_world.txt`, `golden_world_s9_regions.txt`). The history and local goldens stay. The volcanic biome does not appear on any seed. That is a separate problem and is not fixed here. |

## 3. Assumptions

These are not verified. Check them at the given time.

| ID | Assumption | Check at |
|---|---|---|
| A-001 | One sprint is approximately 2 weeks of work for one developer with AI agents. The full plan to M3 is approximately 40 weeks. | End of S1. Adjust the plan. |
| A-002 | All performance targets in `TECHNICAL_DESIGN.md` §12.1 are possible on the development machine. | S1 (P-07, P-08), S2 (P-03, P-04), S8 to S10 (P-05, P-06), S16 (all). |
| A-003 | `macroquad` renders a 256² tile map at 60 FPS with chunk batching. | S1. |
| A-004 | `postcard` and `ron` remain stable and maintained. | S0 (check versions). |
| A-005 | 30 to 50 colonists give enough stories for an interesting run. | M1 and M3 playtests. |
| A-006 | A single outdoor temperature for the map with room insulation is enough for interesting climate decisions. | M1 playtests. |
| A-007 | 3,000 years of history give enough sites near most embark cells. | S11 (rule 9 in `TECHNICAL_DESIGN.md` §11.3). |
| A-008 | A roof needs a supporting wall or natural rock within 6 tiles. | S4 and M1 playtests. |

## 4. Specialist review record

The lead used four specialist reviews for version 0.1: game design, world simulation, Rust architecture, and quality and production. The lead resolved these conflicts:

| Conflict | Resolution |
|---|---|
| Local map size: 200² (design) or 256² (world, architecture). | 256² (D-024). |
| Year length: 40 days (design) or 60 days (world). | 40 days, open as Q-004. |
| First external threat: sealed sites (design) or hostile civilizations (world). | Sealed sites in the MVP. Hostile civilizations in Phase 4 (D-020). |
| Colonist scale: 30 to 80 (design) or 50 to 300 (architecture). | 30 to 50 normal, 100 tested in the MVP. 300 is the stress benchmark and the Phase 6 goal (D-001). |
| Content format: RON (architecture) or a hand-written format (world). | RON (D-014). |
| History length: 3,000 years (design) or 1,000 to 2,000 years (world). | 3,000 years default, 1,000 to 5,000 configurable (D-019). |
| Sprint size: the quality review found sprints with too much work. | 20 sprints (D-037). Scope cuts (D-030). |
