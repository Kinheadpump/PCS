//! Sprint 26 tests (sim part): realms (R-051, R-052). A colony belongs to one realm of the plane.
//! The save keeps the realm and the version of the world generator, and it is refused when the
//! generator version is not the version of this build.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use sim::scenario::sim_from_ascii;
use sim::{SaveError, save};

#[test]
fn r051_a_new_colony_is_in_the_origin_realm_with_the_current_generator() {
    let sim = sim_from_ascii("c..", 1).unwrap();
    assert_eq!(sim.realm(), (0, 0));
    assert_eq!(sim.world_gen_version(), worldgen::WORLD_GEN_VERSION);
}

#[test]
fn r051_the_realm_is_part_of_the_state_and_survives_a_save() {
    let mut a = sim_from_ascii("c..", 1).unwrap();
    let b = sim_from_ascii("c..", 1).unwrap();
    assert_eq!(a.state_hash(), b.state_hash());
    a.set_realm(-2, 3);
    assert_ne!(a.state_hash(), b.state_hash());
    for _ in 0..50 {
        a.step(&[]);
    }
    let loaded = save::from_bytes(&save::to_bytes(&a)).unwrap();
    assert_eq!(loaded.realm(), (-2, 3));
    assert_eq!(loaded.state_hash(), a.state_hash());
}

#[test]
fn r052_a_save_of_another_generator_version_is_refused() {
    let mut sim = sim_from_ascii("c..", 1).unwrap();
    sim.debug_set_world_gen_version(worldgen::WORLD_GEN_VERSION + 1);
    let bytes = save::to_bytes(&sim);
    assert_eq!(
        save::from_bytes(&bytes).unwrap_err(),
        SaveError::WorldMismatch {
            found: worldgen::WORLD_GEN_VERSION + 1,
            expected: worldgen::WORLD_GEN_VERSION
        }
    );
    assert!(
        SaveError::WorldMismatch {
            found: 2,
            expected: 1
        }
        .to_string()
        .contains("world")
    );
}

#[test]
fn r052_the_save_format_is_18() {
    assert_eq!(save::FORMAT_VERSION, 19);
}
