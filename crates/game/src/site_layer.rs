//! The stamped sites and roads of a local map (Sprint 11): walls, floors, rubble, tunnels, seals,
//! debris, and old roads. They are drawn as flat colored tiles over the baked ground. The sim
//! does not use these layers yet.

use crate::camera::{Camera, View};
use crate::overlay::screen_rect;
use crate::palette::Rgb;
use crate::render::color as rgb_color;
use macroquad::prelude::draw_rectangle;
use worldgen::{LocalMapData, SiteTile};

/// Color and alpha of a site tile, or `None` for no structure.
pub fn tile_color(tile: SiteTile) -> Option<(Rgb, u8)> {
    match tile {
        SiteTile::None => None,
        SiteTile::Wall => Some(([104, 96, 112], 255)),
        SiteTile::Floor => Some(([150, 132, 108], 200)),
        SiteTile::Door => Some(([140, 92, 52], 255)),
        SiteTile::Rubble => Some(([122, 114, 104], 230)),
        SiteTile::Tunnel => Some(([30, 24, 40], 230)),
        SiteTile::Seal => Some(([78, 74, 98], 255)),
        SiteTile::MineEntrance => Some(([210, 156, 72], 255)),
        SiteTile::Debris => Some(([150, 74, 62], 220)),
    }
}

/// Color and alpha of an old road.
pub const ROAD: (Rgb, u8) = ([176, 156, 116], 150);

/// The color of a tile of the layers: a structure first, then a road.
pub fn layer_color(map: &LocalMapData, x: i32, y: i32) -> Option<(Rgb, u8)> {
    let structure = map.site.get(x, y).copied().and_then(tile_color);
    if structure.is_some() {
        return structure;
    }
    map.road.get(x, y).copied().unwrap_or(false).then_some(ROAD)
}

/// Draw the visible tiles of the layers.
pub fn draw(map: &LocalMapData, camera: &Camera, view: View) {
    let size = (map.ground.width(), map.ground.height());
    let Some(visible) = camera.visible_tiles(view, size) else {
        return;
    };
    for y in visible.min_y..=visible.max_y {
        for x in visible.min_x..=visible.max_x {
            if let Some((color, alpha)) = layer_color(map, x, y) {
                let (sx, sy, w, h) = screen_rect(camera, view, x, y, 1, 1);
                draw_rectangle(sx, sy, w, h, rgb_color(color, alpha));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use worldgen::history::SiteKind;
    use worldgen::{EmbarkSite, EmbarkSites, EmbarkSummary, generate_local_with_sites};

    #[test]
    fn every_structure_has_a_color_and_none_has_none() {
        assert_eq!(tile_color(SiteTile::None), None);
        let all = [
            SiteTile::Wall,
            SiteTile::Floor,
            SiteTile::Door,
            SiteTile::Rubble,
            SiteTile::Tunnel,
            SiteTile::Seal,
            SiteTile::MineEntrance,
            SiteTile::Debris,
        ];
        let colors: std::collections::BTreeSet<Rgb> = all
            .iter()
            .map(|&t| tile_color(t).expect("color").0)
            .collect();
        assert_eq!(colors.len(), all.len());
    }

    #[test]
    fn a_stamped_map_has_visible_site_tiles_and_a_road_shows_under_a_structure() {
        let sites = EmbarkSites {
            sites: vec![EmbarkSite {
                id: 3,
                kind: SiteKind::Settlement,
                age: 0,
                culture: 0,
                mineral: 1,
                level: 2,
            }],
            site_count: 1,
            road_here: true,
            road_near: true,
            road_exits: [Some(100), None, Some(60), None],
            ..EmbarkSites::default()
        };
        let map = generate_local_with_sites(&EmbarkSummary::temperate_forest(), &sites);
        let mut structures = 0;
        let mut roads = 0;
        for y in 0..map.ground.height() as i32 {
            for x in 0..map.ground.width() as i32 {
                match layer_color(&map, x, y) {
                    Some((_, a)) if a == ROAD.1 => roads += 1,
                    Some(_) => structures += 1,
                    None => {}
                }
                if map.site.get(x, y).copied().and_then(tile_color).is_some() {
                    assert_eq!(
                        layer_color(&map, x, y),
                        map.site.get(x, y).copied().and_then(tile_color)
                    );
                }
            }
        }
        assert!(
            structures > 0 && roads > 100,
            "structures {structures} roads {roads}"
        );
    }
}
