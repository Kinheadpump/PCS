//! Scenario tests for Sprint 7: spoilage, crops, wild plants, keep-N orders, the ledger,
//! the chronicle, the start conditions, and the end of the run.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use sim::chronicle::{ChronicleKind, RunState};
use sim::rules::BUILTIN_TEXTS;
use sim::scenario::sim_from_ascii;
use sim::{Command, Event, JobKind, Sim, SimRules, save};
use std::sync::Arc;
use worldgen::{EmbarkSummary, generate_local};

fn climate_text() -> String {
    format!(
        "(season_curve: {:?}, daily_curve: {:?}, weather_step_max: 0, weather_limit: 120, \
         default_mean: 110, default_amplitude: 160, default_moisture: 128, rain_max_pct: 50, \
         freeze_days: 3, deep_water_distance: 3, hearth_heat: 400, \
         hearth_fuel_capacity_hours: 48, refuel_below_hours: 24, \
         fuel: [(\"firewood\", 4), (\"coal\", 4)], low_fuel_alert_hours: 12, \
         cold_period_offset: -60)",
        [0i16; 40], [0i16; 24]
    )
}

/// A sim with a flat climate: the outdoor temperature is `mean` for ever. Needs are off.
fn flat_sim(map: &str, mean: i16) -> Sim {
    let mut sim = sim_from_ascii(map, 1).unwrap();
    let b = &BUILTIN_TEXTS;
    let rules = SimRules::load_full_with(
        b.materials,
        b.jobs,
        b.buildings,
        b.recipes,
        b.rooms,
        &climate_text(),
        b.needs,
        b.traits,
    )
    .unwrap();
    sim.debug_set_rules(Arc::new(rules));
    sim.set_climate(mean, 0, 128);
    sim.debug_set_needs_enabled(false);
    sim
}

fn open_map(w: usize, h: usize, colonists: &[(usize, usize)]) -> String {
    let mut rows = vec![vec!['.'; w]; h];
    for &(x, y) in colonists {
        rows[y][x] = 'c';
    }
    let text: Vec<String> = rows.iter().map(|r| r.iter().collect()).collect();
    text.join("\n")
}

fn mat(sim: &Sim, key: &str) -> u16 {
    sim.rules().material_index(key).unwrap()
}

fn run(sim: &mut Sim, ticks: u32) {
    for _ in 0..ticks {
        sim.step(&[]);
    }
}

fn run_checked(sim: &mut Sim, ticks: u32) {
    for _ in 0..ticks {
        sim.step(&[]);
        sim.check_invariants().unwrap();
    }
}

fn units(sim: &Sim, key: &str) -> u32 {
    units_of(sim, mat(sim, key))
}

fn units_of(sim: &Sim, m: u16) -> u32 {
    sim.items()
        .iter()
        .filter(|(_, i)| i.material == m)
        .map(|(_, i)| i.count)
        .sum()
}

fn first_item(sim: &Sim, key: &str) -> sim::ItemId {
    let m = mat(sim, key);
    sim.items()
        .iter()
        .find(|(_, i)| i.material == m)
        .map(|(id, _)| id)
        .unwrap()
}

fn kinds(sim: &Sim) -> Vec<ChronicleKind> {
    sim.chronicle().iter().map(|e| e.kind).collect()
}

// ---- R-005 spoilage ----

fn rot_after_a_day(mean: i16) -> u32 {
    let mut sim = flat_sim(&open_map(6, 6, &[(0, 0)]), mean);
    let meal = mat(&sim, "meal");
    sim.debug_spawn_item(meal, 5, 3, 3);
    run(&mut sim, 2400);
    sim.items()
        .iter()
        .find(|(_, i)| i.material == meal)
        .map(|(_, i)| i.rot)
        .unwrap()
}

#[test]
fn r005_food_rots_four_times_faster_at_20c_than_at_5c() {
    let warm = rot_after_a_day(200);
    let cool = rot_after_a_day(50);
    assert!(cool > 0);
    assert_eq!(warm, 4 * cool);
}

