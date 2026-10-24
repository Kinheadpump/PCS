//! Sprint 26 tests (worldgen part): realms (R-051, R-052). A realm is a finite world of
//! 256 x 256 cells with a sub-seed. Realm (0, 0) is the world of the seed itself. The edge of
//! each realm is ocean. A cache holds a few realms and makes one missing realm at a time.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::BTreeSet;
use worldgen::realm::{
    RealmCache, RealmPos, WORLD_GEN_VERSION, generate_realm, realm_distance, realm_neighbors,
    realm_seed, summarize,
};
use worldgen::{Stage, WORLD_SIZE, WorldBiome, WorldParams, generate_world};

fn pos(rx: i32, ry: i32) -> RealmPos {
    RealmPos { rx, ry }
}

fn golden_path() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("golden_realm.txt")
}

#[test]
fn r051_realm_origin_is_the_world_of_the_seed() {
    let p = WorldParams::default();
    assert_eq!(realm_seed(7, pos(0, 0)), 7);
    let a = generate_realm(7, pos(0, 0), &p);
    let b = generate_world(7, &p);
    assert_eq!(a.stage_hashes, b.stage_hashes);
    assert_eq!(a.region_hashes, b.region_hashes);
}

#[test]
fn r051_each_realm_of_the_plane_has_its_own_sub_seed() {
    let mut seen = BTreeSet::new();
    for rx in -6..=6 {
        for ry in -6..=6 {
            assert!(seen.insert(realm_seed(1, pos(rx, ry))), "{rx},{ry}");
        }
    }
    assert_ne!(realm_seed(1, pos(1, 0)), realm_seed(2, pos(1, 0)));
    assert_ne!(realm_seed(1, pos(1, 2)), realm_seed(1, pos(2, 1)));
}

#[test]
fn r051_a_realm_depends_only_on_seed_and_position() {
    let p = WorldParams::default();
    let first = generate_realm(1, pos(2, -1), &p);
    let _other = generate_realm(1, pos(-3, 4), &p);
    let again = generate_realm(1, pos(2, -1), &p);
    assert_eq!(first.stage_hashes, again.stage_hashes);
    let near = generate_realm(1, pos(2, 0), &p);
    assert_ne!(
        first.stage_hashes[Stage::Height as usize],
        near.stage_hashes[Stage::Height as usize]
    );
}

#[test]
fn r051_the_edge_of_every_realm_is_ocean() {
    let p = WorldParams::default();
    for at in [pos(0, 0), pos(1, 0), pos(-1, 2), pos(3, -3)] {
        let w = generate_realm(1, at, &p);
        let size = WORLD_SIZE as i32;
        for i in 0..size {
            for (x, y) in [(i, 0), (i, size - 1), (0, i), (size - 1, i)] {
                assert!(w.height_above_sea(x, y) < 0, "{at:?} {x},{y}");
                assert_eq!(w.biome.get(x, y).copied(), Some(WorldBiome::Ocean));
            }
        }
    }
}

#[test]
fn r051_neighbors_and_distance_on_the_plane() {
    let n = realm_neighbors(pos(2, -1));
    let set: BTreeSet<_> = n.iter().copied().collect();
    assert_eq!(set.len(), 8);
    assert!(n.iter().all(|&q| realm_distance(pos(2, -1), q) == 1));
    assert_eq!(realm_distance(pos(0, 0), pos(3, -5)), 5);
    assert_eq!(realm_distance(pos(-2, 4), pos(-2, 4)), 0);
    assert_eq!(
        realm_distance(pos(1, 2), pos(-1, 0)),
        realm_distance(pos(-1, 0), pos(1, 2))
    );
}

#[test]
fn r052_a_summary_counts_the_cells_of_a_realm() {
    let p = WorldParams::default();
    let w = generate_realm(1, pos(1, 1), &p);
    let s = summarize(pos(1, 1), &w);
    assert_eq!(s.pos, pos(1, 1));
    assert_eq!(s.seed, realm_seed(1, pos(1, 1)));
    assert_eq!(s.land_per_mille, w.land_per_mille());
    assert_eq!(s.biome_cells.iter().sum::<u32>(), WORLD_SIZE * WORLD_SIZE);
    assert!(s.biome_cells[WorldBiome::Ocean as usize] > 0);
    assert_eq!(s.hash, w.region_hashes[0]);
}

#[test]
fn r052_the_cache_keeps_a_few_realms_and_drops_the_oldest() {
    let mut cache = RealmCache::new(1, WorldParams::default(), 2);
    let a = cache.get(pos(1, 0));
    assert_eq!((cache.hits(), cache.misses(), cache.len()), (0, 1, 1));
    cache.get(pos(0, 1));
    cache.get(pos(1, 0));
    assert_eq!((cache.hits(), cache.misses()), (1, 2));
    // (0, 1) is now the least recently used.
    cache.get(pos(-1, 0));
    assert_eq!(cache.len(), 2);
    assert!(cache.contains(pos(1, 0)));
    assert!(cache.contains(pos(-1, 0)));
    assert!(!cache.contains(pos(0, 1)));
    // A realm from the cache equals a fresh one.
    let fresh = generate_realm(1, pos(1, 0), &WorldParams::default());
    assert_eq!(a.stage_hashes, fresh.stage_hashes);
}

#[test]
fn r052_the_cache_makes_one_missing_realm_for_each_call() {
    let mut cache = RealmCache::new(1, WorldParams::default(), 4);
    let wanted = [pos(0, 0), pos(1, 0), pos(0, 1)];
    assert_eq!(cache.generate_next(&wanted), Some(pos(0, 0)));
    assert_eq!(cache.generate_next(&wanted), Some(pos(1, 0)));
    assert_eq!(cache.len(), 2);
    assert_eq!(cache.generate_next(&wanted), Some(pos(0, 1)));
    assert_eq!(cache.generate_next(&wanted), None);
    assert_eq!(cache.len(), 3);
}

#[test]
fn r051_realm_hashes_match_the_golden_file() {
    let p = WorldParams::default();
    let mut out = String::new();
    for at in [pos(1, 0), pos(-1, 1)] {
        let w = generate_realm(1, at, &p);
        out.push_str(&format!(
            "realm{}_{}.height={:016x}\nrealm{}_{}.regions={:016x}\n",
            at.rx,
            at.ry,
            w.stage_hashes[Stage::Height as usize],
            at.rx,
            at.ry,
            w.region_hashes[0]
        ));
    }
    out.push_str(&format!("version={WORLD_GEN_VERSION}\n"));
    if std::env::var_os("UPDATE_GOLDEN").is_some() {
        std::fs::write(golden_path(), &out).unwrap();
    }
    let want = std::fs::read_to_string(golden_path())
        .unwrap()
        .replace('\r', "");
    assert_eq!(
        out, want,
        "golden hash changed; run with UPDATE_GOLDEN=1 only if the task permits"
    );
}
