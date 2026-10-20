//! Site stamping (S11, TECHNICAL_DESIGN §8.2): the history of the embark area becomes
//! structure on the local map. Buildings, rubble, old tunnels, battle debris, seals, and roads.
//! Every number comes from `embark.ron`. All rolls are hashes of the map seed and the site ID,
//! so the result does not depend on the order of other sites.

use crate::embark::{EXIT_N, EXIT_S, EXIT_W, EmbarkSite, EmbarkSites};
use crate::history::SiteKind;
use crate::local::{Ground, LocalMapData, Ore, RockKind};
use base::content::EmbarkRules;
use base::rng::mix;

const TAG_SITE: u64 = 0x5354_414d_505f_5349;
const TAG_DEPLETE: u64 = 0x5354_414d_505f_4450;
const TAG_RUBBLE: u64 = 0x5354_414d_505f_5242;
const TAG_TREE: u64 = 0x5354_414d_505f_5452;
const TAG_SALVAGE: u64 = 0x5354_414d_505f_534c;
const TAG_ROAD: u64 = 0x5354_414d_505f_5244;

/// Structure layer of a local map tile.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum SiteTile {
    /// No structure.
    None = 0,
    /// Wall of a building.
    Wall = 1,
    /// Floor of a building.
    Floor = 2,
    /// Door of a building.
    Door = 3,
    /// Rubble: a wall or a floor that decayed.
    Rubble = 4,
    /// Tunnel of an old mine.
    Tunnel = 5,
    /// Seal: stone that closes a sealed site.
    Seal = 6,
    /// Entrance of a mine.
    MineEntrance = 7,
    /// Debris of a battle.
    Debris = 8,
}

/// A site that was placed on the local map.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlacedSite {
    /// Site ID in the history (0 for the ruin that R-041 adds).
    pub id: u32,
    /// Kind.
    pub kind: SiteKind,
    /// Center x.
    pub x: u16,
    /// Center y.
    pub y: u16,
    /// Width of the building (0 for a site without building).
    pub width: u8,
    /// Height of the building (0 for a site without building).
    pub height: u8,
    /// Age in years.
    pub age: u32,
    /// True for the ruin that R-041 adds because the history gave no site.
    pub fallback: bool,
}

fn roll(seed: u64, a: u64, b: u64) -> u64 {
    mix(seed ^ mix((a << 32) ^ b))
}

/// Value from `lo` to `hi`, both included.
fn between(r: u64, lo: u32, hi: u32) -> i32 {
    (lo + (r % u64::from(hi - lo + 1)) as u32) as i32
}

/// Remove nature from a tile: water, rock, ore, and tree. Structure goes on open ground.
fn clear(map: &mut LocalMapData, x: i32, y: i32) {
    if matches!(map.ground.get(x, y), Some(Ground::Water)) {
        map.ground.set(x, y, Ground::Soil);
    }
    map.rock.set(x, y, RockKind::None);
    map.ore.set(x, y, Ore::None);
    map.tree.set(x, y, false);
}

fn put(map: &mut LocalMapData, x: i32, y: i32, tile: SiteTile) {
    map.site.set(x, y, tile);
}

/// Stamp the history of the embark area on a local map.
pub(crate) fn stamp(map: &mut LocalMapData, sites: &EmbarkSites, rules: &EmbarkRules) {
    deplete(map, sites.depletion);
    draw_roads(map, sites, rules);
    let mut list: Vec<EmbarkSite> = sites.sites.clone();
    let mut fallback = false;
    if list.is_empty() {
        // R-041: each local map has at least 1 site.
        list.push(EmbarkSite {
            id: 0,
            kind: SiteKind::Ruin,
            age: rules.fallback_age,
            culture: 0,
            mineral: 0,
            level: 0,
        });
        fallback = true;
    }
    for s in &list {
        place(map, s, fallback, rules);
    }
}

/// Mined-out ground has less ore. Each ore tile is lost with the chance depletion / 255.
fn deplete(map: &mut LocalMapData, depletion: u8) {
    if depletion == 0 {
        return;
    }
    let chance = u64::from(depletion) * 1000 / 255;
    let seed = mix(map.seed ^ TAG_DEPLETE);
    let (w, h) = (map.ore.width() as i32, map.ore.height() as i32);
    for y in 0..h {
        for x in 0..w {
            let has_ore = map.ore.get(x, y).is_some_and(|&o| o != Ore::None);
            if has_ore && roll(seed, x as u64, y as u64) % 1000 < chance {
                map.ore.set(x, y, Ore::None);
            }
        }
    }
}

/// Draw a run of road tiles on a line.
fn road_line(map: &mut LocalMapData, from: (i32, i32), to: (i32, i32)) {
    let (mut x, mut y) = from;
    loop {
        clear(map, x, y);
        map.road.set(x, y, true);
        if (x, y) == to {
            break;
        }
        x += (to.0 - x).signum();
        y += (to.1 - y).signum();
    }
}

