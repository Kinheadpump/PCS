//! Sprint 14 tests (sim part): Notice (R-030), its effects (R-031), and dormant threats.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use sim::scenario::sim_from_ascii;
use sim::time::TICKS_PER_DAY;
use sim::{Command, JobKind, NoticeCause, NoticeEvent, NoticeSource, Sim, save};
use worldgen::RockKind;

fn quiet(text: &str) -> Sim {
    let mut sim = sim_from_ascii(text, 1).unwrap();
    sim.debug_set_needs_enabled(false);
    sim
}

fn run(sim: &mut Sim, ticks: u64) {
    for _ in 0..ticks {
        sim.step(&[]);
    }
}

fn run_days(sim: &mut Sim, days: u64) {
    run(sim, days * TICKS_PER_DAY);
}

fn part(sim: &Sim, cause: NoticeCause) -> u32 {
    sim.notice()
        .parts
        .iter()
        .filter(|p| p.cause == cause)
        .map(|p| p.value)
        .sum()
}

#[test]
fn r030_notice_is_the_sum_of_its_named_parts() {
    let mut sim = quiet("cc....\n......");
    let granite = sim.rules().rock_material(RockKind::Granite);
    sim.debug_spawn_item(granite, 600, 2, 0);
    sim.debug_spawn_item(granite, 900, 3, 1);
    sim.debug_add_noise(4, 0, 3);
    sim.debug_add_disturbance(5, 1, 7);
    let report = sim.notice();
    let sum: u32 = report.parts.iter().map(|p| p.value).sum();
    assert_eq!(report.total, sum);
    let value = u32::from(sim.rules().materials[usize::from(granite)].value);
    let wealth = 1500 * value / sim.rules().notice.wealth_div;
    assert_eq!(part(&sim, NoticeCause::Wealth), wealth);
    assert_eq!(
        part(&sim, NoticeCause::Population),
        2 * sim.rules().notice.per_colonist
    );
    assert_eq!(part(&sim, NoticeCause::Noise), 3);
    assert_eq!(part(&sim, NoticeCause::Disturbance), 7);
}

#[test]
fn r030_each_part_has_a_location_inside_the_map() {
    let mut sim = quiet("cc....\n......");
    sim.debug_add_noise(4, 0, 3);
    sim.debug_add_disturbance(5, 1, 7);
    for p in sim.notice().parts {
        assert!(sim.map().in_bounds(i32::from(p.x), i32::from(p.y)), "{p:?}");
    }
    let noise = sim
        .notice()
        .parts
        .into_iter()
        .find(|p| p.cause == NoticeCause::Noise)
        .unwrap();
    assert_eq!((noise.x, noise.y), (4, 0));
}

#[test]
fn r030_wealth_counts_a_finished_building() {
    let mut sim = quiet("cc....");
    let wall = sim.rules().building_index("wall").unwrap();
    let granite = sim.rules().material_index("granite").unwrap();
    assert_eq!(part(&sim, NoticeCause::Wealth), 0);
    assert!(sim.debug_build(wall, 3, 0, granite));
    let def = &sim.rules().buildings[usize::from(wall)];
    let value = u32::from(sim.rules().materials[usize::from(granite)].value);
    let expected = def.cost * value / sim.rules().notice.wealth_div;
    assert_eq!(part(&sim, NoticeCause::Wealth), expected);
}

#[test]
fn r030_a_completed_mining_job_adds_noise_at_its_tile() {
    let mut sim = quiet("cR..");
    assert_eq!(part(&sim, NoticeCause::Noise), 0);
    sim.step(&[Command::Designate {
        kind: JobKind::Mine,
        x: 1,
        y: 0,
    }]);
    run(&mut sim, 1_500);
    assert_eq!(sim.map().rock(1, 0), Some(RockKind::None));
    let noise: Vec<_> = sim
        .notice()
        .parts
        .into_iter()
        .filter(|p| p.cause == NoticeCause::Noise)
        .collect();
    assert_eq!(noise.len(), 1);
    assert_eq!((noise[0].x, noise[0].y), (1, 0));
    assert_eq!(noise[0].value, sim.rules().notice.noise_per_job);
}

