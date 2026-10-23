//! Sprint 14 tests (worldgen part): legends (R-044) and the sealed site records.

#![allow(clippy::expect_used)]

use worldgen::history::SiteKind;
use worldgen::{
    EmbarkSite, EmbarkSites, EmbarkSummary, LegendKind, Reliability, SiteTile,
    generate_local_with_sites, make_legends,
};

fn site(id: u32, kind: SiteKind) -> EmbarkSite {
    EmbarkSite {
        id,
        kind,
        age: 300,
        culture: 0,
        mineral: 1,
        level: 3,
    }
}

fn map_with(seed: u64, kinds: &[SiteKind]) -> worldgen::LocalMapData {
    let list: Vec<EmbarkSite> = kinds
        .iter()
        .enumerate()
        .map(|(i, &k)| site(i as u32 + 1, k))
        .collect();
    let sites = EmbarkSites {
        site_count: list.len() as u32,
        sites: list,
        ..EmbarkSites::default()
    };
    let embark = EmbarkSummary::temperate_forest().with_seed(seed);
    generate_local_with_sites(&embark, &sites)
}

const KINDS: [SiteKind; 4] = [
    SiteKind::Ruin,
    SiteKind::Mine,
    SiteKind::Battlefield,
    SiteKind::Settlement,
];

#[test]
fn legends_are_repeatable() {
    let map = map_with(5, &KINDS);
    assert_eq!(make_legends(&map), make_legends(&map));
}

#[test]
fn r044_a_certain_legend_always_points_to_a_real_site() {
    for seed in 1..=40 {
        let map = map_with(seed, &KINDS);
        for l in make_legends(&map) {
            if l.reliability == Reliability::Certain {
                let i = l.site.expect("certain legend has a site");
                let p = map.placed[i];
                assert_eq!((l.x, l.y), (p.x, p.y), "seed {seed}");
            }
        }
    }
}

#[test]
fn r044_about_half_of_the_rumors_are_false_and_a_false_rumor_has_no_site() {
    let (mut rumors, mut false_rumors) = (0, 0);
    for seed in 1..=120 {
        let map = map_with(seed, &KINDS);
        for l in make_legends(&map) {
            if l.reliability != Reliability::Rumor {
                continue;
            }
            rumors += 1;
            if l.site.is_none() {
                false_rumors += 1;
                for p in &map.placed {
                    let d = (i32::from(l.x) - i32::from(p.x))
                        .abs()
                        .max((i32::from(l.y) - i32::from(p.y)).abs());
                    assert!(d >= 8, "false rumor too near a site, seed {seed}");
                }
            }
        }
    }
    assert!(rumors > 100, "{rumors} rumors");
    let share = false_rumors * 100 / rumors;
    assert!((35..=65).contains(&share), "false share {share}%");
}

#[test]
fn a_sealed_site_has_a_certain_warning_legend_on_its_site() {
    for seed in 1..=10 {
        let map = map_with(seed, &[SiteKind::Sealed, SiteKind::Ruin]);
        let warnings: Vec<_> = make_legends(&map)
            .into_iter()
            .filter(|l| l.kind == LegendKind::Warning)
            .collect();
        assert_eq!(warnings.len(), 1, "seed {seed}");
        let w = warnings[0];
        assert_eq!(w.reliability, Reliability::Certain);
        let p = map.placed[w.site.expect("warning has a site")];
        assert_eq!(p.kind, SiteKind::Sealed);
        assert_eq!(p.level, 3);
        assert!(map.site.as_slice().contains(&SiteTile::Seal));
    }
}

#[test]
fn a_map_without_a_sealed_site_has_no_warning() {
    let map = map_with(3, &KINDS);
    assert!(
        make_legends(&map)
            .iter()
            .all(|l| l.kind == LegendKind::Lead)
    );
}
