//! Climate tests (Sprint 5): the outdoor temperature (R-002), the weather (R-003), room
//! temperature (R-022), freezing water (R-004), hearth fuel, and alerts.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use sim::climate::{day_mean, next_offset, offset_on_day, outdoor_parts, rain_on_day};
use sim::rules::BUILTIN_TEXTS;
use sim::scenario::sim_from_ascii;
use sim::{AlertKind, Command, Sim, SimRules, TempCause, TempPlace, save};
use std::sync::Arc;

const HOUR: u32 = 100;
const DAY: u32 = 2_400;

fn mat(sim: &Sim, key: &str) -> u16 {
    sim.rules().material_index(key).unwrap()
}

fn bld(sim: &Sim, key: &str) -> u16 {
    sim.rules().building_index(key).unwrap()
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

/// Climate text with flat curves, a chosen weather step, and a cold period limit.
fn climate_text(season: i16, daily: i16, step: u16, cold: i16) -> String {
    format!(
        "(season_curve: {:?}, daily_curve: {:?}, weather_step_max: {step}, weather_limit: 120, \
         default_mean: 110, default_amplitude: 160, default_moisture: 128, rain_max_pct: 50, \
         freeze_days: 3, deep_water_distance: 3, hearth_heat: 400, \
         hearth_fuel_capacity_hours: 48, refuel_below_hours: 24, \
         fuel: [(\"firewood\", 4), (\"coal\", 4)], low_fuel_alert_hours: 12, \
         cold_period_offset: {cold})",
        [season; 40], [daily; 24]
    )
}

fn set_rules(sim: &mut Sim, text: &str) {
    sim.debug_set_rules(Arc::new(SimRules::load_with_climate(text).unwrap()));
}

/// A sim with a fixed climate: outdoor = mean + amplitude * season / 1000 for ever.
fn flat_sim(map: &str, mean: i16, amplitude: i16, season: i16) -> Sim {
    let mut sim = sim_from_ascii(map, 1).unwrap();
    set_rules(&mut sim, &climate_text(season, 0, 0, -60));
    sim.set_climate(mean, amplitude, 128);
    sim
}

fn room_map() -> String {
    let mut rows = vec![".".repeat(10); 10];
    rows[0].replace_range(0..1, "c");
    rows.join("\n")
}

/// A walled room with a 4x4 interior (x and y from 3 to 6), granite walls (insulation 60),
/// and a roof.
fn build_room(sim: &mut Sim) {
    let (granite, log) = (mat(sim, "granite"), mat(sim, "log"));
    let (wall, roof) = (bld(sim, "wall"), bld(sim, "roof"));
    for i in 2..=7u16 {
        for (x, y) in [(i, 2), (i, 7), (2, i), (7, i)] {
            if sim.buildings().object_at(x, y).is_none() {
                assert!(sim.debug_build(wall, x, y, granite));
            }
        }
    }
    for y in 3..=6 {
        for x in 3..=6 {
            assert!(sim.debug_build(roof, x, y, log));
        }
    }
    sim.step(&[]);
}

fn heated_room(fuel: u32) -> Sim {
    let mut sim = flat_sim(&room_map(), -100, 0, 0);
    build_room(&mut sim);
    let hearth = bld(&sim, "hearth");
    assert!(sim.debug_build(hearth, 4, 4, mat(&sim, "granite")));
    sim.debug_set_hearth_fuel(4, 4, fuel);
    sim.step(&[]);
    sim
}

// ---- R-002 ----

#[test]
fn r002_outdoor_temperature_equals_the_formula_for_each_hour_of_a_year() {
    let text = BUILTIN_TEXTS
        .climate
        .replace("weather_step_max: 30", "weather_step_max: 0");
    let mut sim = sim_from_ascii("c..", 7).unwrap();
    set_rules(&mut sim, &text);
    let rules = sim.rules().climate.clone();
    let p = sim.climate_params();
    assert_eq!(rules.season_curve.len(), 40);
    assert_eq!(rules.daily_curve.len(), 24);
    for hour_of_year in 0..(40 * 24usize) {
        let (day, hour) = (hour_of_year / 24, hour_of_year % 24);
        let expected = i32::from(p.mean)
            + i32::from(p.amplitude) * i32::from(rules.season_curve[day]) / 1000
            + i32::from(rules.daily_curve[hour]);
        assert_eq!(sim.weather_today().offset, 0);
        assert_eq!(
            i32::from(sim.outdoor_temp()),
            expected,
            "day {} hour {hour}",
            day + 1
        );
        run(&mut sim, HOUR);
    }
}

#[test]
fn r002_the_outdoor_temperature_is_the_sum_of_its_named_parts() {
    let mut sim = sim_from_ascii("c..", 3).unwrap();
    for _ in 0..30 {
        run(&mut sim, 7 * HOUR + 13);
        let parts = sim.outdoor_parts();
        assert_eq!(i32::from(sim.outdoor_temp()), parts.total());
        let chain = sim.tile_temperature(0, 0).unwrap();
        assert_eq!(chain.place, TempPlace::Outdoor);
        assert_eq!(
            chain.parts.iter().map(|p| p.tenths).sum::<i32>(),
            i32::from(chain.total)
        );
        assert_eq!(chain.total, sim.outdoor_temp());
    }
}

#[test]
fn r002_winter_is_colder_than_summer_and_night_colder_than_afternoon() {
    let sim = sim_from_ascii("c..", 1).unwrap();
    let rules = &sim.rules().climate;
    let p = sim.climate_params();
    let at = |day, hour| outdoor_parts(rules, &p, day, hour, 0).total();
    let coldest = (1..=40).min_by_key(|&d| at(d, 12)).unwrap();
    let warmest = (1..=40).max_by_key(|&d| at(d, 12)).unwrap();
    assert!(at(coldest, 12) < at(warmest, 12));
    let night = (0..24).min_by_key(|&h| at(warmest, h)).unwrap();
    let day = (0..24).max_by_key(|&h| at(warmest, h)).unwrap();
    assert!(at(warmest, night) < at(warmest, day));
}

// ---- R-003 ----

#[test]
fn r003_the_weather_offset_stays_within_12_degrees_over_100_years() {
    let sim = sim_from_ascii("c..", 5).unwrap();
    let rules = &sim.rules().climate;
    for seed in [1u64, 2, 99] {
        let mut offset = 0i16;
        let (mut lo, mut hi) = (0i16, 0i16);
        for day in 0..(100 * 40u64) {
            let last = offset;
            offset = next_offset(rules, seed, offset, day);
            assert!((-120..=120).contains(&offset), "day {day}: {offset}");
            assert!((i32::from(offset) - i32::from(last)).abs() <= 30);
            lo = lo.min(offset);
            hi = hi.max(offset);
        }
        assert!(lo < -30 && hi > 30, "the offset moves: {lo} {hi}");
        assert_eq!(offset, offset_on_day(rules, seed, 100 * 40 - 1));
    }
}

#[test]
fn r003_rain_chance_follows_the_moisture() {
    let sim = sim_from_ascii("c..", 5).unwrap();
    let rules = &sim.rules().climate;
    let count = |moisture| {
        (0..4_000u64)
            .filter(|&d| rain_on_day(rules, 11, moisture, d))
            .count()
    };
    assert_eq!(count(0), 0);
    let wet = count(255);
    assert!((1_800..=2_200).contains(&wet), "{wet} of 4000 days");
    let half = count(128);
    assert!((800..=1_200).contains(&half), "{half} of 4000 days");
}

#[test]
fn r003_the_forecast_equals_the_true_weather_3_days_ahead() {
    let mut sim = sim_from_ascii("c..", 21).unwrap();
    let mut seen = Vec::new();
    for _ in 0..60u32 {
        let today = sim.weather_today();
        if let Some(&(_, f)) = seen.iter().find(|&&(d, _)| d == today.day) {
            assert_eq!(f, today, "forecast for day {}", today.day);
        }
        for (i, f) in sim.forecast().iter().enumerate() {
            assert_eq!(f.day, today.day + 1 + i as u64);
            seen.push((f.day, *f));
        }
        run(&mut sim, DAY);
        assert_eq!(sim.weather_today().day, today.day + 1);
    }
}

#[test]
fn r003_the_weather_does_not_use_the_sim_random_stream() {
    let a = sim_from_ascii("c....\n.....", 5).unwrap();
    let mut b = sim_from_ascii("c....\n.....", 5).unwrap();
    let id = b.colonists().iter().next().unwrap().0;
    for _ in 0..200 {
        b.step(&[Command::MoveColonist { id, x: 4, y: 1 }]);
    }
    assert_eq!(a.forecast(), b.forecast());
    assert_eq!(a.weather_today(), b.weather_today());
}

#[test]
fn r003_day_mean_is_the_integer_mean_of_24_hours() {
    let sim = sim_from_ascii("c..", 1).unwrap();
    let rules = &sim.rules().climate;
    let p = sim.climate_params();
    for day in 1..=40 {
        let sum: i32 = (0..24)
            .map(|h| outdoor_parts(rules, &p, day, h, 25).total())
            .sum();
        assert_eq!(i32::from(day_mean(rules, &p, day, 25)), sum / 24);
    }
}

// ---- R-022 ----

#[test]
fn r022_a_16_tile_room_with_insulation_60_and_a_hearth_is_warm_after_3_days() {
    let mut sim = heated_room(1_000);
    let room = sim.room_at(5, 5).unwrap();
    assert_eq!(room.len(), 16);
    assert_eq!(room.insulation.total, 60);
    assert_eq!(sim.outdoor_temp(), -100);
    run(&mut sim, 3 * DAY);
    let t = sim.room_temp(5, 5).unwrap();
    assert!(t >= 100, "room is {t} tenths after 3 days");
    assert!(sim.hearth_burning(4, 4));
}

#[test]
fn r022_a_hearth_without_fuel_gives_no_heat() {
    let mut sim = heated_room(0);
    assert!(!sim.hearth_burning(4, 4));
    run(&mut sim, 3 * DAY);
    assert_eq!(sim.room_temp(5, 5), Some(-100));
    let chain = sim.tile_temperature(5, 5).unwrap();
    assert!(
        chain
            .parts
            .iter()
            .all(|p| p.cause != TempCause::Hearth || p.tenths == 0)
    );
}

#[test]
fn r022_a_room_without_a_hearth_lags_behind_the_outdoor_temperature() {
    let mut sim = flat_sim(&room_map(), 100, 200, -1000);
    assert_eq!(sim.outdoor_temp(), -100);
    build_room(&mut sim);
    assert_eq!(
        sim.room_temp(5, 5),
        Some(-100),
        "a new room starts outdoors"
    );
    set_rules(&mut sim, &climate_text(1000, 0, 0, -60));
    assert_eq!(sim.outdoor_temp(), 300);
    let mut last = -100;
    for _ in 0..10 {
        run(&mut sim, HOUR);
        let t = sim.room_temp(5, 5).unwrap();
        assert!(t > last && t < 300, "{t} after {last}");
        last = t;
    }
}

/// A 5x5 block of rock with a 3x3 room. `walls` border tiles are wall tiles instead of rock.
fn rock_room(walls: usize) -> Sim {
    let mut rows: Vec<Vec<char>> = (0..5).map(|_| vec!['R'; 5]).collect();
    for row in rows.iter_mut().take(4).skip(1) {
        for cell in row.iter_mut().take(4).skip(1) {
            *cell = '.';
        }
    }
    let border = [
        (1, 0),
        (2, 0),
        (3, 0),
        (1, 4),
        (2, 4),
        (3, 4),
        (0, 1),
        (0, 2),
        (0, 3),
        (4, 1),
        (4, 2),
        (4, 3),
    ];
    for &(x, y) in border.iter().take(walls) {
        rows[y][x] = '#';
    }
    let text: Vec<String> = rows.iter().map(|r| r.iter().collect()).collect();
    let mut sim = flat_sim(&text.join("\n"), 100, 200, -1000);
    let (log, roof) = (mat(&sim, "log"), bld(&sim, "roof"));
    for y in 1..=3 {
        for x in 1..=3 {
            assert!(sim.debug_build(roof, x, y, log));
        }
    }
    sim.step(&[]);
    sim
}

#[test]
fn r022_a_room_with_75_percent_rock_border_moves_toward_the_ground_temperature() {
    let mut sim = rock_room(0);
    let room = sim.room_at(2, 2).unwrap();
    assert!(room.is_rock_room());
    // Outdoor is -10 C, the ground is the annual mean, 10 C.
    assert_eq!(sim.outdoor_temp(), -100);
    run(&mut sim, 3 * DAY);
    let t = sim.room_temp(2, 2).unwrap();
    assert!(t > 0, "the rock room is at {t}, toward the ground 100");
    let chain = sim.tile_temperature(2, 2).unwrap();
    assert_eq!(chain.place, TempPlace::RockRoom);
    assert_eq!(
        chain.parts.iter().map(|p| p.tenths).sum::<i32>(),
        i32::from(chain.total)
    );
}

#[test]
fn r022_the_rock_share_must_be_75_percent_or_more() {
    assert!(rock_room(3).room_at(2, 2).unwrap().is_rock_room());
    let mut sim = rock_room(4);
    assert!(!sim.room_at(2, 2).unwrap().is_rock_room());
    run(&mut sim, 3 * DAY);
    assert_eq!(
        sim.room_temp(2, 2),
        Some(-100),
        "follows the outdoor temperature"
    );
}

#[test]
fn r022_the_why_chain_of_a_heated_room_sums_to_the_room_temperature() {
    let mut sim = heated_room(1_000);
    for hours in [1, 5, 30, 80] {
        run(&mut sim, hours * HOUR);
        let chain = sim.tile_temperature(5, 5).unwrap();
        assert_eq!(chain.place, TempPlace::Room);
        assert_eq!(chain.burning_hearths, 1);
        assert_eq!(chain.insulation, 60);
        assert_eq!(
            chain.parts.iter().map(|p| p.tenths).sum::<i32>(),
            i32::from(chain.total)
        );
        assert_eq!(Some(chain.total), sim.room_temp(5, 5));
    }
    assert_eq!(
        sim.tile_temperature(0, 0).unwrap().place,
        TempPlace::Outdoor
    );
    assert!(sim.tile_temperature(99, 99).is_none());
}

#[test]
fn r022_a_new_room_keeps_the_temperature_of_the_tiles_it_covers() {
    let mut sim = heated_room(1_000);
    run(&mut sim, 2 * DAY);
    let warm = sim.room_temp(5, 5).unwrap();
    assert!(warm > 100);
    let wall = bld(&sim, "wall");
    assert!(sim.debug_build(wall, 6, 6, mat(&sim, "granite")));
    run(&mut sim, 2);
    let after = sim.room_temp(5, 5).unwrap();
    assert!((after - warm).abs() <= 5, "{warm} -> {after}");
}

// ---- hearth fuel ----

#[test]
fn r022_a_hearth_burns_one_hour_of_fuel_in_each_game_hour() {
    let mut sim = flat_sim("c.........\n..........", 100, 0, 0);
    let hearth = bld(&sim, "hearth");
    assert!(sim.debug_build(hearth, 5, 1, mat(&sim, "granite")));
    sim.debug_set_hearth_fuel(5, 1, 8);
    assert!(sim.hearth_burning(5, 1));
    run(&mut sim, 3 * HOUR);
    assert_eq!(sim.hearth_fuel(5, 1), 5);
    run(&mut sim, 5 * HOUR);
    assert_eq!(sim.hearth_fuel(5, 1), 0);
    assert!(!sim.hearth_burning(5, 1));
    run(&mut sim, 5 * HOUR);
    assert_eq!(sim.hearth_fuel(5, 1), 0);
}

#[test]
fn r022_a_hauling_job_refuels_the_hearth() {
    let mut sim = flat_sim("c.........\n..........\n..........", 100, 0, 0);
    let hearth = bld(&sim, "hearth");
    assert!(sim.debug_build(hearth, 7, 1, mat(&sim, "granite")));
    let firewood = mat(&sim, "firewood");
    sim.debug_spawn_item(firewood, 20, 2, 1);
    assert_eq!(sim.hearth_fuel(7, 1), 0);
    let mut job_seen = false;
    for _ in 0..(3 * HOUR) {
        sim.step(&[]);
        sim.check_invariants().unwrap();
        job_seen |= sim.jobs().iter().any(|(_, j)| j.is_refuel());
    }
    assert!(job_seen, "the hearth got a refuel job");
    let fuel = sim.hearth_fuel(7, 1);
    assert!((40..=48).contains(&fuel), "fuel {fuel}");
    assert!(sim.items().total_units(firewood) < 20);
    assert!(
        sim.jobs().iter().all(|(_, j)| !j.is_refuel()),
        "job is done"
    );
}

#[test]
fn r022_a_full_hearth_gets_no_refuel_job() {
    let mut sim = flat_sim("c.........\n..........", 100, 0, 0);
    let hearth = bld(&sim, "hearth");
    assert!(sim.debug_build(hearth, 7, 1, mat(&sim, "granite")));
    sim.debug_set_hearth_fuel(7, 1, 48);
    sim.debug_spawn_item(mat(&sim, "firewood"), 5, 2, 1);
    run_checked(&mut sim, 5 * HOUR);
    assert!(sim.jobs().iter().all(|(_, j)| !j.is_refuel()));
    assert_eq!(sim.hearth_fuel(7, 1), 43);
}

#[test]
fn r022_a_refuel_job_waits_without_fuel_and_coal_also_burns() {
    let mut sim = flat_sim("c.........\n..........", 100, 0, 0);
    let hearth = bld(&sim, "hearth");
    assert!(sim.debug_build(hearth, 7, 1, mat(&sim, "granite")));
    run_checked(&mut sim, 3 * HOUR);
    assert_eq!(sim.hearth_fuel(7, 1), 0);
    sim.debug_spawn_item(mat(&sim, "coal"), 3, 1, 0);
    run_checked(&mut sim, 2 * HOUR);
    assert!(sim.hearth_fuel(7, 1) >= 10, "3 coal give 12 hours");
}

// ---- R-004 ----

fn water_map() -> &'static str {
    "c.........\n..........\n.~~~~~~~~~\n..........\n.........."
}