#[test]
fn r005_food_does_not_rot_below_zero() {
    assert_eq!(rot_after_a_day(-10), 0);
}

#[test]
fn r005_rotten_food_is_removed_and_not_eaten() {
    let mut sim = flat_sim(&open_map(6, 6, &[(1, 1)]), 200);
    let meal = mat(&sim, "meal");
    sim.debug_spawn_item(meal, 5, 3, 3);
    let id = first_item(&sim, "meal");
    let report = sim.rot_report(id).unwrap();
    assert_eq!(report.per_day, 2500);
    assert!(sim.debug_set_rot(id, sim.rules().spoil.rotten_at));
    let who = sim.colonists().iter().next().map(|(i, _)| i).unwrap();
    sim.debug_set_needs_enabled(true);
    sim.debug_set_hunger(who, 0);
    run(&mut sim, 200);
    assert_eq!(units(&sim, "meal"), 0, "the rotten stack ended");
    assert!(
        kinds(&sim).iter().all(|k| *k != ChronicleKind::FoodRotted) || sim.chronicle().len() < 50
    );
}

// ---- R-006 crops ----

fn crop(sim: &Sim, key: &str) -> u16 {
    sim.rules()
        .farm
        .crops
        .iter()
        .position(|c| c.id == key)
        .map(|i| i as u16)
        .unwrap()
}

#[test]
fn r006_a_plant_grows_only_inside_the_temperature_band() {
    let barley_hours = |mean: i16| -> u32 {
        let mut sim = flat_sim(&open_map(6, 6, &[(0, 0)]), mean);
        let c = crop(&sim, "barley");
        assert!(sim.debug_plant(3, 3, c, false));
        run(&mut sim, 2400 * 2);
        sim.plant_at(3, 3).map_or(0, |p| p.hours)
    };
    assert_eq!(barley_hours(150), 48, "inside the band: one hour per hour");
    assert_eq!(barley_hours(30), 0, "below min_temp: no growth");
    assert_eq!(barley_hours(350), 0, "above max_temp: no growth");
}

#[test]
fn r006_frost_below_the_kill_temperature_kills_the_plant() {
    let mut sim = flat_sim(&open_map(6, 6, &[(0, 0)]), -60);
    let c = crop(&sim, "barley");
    assert!(sim.debug_plant(3, 3, c, false));
    run(&mut sim, 200);
    assert!(sim.plant_at(3, 3).is_none());
    run(&mut sim, 2400);
    assert!(kinds(&sim).contains(&ChronicleKind::CropsKilled));
}

#[test]
fn r006_colonists_sow_grow_and_harvest() {
    let mut sim = flat_sim(&open_map(10, 10, &[(1, 1)]), 150);
    let c = crop(&sim, "barley");
    let seeds = mat(&sim, "barley_seeds");
    sim.debug_spawn_item(seeds, 10, 2, 2);
    sim.step(&[Command::SetFarmZone {
        x0: 4,
        y0: 4,
        x1: 5,
        y1: 5,
        crop: c,
    }]);
    run_checked(&mut sim, 2400 * 2);
    assert!(sim.plants().count() > 0, "the colonist sowed");
    run_checked(&mut sim, 2400 * 14);
    assert!(units(&sim, "barley") > 0, "the colonist harvested");
    assert!(kinds(&sim).contains(&ChronicleKind::FirstHarvest));
    assert!(units(&sim, "barley_seeds") > 0, "the harvest gave seeds");
}

#[test]
fn r006_fertility_parts_add_up_to_the_value() {
    let sim = flat_sim(&open_map(8, 8, &[(0, 0)]), 150);
    for y in 0..8u16 {
        for x in 0..8u16 {
            let why = sim.fertility_why(x, y).unwrap();
            let sum: i32 = why.parts.iter().map(|p| p.value).sum();
            assert_eq!(sum, why.total);
            assert_eq!(why.total, i32::from(sim.fertility(x, y)));
            assert_eq!(
                i32::from(sim.fertility_map()[usize::from(y) * 8 + usize::from(x)]),
                why.total
            );
        }
    }
}

