//! The start camp: a fresh colony with no orders must not die from cold.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use sim::Sim;
use worldgen::{EmbarkSummary, generate_local};

fn started(preset: EmbarkSummary) -> Sim {
    let map = generate_local(&preset);
    let mut sim = Sim::new(&map, 7, 0);
    let p = sim.climate_params();
    sim.set_climate(preset.mean_temperature, p.amplitude, p.moisture);
    sim
}

#[test]
fn a_fresh_colony_with_no_orders_lives_through_ten_days() {
    for (name, preset) in [
        ("temperate", EmbarkSummary::temperate_forest()),
        ("cold", EmbarkSummary::cold_hills()),
        ("dry", EmbarkSummary::dry_plain()),
    ] {
        let mut sim = started(preset);
        assert_eq!(sim.colonists().len(), 7, "{name}");
        for _ in 0..(10 * 2400) {
            sim.step(&[]);
        }
        assert_eq!(sim.colonists().len(), 7, "{name}: nobody dies");
        sim.check_invariants().unwrap();
    }
}

#[test]
fn the_start_camp_has_a_burning_hearth_and_a_stockpile_with_the_stock() {
    let sim = started(EmbarkSummary::cold_hills());
    let hearths = sim.hearth_tiles();
    assert_eq!(hearths.len(), 1);
    assert!(sim.hearth_burning(hearths[0].0, hearths[0].1));
    assert_eq!(sim.stockpiles().iter().count(), 1);
}
