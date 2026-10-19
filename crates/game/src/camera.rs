//! Pan and zoom camera. Pure math: no `macroquad` calls (floats are allowed in `game`).

/// Smallest zoom, in screen pixels per tile.
pub const MIN_ZOOM: f32 = 2.0;
/// Largest zoom, in screen pixels per tile.
pub const MAX_ZOOM: f32 = 64.0;

/// Size of the viewport in screen pixels: (width, height).
pub type View = (f32, f32);

/// A 2D camera over a tile map.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Camera {
    /// Map position (in tiles) shown at the center of the viewport.
    pub center: (f32, f32),
    /// Screen pixels per tile.
    pub zoom: f32,
}

/// Inclusive range of tile coordinates.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TileRect {
    /// First column.
    pub min_x: i32,
    /// First row.
    pub min_y: i32,
    /// Last column.
    pub max_x: i32,
    /// Last row.
    pub max_y: i32,
}

/// Clamp a zoom value to the allowed range.
pub fn clamp_zoom(zoom: f32) -> f32 {
    zoom.clamp(MIN_ZOOM, MAX_ZOOM)
}

impl Camera {
    /// A camera that shows the middle of a map of `map_size` tiles.
    pub fn new(map_size: (u32, u32), zoom: f32) -> Self {
        Self {
            center: (map_size.0 as f32 / 2.0, map_size.1 as f32 / 2.0),
            zoom: clamp_zoom(zoom),
        }
    }

    /// Convert a map position (tiles) to a screen position (pixels).
    pub fn tile_to_screen(&self, tile: (f32, f32), view: View) -> (f32, f32) {
        (
            (tile.0 - self.center.0) * self.zoom + view.0 / 2.0,
            (tile.1 - self.center.1) * self.zoom + view.1 / 2.0,
        )
    }

    /// Convert a screen position (pixels) to a map position (tiles).
    pub fn screen_to_tile(&self, screen: (f32, f32), view: View) -> (f32, f32) {
        (
            (screen.0 - view.0 / 2.0) / self.zoom + self.center.0,
            (screen.1 - view.1 / 2.0) / self.zoom + self.center.1,
        )
    }

    /// The tile under a screen position, or `None` outside the map.
    pub fn tile_at(
        &self,
        screen: (f32, f32),
        view: View,
        map_size: (u32, u32),
    ) -> Option<(i32, i32)> {
        let (tx, ty) = self.screen_to_tile(screen, view);
        if tx < 0.0 || ty < 0.0 || tx >= map_size.0 as f32 || ty >= map_size.1 as f32 {
            return None;
        }
        Some((tx.floor() as i32, ty.floor() as i32))
    }

    /// Move the camera by a number of tiles.
    pub fn pan_tiles(&mut self, dx: f32, dy: f32, map_size: (u32, u32)) {
        self.center.0 += dx;
        self.center.1 += dy;
        self.clamp_center(map_size);
    }

    /// Drag the map by a number of screen pixels (the map follows the cursor).
    pub fn pan_pixels(&mut self, dx: f32, dy: f32, map_size: (u32, u32)) {
        self.pan_tiles(-dx / self.zoom, -dy / self.zoom, map_size);
    }

    /// Multiply the zoom by `factor`. The map position under `anchor` (screen pixels) stays there.
    pub fn zoom_at(&mut self, factor: f32, anchor: (f32, f32), view: View, map_size: (u32, u32)) {
        let before = self.screen_to_tile(anchor, view);
        self.zoom = clamp_zoom(self.zoom * factor);
        let after = self.screen_to_tile(anchor, view);
        self.center.0 += before.0 - after.0;
        self.center.1 += before.1 - after.1;
        self.clamp_center(map_size);
    }

    /// Keep the center inside the map.
    pub fn clamp_center(&mut self, map_size: (u32, u32)) {
        self.center.0 = self.center.0.clamp(0.0, map_size.0 as f32);
        self.center.1 = self.center.1.clamp(0.0, map_size.1 as f32);
    }