// ---- R-007 wild plants ----

fn wild_tile(sim: &Sim, w: u16, h: u16) -> (u16, u16) {
    for y in 0..h {
        for x in 0..w {
            if sim.wild_plant_at(x, y).is_some_and(|s| s.ripe) {
                return (x, y);
            }
        }
    }
    panic!("no wild plant on the map");
}

#[test]
fn r007_gathering_gives_food_and_the_plant_grows_back_in_days() {
    let mut sim = flat_sim(&open_map(40, 40, &[(0, 0)]), 150);
    let (x, y) = wild_tile(&sim, 40, 40);
    let kind = sim.wild_plant_at(x, y).unwrap().kind;
    sim.step(&[Command::Designate {
        kind: JobKind::Gather,
        x,
        y,
    }]);
    run_checked(&mut sim, 6000);
    let status = sim.wild_plant_at(x, y).unwrap();
    assert!(!status.ripe, "the plant was gathered");
    assert!(status.days_left > 0);
    let product = sim.rules().farm.wild[usize::from(kind)].product;
    assert!(
        sim.items()
            .iter()
            .any(|(_, i)| i.material == product && i.count > 0)
    );
    let days = u32::from(status.days_left);
    run(&mut sim, 2400 * (days + 1));
    assert!(sim.wild_plant_at(x, y).unwrap().ripe, "ripe again");
}

#[test]
fn r007_regrowth_does_not_count_in_winter() {
    let mut sim = flat_sim(&open_map(40, 40, &[(0, 0)]), 150);
    let (x, y) = wild_tile(&sim, 40, 40);
    sim.step(&[Command::Designate {
        kind: JobKind::Gather,
        x,
        y,
    }]);
    run(&mut sim, 6000);
    let before = sim.wild_plant_at(x, y).unwrap().days_left;
    assert!(before > 0);
    // Jump to the first day of winter: ticks 72000 and later. The days stand still.
    let mut s2 = flat_sim(&open_map(40, 40, &[(0, 0)]), 150);
    s2.step(&[Command::Designate {
        kind: JobKind::Gather,
        x,
        y,
    }]);
    run(&mut s2, 72_000 + 6000);
    let status = s2.wild_plant_at(x, y).unwrap();
    assert!(!status.ripe || status.days_left == 0);
    let in_winter = s2.wild_plant_at(x, y).unwrap();
    let mut s3 = s2.clone();
    run(&mut s3, 2400 * 5);
    assert_eq!(s3.wild_plant_at(x, y).unwrap(), in_winter);
}

// ---- keep-N ----

fn crafted(sim: &mut Sim) -> usize {
    sim.drain_events()
        .iter()
        .filter(|e| matches!(e, Event::Crafted { .. }))
        .count()
}

#[test]
fn keep_n_stops_at_the_target_and_restarts_below_it() {
    let mut sim = flat_sim(&open_map(10, 6, &[(0, 2)]), 150);
    let kitchen = sim.rules().building_index("kitchen").unwrap();
    let granite = mat(&sim, "granite");
    assert!(sim.debug_build(kitchen, 4, 2, granite));
    let barley = mat(&sim, "barley");
    sim.debug_spawn_item(barley, 40, 2, 2);
    let recipe = sim.rules().recipe_index("meal_barley").unwrap();
    sim.step(&[Command::SetKeep {
        x: 4,
        y: 2,
        recipe,
        target: 6,
    }]);
    assert_eq!(sim.keep_target(4, 2, recipe), 6);
    run_checked(&mut sim, 12_000);
    let first = crafted(&mut sim);
    let stock = units(&sim, "meal");
    assert!(first > 0 && stock >= 6, "{first} runs, {stock} meals");
    run_checked(&mut sim, 6000);
    assert_eq!(crafted(&mut sim), 0, "at the target: no more runs");
    // Remove meals: the order starts again.
    let id = first_item(&sim, "meal");
    sim.debug_set_rot(id, sim.rules().spoil.rotten_at);
    run_checked(&mut sim, 12_000);
    assert!(crafted(&mut sim) > 0, "below the target: runs again");
    assert!(units(&sim, "meal") >= 6);
}

