//! Sprint 11 tests (worldgen part): embark summary from the world, site stamping, R-041, Rule 9.

#![allow(clippy::expect_used)]

use worldgen::history::SiteKind;
use worldgen::{
    EmbarkSite, EmbarkSites, EmbarkSummary, SiteTile, WORLD_SIZE, WorldParams,
    check_embark_content, embark_from_world, generate_history, generate_local,
    generate_local_with_sites, generate_world, is_habitable,
};

fn site(id: u32, kind: SiteKind, age: u32) -> EmbarkSite {
    EmbarkSite {
        id,
        kind,
        age,
        culture: 0,
        mineral: 1,
        level: 2,
    }
}

fn sites_of(list: Vec<EmbarkSite>) -> EmbarkSites {
    EmbarkSites {
        site_count: list.len() as u32,
        sites: list,
        ..EmbarkSites::default()
    }
}

fn count_tiles(map: &worldgen::LocalMapData, tile: SiteTile) -> usize {
    map.site.as_slice().iter().filter(|&&t| t == tile).count()
}

fn count_ore(map: &worldgen::LocalMapData) -> usize {
    map.ore
        .as_slice()
        .iter()
        .filter(|&&o| o != worldgen::Ore::None)
        .count()
}

#[test]
fn s11_embedded_embark_table_is_valid() {
    assert!(check_embark_content().is_ok());
}

/// R-041: each local map has at least 1 site. With no record, local generation adds a ruin.
#[test]
fn r041_a_map_without_history_gets_a_small_ruin() {
    for embark in [
        EmbarkSummary::temperate_forest(),
        EmbarkSummary::cold_hills(),
        EmbarkSummary::dry_plain(),
    ] {
        let map = generate_local_with_sites(&embark, &EmbarkSites::default());
        assert_eq!(map.placed.len(), 1);
        let p = map.placed[0];
        assert!(p.fallback && p.kind == SiteKind::Ruin && p.id == 0);
        assert!(count_tiles(&map, SiteTile::Rubble) + count_tiles(&map, SiteTile::Floor) > 0);
        assert!(!map.salvage.is_empty());
    }
}

/// R-041: a ruin of age 1,500 years has 2 times or more the rubble of a ruin of age 200.
#[test]
fn r041_old_ruin_has_twice_the_rubble_of_a_young_ruin() {
    let embark = EmbarkSummary::temperate_forest();
    let (mut young, mut old) = (0, 0);
    for id in 1..=40u32 {
        let a = generate_local_with_sites(&embark, &sites_of(vec![site(id, SiteKind::Ruin, 200)]));
        let b = generate_local_with_sites(&embark, &sites_of(vec![site(id, SiteKind::Ruin, 1500)]));
        assert_eq!(a.placed[0].width, b.placed[0].width);
        assert_eq!(a.placed[0].x, b.placed[0].x);
        let (ra, rb) = (
            count_tiles(&a, SiteTile::Rubble),
            count_tiles(&b, SiteTile::Rubble),
        );
        assert!(rb >= ra, "id {id}: older ruin has less rubble");
        young += ra;
        old += rb;
    }
    assert!(young > 0);
    assert!(old >= 2 * young, "old {old} young {young}");
}

#[test]
fn r041_old_ruin_has_less_salvage() {
    let embark = EmbarkSummary::temperate_forest();
    let young = generate_local_with_sites(&embark, &sites_of(vec![site(5, SiteKind::Ruin, 100)]));
    let old = generate_local_with_sites(&embark, &sites_of(vec![site(5, SiteKind::Ruin, 1800)]));
    assert!(old.salvage.len() < young.salvage.len());
    assert!(!old.salvage.is_empty());
}

/// A living settlement stays whole.
#[test]
fn s11_settlement_has_walls_floor_and_a_door_and_no_rubble() {
    let embark = EmbarkSummary::temperate_forest();
    let map = generate_local_with_sites(&embark, &sites_of(vec![site(3, SiteKind::Settlement, 0)]));
    assert!(count_tiles(&map, SiteTile::Wall) > 0);
    assert!(count_tiles(&map, SiteTile::Floor) > 0);
    assert_eq!(count_tiles(&map, SiteTile::Door), 1);
    assert_eq!(count_tiles(&map, SiteTile::Rubble), 0);
    assert!(map.salvage.is_empty());
}

/// Depletion decreases ore.
#[test]
fn s11_depletion_decreases_ore() {
    let embark = EmbarkSummary::cold_hills();
    let none = sites_of(vec![site(1, SiteKind::Ruin, 500)]);
    let mut half = none.clone();
    half.depletion = 128;
    let mut full = none.clone();
    full.depletion = 255;
    let a = count_ore(&generate_local_with_sites(&embark, &none));
    let b = count_ore(&generate_local_with_sites(&embark, &half));
    let c = count_ore(&generate_local_with_sites(&embark, &full));
    assert!(a > 100, "base map has little ore: {a}");
    assert!(b < a && b > 0);
    assert_eq!(c, 0);
}

#[test]
fn s11_mine_has_an_entrance_and_a_tunnel() {
    let embark = EmbarkSummary::cold_hills();
    let map = generate_local_with_sites(&embark, &sites_of(vec![site(2, SiteKind::Mine, 300)]));
    assert_eq!(count_tiles(&map, SiteTile::MineEntrance), 1);
    assert!(count_tiles(&map, SiteTile::Tunnel) > 0);
}

