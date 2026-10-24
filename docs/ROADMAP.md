# Roadmap

Status: Draft 0.1. The lead approved it. Open questions remain (see `DECISIONS.md` §1). Sprint 5 is on `main`. Current sprint: Sprint 7 is complete in code and UI (food chain, ledger, chronicle, soak). The M1 playtests by a human are open (see §6.1 and D-075). Sprint 12 slice A (tools, locks, chains) is done; the rest of Sprint 12 is open. Sprints 0 to 3 are on `main`. The world generation work of Sprints 8 and 9 (steps 1 to 12) is also on `main`. P-03 (one path) is about 0.55 ms against the 0.5 ms target.

This document is the canonical source for phases, sprints, milestones, the Definition of Done, the playtest record, and the backlog. Game rules (`R-nnn`) are in `GAME_DESIGN.md`. Technical rules, required checks, and performance targets (`P-nn`) are in `TECHNICAL_DESIGN.md`.

Writing rule: This document uses ASD-STE 100 Simplified Technical English.

## 1. Planning rules

- Each sprint has one goal, a testable deliverable, small tasks, acceptance criteria, tests, and risks.
- Sprints have approximately equal effort (A-001).
- A sprint uses only systems from the same sprint or from earlier sprints. Never from a later sprint.
- Each sprint adds the inspector rows, why chains, alerts, and chronicle entries for its own systems. Readability is not a separate sprint.
- Tests for a game rule use the rule ID in their name (`GAME_DESIGN.md`, writing rules).
- Do not add work to an active sprint without a clear reason. Put new ideas in the backlog (§9).
- At the end of each sprint: update the status line at the top of this document and the status in `README.md`.
- At the end of each phase: do the milestone review (§6).

## 2. Definition of Done

A change is done when all of these are true:

1. It meets its acceptance criteria.
2. New rules have tests. The tests pass.
3. The required checks in `TECHNICAL_DESIGN.md` §11.2 pass.
4. Golden hashes did not change, or the task permits the change and the commit message gives the reason.
5. New dependencies are recorded in `TECHNICAL_DESIGN.md` §13.
6. Changed rules or decisions are updated in their canonical document.
7. The report lists each check that ran and its result. It lists each check that could not run and the reason.

Never report that a test passed if you did not run it.

A sprint is done when all of its tasks are done and its deliverable works.

## 3. Phase overview

| Phase | Name | Sprints | Result |
|---|---|---|---|
| 0 | Foundation | S0 | Workspace, deterministic base library, checks. |
| 1 | Colony prototype | S1 to S7 | Colonists survive or fail a first winter on a test map. This tests the core survival loop. |
| 2 | World and history | S8 to S11 | World generation, history, and embark make the local map. |
| 3 | MVP | S12 to S19, and S11b, S11c, S15a, S18a | The full scope in `GAME_DESIGN.md` §16.1. The four lettered sprints are the game shell: main menu, worlds, settings, and saves (§7.5). |
| 4 | External threats | Later | See §7. |
| 5 | Society | Later | See §7. |
| 6 | Scale and variety | Later | See §7. |

**Reason for the order (D-006):** The survival loop is the largest design risk. Phase 1 tests it first on a test map. Phase 2 then adds the world and the history. Local map generation reads an `EmbarkSummary`. In Phase 1, test presets make it. In Phase 2, the world makes it. Thus Phase 1 does not depend on Phase 2.

## 4. Sprint plan: Phase 0 and Phase 1

### Sprint 0: Foundation

**Goal:** Make a workspace with deterministic building blocks and automatic checks.

**Deliverable:** A Cargo workspace. The required checks pass. `tools hash --seed 1` prints a stable hash.

**Tasks:**

1. Run `git init`. Add `.gitignore`.
2. Make the workspace: `Cargo.toml`, `rust-toolchain.toml`, `clippy.toml`, and the crates `base`, `worldgen`, `sim`, `game`, `tools`. (Depends on 1.)
3. Add `[workspace.lints]` and the build profiles from `TECHNICAL_DESIGN.md` §12.2. (Depends on 2.)
4. Write `Rng` (SplitMix64, PCG32) with known-answer tests. (Depends on 2.)
5. Write the stable FNV-1a hash and the `mix` function. (Depends on 2.)
6. Write `Grid<T>`, `Arena<T>` with typed IDs, and the fixed-point type. (Depends on 2.)
7. Write `tools hash`. It makes a grid of random values from a seed and prints its hash. (Depends on 4, 5, 6.)
8. Make `game` open an empty window with `macroquad`. Escape closes it. (Depends on 2.)
9. Add GitHub Actions CI for Windows and Linux. Push to `origin` (D-028). (Depends on 3, Q-010.)

**Acceptance criteria:**

- The required checks pass on Windows.
- RNG output for 3 seeds equals the stored known-answer values.
- `tools hash --seed 1` prints the same value in 3 separate runs, in debug and in release builds.
- A `HashMap` in `sim` causes a clippy error.
- `Arena`: a removed ID is invalid after its slot is used again.
- The window opens and closes without errors.

**Tests:** RNG known-answer tests. Hash known-answer tests. Arena, Grid, and fixed-point unit tests.

**Risks:** CI minutes or runner limits on GitHub. Run the checks locally if CI is not available.

### Sprint 1: Local map and view

**Goal:** Show a generated local map that the player can pan and zoom.

**Deliverable:** `game` shows a 256 × 256 local map from an `EmbarkSummary` test preset.

**Tasks:**