#[test]
fn r004_shallow_water_freezes_after_3_days_below_zero_and_thaws_after_3_days_above() {
    let mut sim = flat_sim(water_map(), -50, 0, 0);
    assert!(!sim.map().passable(3, 2));
    assert!(!sim.ice());
    run(&mut sim, 3 * DAY - 1);
    assert!(!sim.ice(), "not yet after 2 days and 23 hours");
    run(&mut sim, 1);
    assert!(sim.ice(), "frozen after 3 days");
    assert!(sim.ice_at(3, 2));
    assert!(sim.map().passable(3, 2), "colonists can walk on ice");
    sim.set_climate(50, 0, 128);
    run(&mut sim, 3 * DAY - 1);
    assert!(sim.ice(), "ice holds until the third warm day ends");
    run(&mut sim, 1);
    assert!(!sim.ice(), "thawed after 3 days above 0");
    assert!(!sim.map().passable(3, 2));
}

#[test]
fn r004_a_day_at_exactly_zero_resets_the_count() {
    let mut sim = flat_sim(water_map(), -50, 0, 0);
    run(&mut sim, 2 * DAY);
    assert_eq!(sim.ice_counters(), (2, 0));
    sim.set_climate(0, 0, 128);
    run(&mut sim, DAY);
    assert_eq!(sim.ice_counters(), (0, 0));
    sim.set_climate(-50, 0, 128);
    run(&mut sim, 2 * DAY);
    assert!(!sim.ice());
    run(&mut sim, DAY);
    assert!(sim.ice());
}