#[test]
fn s11_sealed_site_has_a_stone_ring() {
    let embark = EmbarkSummary::cold_hills();
    let map = generate_local_with_sites(&embark, &sites_of(vec![site(2, SiteKind::Sealed, 900)]));
    assert!(count_tiles(&map, SiteTile::Seal) >= 8);
    assert_eq!(count_tiles(&map, SiteTile::MineEntrance), 0);
}

#[test]
fn s11_old_battle_leaves_less_debris() {
    let embark = EmbarkSummary::temperate_forest();
    let mut young = 0;
    let mut old = 0;
    for id in 1..=20u32 {
        let a = generate_local_with_sites(
            &embark,
            &sites_of(vec![site(id, SiteKind::Battlefield, 10)]),
        );
        let b = generate_local_with_sites(
            &embark,
            &sites_of(vec![site(id, SiteKind::Battlefield, 3000)]),
        );
        young += count_tiles(&a, SiteTile::Debris);
        old += count_tiles(&b, SiteTile::Debris);
    }
    assert!(young > old && old > 0, "young {young} old {old}");
}

#[test]
fn s11_roads_join_the_exits_to_the_center() {
    let embark = EmbarkSummary::temperate_forest();
    let mut sites = sites_of(vec![site(4, SiteKind::Ruin, 400)]);
    sites.road_here = true;
    sites.road_near = true;
    sites.road_exits[worldgen::embark::EXIT_N] = Some(100);
    sites.road_exits[worldgen::embark::EXIT_E] = Some(60);
    let map = generate_local_with_sites(&embark, &sites);
    let road = |x: i32, y: i32| map.road.get(x, y).copied().unwrap_or(false);
    assert!(road(100, 0), "north exit");
    assert!(road(255, 60), "east exit");
    assert!(road(128, 128), "center");
    let tiles = map.road.as_slice().iter().filter(|&&r| r).count();
    assert!(tiles > 200, "road has {tiles} tiles");
}

#[test]
fn s11_stamping_is_deterministic_and_has_its_own_hash() {
    let embark = EmbarkSummary::dry_plain();
    let list = vec![
        site(9, SiteKind::Ruin, 700),
        site(10, SiteKind::Mine, 100),
        site(11, SiteKind::Battlefield, 50),
    ];
    let a = generate_local_with_sites(&embark, &sites_of(list.clone()));
    let b = generate_local_with_sites(&embark, &sites_of(list));
    assert_eq!(a, b);
    assert_eq!(a.site_hash(), b.site_hash());
    assert_ne!(a.site_hash(), generate_local(&embark).site_hash());
    assert_eq!(a.placed.len(), 3);
}

fn world_cells(seed: u64) -> (worldgen::WorldData, worldgen::History) {
    let w = generate_world(seed, &WorldParams::default());
    let h = generate_history(&w, None).expect("tables");
    (w, h)
}

#[test]
fn s11_embark_summary_comes_from_the_world() {
    let (w, h) = world_cells(1);
    let mut checked = 0;
    for y in (4..WORLD_SIZE as i32).step_by(23) {
        for x in (4..WORLD_SIZE as i32).step_by(23) {
            let got = embark_from_world(&w, &h, x, y);
            assert_eq!(got.is_some(), is_habitable(&w, x, y));
            let Some((s, sites)) = got else { continue };
            checked += 1;
            assert!(s.water_amount <= 1000 && s.mountain_amount <= 1000);
            assert!(s.ore_richness <= 3);
            assert_eq!(
                s.mean_temperature,
                i16::from(*w.temperature.get(x, y).expect("cell")) * 10
            );
            assert!(sites.sites.len() <= 8);
            let again = embark_from_world(&w, &h, x, y).expect("again");
            assert_eq!((s, sites), again);
        }
    }
    assert!(checked > 20);
    assert!(embark_from_world(&w, &h, -1, 0).is_none());
}

#[test]
fn s11_a_world_cell_makes_a_playable_local_map() {
    let (w, h) = world_cells(2);
    let mut done = 0;
    for y in (10..WORLD_SIZE as i32).step_by(37) {
        for x in (10..WORLD_SIZE as i32).step_by(37) {
            if done >= 12 {
                break;
            }
            if let Some((s, sites)) = embark_from_world(&w, &h, x, y) {
                let map = generate_local_with_sites(&s, &sites);
                assert!(!map.placed.is_empty(), "R-041 at {x},{y}");
                done += 1;
            }
        }
    }
    assert!(done > 5);
}

/// Rule 9: on 80% of seeds, at least 30% of the habitable cells have a site record or a road
/// in the embark area.
fn coverage_per_mille(seed: u64) -> u32 {
    let (w, h) = world_cells(seed);
    let (mut habitable, mut covered) = (0u32, 0u32);
    for y in 0..WORLD_SIZE as i32 {
        for x in 0..WORLD_SIZE as i32 {
            if let Some((_, sites)) = embark_from_world(&w, &h, x, y) {
                habitable += 1;
                covered += u32::from(sites.has_history());
            }
        }
    }
    covered * 1000 / habitable.max(1)
}

#[test]
fn rule_9_history_covers_the_habitable_cells() {
    let seeds: Vec<u64> = (1..=20).collect();
    let results: Vec<u32> = seeds.iter().map(|&s| coverage_per_mille(s)).collect();
    println!("coverage per mille: {results:?}");
    let passing = results.iter().filter(|&&c| c >= 300).count();
    assert!(
        passing * 10 >= seeds.len() * 8,
        "only {passing} seeds pass: {results:?}"
    );
}
