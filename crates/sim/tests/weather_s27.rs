//! Sprint 27 tests (sim part): snow, floods, and storms (R-053, R-054, R-055). Snow falls on cold
//! days with precipitation and slows walking outdoors. The spring melt raises the river level,
//! and a high river floods the land near water. A wall line is a dike. A storm can tear roofs
//! off, wood roofs first. The first days of a colony are safe.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use sim::scenario::sim_from_ascii;
use sim::{
    BuildLayer, ChronicleKind, ColonistId, Command, MaterialCategory, MoodCause, Sim, SimRules,
    WalkCause, WeatherEvent, save,
};
use std::sync::Arc;

const HOUR: u64 = 100;

fn with_rules(sim: &mut Sim, edit: impl FnOnce(&mut SimRules)) {
    let mut rules = sim.rules().clone();
    edit(&mut rules);
    sim.debug_set_rules(Arc::new(rules));
}

fn ids(sim: &Sim) -> Vec<ColonistId> {
    sim.colonists().iter().map(|(id, _)| id).collect()
}

fn run_hours(sim: &mut Sim, hours: u64) {
    for _ in 0..hours {
        for id in ids(sim) {
            sim.debug_set_hunger(id, 100);
            sim.debug_set_rest(id, 100);
            sim.debug_set_health(id, 100);
        }
        for _ in 0..HOUR {
            sim.step(&[]);
        }
    }
}

fn run_days(sim: &mut Sim, days: u64) {
    run_hours(sim, days * 24);
}

/// Water along the top row, open land below, a colonist near the bottom.
const SHORE: &str = "
~~~~~~~~~~~~
............
............
............
............
............
............
.....c......
";

/// A shore map with a closed wall line (a dike) between the water and the land.
const DIKED: &str = "
~~~~~~~~~~~~
############
............
............
............
............
............
.....c......
";

/// A dry, warm colony: no rain ever, so only the debug values move the weather numbers.
fn dry(map: &str) -> Sim {
    let mut sim = sim_from_ascii(map, 1).unwrap();
    sim.set_climate(200, 0, 0);
    with_rules(&mut sim, |r| r.weather.grace_days = 0);
    sim
}

/// A cold colony with precipitation on about half of the days.
fn cold() -> Sim {
    let mut sim = sim_from_ascii(SHORE, 1).unwrap();
    sim.set_climate(-100, 0, 255);
    with_rules(&mut sim, |r| r.weather.grace_days = 0);
    sim
}

fn memory_value(sim: &Sim, id: ColonistId, key: &str) -> Option<i32> {
    let index = sim.rules().needs.memory_index(key)?;
    sim.mood(id)?
        .parts
        .iter()
        .find(|p| p.cause == MoodCause::Memory { key: index })
        .map(|p| p.value)
}

fn material(sim: &Sim, category: MaterialCategory) -> u16 {
    sim.rules()
        .materials
        .iter()
        .position(|m| m.category == category)
        .map(|i| i as u16)
        .unwrap()
}

#[test]
fn r053_the_content_is_ordered_and_has_the_keys() {
    let rules = SimRules::builtin();
    let w = &rules.weather;
    assert!(w.flood_end < w.flood_start && w.flood_start <= w.level_max);
    assert!(w.snow_slow_max < 100 && w.flood_slow_pct < 100);
    assert!(w.flood_reach_min <= w.flood_reach_max);
    assert!(rules.needs.memory_index("storm_damage").is_some());
    assert!(w.storm_roof_permille.iter().all(|&p| p <= 1000));
    assert!(w.stone_roof_pct <= 100);
}

#[test]
fn r053_snow_falls_on_cold_days_and_never_on_warm_days() {
    let mut sim = cold();
    let max = sim.rules().weather.snow_max;
    run_days(&mut sim, 12);
    assert!(sim.snow_depth() > 0);
    assert!(sim.snow_depth() <= max);
    assert!(
        sim.drain_weather_events()
            .iter()
            .any(|e| matches!(e, WeatherEvent::SnowFell { .. }))
    );
    let mut warm = sim_from_ascii(SHORE, 1).unwrap();
    warm.set_climate(200, 0, 255);
    run_days(&mut warm, 12);
    assert_eq!(warm.snow_depth(), 0);
}