#[test]
fn r004_deep_water_does_not_freeze() {
    let mut rows: Vec<String> = vec!["c".to_string() + &".".repeat(11)];
    for _ in 0..9 {
        rows.push(".".to_string() + &"~".repeat(11));
    }
    rows.push(".".repeat(12));
    let mut sim = flat_sim(&rows.join("\n"), -50, 0, 0);
    assert!(sim.is_deep_water(6, 5));
    assert!(!sim.is_deep_water(1, 5));
    run(&mut sim, 3 * DAY);
    assert!(sim.ice());
    assert!(sim.map().passable(1, 5), "shallow water is ice");
    assert!(!sim.map().passable(6, 5), "deep water stays open");
    assert!(!sim.ice_at(6, 5));
}

#[test]
fn r004_a_colonist_on_the_ice_is_moved_to_land_when_it_thaws() {
    let mut sim = flat_sim("c...\n.~~.\n....", -50, 0, 0);
    let id = sim.colonists().iter().next().unwrap().0;
    run(&mut sim, 3 * DAY);
    assert!(sim.ice());
    sim.step(&[Command::MoveColonist { id, x: 1, y: 1 }]);
    run_checked(&mut sim, 400);
    assert_eq!(
        sim.colonist(id).unwrap().tile(),
        (1, 1),
        "stands on the ice"
    );
    sim.set_climate(50, 0, 128);
    run_checked(&mut sim, 3 * DAY);
    assert!(!sim.ice());
    let (x, y) = sim.colonist(id).unwrap().tile();
    assert!(sim.map().passable(i32::from(x), i32::from(y)));
}