#[test]
fn r030_noise_loses_10_percent_each_day_rounded_up() {
    let mut sim = quiet("cc....");
    sim.debug_add_noise(2, 0, 15);
    sim.debug_add_noise(3, 0, 5);
    sim.debug_add_noise(4, 0, 1);
    run_days(&mut sim, 1);
    // 15 - 2 = 13, 5 - 1 = 4, 1 - 1 = 0.
    assert_eq!(part(&sim, NoticeCause::Noise), 13 + 4);
    assert_eq!(
        sim.notice()
            .parts
            .iter()
            .filter(|p| p.cause == NoticeCause::Noise)
            .count(),
        2,
        "a source at 0 is removed"
    );
}

#[test]
fn r030_site_disturbance_loses_1_each_day_until_0() {
    let mut sim = quiet("cc....");
    sim.debug_add_disturbance(3, 0, 3);
    run_days(&mut sim, 2);
    assert_eq!(part(&sim, NoticeCause::Disturbance), 1);
    run_days(&mut sim, 2);
    assert_eq!(part(&sim, NoticeCause::Disturbance), 0);
}

#[test]
fn r030_excavation_and_relic_add_the_values_from_the_content() {
    let mut sim = quiet("cc....");
    let n = sim.rules().notice;
    sim.notice_disturb(2, 0, NoticeSource::Excavation);
    sim.notice_disturb(3, 0, NoticeSource::Relic);
    assert_eq!(part(&sim, NoticeCause::Disturbance), n.excavation + n.relic);
}

fn last_roll(events: &[NoticeEvent]) -> Option<(u32, bool)> {
    events.iter().rev().find_map(|e| match *e {
        NoticeEvent::BeastRoll {
            chance_pct,
            arrived,
            ..
        } => Some((chance_pct, arrived)),
        _ => None,
    })
}

#[test]
fn r031_beast_chance_is_notice_over_4_percent() {
    let mut sim = quiet("cccccccc");
    // 8 colonists give 16 Notice: 16 / 4 = 4 percent.
    run_days(&mut sim, 1);
    let (chance, _) = last_roll(&sim.drain_notice_events()).unwrap();
    assert_eq!(chance, 4);
}

#[test]
fn r031_beast_chance_is_at_most_50_percent() {
    let mut sim = quiet("cc....");
    sim.debug_add_disturbance(3, 0, 100_000);
    run_days(&mut sim, 1);
    let (chance, _) = last_roll(&sim.drain_notice_events()).unwrap();
    assert_eq!(chance, 50);
}

#[test]
fn r031_about_half_the_days_bring_a_beast_at_50_percent() {
    let mut sim = quiet("cc....");
    sim.debug_add_disturbance(3, 0, 100_000);
    let mut arrived = 0;
    let days = 60;
    for _ in 0..days {
        run_days(&mut sim, 1);
        if last_roll(&sim.drain_notice_events()).is_some_and(|(_, a)| a) {
            arrived += 1;
        }
    }
    assert!(
        (15..=45).contains(&arrived),
        "{arrived} beasts in {days} days"
    );
}

#[test]
fn r031_no_beast_with_chance_0() {
    let mut sim = sim_from_ascii("......", 1).unwrap();
    sim.debug_set_needs_enabled(false);
    assert_eq!(sim.notice().total, 0);
    run_days(&mut sim, 20);
    let events = sim.drain_notice_events();
    assert!(!events.is_empty(), "a roll happens each day");
    for e in events {
        if let NoticeEvent::BeastRoll {
            chance_pct,
            arrived,
            ..
        } = e
        {
            assert_eq!(chance_pct, 0);
            assert!(!arrived);
        }
    }
}

#[test]
fn r031_lair_beasts_wake_at_30_notice() {
    let mut sim = quiet("cc....");
    assert!(!sim.lairs_active());
    sim.debug_add_disturbance(3, 0, 25);
    assert!(!sim.lairs_active(), "4 + 25 = 29");
    sim.debug_add_disturbance(4, 0, 1);
    assert!(sim.lairs_active(), "4 + 26 = 30");
}