#[test]
fn r053_warm_days_melt_the_snow_and_raise_the_river() {
    let mut sim = dry(SHORE);
    sim.debug_set_snow(30);
    let w = sim.rules().weather.clone();
    // The melt is melt_base + the day mean in tenths x melt_per_deg / 10.
    let mean = u32::try_from(sim.forecast()[0].mean_temp).unwrap();
    let melt = w.melt_base + mean * w.melt_per_deg / 10;
    run_days(&mut sim, 1);
    let left = 30u32.saturating_sub(melt);
    assert_eq!(sim.snow_depth(), left);
    let melted = 30 - left;
    assert_eq!(
        sim.river_level(),
        (melted * w.melt_to_level).min(w.level_max)
    );
}

#[test]
fn r053_snow_slows_walking_outdoors_with_named_parts() {
    let mut sim = dry(SHORE);
    let w = sim.rules().weather.clone();
    let open = sim.walk_speed(3, 3);
    assert_eq!(open.pct, 100);
    assert_eq!(open.parts.len(), 1);
    sim.debug_set_snow(20);
    let slowed = sim.walk_speed(3, 3);
    let loss = (20 / 10 * w.snow_slow_per_10).min(w.snow_slow_max);
    assert_eq!(slowed.pct, 100 - loss);
    assert!(
        slowed
            .parts
            .iter()
            .any(|p| p.cause == WalkCause::Snow && p.value == -i32::try_from(loss).unwrap())
    );
    // The sum of the parts is the speed.
    assert_eq!(
        slowed.parts.iter().map(|p| p.value).sum::<i32>(),
        i32::try_from(slowed.pct).unwrap()
    );
    // A tile under a roof has no snow.
    let plank = material(&sim, MaterialCategory::Wood);
    let roof = sim.rules().building_index("roof").unwrap();
    assert!(sim.debug_build(roof, 3, 3, plank));
    assert_eq!(sim.walk_speed(3, 3).pct, 100);
    // The snow never slows more than the limit.
    sim.debug_set_snow(1000);
    assert!(sim.walk_speed(4, 4).pct >= 100 - w.snow_slow_max);
}

#[test]
fn r053_a_colonist_walks_slower_in_snow() {
    let walk = |snow: u32| {
        let mut sim = dry(SHORE);
        sim.debug_set_snow(snow);
        // The snow must stay: no melt for this test.
        with_rules(&mut sim, |r| {
            r.weather.melt_base = 0;
            r.weather.melt_per_deg = 0;
        });
        let id = ids(&sim)[0];
        sim.step(&[Command::MoveColonist { id, x: 0, y: 7 }]);
        let mut ticks = 1;
        while sim.colonist(id).unwrap().target.is_some() && ticks < 2000 {
            sim.step(&[]);
            ticks += 1;
        }
        ticks
    };
    assert!(walk(40) > walk(0));
}

#[test]
fn r054_a_high_river_floods_the_land_near_the_water() {
    let mut sim = dry(SHORE);
    sim.debug_set_river_level(100);
    sim.drain_weather_events();
    run_days(&mut sim, 1);
    assert!(sim.flood_active());
    let tiles = sim.flood_tiles();
    assert!(!tiles.is_empty());
    // Only land tiles near the water flood: row 1 is next to the water.
    assert!(tiles.contains(&(5, 1)));
    assert!(tiles.iter().all(|&(_, y)| y >= 1));
    let reach = sim.flood_reach();
    assert!(tiles.iter().all(|&(_, y)| u32::from(y) <= reach));
    assert!(tiles.contains(&(5, u16::try_from(reach).unwrap())));
    assert!(!sim.flooded(5, u16::try_from(reach).unwrap() + 1));
    assert!(
        sim.drain_weather_events()
            .iter()
            .any(|e| matches!(e, WeatherEvent::FloodBegan { .. }))
    );
    assert!(
        sim.chronicle()
            .iter()
            .any(|e| e.kind == ChronicleKind::Flood)
    );
    // A flooded tile slows walking, with a named part.
    let speed = sim.walk_speed(5, 1);
    let pct = sim.rules().weather.flood_slow_pct;
    assert_eq!(speed.pct, 100 - pct);
    assert!(speed.parts.iter().any(|p| p.cause == WalkCause::Flood));
}

