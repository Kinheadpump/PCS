//! Legends (S14, R-044): one line leads to a site on the local map. A "certain" legend always
//! points to a real site. A "rumor" points to a real site with a chance from `embark.ron`. A
//! false rumor points to a tile with no site. A sealed site always has a certain warning
//! legend. All rolls are hashes of the map seed and the site ID. The result does not depend on
//! other sites.

use crate::embark::embark_rules;
use crate::history::SiteKind;
use crate::local::{Ground, LocalMapData, RockKind};
use base::rng::mix;

const TAG_LEGEND: u64 = 0x4c45_4745_4e44_5f31;

/// How much the player can trust a legend (R-044). The interface shows it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reliability {
    /// Points to a real site.
    Certain,
    /// Points to a real site or to nothing.
    Rumor,
}

/// What a legend says.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LegendKind {
    /// A lead to a reward.
    Lead,
    /// A warning about a sealed site with a dormant threat.
    Warning,
}

/// One legend with its site marker.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Legend {
    /// Lead or warning.
    pub kind: LegendKind,
    /// How much to trust it.
    pub reliability: Reliability,
    /// Marker tile column.
    pub x: u16,
    /// Marker tile row.
    pub y: u16,
    /// Index in `LocalMapData::placed` of the real site. `None` for a false rumor. This is
    /// the truth: the interface must not show it.
    pub site: Option<usize>,
}

fn roll(seed: u64, site: u32, n: u64) -> u64 {
    mix(seed ^ TAG_LEGEND ^ mix((u64::from(site) << 32) ^ n))
}

/// Make the legends of a local map: first the warnings, then one lead for each other site, in
/// placement order. Same map gives the same legends.
pub fn make_legends(map: &LocalMapData) -> Vec<Legend> {
    let Ok(rules) = embark_rules() else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for (i, p) in map.placed.iter().enumerate() {
        if p.kind == SiteKind::Sealed {
            out.push(Legend {
                kind: LegendKind::Warning,
                reliability: Reliability::Certain,
                x: p.x,
                y: p.y,
                site: Some(i),
            });
        }
    }
    for (i, p) in map.placed.iter().enumerate() {
        if p.kind == SiteKind::Sealed {
            continue;
        }
        let certain = roll(map.seed, p.id, 1) % 100 < u64::from(rules.legend_certain_pct);
        let real = roll(map.seed, p.id, 2) % 100 < u64::from(rules.legend_rumor_true_pct);
        let lead = |reliability, x, y, site| Legend {
            kind: LegendKind::Lead,
            reliability,
            x,
            y,
            site,
        };
        if certain {
            out.push(lead(Reliability::Certain, p.x, p.y, Some(i)));
        } else if real {
            out.push(lead(Reliability::Rumor, p.x, p.y, Some(i)));
        } else if let Some((x, y)) = empty_tile(map, p.id, rules.legend_min_gap) {
            out.push(lead(Reliability::Rumor, x, y, None));
        }
    }
    out
}

/// An open tile at least `gap` tiles from every placed site. `None` if 64 tries find none.
fn empty_tile(map: &LocalMapData, id: u32, gap: u32) -> Option<(u16, u16)> {
    let (w, h) = (map.ground.width(), map.ground.height());
    for n in 0..64u64 {
        let x = (roll(map.seed, id, 10 + 2 * n) % u64::from(w)) as i32;
        let y = (roll(map.seed, id, 11 + 2 * n) % u64::from(h)) as i32;
        let open = map.ground.get(x, y).is_some_and(|&g| g != Ground::Water)
            && map.rock.get(x, y).is_some_and(|&r| r == RockKind::None);
        let far = map.placed.iter().all(|p| {
            let d = (x - i32::from(p.x)).abs().max((y - i32::from(p.y)).abs());
            d >= gap as i32
        });
        if open && far {
            return Some((x as u16, y as u16));
        }
    }
    None
}