fn draw_roads(map: &mut LocalMapData, sites: &EmbarkSites, rules: &EmbarkRules) {
    let (w, h) = (map.ground.width() as i32, map.ground.height() as i32);
    let (cx, cy) = (w / 2, h / 2);
    let seed = mix(map.seed ^ TAG_ROAD);
    if sites.road_here {
        clear(map, cx, cy);
        map.road.set(cx, cy, true);
    }
    let jitter = rules.road_jitter;
    for (i, exit) in sites.road_exits.iter().enumerate() {
        let Some(offset) = *exit else { continue };
        let offset = i32::from(offset);
        let shift = between(roll(seed, i as u64, 0), 0, 2 * jitter) - jitter as i32;
        let vertical = i == EXIT_N || i == EXIT_S;
        let (edge, along_max, center_along, center_cross) = if vertical {
            (if i == EXIT_N { 0 } else { h - 1 }, w - 1, cx, cy)
        } else {
            (if i == EXIT_W { 0 } else { w - 1 }, h - 1, cy, cx)
        };
        let offset = offset.clamp(0, along_max);
        let bend = (edge + center_cross) / 2 + shift;
        let bend = bend.clamp(0, if vertical { h - 1 } else { w - 1 });
        // Points of the path: edge, bend line, center.
        let points: [(i32, i32); 4] = if vertical {
            [
                (offset, edge),
                (offset, bend),
                (center_along, bend),
                (cx, cy),
            ]
        } else {
            [
                (edge, offset),
                (bend, offset),
                (bend, center_along),
                (cx, cy),
            ]
        };
        for pair in points.windows(2) {
            road_line(map, pair[0], pair[1]);
        }
    }
}

/// Place one site on the map.
fn place(map: &mut LocalMapData, s: &EmbarkSite, fallback: bool, rules: &EmbarkRules) {
    let (w, h) = (map.ground.width() as i32, map.ground.height() as i32);
    let sseed = mix(map.seed ^ TAG_SITE ^ u64::from(s.id));
    let margin = rules.site_margin as i32;
    let span_x = ((w - 2 * margin).max(1)) as u64;
    let span_y = ((h - 2 * margin).max(1)) as u64;
    let cx = (margin + (roll(sseed, 1, 0) % span_x) as i32).min(w - 1);
    let cy = (margin + (roll(sseed, 2, 0) % span_y) as i32).min(h - 1);
    let mut placed = PlacedSite {
        id: s.id,
        kind: s.kind,
        x: cx as u16,
        y: cy as u16,
        width: 0,
        height: 0,
        age: s.age,
        fallback,
    };
    match s.kind {
        SiteKind::Settlement | SiteKind::Ruin => {
            let (bw, bh) = building(map, s, (cx, cy), sseed, rules);
            placed.width = bw as u8;
            placed.height = bh as u8;
        }
        SiteKind::Mine => mine(map, (cx, cy), sseed, rules, false),
        SiteKind::Sealed => mine(map, (cx, cy), sseed, rules, true),
        SiteKind::Battlefield => battlefield(map, s, (cx, cy), sseed, rules),
    }
    map.placed.push(placed);
}

/// Stamp a building with walls, floor, and a door. Old buildings decay into rubble.
/// Returns the width and the height.
fn building(
    map: &mut LocalMapData,
    s: &EmbarkSite,
    center: (i32, i32),
    sseed: u64,
    rules: &EmbarkRules,
) -> (i32, i32) {
    let (w, h) = (map.ground.width() as i32, map.ground.height() as i32);
    let bw = between(roll(sseed, 3, 0), rules.ruin_w_min, rules.ruin_w_max).min(w);
    let bh = between(roll(sseed, 4, 0), rules.ruin_h_min, rules.ruin_h_max).min(h);
    let x0 = (center.0 - bw / 2).clamp(0, w - bw);
    let y0 = (center.1 - bh / 2).clamp(0, h - bh);
    let age = u64::from(s.age);
    let wall_chance = age * 1000 / (age + u64::from(rules.decay_k));
    let floor_chance = wall_chance / u64::from(rules.floor_rubble_div);
    let door = (x0 + bw / 2, y0 + bh - 1);
    let ruin = matches!(s.kind, SiteKind::Ruin);
    let rubble_seed = mix(sseed ^ TAG_RUBBLE);
    let tree_seed = mix(sseed ^ TAG_TREE);
    let mut open: Vec<(i32, i32)> = Vec::new();
    for y in y0..y0 + bh {
        for x in x0..x0 + bw {
            let edge = x == x0 || y == y0 || x == x0 + bw - 1 || y == y0 + bh - 1;
            clear(map, x, y);
            map.road.set(x, y, false);
            let r = roll(rubble_seed, x as u64, y as u64) % 1000;
            let tile = if (x, y) == door {
                SiteTile::Door
            } else if edge {
                if ruin && r < wall_chance {
                    SiteTile::Rubble
                } else {
                    SiteTile::Wall
                }
            } else if ruin && r < floor_chance {
                SiteTile::Rubble
            } else {
                SiteTile::Floor
            };
            put(map, x, y, tile);
            if matches!(tile, SiteTile::Floor | SiteTile::Rubble) {
                open.push((x, y));
                let overgrown = ruin
                    && s.age >= rules.overgrow_age
                    && roll(tree_seed, x as u64, y as u64) % 1000 < u64::from(rules.overgrow_pm);
                if overgrown {
                    map.tree.set(x, y, true);
                }
            }
        }
    }
    if ruin && !open.is_empty() {
        let n = rules
            .salvage_base
            .saturating_sub(s.age / rules.salvage_age_div)
            .max(1);
        let salvage_seed = mix(sseed ^ TAG_SALVAGE);
        for k in 0..n {
            let (x, y) = open[(roll(salvage_seed, u64::from(k), 0) % open.len() as u64) as usize];
            let spot = (x as u16, y as u16);
            if !map.salvage.contains(&spot) {
                map.salvage.push(spot);
            }
        }
    }
    (bw, bh)
}