#[test]
fn r054_a_wall_line_is_a_dike() {
    let mut sim = dry(DIKED);
    sim.debug_set_river_level(100);
    run_days(&mut sim, 1);
    assert!(sim.flood_active());
    // The wall tiles are not flooded and the land behind them stays dry.
    assert!(sim.flood_tiles().iter().all(|&(_, y)| y < 2));
    assert!(!sim.flooded(5, 2));
}

#[test]
fn r054_a_flood_destroys_the_crops_on_flooded_tiles() {
    let mut sim = dry(SHORE);
    let crop = 0;
    assert!(sim.debug_plant(5, 1, crop, false));
    assert!(sim.debug_plant(5, 7, crop, false));
    sim.debug_set_river_level(100);
    sim.drain_weather_events();
    run_days(&mut sim, 1);
    assert!(sim.plant_at(5, 1).is_none());
    assert!(sim.plant_at(5, 7).is_some());
    assert!(
        sim.drain_weather_events()
            .iter()
            .any(|e| matches!(e, WeatherEvent::CropsFlooded { count: 1 }))
    );
}

#[test]
fn r054_the_flood_ends_when_the_river_falls() {
    let mut sim = dry(SHORE);
    sim.debug_set_river_level(100);
    run_days(&mut sim, 1);
    assert!(sim.flood_active());
    sim.drain_weather_events();
    run_days(&mut sim, 8);
    assert!(!sim.flood_active());
    assert!(sim.flood_tiles().is_empty());
    assert!(
        sim.drain_weather_events()
            .iter()
            .any(|e| matches!(e, WeatherEvent::FloodEnded { .. }))
    );
    assert_eq!(sim.weather_counts().0, 1);
}

#[test]
fn r054_no_flood_in_the_grace_days() {
    let mut sim = sim_from_ascii(SHORE, 1).unwrap();
    sim.set_climate(200, 0, 0);
    sim.debug_set_river_level(100);
    run_days(&mut sim, 2);
    assert!(!sim.flood_active());
    with_rules(&mut sim, |r| r.weather.grace_days = 0);
    run_days(&mut sim, 1);
    assert!(sim.flood_active());
}

#[test]
fn r054_the_forecast_names_the_river_level_of_the_next_days() {
    let mut sim = dry(SHORE);
    sim.debug_set_snow(25);
    sim.debug_set_river_level(40);
    let forecast = sim.flood_forecast();
    assert_eq!(forecast.len(), 3);
    let mut levels = Vec::new();
    for _ in 0..3 {
        run_days(&mut sim, 1);
        levels.push(sim.river_level());
    }
    for (i, f) in forecast.iter().enumerate() {
        assert_eq!(f.level, levels[i], "day {i}");
        assert_eq!(f.flood, levels[i] >= sim.rules().weather.flood_start);
    }
}