    /// The tiles that the viewport touches, clamped to the map. `None` if no tile is visible.
    pub fn visible_tiles(&self, view: View, map_size: (u32, u32)) -> Option<TileRect> {
        if map_size.0 == 0 || map_size.1 == 0 {
            return None;
        }
        let (x0, y0) = self.screen_to_tile((0.0, 0.0), view);
        let (x1, y1) = self.screen_to_tile(view, view);
        let max_x = map_size.0 as i32 - 1;
        let max_y = map_size.1 as i32 - 1;
        let rect = TileRect {
            min_x: (x0.floor() as i32).max(0),
            min_y: (y0.floor() as i32).max(0),
            max_x: (x1.floor() as i32).min(max_x),
            max_y: (y1.floor() as i32).min(max_y),
        };
        (rect.min_x <= rect.max_x && rect.min_y <= rect.max_y).then_some(rect)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const VIEW: View = (1280.0, 720.0);
    const MAP: (u32, u32) = (256, 256);

    #[test]
    fn screen_and_tile_round_trip() {
        let mut cam = Camera::new(MAP, 16.0);
        cam.center = (100.5, 70.25);
        let tile = cam.screen_to_tile((300.0, 200.0), VIEW);
        let back = cam.tile_to_screen(tile, VIEW);
        assert!((back.0 - 300.0).abs() < 1e-3 && (back.1 - 200.0).abs() < 1e-3);
    }

    #[test]
    fn viewport_center_shows_camera_center() {
        let cam = Camera::new(MAP, 8.0);
        let t = cam.screen_to_tile((VIEW.0 / 2.0, VIEW.1 / 2.0), VIEW);
        assert_eq!(t, (128.0, 128.0));
    }

    #[test]
    fn zoom_is_clamped() {
        assert_eq!(clamp_zoom(0.1), MIN_ZOOM);
        assert_eq!(clamp_zoom(1000.0), MAX_ZOOM);
        let mut cam = Camera::new(MAP, 8.0);
        cam.zoom_at(1000.0, (0.0, 0.0), VIEW, MAP);
        assert_eq!(cam.zoom, MAX_ZOOM);
        cam.zoom_at(0.0001, (0.0, 0.0), VIEW, MAP);
        assert_eq!(cam.zoom, MIN_ZOOM);
    }

    #[test]
    fn zoom_keeps_anchor_tile_fixed() {
        let mut cam = Camera::new(MAP, 8.0);
        let anchor = (900.0, 150.0);
        let before = cam.screen_to_tile(anchor, VIEW);
        cam.zoom_at(2.0, anchor, VIEW, MAP);
        let after = cam.screen_to_tile(anchor, VIEW);
        assert!((before.0 - after.0).abs() < 1e-3 && (before.1 - after.1).abs() < 1e-3);
    }

    #[test]
    fn pan_is_clamped_to_map() {
        let mut cam = Camera::new(MAP, 8.0);
        cam.pan_tiles(10_000.0, -10_000.0, MAP);
        assert_eq!(cam.center, (256.0, 0.0));
    }

    #[test]
    fn drag_moves_map_with_cursor() {
        let mut cam = Camera::new(MAP, 10.0);
        cam.pan_pixels(50.0, 0.0, MAP);
        assert_eq!(cam.center, (123.0, 128.0));
    }

    #[test]
    fn visible_tiles_are_clamped_to_map() {
        let cam = Camera::new(MAP, MIN_ZOOM);
        let r = cam.visible_tiles(VIEW, MAP);
        assert_eq!(
            r,
            Some(TileRect {
                min_x: 0,
                min_y: 0,
                max_x: 255,
                max_y: 255
            })
        );
    }

    #[test]
    fn visible_tiles_at_high_zoom_are_few() {
        let cam = Camera::new(MAP, 64.0);
        let r = cam.visible_tiles(VIEW, MAP);
        assert_eq!(
            r,
            Some(TileRect {
                min_x: 118,
                min_y: 122,
                max_x: 138,
                max_y: 133
            })
        );
    }

    #[test]
    fn tile_at_rejects_outside_map() {
        let mut cam = Camera::new(MAP, 8.0);
        cam.center = (0.0, 0.0);
        assert_eq!(cam.tile_at((0.0, 0.0), VIEW, MAP), None);
        assert_eq!(
            cam.tile_at((VIEW.0 / 2.0 + 1.0, VIEW.1 / 2.0 + 1.0), VIEW, MAP),
            Some((0, 0))
        );
    }
}
