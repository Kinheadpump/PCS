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
| Q-001 | What is the working title? | "Ruinstead", after a trademark and store name search. | "Hearthfall", "Under Old Stars", a new name. | Names in the window title and the store. | Nothing. Code uses neutral names. |
| Q-002 | Do we add an underground layer? | No z-levels. Mountains are solid mineable tiles. One deep layer is possible in Phase 6. Tile storage supports layers from the start. | Full z-levels. 2 or 3 fixed layers in the MVP. | Z-levels multiply the cost of pathfinding, rendering, temperature, and UI. | Nothing before Phase 6. |
| Q-003 | Which save mode is the default? | Ironman by default. Free saves are an option. | Free saves only. Ironman only. | Failure is the central experience. Free saves remove the consequences. | S15. |
| Q-004 | How long is a year? | 40 game days (4 seasons × 10 days), approximately 40 real minutes at 1×. | 60 days (more building time). 24 days (fast runs). | Controls the pace of the game and the balance of food and fuel. | Nothing. The value is content data. |
| Q-005 | Do we add legacy worlds (a dead colony becomes a site in the next run)? | Yes, in Phase 4. | A new world for each run. | High value for low cost. It uses the site system. It changes the save structure. | Nothing before Phase 4. |
| Q-006 | Which tone and magic level? | Grounded, low magic. Serious, with some dark humor. | High fantasy with visible magic (more art and systems). | Affects art, content, and the legend text. | Content in S10 and S14. |
| Q-007 | Who makes the art? | The developer makes 16 × 16 placeholder sprites. Tint by material color. Get final art later. | Buy an asset pack (license risk, style limits). Commission art now. | Art can block playtests. | Nothing. Placeholders are sufficient. |
| Q-008 | Where is the repository hosted? | **Answered 2026-10-19:** `https://github.com/Kinheadpump/PCS` (D-028). | — | — | — |
| Q-009 | How detailed is combat in the MVP? | Melee only. Health and the condition "wounded". No body parts. Ranged combat in Phase 4. | Automatic abstract combat. Body regions in the MVP. | Combat cost can grow fast. | S13. |
| Q-010 | Which platforms do we support? | Windows first. Linux in CI. macOS later and not tested. | Windows only. All three from the start. | Affects CI and test cost. | S0 task 9. |

## 2. Decisions

| ID | Decision | Status | Reason | Alternatives rejected |
|---|---|---|---|---|
| D-001 | Colonists are individual. The player controls them indirectly. Scale targets are in `GAME_DESIGN.md` §7.1. | Confirmed | Emergent stories need named persons. A small scale is possible for a small team. | Abstract population (loses stories, needs heavy optimization). Direct control (micromanagement). |
| D-002 | The world has two levels: a 256 × 256 world map and a local map. | Confirmed | History runs cheaply on the world. Detail exists only where the colony lives. | One large detailed map (too expensive). |
| D-003 | No z-levels in the MVP. Mountains are solid mineable tiles. Tile layers are a `Vec<Layer>`. | Proposed (Q-002) | Controls cost. Keeps a later layer possible. | Full z-levels. Fixed layers. |
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
| D-017 | A year has 4 seasons of 10 days. | Proposed (Q-004) | A pace of approximately 40 minutes for each year. | 60 or 24 days. |
| D-018 | One outdoor temperature for the full local map. Room temperature from insulation. No tile-to-tile heat diffusion. | Confirmed | Low cost. Easy for the player to understand. | Per-tile heat diffusion. |
| D-019 | History runs on the region graph for 3,000 years by default, with hard limits. Each event type must write a map result. | Confirmed | Bounded time and memory. History must change gameplay. | Per-person simulation. Unlimited history. |
| D-020 | MVP history features are the five features in `GAME_DESIGN.md` §9.3. Hostile descendant civilizations wait for Phase 4. | Confirmed | Best value for cost. Resolves a conflict between the design review (sealed sites first) and the world review (civilizations first). Raids need more combat and AI. | Hostile civilizations in the MVP. |
| D-021 | Each player-visible derived value is a sum of named contributions (why chain). | Confirmed | Pillar 3. Makes deaths and failures understandable. | Hidden formulas with tooltips added later. |
| D-022 | MVP dependencies are the crates in `TECHNICAL_DESIGN.md` §13.1. Nothing more without a record. | Confirmed | Lightweight. Each one replaces large, risky code. | `rand`, `log`, `clap`, `criterion`, ECS crates. |
| D-023 | Excluded systems are the list in `GAME_DESIGN.md` §16.3. | Confirmed | High cost, low value for the core loop. | — |
| D-024 | The local map is 256 × 256 tiles. | Confirmed | Resolves a conflict: the design review suggested 200 × 200. A power of two fits 32 × 32 chunks. The performance difference is small. | 200 × 200. 512 × 512 in the MVP. |
| D-025 | All project documents use ASD-STE 100 Simplified Technical English. | Confirmed | Owner requirement. Clear text for humans and agents. | — |
| D-026 | Our own benchmark harness and invariant test loops. No `criterion`. `proptest` only if needed. | Confirmed | Fewer dependencies. Sufficient for current needs. | `criterion`. |
| D-027 | The MVP has no free text input. The only input field is a numeric seed widget. Save names are automatic. | Confirmed | A text input widget with cursor, selection, and keyboard layouts is a large cost. | Our own text widget. `egui`. |
| D-028 | The repository is `https://github.com/Kinheadpump/PCS`. CI uses GitHub Actions on Windows and Linux. | Confirmed (owner, 2026-10-19) | The owner made the repository. GitHub Actions has Windows and Linux runners. | Local checks only. |
| D-029 | The plan has 17 sprints (S0 to S16) in `ROADMAP.md`. It replaces the first plan with 15 sprints. | Confirmed | The quality review found sprints with too much work (needs and climate together, the full Phase 3 readability work in one sprint). Each sprint now delivers its own readability work. | 15 sprints with a readability sprint. |
| D-030 | Scope cuts from the quality review: no drought, cave-ins, fishing, water need, well, cart, salt, or chronicle export in the MVP. One metal (iron). Legends have two reliability levels. | Confirmed | Each cut item has low value for the core loop or needs a new model. The items are in the backlog (`ROADMAP.md` §9). | Keep the items in Phase 3. |
| D-031 | `AGENTS.md` is the canonical file for agent rules. `CLAUDE.md` imports it and adds only rules for Claude Code. | Confirmed (owner request, 2026-10-19) | Many AI tools read `AGENTS.md`. One canonical file prevents conflicts. | Two full copies. |

## 3. Assumptions

These are not verified. Check them at the given time.

| ID | Assumption | Check at |
|---|---|---|
| A-001 | One sprint is approximately 2 weeks of work for one developer with AI agents. The full plan to M3 is approximately 34 weeks. | End of S1. Adjust the plan. |
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
| Sprint size: the quality review found sprints with too much work. | 17 sprints (D-029). Scope cuts (D-030). |