#[test]
fn r055_a_storm_tears_wood_roofs_off_and_spares_stone_roofs() {
    let mut sim = dry(SHORE);
    with_rules(&mut sim, |r| {
        r.weather.storm_permille = 1000;
        r.weather.storm_roof_permille = [1000, 1000, 1000];
        r.weather.stone_roof_pct = 0;
    });
    let wood = material(&sim, MaterialCategory::Wood);
    let stone = material(&sim, MaterialCategory::Stone);
    let roof = sim.rules().building_index("roof").unwrap();
    for x in 2..5 {
        assert!(sim.debug_build(roof, x, 4, wood));
        assert!(sim.debug_build(roof, x, 5, stone));
    }
    let forecast = sim.storm_forecast();
    assert!(forecast[0].is_some());
    sim.drain_weather_events();
    run_days(&mut sim, 1);
    for x in 2..5 {
        assert!(sim.buildings().roof_at(x, 4).is_none(), "wood roof {x}");
        assert!(sim.buildings().roof_at(x, 5).is_some(), "stone roof {x}");
    }
    let events = sim.drain_weather_events();
    let hit = events
        .iter()
        .find_map(|e| match e {
            WeatherEvent::Storm {
                strength,
                roofs_lost,
            } => Some((*strength, *roofs_lost)),
            _ => None,
        })
        .unwrap();
    assert_eq!(Some(hit.0), forecast[0]);
    assert_eq!(hit.1, 3);
    assert!(
        sim.chronicle()
            .iter()
            .any(|e| e.kind == ChronicleKind::Storm)
    );
    let id = ids(&sim)[0];
    assert!(memory_value(&sim, id, "storm_damage").is_some());
    assert_eq!(sim.weather_counts().1, 1);
}

#[test]
fn r055_a_storm_that_tears_off_nothing_gives_no_memory() {
    let mut sim = dry(SHORE);
    with_rules(&mut sim, |r| r.weather.storm_permille = 1000);
    run_days(&mut sim, 1);
    let id = ids(&sim)[0];
    assert!(memory_value(&sim, id, "storm_damage").is_none());
    assert_eq!(sim.weather_counts().1, 1);
}

#[test]
fn r055_the_removed_roof_opens_the_room() {
    let mut sim = dry(SHORE);
    with_rules(&mut sim, |r| {
        r.weather.storm_permille = 1000;
        r.weather.storm_roof_permille = [1000, 1000, 1000];
    });
    let wood = material(&sim, MaterialCategory::Wood);
    let roof = sim.rules().building_index("roof").unwrap();
    assert!(sim.debug_build(roof, 3, 4, wood));
    run_days(&mut sim, 1);
    assert!(sim.buildings().roof_at(3, 4).is_none());
    assert!(!sim.debug_remove_building(3, 4, BuildLayer::Roof));
}

#[test]
fn r055_there_are_no_storms_in_winter() {
    let mut sim = dry(SHORE);
    with_rules(&mut sim, |r| r.weather.storm_permille = 1000);
    // Winter is the game days 30 to 39 of the year. Day 40 is spring again.
    run_days(&mut sim, 30);
    sim.drain_weather_events();
    let before = sim.weather_counts().1;
    run_days(&mut sim, 9);
    assert_eq!(
        sim.weather_counts().1,
        before,
        "no storm in the winter days"
    );
}

#[test]
fn r053_the_first_days_of_a_colony_are_safe() {
    let mut sim = sim_from_ascii(SHORE, 1).unwrap();
    sim.set_climate(110, 160, 255);
    let grace = u64::from(sim.rules().weather.grace_days);
    assert!(grace >= 10);
    run_days(&mut sim, 12);
    assert_eq!(sim.weather_counts(), (0, 0));
    assert!(!sim.flood_active());
}

#[test]
fn r054_snow_flood_and_storm_state_survives_a_save() {
    let mut sim = dry(SHORE);
    sim.debug_set_snow(15);
    sim.debug_set_river_level(100);
    with_rules(&mut sim, |r| {
        r.weather.melt_base = 0;
        r.weather.melt_per_deg = 0;
    });
    run_days(&mut sim, 2);
    assert!(sim.flood_active());
    let loaded = save::from_bytes(&save::to_bytes(&sim)).unwrap();
    assert_eq!(loaded.snow_depth(), 15);
    assert_eq!(loaded.river_level(), sim.river_level());
    assert_eq!(loaded.flood_tiles(), sim.flood_tiles());
    assert_eq!(loaded.state_hash(), sim.state_hash());
    assert_eq!(save::FORMAT_VERSION, 19);
}

#[test]
fn r053_two_runs_agree() {
    let make = || {
        let mut sim = cold();
        run_days(&mut sim, 20);
        sim
    };
    assert_eq!(make().state_hash(), make().state_hash());
}