// ---- alerts ----

#[test]
fn alert_low_fuel_names_the_hearth_with_less_than_12_hours() {
    let mut sim = flat_sim("c.........\n..........", 100, 0, 0);
    let hearth = bld(&sim, "hearth");
    assert!(sim.debug_build(hearth, 7, 1, mat(&sim, "granite")));
    assert!(sim.debug_build(hearth, 8, 0, mat(&sim, "granite")));
    sim.debug_set_hearth_fuel(7, 1, 12);
    sim.debug_set_hearth_fuel(8, 0, 11);
    let low: Vec<_> = sim
        .alerts()
        .into_iter()
        .filter(|a| matches!(a.kind, AlertKind::LowFuel { .. }))
        .collect();
    assert_eq!(low.len(), 1);
    assert_eq!(low[0].tile(), Some((8, 0)));
    assert_eq!(
        low[0].kind,
        AlertKind::LowFuel {
            x: 8,
            y: 0,
            hours: 11
        }
    );
    sim.debug_set_hearth_fuel(7, 1, 40);
    sim.debug_set_hearth_fuel(8, 0, 40);
    assert!(sim.alerts().is_empty());
}

#[test]
fn alert_cold_period_shows_now_and_in_the_forecast() {
    let mut sim = sim_from_ascii("c..", 4).unwrap();
    set_rules(&mut sim, &climate_text(0, 0, 30, 120));
    let alerts = sim.alerts();
    assert_eq!(alerts.len(), 1);
    assert!(matches!(
        alerts[0].kind,
        AlertKind::ColdPeriod { days_ahead: 0, .. }
    ));
    set_rules(&mut sim, &climate_text(0, 0, 30, -121));
    assert!(sim.alerts().is_empty());
}