1. Write the content loader and validator in `base`. Add `materials.ron` with approximately 15 materials. (Depends on S0.)
2. Define `EmbarkSummary` and `LocalMapData` in `worldgen`. Add 3 test presets: temperate forest, cold hills, dry plain. (Depends on S0.)
3. Write integer value noise and fBm in `worldgen`. (Depends on S0.)
4. Generate local terrain: soil, water, trees, rock walls, ore veins. (Depends on 1, 2, 3.)
5. Write tile rendering with chunk batching and material tint. Write a camera with pan and zoom. Write the input binding table. (Depends on 4.)
6. Write `tools dump-layers`. It writes PPM images of the local map. (Depends on 4.)

**Acceptance criteria:**

- Content validation reports a missing material reference with the file name and the entry name.
- Content validation refuses a float literal.
- The same preset and seed give the same local map hash.
- Each preset has a minimum of 1 connected rock area of 200 tiles or more and 1 water area. Temperate forest and cold hills have trees.
- P-08 (frame time) and P-07 (local map generation) are met. The results are in `bench/results.csv`.

**Tests:** Content loader tests: valid file, missing reference, duplicate ID, float literal. Local map golden hashes for 3 presets. Terrain invariant tests.

**Risks:** Art. Use placeholder sprites (Q-007).

### Sprint 2: Simulation core, movement, and save

**Goal:** Colonists move with deterministic paths. The state can be saved and loaded.

**Deliverable:** 7 colonists walk to tiles from a debug move command. Save and load work. The UI shows the time and the speed.

**Tasks:**

1. Write `Sim`, `Sim::step`, `Command`, `Event`, and time (ticks, hours, days, seasons). (Depends on S1.)
2. Write the colonist struct with position and sub-tile movement. (Depends on 1.)
3. Write A* with reusable buffers and the tie-break rule. (Depends on 1.)
4. Write connectivity regions. (Depends on 3.)
5. Write the path request queue with a budget. (Depends on 3.)
6. Write the debug move command. Only debug builds and scenario tests use it. (Depends on 2, 5.)
7. Write the save format with its header, and `tools dump-save`. (Depends on 1.)
8. Write the time accumulator and the speeds pause, 1×, 3×, 6×. (Depends on 1.)
9. Write the bitmap font and the first widgets: panel, label, button. Show the time, the speed, and the tile under the mouse. (Depends on 8.)
10. Write `tools bench` with a 300-colonist random-walk scenario. (Depends on 5.)
11. Write the scenario test helper: maps from ASCII text. (Depends on 1.)

**Acceptance criteria:**

- A colonist reaches a reachable target. A move to an unreachable target fails in the same tick with an event.
- The path golden hash is identical over 100 runs and on Windows and Linux CI.
- P-03 (one path) and P-04 (path budget) are met.
- The 300-colonist benchmark runs. Its tick time is in `bench/results.csv`.
- The save tests in `TECHNICAL_DESIGN.md` §11.1 pass.
- At 6×, a headless run of 60 real seconds does 14,400 ticks ±1%.

**Tests:** A* unit tests on small maps. Connectivity tests. Save tests. Replay test. Scenario test: a colonist walks around a wall.

**Risks:** Path cost at scale. Measure early with the benchmark.

### Sprint 3: Items, jobs, and designations

**Goal:** Colonists cut trees, mine rock, and haul items to stockpiles because of player orders.

**Deliverable:** The player designates trees and rock. Colonists do the work. Items go to a stockpile. The inspector shows colonists and tiles.

**Tasks:**

1. Write items with material, stack count, and location. (Depends on S2.)
2. Write the `JobBoard`, the job state machines, and reservations. (Depends on 1.)
3. Write the designations cut, mine, and haul. (Depends on 2.)
4. Write stockpile zones with item filters. (Depends on 2.)
5. Write work priorities and the job selection rule. (Depends on 2.)
6. Write skills: work speed and skill gain (R-010). (Depends on 5.)
7. Write the work panel and the designation tools. (Depends on 3, 4, 5.)
8. Write the inspector for colonists and tiles, with key and value lists. Add the why chain for work speed. (Depends on 6, 7.)

**Acceptance criteria:**

- A designated tree becomes a log item. A designated rock tile becomes a floor tile and a stone item.
- No item has a reservation by two colonists (invariant test, 1,000 seeded cases).
- Hauling does not change the item count (invariant test).
- A colonist with priority 1 for mining selects mining before hauling with priority 2.
- The job board does not give a job in a different connectivity region.
- R-010: work speed at skill level 4 is 120%.

**Tests:** Job state machine unit tests. Reservation invariant tests. Scenario tests for cut, mine, haul, and priorities.

**Risks:** Job selection can be slow with many jobs. Measure with `tools bench`.

### Sprint 4: Construction and rooms

**Goal:** Colonists build structures from materials. The game finds rooms.

**Deliverable:** The player places walls, doors, floors, roofs, beds, a hearth, and a workshop. The rooms overlay shows closed rooms.

**Tasks:**

1. Add the `buildings.ron` and `recipes.ron` formats with validation. (Depends on S3.)
2. Write blueprints and construction jobs that use materials. (Depends on 1.)
3. Write deconstruction. It returns part of the materials. (Depends on 2.)
4. Write room detection by flood fill (R-020, A-008). It runs only after a change to walls, doors, or roofs. (Depends on 2.)
5. Write room insulation (R-021) and room beauty as why chains. (Depends on 4.)
6. Write the rooms overlay and the room inspector. (Depends on 5.)
7. Write the workshop and the recipes log → plank and log → firewood. (Depends on 2.)

