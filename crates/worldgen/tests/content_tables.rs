//! Tests of the embedded rock and mineral tables (rocks.ron and deposits.ron).

use worldgen::deposit::affinity;
use worldgen::geology::{check_tables, tables};
use worldgen::{Mineral, Rock, Setting};

#[test]
fn embedded_content_tables_are_valid() {
    let result = check_tables();
    assert!(result.is_ok(), "{result:?}");
    assert!(tables().is_ok());
}

/// The tables as the code had them before the move to RON. Output must not change.
#[rustfmt::skip]
const OLD_AFFINITY: [[u8; Mineral::COUNT]; Rock::COUNT] = [
    [30, 20, 10,  0,  0,  0],
    [60, 10,  0,  0,  0,  0],
    [20, 40, 20,  0,  0,  5],
    [ 0, 20, 30, 60, 30, 10],
    [ 0, 30, 30, 30, 20, 20],
    [ 0,  0, 10,  0, 30, 50],
    [ 0, 20, 50,  0, 10, 20],
    [ 0, 10, 30,  0, 20, 40],
];

const OLD_POOLS: [[Rock; 4]; 4] = [
    [Rock::Sandstone, Rock::Shale, Rock::Limestone, Rock::Granite],
    [Rock::Granite, Rock::Gneiss, Rock::Marble, Rock::Limestone],
    [Rock::Basalt, Rock::Tuff, Rock::Granite, Rock::Gneiss],
    [Rock::Limestone, Rock::Sandstone, Rock::Shale, Rock::Marble],
];

#[test]
fn embedded_affinity_equals_the_old_table() {
    for rock in Rock::ALL {
        for mineral in Mineral::ALL {
            let row = OLD_AFFINITY.get(rock as usize);
            let old = row.and_then(|r| r.get(mineral as usize));
            assert_eq!(Some(&affinity(rock, mineral)), old, "{rock:?} {mineral:?}");
        }
    }
}

#[test]
fn embedded_pools_equal_the_old_table() {
    let t = tables().unwrap_or_else(|e| panic!("{e}"));
    let settings = [
        Setting::Plain,
        Setting::Mountain,
        Setting::Volcanic,
        Setting::Coast,
    ];
    for (setting, old) in settings.iter().zip(OLD_POOLS.iter()) {
        assert_eq!(&t.pool(*setting), old, "{setting:?}");
    }
}