// ---- save ----

#[test]
fn climate_state_survives_a_save_and_the_run_continues_the_same() {
    let mut sim = flat_sim(&room_map(), -100, 0, 0);
    set_rules(&mut sim, BUILTIN_TEXTS.climate);
    sim.set_climate(30, 160, 200);
    build_room(&mut sim);
    let hearth = bld(&sim, "hearth");
    assert!(sim.debug_build(hearth, 4, 4, mat(&sim, "granite")));
    sim.debug_set_hearth_fuel(4, 4, 30);
    sim.debug_spawn_item(mat(&sim, "firewood"), 6, 1, 1);
    run(&mut sim, 5 * DAY + 250);
    let bytes = save::to_bytes(&sim);
    let mut loaded = save::from_bytes(&bytes).unwrap();
    assert_eq!(loaded.state_hash(), sim.state_hash());
    assert_eq!(loaded.weather_today(), sim.weather_today());
    assert_eq!(loaded.forecast(), sim.forecast());
    assert_eq!(loaded.room_temp(5, 5), sim.room_temp(5, 5));
    assert_eq!(loaded.hearth_fuel(4, 4), sim.hearth_fuel(4, 4));
    for _ in 0..(8 * DAY) {
        sim.step(&[]);
        loaded.step(&[]);
    }
    assert_eq!(loaded.state_hash(), sim.state_hash());
    assert_eq!(loaded.ice(), sim.ice());
}

// ---- content ----

#[test]
fn climate_content_loads_and_rejects_bad_files() {
    let rules = SimRules::default();
    assert_eq!(rules.climate.season_curve.len(), 40);
    assert_eq!(rules.fuel.len(), 2);
    let bad = BUILTIN_TEXTS
        .climate
        .replace("hearth_heat: 400", "hearth_heat: 4.5");
    assert!(SimRules::load_with_climate(&bad).is_err());
    let missing = BUILTIN_TEXTS.climate.replace("\"firewood\"", "\"timber\"");
    let err = SimRules::load_with_climate(&missing)
        .unwrap_err()
        .to_string();
    assert!(err.contains("timber"), "{err}");
}