**Acceptance criteria:**

- R-020: a closed area with walls, a door, and a roof is one room. An area with one missing wall tile is not a room.
- R-021: a room with 10 border tiles of insulation 60 and 2 border tiles of insulation 30 has insulation 55.
- A wall from granite uses granite items, and the wall has the material granite.
- Room detection runs only in the tick after a structural change (test with a counter).
- Construction does not make or destroy material (invariant test).

**Tests:** Room detection scenario tests from ASCII maps. Construction invariant tests. Content validation tests.

**Risks:** The roof rule (A-008) can feel wrong. Check it in the M1 playtests.

### Sprint 5: Climate

**Goal:** The weather and the rooms control the temperature.

**Deliverable:** The temperature changes with the season, the hour, and the weather. A room with a burning hearth stays warm. Water freezes. The temperature overlay and the forecast work.

**Tasks:**

1. Write the calendar and the season curve. (Depends on S2.)
2. Write the weather: daily offset and rain (R-003). Calculate it 3 days in advance. (Depends on 1.)
3. Write the outdoor temperature (R-002). (Depends on 1, 2.)
4. Write room temperature with the ground reference for rock rooms (R-022). (Depends on 3, S4.)
5. Write hearth heat and fuel use. A hauling job refuels the hearth. (Depends on 4.)
6. Write freeze and thaw of shallow water (R-004). (Depends on 3.)
7. Write the alert module and the first alerts: low fuel, cold period. (Depends on 5.)
8. Write the temperature overlay, the 3-day forecast, and the tile temperature why chain. (Depends on 4.)

**Acceptance criteria:**

- R-002: with weather offset 0, the outdoor temperature equals the formula value for each hour of one year (exact test in tenths of °C).
- R-003: the offset stays between −12 °C and +12 °C over 100 years. The forecast equals the true weather.
- R-022: a 16-tile room with insulation 60 and one burning hearth, at −10 °C outdoors, is at 10 °C or more after 3 game days.
- R-022: a room with 75% or more rock border moves toward the ground temperature.
- A hearth without fuel gives no heat.
- R-004: shallow water freezes after 3 days below 0 °C and thaws after 3 days above 0 °C.

**Tests:** Climate unit tests. Room temperature scenario tests. Freeze tests.

**Risks:** The room model can be too simple (A-006). Check it in the M1 playtests.

### Sprint 6: Needs, health, and mood

**Goal:** Colonists have needs. They can suffer and die.

**Deliverable:** Colonists eat stored meals, sleep in beds, and keep warm. Without shelter in winter, they get hypothermia and die.

**Tasks:**

1. Write food items and eating (R-011). (Depends on S3.)
2. Write rest and sleeping in beds (R-012). (Depends on S4.)
3. Write clothes and the comfort band (R-001). (Depends on S5.)
4. Write warmth, hypothermia, and heatstroke (R-013). (Depends on 3.)
5. Write health, conditions, and death (R-016). (Depends on 1, 2, 4.)
6. Write mood, mood modifiers, and mood effects (R-014, R-015). Add room beauty as a modifier. (Depends on 1, 2, 4.)
7. Write the MVP traits (`GAME_DESIGN.md` §7.2). (Depends on 6.)
8. Add the alerts for hunger, hypothermia, low mood, and death. Add the why chains for mood and health. (Depends on 5, 6.)
9. Done (Q-018): write the sound engine in `game` (play, volume, mixer for effects, ambience, and music). Make the first sound set with `tools assetgen` (`sound-designer` subagent): alert sounds, UI click, footsteps, tools at work. Each alert plays its sound. (Depends on 8.)

**Acceptance criteria:**

- R-013: a colonist in simple clothes at −10 °C gets hypothermia 50 after 5 game hours and dies if nothing changes (scenario test).
- R-013: the same colonist in the warm room from the S5 test does not get hypothermia (scenario test).
- R-011: a colonist without food gets "starving" and loses 2 health each game hour.
- R-015: at mood 10, the colonist stops work for 4 game hours.
- Each mood modifier is visible in the inspector with its value.
- The Hardy trait lowers the comfort band lower limit by 5 °C.

**Tests:** Need scenario tests. Health and death scenario tests. Mood unit tests. Trait unit tests.

**Risks:** Balance of need rates. Keep all rates in the content data.

### Sprint 7: Food chain and the first winter

**Goal:** The player can prepare food for winter or fail.

**Deliverable:** A playable test map. 7 colonists must survive the first winter. Milestone M1.

**Tasks:**

1. Write crops with temperature bands, fertility, sow, and harvest (R-006). Add the fertility overlay. (Depends on S5.)
2. Write wild plants and gathering (R-007). (Depends on S3.)
3. Write cooking (crop → simple meal) and production orders ("keep N"). (Depends on S4.)
4. Write food spoilage (R-005). (Depends on S5, S6.)
5. Write the ledger: stock, trend, and days remaining for food and fuel. (Depends on 3, 4.)
6. Write the start conditions (`GAME_DESIGN.md` §5) and the end-of-run check (R-050). (Depends on 1 to 4.)
7. Write the chronicle core: event entries from `chronicle_templates.ron`, grouped by season. Add entries for deaths and for the first winter. (Depends on S6.)
8. Write `tools soak`. (Depends on 6.)

**Acceptance criteria:**