/// Nearest tile with a rock wall, searched in rings of growing size. Row-major inside a ring.
fn nearest_rock(map: &LocalMapData, c: (i32, i32)) -> Option<(i32, i32)> {
    let (w, h) = (map.ground.width() as i32, map.ground.height() as i32);
    for r in 0..w.max(h) {
        for y in (c.1 - r).max(0)..=(c.1 + r).min(h - 1) {
            for x in (c.0 - r).max(0)..=(c.0 + r).min(w - 1) {
                let on_ring = (x - c.0).abs().max((y - c.1).abs()) == r;
                let rocky = map.rock.get(x, y).is_some_and(|&k| k != RockKind::None);
                if on_ring && rocky {
                    return Some((x, y));
                }
            }
        }
    }
    None
}

/// Stamp the entrance and the old tunnel of a mine. A sealed site gets a stone ring.
fn mine(map: &mut LocalMapData, c: (i32, i32), sseed: u64, rules: &EmbarkRules, sealed: bool) {
    let (w, h) = (map.ground.width() as i32, map.ground.height() as i32);
    let (ex, ey) = nearest_rock(map, c).unwrap_or(c);
    let dirs = [(0, -1), (-1, 0), (1, 0), (0, 1)];
    let (dx, dy) = dirs[(roll(sseed, 5, 0) % 4) as usize];
    let ring = rules.seal_radius as i32;
    let length = if sealed {
        (ring - 1).max(0)
    } else {
        rules.tunnel_len as i32
    };
    clear(map, ex, ey);
    put(
        map,
        ex,
        ey,
        if sealed {
            SiteTile::Seal
        } else {
            SiteTile::MineEntrance
        },
    );
    for i in 1..=length {
        let (x, y) = (ex + dx * i, ey + dy * i);
        if !(0..w).contains(&x) || !(0..h).contains(&y) {
            break;
        }
        clear(map, x, y);
        if matches!(
            map.ground.get(x, y),
            Some(Ground::Soil | Ground::Clay | Ground::Sand)
        ) {
            map.ground.set(x, y, Ground::Stone);
        }
        put(map, x, y, SiteTile::Tunnel);
    }
    if sealed {
        for y in (ey - ring).max(0)..=(ey + ring).min(h - 1) {
            for x in (ex - ring).max(0)..=(ex + ring).min(w - 1) {
                if (x - ex).abs().max((y - ey).abs()) == ring {
                    clear(map, x, y);
                    map.ground.set(x, y, Ground::Stone);
                    map.rock.set(x, y, RockKind::Granite);
                    put(map, x, y, SiteTile::Seal);
                }
            }
        }
    }
}

/// Scatter battle debris. Old battles leave less debris.
fn battlefield(
    map: &mut LocalMapData,
    s: &EmbarkSite,
    c: (i32, i32),
    sseed: u64,
    rules: &EmbarkRules,
) {
    let (w, h) = (map.ground.width() as i32, map.ground.height() as i32);
    let k = u64::from(rules.debris_decay_k);
    let count = (u64::from(rules.debris_count) * k / (u64::from(s.age) + k)).max(1);
    let r = rules.debris_radius;
    let span = 2 * r;
    for n in 0..count {
        let x = c.0 + between(roll(sseed, 6, n), 0, span) - r as i32;
        let y = c.1 + between(roll(sseed, 7, n), 0, span) - r as i32;
        let open = map.ground.get(x, y).is_some_and(|&g| g != Ground::Water)
            && map.rock.get(x, y).is_some_and(|&k| k == RockKind::None);
        if (0..w).contains(&x) && (0..h).contains(&y) && open {
            put(map, x, y, SiteTile::Debris);
        }
    }
}