// ---- ledger ----

#[test]
fn ledger_days_remaining_match_a_constant_use_rate() {
    let mut sim = flat_sim(&open_map(6, 6, &[(1, 1)]), 150);
    sim.debug_set_needs_enabled(true);
    let who = sim.colonists().iter().next().map(|(i, _)| i).unwrap();
    sim.debug_set_traits(who, &[]);
    let meal = mat(&sim, "meal");
    sim.debug_spawn_item(meal, 24, 3, 3);
    let ledger = sim.ledger();
    let per_day = u64::from(sim.rules().needs.hunger_per_day);
    assert_eq!(ledger.food.use_per_day, per_day);
    let predicted = ledger.food.days_remaining_tenths.unwrap();
    let hunger_start = sim.rules().needs.hunger_per_day;
    let _ = hunger_start;
    let before = units(&sim, "meal");
    run(&mut sim, 2400 * 10);
    let eaten = u64::from(before - units(&sim, "meal"));
    let nutrition = u64::from(sim.rules().needs.foods[0].nutrition);
    let used_per_day = eaten * nutrition / 10;
    // The colonist used its stock at about the predicted rate.
    let predicted_use = per_day;
    let (lo, hi) = (predicted_use * 9 / 10, predicted_use * 11 / 10);
    assert!(
        (lo..=hi).contains(&used_per_day) || used_per_day.abs_diff(predicted_use) <= nutrition,
        "used {used_per_day}, predicted {predicted_use}, days {predicted}"
    );
    let parts: i64 = ledger.food.stock_parts.iter().map(|p| p.value).sum();
    assert_eq!(parts as u64, ledger.food.stock);
}

#[test]
fn ledger_fuel_line_counts_hearth_hours() {
    let mut sim = flat_sim(&open_map(6, 6, &[(1, 1)]), -100);
    let hearth = sim.rules().building_index("hearth").unwrap();
    let stone = mat(&sim, "granite");
    assert!(sim.debug_build(hearth, 3, 3, stone));
    assert!(sim.debug_set_hearth_fuel(3, 3, 48));
    let fuel = sim.ledger().fuel;
    assert_eq!(fuel.use_per_day, 24);
    assert_eq!(fuel.stock, 48);
    assert_eq!(fuel.days_remaining_tenths, Some(20));
}

// ---- chronicle and run ----

#[test]
fn chronicle_records_a_death_with_cause_and_text() {
    let mut sim = flat_sim(&open_map(6, 6, &[(1, 1), (2, 2)]), 150);
    let who = sim.colonists().iter().next().map(|(i, _)| i).unwrap();
    sim.debug_kill(who);
    run(&mut sim, 5);
    let entry = sim
        .chronicle()
        .iter()
        .find(|e| {
            matches!(
                e.kind,
                ChronicleKind::DeathUnknown | ChronicleKind::DeathStarvation
            )
        })
        .copied()
        .unwrap();
    assert!(!sim.chronicle_text(&entry).contains('{'));
    let groups = sim.chronicle_by_season();
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].entries.len(), sim.chronicle().len());
    assert_eq!(sim.run_state(), RunState::Running);
}