#[test]
fn r031_mining_a_seal_tile_wakes_the_threat_and_adds_disturbance() {
    let mut sim = quiet("c..R\nR...");
    sim.debug_add_threat(7, 3, 0, 1, &[(0, 1)]);
    assert_eq!(sim.threats().len(), 1);
    assert!(!sim.threats()[0].awake);
    let before = part(&sim, NoticeCause::Disturbance);
    sim.step(&[Command::Designate {
        kind: JobKind::Mine,
        x: 3,
        y: 0,
    }]);
    run(&mut sim, 1_500);
    assert!(!sim.threats()[0].awake, "another rock does not wake it");
    sim.step(&[Command::Designate {
        kind: JobKind::Mine,
        x: 0,
        y: 1,
    }]);
    run(&mut sim, 1_500);
    assert!(sim.threats()[0].awake);
    assert!(part(&sim, NoticeCause::Disturbance) > before);
    let woke: Vec<_> = sim
        .drain_notice_events()
        .into_iter()
        .filter(|e| matches!(e, NoticeEvent::ThreatWoke { site: 7, .. }))
        .collect();
    assert_eq!(woke.len(), 1);
}

#[test]
fn r031_an_awake_threat_gains_1_strength_for_each_25_notice() {
    let mut sim = quiet("cRc.");
    sim.debug_add_threat(7, 3, 1, 0, &[(1, 0)]);
    assert_eq!(sim.threat_strength(0), None, "a dormant threat has none");
    sim.debug_wake_threat(0);
    // 4 for the colonists and 50 for the opening: 54 Notice give 2 levels.
    let notice = sim.notice().total;
    assert_eq!(notice, 4 + sim.rules().notice.sealed_open);
    assert_eq!(sim.threat_strength(0), Some(3 + notice / 25));
    sim.debug_add_disturbance(2, 0, 25);
    assert_eq!(sim.threat_strength(0), Some(3 + (notice + 25) / 25));
}

#[test]
fn r031_a_threat_wakes_once() {
    let mut sim = quiet("cR..");
    sim.debug_add_threat(7, 3, 1, 0, &[(1, 0)]);
    sim.debug_wake_threat(0);
    let first = sim.notice().total;
    sim.debug_wake_threat(0);
    assert_eq!(sim.notice().total, first);
}

#[test]
fn notice_state_survives_save_and_load() {
    let mut a = quiet("cR..");
    a.debug_add_threat(7, 3, 1, 0, &[(1, 0)]);
    a.debug_add_noise(2, 0, 9);
    a.debug_add_disturbance(3, 0, 11);
    run_days(&mut a, 1);
    let mut b = save::from_bytes(&save::to_bytes(&a)).unwrap();
    b.debug_set_needs_enabled(false);
    assert_eq!(b.notice(), a.notice());
    assert_eq!(b.threats(), a.threats());
    run_days(&mut a, 2);
    run_days(&mut b, 2);
    assert_eq!(a.state_hash(), b.state_hash());
}

#[test]
fn r031_a_new_colony_has_one_dormant_threat_for_each_sealed_site() {
    use worldgen::history::SiteKind;
    use worldgen::{EmbarkSite, EmbarkSites, EmbarkSummary, SiteTile, generate_local_with_sites};
    let site = |id, kind| EmbarkSite {
        id,
        kind,
        age: 300,
        culture: 0,
        mineral: 1,
        level: 2,
    };
    let sites = EmbarkSites {
        site_count: 2,
        sites: vec![site(1, SiteKind::Sealed), site(2, SiteKind::Ruin)],
        ..EmbarkSites::default()
    };
    let data = generate_local_with_sites(&EmbarkSummary::temperate_forest().with_seed(4), &sites);
    let sim = Sim::new(&data, 1, 0);
    assert_eq!(sim.threats().len(), 1);
    let t = &sim.threats()[0];
    assert_eq!((t.site, t.level, t.awake), (1, 2, false));
    let seals = data
        .site
        .as_slice()
        .iter()
        .filter(|&&s| s == SiteTile::Seal)
        .count();
    assert!(seals > 0);
    assert_eq!(t.seals.len(), seals);
}

#[test]
fn builtin_notice_content_is_valid_and_rejects_bad_numbers() {
    let text = include_str!("../../../assets/content/notice.ron");
    let ok = base::content::load_notice_rules("notice.ron", text).unwrap();
    assert_eq!(ok, sim::SimRules::default().notice);
    let bad = text.replace("beast_div: 4", "beast_div: 0");
    assert!(base::content::load_notice_rules("notice.ron", &bad).is_err());
    let bad = text.replace("beast_max_pct: 50", "beast_max_pct: 101");
    assert!(base::content::load_notice_rules("notice.ron", &bad).is_err());
}