- R-006: a crop does not grow outside its temperature band. It dies below its kill temperature.
- R-005: food at 20 °C rots 4 times faster than food at 5 °C. Food at −1 °C does not rot.
- "Keep 30 meals" stops cooking at 30 meals and starts again below 30 (scenario test).
- The ledger "days remaining" value is correct within 10% for a constant use rate.
- Each colonist death makes a chronicle entry with its cause.
- `tools soak` runs 1,000,000 ticks on each test preset without an invariant failure.
- 3 playtests of the first year are done and recorded (§6.1). Status: 3 scripted runs are in `docs/playtests/`. They are not human playtests. The owner must do the human playtests (D-075).

**Tests:** Crop unit tests. Spoilage unit tests. Production order scenario tests. Chronicle tests. Soak test.

**Risks:** The loop is not interesting. The M1 review decides if the design must change before Phase 2.

## 5. Sprint plan: Phase 2 and Phase 3

These sprints have less detail. Add the full detail at the start of each phase. Rules 1 to 9 are in `TECHNICAL_DESIGN.md` §11.3.

### Phase 2: World and history

| Sprint | Goal | Deliverable | Main acceptance criteria |
|---|---|---|---|
| S8 World terrain | Generate the world terrain. | `tools gen-world` and `tools dump-layers` write height, temperature, moisture, river, and biome images. | Rules 1 to 7. Status: done on `main` (PR #5). P-05 for steps 1 to 9 is about 40 ms against 300 ms. Biome balance was off (temperate forest 1 to 3%, taiga 22% of land, D-046). It is fixed in D-065 and D-066. P-05 for steps 1 to 9. Plates are a stretch task. Ridged noise is the fallback. |
| S9 Geology, deposits, regions, world view | Complete the world data and show it. | `game` shows the world map with overlays. The seed widget accepts numbers. | Each land region has a minimum of 1 deposit type. Each region is one connected area. P-05. Status: world steps 10 and 11 are done on `main` (PR #9). Step 12 (regions, region graph, deposit repair) is done on `main` (PR #11). The rock and mineral tables are in `rocks.ron` and `deposits.ron`, embedded in `worldgen` (D-049). P-05 for steps 1 to 12 is about 61 ms against 300 ms. The world map view, the seed box, and the cell inspector are done in `game` (D-071). Golden files: `golden_world_s9.txt`, `golden_world_s9_regions.txt`. |
| S10 History | Simulate the history. | `tools gen-world --history` prints a chronicle summary and statistics. | Rule 8. P-06. Approximately 10 event types. Each event type writes a map result (R-040, test). Status: step 13 is in `worldgen::history` (D-053 to D-055). `tools gen-world --seed <n> --history [--years <n>]` prints the chronicle summary and statistics. P-06 is below 20 ms in release against 1 s. Golden file: `golden_history_s10.txt`. |
| S11 Embark and site stamping | Make the local map from the world. | The player selects a world cell. The local map has ruins, roads, old tunnels, and depleted deposits. Sites contain structure and salvage. | Rule 9. R-041. A ruin of age 1,500 years has 2 times or more the rubble of a ruin of age 200 years. Depletion decreases ore (test). The history overlay works. Milestone M2. Status: the `worldgen` part is done (D-063, D-064). `embark_from_world` makes the `EmbarkSummary` and the `EmbarkSites` of a world cell. `generate_local_with_sites` stamps them. Rule 9 holds on 20 of 20 seeds (the lowest coverage is 31%). The cell selection and the embark in `game` are done (D-071). Not done: the salvage items and the legends (R-042, R-043) in `sim`, and the block of movement by stamped walls. M2 playtests wait for them. Tests: `embark_s11.rs`. |

Risks: civilizations that grow or die without limit, local maps that are not different enough, generation time.

### Phase 3: MVP

| Sprint | Goal | Deliverable | Main acceptance criteria |
|---|---|---|---|
| S11b Main menu and new world | Give the game a start screen and a way to make worlds. | The game starts on a title screen. The player makes a world, lists the worlds, deletes a world, and goes to the embark screen. The pause menu has Resume, Settings, Save and quit to menu, and Quit to desktop. | The player enters a seed, or presses Random, and names the world (D-091). The same seed makes the same world (golden world hash). A world record is a small file (name, seed, history length, worldgen version, creation date). The world is made again from the seed on load. The screen state machine is `Title`, `NewWorld`, `Worlds`, `WorldMap`, `Play`, `Pause`. The UI follows `ART_DIRECTION.md` and has no explanation text. Tests: record round trip, bad file gives an error and not a panic, name rules, seed parse. The first start has no world and opens `NewWorld`. Risks: the text widget (D-091), the `main.rs` start-up flow (it starts a preset colony today). |
| S11c Settings | Let the player change the game and keep the choice. | A settings screen from the title and from the pause menu, and a `settings.ron` file. | **Graphics:** window mode (windowed, borderless, fullscreen), resolution, vsync, frame rate limit, UI scale (100, 150, 200 percent). **Audio:** master, music, effects, and ambience volume (integers 0 to 100), mute when the window is not in focus. **Gameplay:** edge scroll on or off, scroll and zoom speed, auto-pause on a raid, a death, and a cold alert. A change applies at once, or after a restart if the engine needs it (the screen says "restart"). A resolution change reverts after 10 seconds if the player does not confirm it. A Defaults button. A bad or missing file gives the defaults and a note in the log. No setting changes `Sim`: the golden hashes stay. The sound engine (S6) gets four volume buses. Tests: round trip, clamp of wrong values, defaults. Risks: `macroquad` cannot change some window settings at run time. |
| S12 Production and knowledge | Add the production chains and knowledge. | Tool requirement and wear, tool crafting, charcoal, bricks, smoked food, smelting, iron tools. Lore fragment items, the excavate designation, and reading. Proposed (Q-014): job choice by colony need (§7.2) and a small injury chance at risky jobs. | R-008, R-042, R-043. Each chain in `GAME_DESIGN.md` §11.3 works in a scenario test. A locked recipe cannot be ordered. |
| S13 Creatures and combat | Add creatures and melee combat. | Game animals, hunting, meat and leather, beasts, drafting, melee, the condition "wounded". | R-032. A drafted colonist obeys a move order in the same tick. Creatures use the path budget (P-04). Status: the `sim` part is done (D-081): creatures, game animals, hunting by order, meat, hide, leather, beasts, lair beasts, threat horrors, drafting, melee, wounds, and real raiders. Open: the interface (draft, orders, drawing, alerts), hunting as a board job and weapons (S12), balance (S19). |
| S14 Notice, legends, and sealed sites | Make history dangerous and useful. | Notice and its overlay, beast spawns, lair beasts, dormant threats, legends with site markers. | R-030, R-031, R-044. Notice equals the sum of its contributions. Opening a sealed site wakes its threat. Status: the `sim` and `worldgen` part is done (D-074): Notice, the daily beast roll, dormant threats, and legends. Open: beast and lair creatures (S13), the Notice overlay and the legend list in the UI, excavation and relic hooks (S12). |
| S15 Fire, migrants, and the full run | Complete the run from embark to collapse. | Fire, migrants, the lore panel, the end-of-run screen, the legacy score, ironman save, one music loop for each season, day and night lighting. Proposed (Q-014): migrants with a short background line, and bonds (§7.1). | R-017, R-023, R-024, R-050. Ironman writes a save at 00:00 each game day and at exit. Status: the `sim` part is done (D-080): hearth fires, burning items and buildings, extinguish jobs, migrants with background lines, the legacy score, and the ironman hook. Open: bonds (Q-014 slice B), and all interface work (fire alert, migrant dialog, lore panel, end-of-run screen, music, day and night lighting). |
| S15a Saves and runs | Save, continue, and end runs under the ironman rules. | The file layer in `game` for the ironman save of S15. Continue, Load Game list, and graveyard list on the title screen. | A run is one colony on one world and has one save file (D-032, D-092). The save is written to a temporary file, then renamed, and the old file stays as a backup (`TECHNICAL_DESIGN.md` §9). It is written at 00:00 each game day, on quit to menu, and on window close. A small `.meta` file next to the save holds world name, seed, day, year, colonists, and state, so the list needs no full load. If the `.meta` file is missing, the game makes it again. The list shows the runs of each world, newest first. A save of another format version, a corrupt save, or a missing file shows in the list with its reason and cannot be loaded. If the save is corrupt and a backup exists, the player can load the backup. There is no manual save slot and no load of an older state. When the run ends (R-050), the save is marked ended, becomes read-only, and moves to the graveyard list with its chronicle and score. A world can have 1 living run and many ended runs. Tests (temporary folder): save and load give the same `state_hash`; the old file stays if a write fails; backup rotation; corrupt file, empty file, and wrong magic bytes give an error and not a panic; an ended run cannot be loaded as living. The time to write a save is measured and recorded. Risks: write time at a large colony, antivirus locks on Windows (retry the rename 3 times). |
| S16 Drought and wildfire | Make dry summers a crisis. | Dry-day counter in the ledger, drought effects, irrigation ditches, dry-soil crops, lightning, weather ambience (rain, wind). | R-025, R-026. A crop on an irrigated tile grows at normal speed in a drought. A ditch that does not touch water does not irrigate. Fire does not cross a firebreak of 2 soil tiles. Status: the `sim` part is done (D-076): dry-day counter, drought growth chain, irrigation, ditches, lightning, and fire on trees. Open: hearth fires and extinguish jobs (S15), the ledger counter, weather ambience, and the UI. |
| S17 Disease and medicine | Make sickness a crisis that the player can manage. | Food poisoning, wound infection, winter fever, the medicine skill, herbal medicine, the doctor job, sick rooms, disease alerts. | R-060 to R-064. Winter fever does not spread between colonists in different rooms. Treatment decreases severity by the content value. Status: the `sim` part is done (D-082): three diseases, the Medicine skill, herbal medicine, the doctor job, sick rooms, and a 10-day grace period. Open: disease alerts and the UI. |
| S18 Rival colonies and raids | Add attacks by other colonies. | Rival colonies from history, hostility, scouts, raids, door and wall breaking, item theft, the tribute choice. | R-070 to R-072. A scout comes 1 game day before each raid. Paying the tribute removes goods of the correct value and the raiders leave. Status: the `sim` and `worldgen` part is done (D-078): rival colonies from history, hostility with its parts, the daily raid roll, the scout, the tribute and fight commands, and the S13 hooks. Open: raider creatures, combat, and damage to colonists (S13), door and wall breaking by raiders (only the time is ready), the warning alert, the tribute dialog, the rival list in the lore panel, and balance of the tribute value (S19). |
| S18a Controls, credits, and menu polish | Finish the shell of a shipped game. | Key rebinding screen, credits, quit confirmation, title screen art and music, window title and icon, version string. | The rebinding screen uses the binding table of S1 and stores keys in `settings.ron`. A key that two actions use in the same context shows a conflict and is not saved. A Reset button for one action and for all. The credits screen reads `credits.ron` and a licence list that `tools licenses` writes from `cargo metadata` (no new crate). Quit to desktop asks to confirm only if a run is living and the last save is older than 1 game hour. The title screen has the menu music loop of S15 and the build version. The title menu has the entries Tutorial and Help as disabled placeholders that S19 enables. The `controls-reviewer` subagent reviews the rebinding flow. Tests: binding round trip, conflict detection, reset. Risks: key names on other keyboard layouts. |
| S19 Balance and MVP review | Make the MVP fair and stable. | A scripted autoplayer in `tools run`. Balance changes. A sound mix pass. An in-game tutorial that holds the explanations removed from the HUD (the HUD rework strips the long text). The Tutorial and Help entries of the title menu (S18a) and of the pause menu open it. Milestone M3. | The autoplayer survival rate over 50 seeds is recorded. Each P-nn target is met, or each miss has a decision record. Soak test 1,000,000 ticks. 5 playtests of 3 years or more. Second controls review (Q-017). A new player can finish the tutorial and see every explanation again from a help menu. |

Risks: balance, scope pressure, playtest capacity.

## 6. Milestones

| Milestone | After | Review questions | Exit rule |
|---|---|---|---|
| M0 Foundation | S0 | Do the checks run? Is the workspace clean? | All S0 criteria pass. |
| M1 First winter | S7 | Does winter make real decisions? Do players understand deaths? Is the first year interesting? | 3 recorded playtests. A decision to continue, change the design, or cut scope, recorded in `DECISIONS.md`. Check A-001, A-005, A-006, A-008. First controls review (Q-017). |
| M2 World and history | S11 | Does a site change a player decision in each playtest? Are the local maps different enough? | 3 recorded playtests on 3 seeds. The result is recorded. |
| M3 MVP | S19 | Is the full loop interesting for 3 years or more? Is failure fair? | 5 recorded playtests. The performance results are recorded. |

### 6.1 Playtest record

Each playtest gets one file in `docs/playtests/`, named `YYYY-MM-DD-tester.md`. The file has these fields:

- Tester, build (commit), seed, duration in real minutes and in game days.
- Result: survived or failed. Each death and its cause.
- Each decision that a site, a legend, or a history result changed.
- Scores from 1 to 5:
  1. I understood why colonists died or suffered.
  2. I had real choices to make.
  3. The world and its history affected my plan.
  4. I want to play again.
- Free notes.

Get a minimum of 1 tester who is not the developer for each milestone.

## 7. Later phases (summary)

Plan these phases in detail only after M3. The connection of each system to the core loop is in `GAME_DESIGN.md` §16.2.

- **Phase 4, Colonist life and external threats:** colonist life (§7.1), larger wars with rival civilizations, ranged combat, trade caravans, world map expeditions, legacy worlds (Q-005), more traits, and infinite terrain (S28, §7.3).
- **Phase 5, Society:** religion from sites, social conflict and colony rules, diplomacy and allies, raids on other settlements, more metals and longer chains, cave-ins, floods, plague, crop blight, thieves.
- **Phase 6, Scale and variety:** 300 colonists (`GAME_DESIGN.md` §7.1), 512² maps, social classes, playable races, a deep underground layer (Q-002).

### 7.1 Colonist life (proposal, Q-014)

The owner wants colonists with their own goals, who form relationships, start families, and make the colony grow. Today the plan has only needs, mood, traits, and migrants (S6, S15), and one line for "relationships and births" in Phase 4. The table shows the proposed steps. The design is in `GAME_DESIGN.md` §7.8.

| Step | Phase | Content | Depends on |
|---|---|---|---|
| MVP slice A | S6 (no new task) | Keep the mood modifier list open for social sources. Colonist age, sex, and race stay in the data (all colonists have the same race in the MVP). | S6 mood |
| MVP slice B | S15 | Migrants have a short background line. Colonists who work or sleep near each other get a bond. The modifier "friend died" scales with the bond. This makes the example in `GAME_DESIGN.md` §15 true. | S6, S15 |
| L1 Wants and personality (S20) | Phase 4 | Each colonist has 1 life goal and a few short wants. Social trait: introvert or extrovert. An extrovert wants company. An introvert wants time alone. A met want adds a mood modifier. An unmet want adds a negative one. Status: the `sim` part is done (D-083): social traits, 5 life goals, and 6 want memories; the bonds of slice B are done in S21 (D-084). Open: the interface for wants, bonds, and friend wants. | MVP slice B |
| L2 Relationships (S21) | Phase 4 | Opinion between two colonists. Friend, rival, enemy, and partner states. Each change is a named contribution (why chain). Status: the `sim` part is done (D-084), with the bonds of slice B: 5 opinion parts, 5 standings, partners, and the friend and partner death modifiers. Open: the interface, break-ups, grudges from attacks (S24), and families (S22). | L1 |
| L3 Families, ages, and homes (S22) | Phase 4 | Partners, pregnancy, birth, children, growth to adult, aging and death by age. Children do not work. Colonists and families live in assigned houses (rooms with beds). Status: the `sim` part is done (D-085): ages, children, old age, pregnancy, birth, family mood modifiers, and assigned beds. Open: the interface, adoption, marriage, and the balance of the age year. | L2, beds and food from S6 |
| L4 Colony growth (S23) | Phase 4 | The colony grows from inside and by migrants. Mood and bonds change the migrant chance (R-017). Colonists leave if the colony fails their goals. Status: the `sim` part is done (D-086): the migrant chance has the named parts mood and bonds, and a colonist with a failed goal and a low mood leaves. Open: the warning alert, the interface, and balance (S19). | L3 |
| L5 People of other colonies (S24) | Phase 4 | Friendship, partners, and enmity between colonists and people of rival colonies. Marriage and trade contacts across colonies. Old grudges after raids. Status: the `sim` part is done (D-087): ties with the people of calm rival colonies, weddings, raid grudges, and a Ties part in the hostility. Open: the interface, trade (Phase 5 diplomacy), and break-ups. | L2, S18, diplomacy (Phase 5) |
| L6 Races (S25) | Phase 4 | A few other peoples (3 or 4) as colonists and migrants (Q-015). Each race is a set of content values: age span, trait chances, comfort band, who can be a partner, and who can have offspring together and which race the child gets. Races have no spells. Status: the `sim` part is done (D-088): 4 races, age speed, elder age, comfort band, migrant weight, and a rule for each pair (opinion, partners, offspring, child race). Open: the interface, art, trait chances per race, and balance. | L3, content files |

The S20 to S25 numbers are proposed. The infinite world spike S11a is in §7.3. They follow M3 and do not change S0 to S19.

### 7.2 Where each owner request goes

| Request | Sprint or step | Note |
|---|---|---|
| Each person wants a job. They pick jobs by colony need and by how many others do each job. | New task in S12 (MVP slice C) | Extends the job selection of D-040. A colonist without work gets the modifier "no work". The jobs from S12 and S13 give enough choices. |
| Moods | S6 | Already planned. |
| Different houses | S4 (rooms), S6 (beds), L3 | Rooms and beds exist. Assigning a house to a family is in L3. |
| Introvert and extrovert | L1 | A trait with a need for company or solitude. |
| Friendships, partners, enemies in the colony | L2 | |
| Friendships and enemies between colonies | L5 | Uses S18 and Phase 5 diplomacy. |
| Children, ages | L3 | |
| Death from disease | S17 | Already planned. |
| Death from natural disaster | S16 (drought, wildfire), S15 (fire) | Already planned. |
| Death from accident | New task in S12 (MVP slice D) | A small chance of injury at risky jobs (mining, tree felling). It uses the condition "wounded". |
| Death from war | S13 (melee), S18 (raids) | Already planned. |
| Death by age | L3 | |
| Mystical races (a few, with rules for love and offspring) | MVP slice A (race field in the data), L6 | Q-015: no spells unless the owner says so. |

Each step needs a rule ID, a content file, and a test before it goes into a sprint. Do not start a step before M3. The MVP slices C and D are small tasks in S12.

### 7.3 Infinite world and controls review (proposal)

- **S11a Infinite world spike (Q-016):** a design spike, not a feature. It comes right after M2 (S11) and before S12, because an infinite world changes world generation, zoom, the world map, and embark. Output: a written design and a small prototype in `tools` that makes world chunks on demand from the seed. It answers how the zoom levels, the world map, history, and the embark choice work without a fixed world edge, and what changes in the golden hashes and in P-05. The owner decides after the spike if the infinite world goes into the MVP. Worldgen steps 5 to 12 are not changed before then. Status: the study is in `docs/INFINITE_WORLD.md`. The prototype is `crates/worldgen/src/chunk.rs` (not in `tools`, the game does not use it). It makes height, temperature, and moisture per chunk with no seams. Decision (D-073): no infinite world in the MVP, and the sprint S28 in Phase 4 reworks it into truly infinite terrain (§7.3). The owner accepted this on 2026-10-20.
- **S26 Realms (Phase 4, option B of `INFINITE_WORLD.md`, D-073):** the world becomes a plane of 256 by 256 realms with sub-seeds. Realm (0, 0) is the world of the seed. The edge of each realm is ocean. A cache holds a few realms. A save keeps the realm and the world generator version. Status: the `worldgen` and `sim` part is done (D-089). Open: the plane map and the realm choice in the interface, expeditions and trade between realms, and S28.
- **Controls and game feel review (Q-017):** the `controls-reviewer` subagent checks how the controls feel and fit the gameplay. First review at M1 (S7 is the first sprint with real gameplay). Second review in S19, with the playtests. The review lists problems with camera, zoom, selection, and build input. The lead agent turns them into tasks.

### 7.4 Weather, seasons, sound, and music (proposal, Q-018)

| Request | Where | Status |
|---|---|---|
| Seasons, the season curve, daily weather, rain, the 3-day forecast, freeze and thaw | S5 | Already planned. Not changed. |
| Cold periods and heat waves | S5 (weather offset) | Already planned. |
| Day and night in the simulation (hour of day, daily temperature curve, sleep at night) | S2 (time), S5 (daily curve), S6 (sleep) | Already planned. |
| Yearly cycle (4 seasons of 10 days) | S2 (calendar), S5 (season curve), D-017 | Already planned. |
| Day and night in the picture: daylight tint by hour and season, dark nights, light around hearths | S15 | New. Display only. It does not change the simulation. |
| Drought and wildfire | S16 (R-025, R-026) | Already planned. |
| Snow cover that slows movement | S27 | Moved from the backlog. |
| Floods (snow melt raises the river in spring) and storms | S27 Floods and weather events | New. A river level model, a flood forecast, dikes, and storm damage to roofs. Phase 4, after S11a shows how the world model changes. Status: the `sim` part is done (D-090). Open: the interface (forecast, flood overlay, storm alert). |
| Sound engine and first sounds | S6 | New task 9. |
| Weather ambience | S16 | New. Rain, wind, and fire loops. |
| Music | S15 | New. One loop for each season, made by `tools assetgen`. |
| Sound mix and volume settings | S19 | New. Part of the balance pass. |

The `sound-designer` subagent writes the synthesizers in `tools assetgen` and makes the `.wav` files. The sound engine in `game` plays them. Generated files are committed (D-034). No new crate: use the `macroquad` audio feature (`TECHNICAL_DESIGN.md` §13). Sprint 5 scope does not change.

### 7.5 Main menu and game shell (D-079, D-091, D-092)

The owner asked for a main menu, world saving, new worlds, world seeds, and settings (2026-10-23). Today the game starts a preset colony. The world map and the embark screen exist (S9, S11). The pause menu has Resume and Quit (D-077). The save format and its header exist (S2). Nothing writes a save file in `game`. There is no settings file. The seed box accepts numbers only (D-027).

**Model.** A *world* is a record: name, seed, history length. The game makes the world data again from the seed. A *run* is one colony in one world. A run has one ironman save file. A world has one living run and many ended runs. Phase 4 legacy worlds (D-033) use the ended runs of a world as sites.

| Request | Sprint | Note |
|---|---|---|
| Main menu | S11b (S18a adds credits and polish) | Continue is disabled until S15a. |
| New worlds, world seeds | S11b | Seed box (D-071), Random button, history length, optional name. |
| World list, delete, rename | S11b | Delete asks to confirm. |
| World saving and loading | S15a | Ironman only (D-032). Atomic writes, backup, list with reasons. |
| Continue, load, graveyard | S15a | No manual slots. |
| Settings (graphics, audio, gameplay) | S11c | A settings file. No effect on `Sim`. |
| Key rebinding | S18a | Moved from the backlog. |
| Credits, quit, window title and icon | S18a | |
| Tutorial and help entries | S19 | Placeholders from S18a. |
| Legacy world list | Phase 4 | Uses the graveyard of S15a. |

**Order.** S11b and S11c touch only `game` screens, so they can run in parallel with S12 (`sim` work) on a different branch. They need S11 only. S15a needs S15 (ironman rules, end of run). S18a needs S15a and S11c. The `game` screen files belong to the UI thread: a `sim` sprint never edits them.

**Files.** The game stores worlds in `worlds/`, runs in `saves/`, and settings in `settings.ron`. The base folder is `%APPDATA%/Ruinstead` on Windows. It comes from `std::env`, with no new crate. `PCS_DATA_DIR` overrides it for tests.

**Not in this plan (backlog):** languages, gamepad, cloud saves, achievements, a mod menu.

## 8. Main project risks

| Risk | Effect | Control |
|---|---|---|
| Scope creep | The MVP does not finish. | Phase gates. Backlog. Milestone reviews. |
| The survival loop is not interesting | Work in later phases is wasted. | Phase 1 tests it first. M1 review. |
| History does not change decisions | Pillar 1 fails. | R-040. M2 review. |
| Determinism breaks | Saves, replays, and tests fail. | Golden hashes from S0. Save tests from S2. CI on 2 platforms. |
| Golden hash changes are too frequent | Agents accept wrong changes. | Hashes for each subsystem. Behavior tests in addition to hashes. `AGENTS.md` rule for `UPDATE_GOLDEN`. |
| Rules and tests do not agree | Wrong behavior passes the tests. | Rule IDs in test names. |
| Too few playtests | Bad design decisions. | The playtest record (§6.1). External testers. |
| Balance without data | Runs are too easy or too hard. | The autoplayer in S19. All numbers in content data. |
| UI cost | Phase 3 takes too long. | A small widget set. Inspector from key and value lists from S3. |
| Agents work in different directions | Code and documents conflict. | `AGENTS.md` rules. Canonical documents. One branch for each agent task. |
| Art cost | Visual quality blocks playtests. | Placeholder art. Material tint. Q-007. |

## 9. Backlog

These ideas are not in a sprint. Each item needs a reason and a phase before it goes into a sprint.

| Idea | Possible phase | Note |
|---|---|---|
| Embark goals ("survive 10 winters") | 3 or 4 | Small. Good for long-term interest. |
| Colony size goal ("reach 20 colonists") | 4 | Uses the colonist life steps L1 to L4 (§7.1). |
| Chronicle export to a text file | 4 | Small. Uses the chronicle core from S7. |
| Livestock and animal handling | 4 | A food source that cold periods and beasts can threaten. |
| Carts for faster hauling | 4 | Only if hauling time is a playtest problem. |
| Fishing and a water need for colonists | 4 | Uses the existing water tiles. |
| Snow cover that slows movement | 4 (S27) | Uses the existing climate model. |
| More legend reliability levels | 4 | The MVP has "certain" and "rumor" only (R-044). |
| Salt and salted food | 5 | A longer food chain. |
| Cave-ins | 5 | Moved from the MVP to control scope. |
| Seasonal river floods | 4 | Moved to the Phase 4 sprint S27 (§7.4). |
| History continues slowly after embark | 4 | Necessary for civilizations that change during play. |
| Sound effects | 3 | Moved to S6 (sound engine), S16 (weather ambience), and S15 (music) (§7.4). |
| Key rebinding UI | 3 (S18a) | Moved to S18a (§7.5). |
| Free text input (seed words) | 4 | D-027. A small widget for world names is in S11b (D-091). |
| Modding support beyond content files | 6 | Content files are already data. |
| Languages (text tables) | 4 | The UI has little text. Decide before the Steam page. |
| Gamepad support | 4 | Needs a new control scheme. |
| Cloud saves | 4 | Only after the save folder is stable (S15a). |
| Hierarchical pathfinding | 6 | Only if a benchmark shows a need. |