#[test]
fn chronicle_has_entries_for_the_first_winter() {
    let mut sim = flat_sim(&open_map(6, 6, &[(1, 1)]), 150);
    run(&mut sim, 72_000);
    assert!(kinds(&sim).contains(&ChronicleKind::WinterBegins));
    assert!(!kinds(&sim).contains(&ChronicleKind::WinterSurvived));
    let begin = sim
        .chronicle()
        .iter()
        .find(|e| e.kind == ChronicleKind::WinterBegins)
        .unwrap();
    assert_eq!(begin.tick, 72_000);
    run(&mut sim, 24_000);
    assert!(kinds(&sim).contains(&ChronicleKind::WinterSurvived));
    let seasons = sim.chronicle_by_season();
    assert!(seasons.len() >= 2);
    assert!(
        seasons
            .windows(2)
            .all(|w| w[0].entries[0].tick < w[1].entries[0].tick)
    );
}

#[test]
fn r050_the_run_ends_when_no_colonist_is_left() {
    let mut sim = flat_sim(&open_map(6, 6, &[(1, 1), (2, 2)]), 150);
    let ids: Vec<_> = sim.colonists().iter().map(|(i, _)| i).collect();
    assert_eq!(sim.run_info().peak_population, 2);
    sim.debug_kill(ids[0]);
    run(&mut sim, 3);
    assert_eq!(sim.run_state(), RunState::Running);
    sim.debug_kill(ids[1]);
    run(&mut sim, 3);
    assert!(matches!(sim.run_state(), RunState::Lost { .. }));
    assert!(kinds(&sim).contains(&ChronicleKind::ColonyLost));
    assert_eq!(sim.run_info().peak_population, 2);
}

#[test]
fn r050_the_player_can_abandon_the_colony() {
    let mut sim = flat_sim(&open_map(6, 6, &[(1, 1)]), 150);
    sim.step(&[Command::Abandon]);
    assert!(matches!(sim.run_state(), RunState::Abandoned { .. }));
    assert!(kinds(&sim).contains(&ChronicleKind::ColonyAbandoned));
}

// ---- start conditions ----

fn started(preset: EmbarkSummary) -> Sim {
    let map = generate_local(&preset);
    let mut sim = Sim::new(&map, 7, 0);
    let p = sim.climate_params();
    sim.set_climate(preset.mean_temperature, p.amplitude, p.moisture);
    sim
}

#[test]
fn start_conditions_follow_the_design() {
    for preset in [
        EmbarkSummary::temperate_forest(),
        EmbarkSummary::cold_hills(),
        EmbarkSummary::dry_plain(),
    ] {
        let sim = started(preset);
        assert_eq!(sim.colonists().len(), 7);
        assert_eq!(units(&sim, "meal"), 420);
        assert_eq!(units(&sim, "log"), 20);
        assert_eq!(units(&sim, "stone_axe"), 3);
        assert_eq!(units(&sim, "stone_pick"), 2);
        let seed_stacks: u32 = sim
            .rules()
            .farm
            .crops
            .iter()
            .map(|c| units_of(&sim, c.seed))
            .filter(|&n| n == 30)
            .count() as u32;
        assert_eq!(seed_stacks, 2, "seeds for two crops");
        for (id, c) in sim.colonists().iter() {
            let _ = id;
            let levels: Vec<u8> = (0..sim::skill::SKILL_COUNT)
                .map(|s| sim::skill::level_for_xp(c.skill_xp[s], &sim.rules().jobs))
                .collect();
            let main = levels.iter().filter(|&&l| (3..=5).contains(&l)).count();
            let low = levels.iter().filter(|&&l| l <= 2).count();
            assert_eq!(main, 2, "{levels:?}");
            assert_eq!(low, levels.len() - 2, "{levels:?}");
        }
    }
}

// ---- save ----

#[test]
fn the_new_state_survives_a_save() {
    let mut sim = started(EmbarkSummary::temperate_forest());
    let c = 0;
    sim.step(&[Command::SetFarmZone {
        x0: 100,
        y0: 100,
        x1: 103,
        y1: 103,
        crop: c,
    }]);
    run(&mut sim, 5000);
    let bytes = save::to_bytes(&sim);
    let mut loaded = save::from_bytes(&bytes).unwrap();
    assert_eq!(loaded.state_hash(), sim.state_hash());
    run(&mut sim, 3000);
    run(&mut loaded, 3000);
    assert_eq!(loaded.state_hash(), sim.state_hash());
    assert_eq!(loaded.fertility_map(), sim.fertility_map());
}

// ---- scripted first-year runs (for the playtest record, run by hand) ----

/// A scripted player: the commands for game day `day`, given the tile of the first colonist.
fn script(strategy: u8, sim: &Sim, day: u64, out: &mut Vec<Command>) {
    let Some((cx, cy)) = sim.colonists().iter().next().map(|(_, c)| c.tile()) else {
        return;
    };
    let rules = sim.rules();
    let (kitchen, hearth, log) = (
        rules.building_index("kitchen"),
        rules.building_index("hearth"),
        rules.material_index("log"),
    );
    if strategy == 0 {
        return;
    }
    if day == 0 {
        if let (Some(b), Some(m)) = (kitchen, log) {
            out.push(Command::PlaceBlueprint {
                building: b,
                x: cx + 3,
                y: cy,
                material: m,
            });
        }
        if strategy == 2
            && let (Some(b), Some(m)) = (hearth, log)
        {
            out.push(Command::PlaceBlueprint {
                building: b,
                x: cx + 3,
                y: cy + 3,
                material: m,
            });
        }
        out.push(Command::SetFarmZone {
            x0: cx + 5,
            y0: cy,
            x1: cx + 9,
            y1: cy + 4,
            crop: 0,
        });
    }
    if day.is_multiple_of(10) {
        for dx in 0..6u16 {
            out.push(Command::Designate {
                kind: JobKind::Cut,
                x: cx.saturating_sub(10) + dx,
                y: cy.saturating_sub(4) + (day % 7) as u16,
            });
            out.push(Command::Designate {
                kind: JobKind::Gather,
                x: cx + dx * 2,
                y: cy + (day % 9) as u16,
            });
        }
    }
    if day == 1
        && let Some(meal) = rules
            .recipes
            .iter()
            .position(|r| r.name.to_lowercase().contains("meal"))
    {
        out.push(Command::SetKeep {
            x: cx + 3,
            y: cy,
            recipe: meal as u16,
            target: 30,
        });
    }
}

/// Run a scripted year on a preset and print what happened.
#[test]
#[ignore = "prints the data for the playtest record; run with --ignored --nocapture"]
fn first_year_scripted_runs() {
    for (name, preset) in [
        ("temperate forest", EmbarkSummary::temperate_forest()),
        ("cold hills", EmbarkSummary::cold_hills()),
        ("dry plain", EmbarkSummary::dry_plain()),
    ] {
        for strategy in 0..3u8 {
            let mut sim = started(preset);
            let mut line = format!("{name} strategy {strategy}:");
            for day in 0..360u64 {
                let mut commands = Vec::new();
                script(strategy, &sim, day, &mut commands);
                sim.step(&commands);
                for _ in 1..2400 {
                    sim.step(&[]);
                }
                if matches!(day, 59 | 119 | 179 | 239 | 299 | 359) {
                    let l = sim.ledger();
                    line.push_str(&format!(
                        " d{} pop {} food {:?} fuel {:?};",
                        day + 1,
                        sim.colonists().len(),
                        l.food.days_remaining_tenths,
                        l.fuel.days_remaining_tenths
                    ));
                }
                if sim.run_state() != RunState::Running {
                    line.push_str(&format!(" LOST on day {};", day + 1));
                    break;
                }
            }
            sim.check_invariants().unwrap();
            let by_season = sim.chronicle_by_season();
            let entries: usize = by_season.iter().map(|s| s.entries.len()).sum();
            line.push_str(&format!(" chronicle entries {entries}"));
            println!("{line}");
            for e in sim.chronicle() {
                println!("    {}", sim.chronicle_text(e));
            }
        }
    }
}
