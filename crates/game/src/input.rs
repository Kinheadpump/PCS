//! Input: maps keys and mouse to `Action` values through a binding table (TECHNICAL_DESIGN §4.4).
//! This module and `render` are the only places that read `macroquad` input.

use macroquad::prelude::{
    KeyCode, MouseButton, is_key_down, is_key_pressed, is_mouse_button_down, mouse_position,
    mouse_wheel,
};

/// A player intent, independent of the key that caused it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Action {
    /// Pan the camera up (held).
    PanUp,
    /// Pan the camera down (held).
    PanDown,
    /// Pan the camera left (held).
    PanLeft,
    /// Pan the camera right (held).
    PanRight,
    /// Drag the map by this many screen pixels.
    DragPan(f32, f32),
    /// Zoom in one step around the viewport center.
    ZoomIn,
    /// Zoom out one step around the viewport center.
    ZoomOut,
    /// Zoom by wheel steps (positive = in) around a screen position.
    WheelZoom {
        /// Wheel steps; positive zooms in.
        steps: f32,
        /// Cursor x in screen pixels.
        x: f32,
        /// Cursor y in screen pixels.
        y: f32,
    },
    /// Switch to embark preset number N (0-based).
    Preset(usize),
    /// Show or hide the tile grid.
    ToggleGrid,
    /// Close the game.
    Close,
}

/// When a key produces its action.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Trigger {
    /// Every frame while the key is down.
    Held(KeyCode),
    /// Once, on the frame the key goes down.
    Pressed(KeyCode),
}

/// The key binding table. Several keys can map to one action.
pub const KEY_BINDINGS: &[(Trigger, Action)] = &[
    (Trigger::Held(KeyCode::W), Action::PanUp),
    (Trigger::Held(KeyCode::Up), Action::PanUp),
    (Trigger::Held(KeyCode::S), Action::PanDown),
    (Trigger::Held(KeyCode::Down), Action::PanDown),
    (Trigger::Held(KeyCode::A), Action::PanLeft),
    (Trigger::Held(KeyCode::Left), Action::PanLeft),
    (Trigger::Held(KeyCode::D), Action::PanRight),
    (Trigger::Held(KeyCode::Right), Action::PanRight),
    (Trigger::Pressed(KeyCode::Equal), Action::ZoomIn),
    (Trigger::Pressed(KeyCode::KpAdd), Action::ZoomIn),
    (Trigger::Pressed(KeyCode::Minus), Action::ZoomOut),
    (Trigger::Pressed(KeyCode::KpSubtract), Action::ZoomOut),
    (Trigger::Pressed(KeyCode::Key1), Action::Preset(0)),
    (Trigger::Pressed(KeyCode::Key2), Action::Preset(1)),
    (Trigger::Pressed(KeyCode::Key3), Action::Preset(2)),
    (Trigger::Pressed(KeyCode::G), Action::ToggleGrid),
    (Trigger::Pressed(KeyCode::Escape), Action::Close),
];

/// Mouse buttons that drag the map.
pub const DRAG_BUTTONS: &[MouseButton] = &[MouseButton::Middle, MouseButton::Right];

/// Reads input each frame. Keeps the last mouse position to compute drags.
#[derive(Default)]
pub struct InputState {
    last_mouse: Option<(f32, f32)>,
}

impl InputState {
    /// Collect the actions of this frame.
    pub fn poll(&mut self) -> Vec<Action> {
        let mut actions = Vec::new();
        for (trigger, action) in KEY_BINDINGS {
            let fired = match trigger {
                Trigger::Held(key) => is_key_down(*key),
                Trigger::Pressed(key) => is_key_pressed(*key),
            };
            if fired {
                actions.push(*action);
            }
        }
        let mouse = mouse_position();
        let dragging = DRAG_BUTTONS.iter().any(|b| is_mouse_button_down(*b));
        if let (true, Some(last)) = (dragging, self.last_mouse) {
            let (dx, dy) = (mouse.0 - last.0, mouse.1 - last.1);
            if dx != 0.0 || dy != 0.0 {
                actions.push(Action::DragPan(dx, dy));
            }
        }
        self.last_mouse = Some(mouse);
        let wheel = mouse_wheel().1;
        if wheel != 0.0 {
            actions.push(Action::WheelZoom {
                steps: wheel.signum(),
                x: mouse.0,
                y: mouse.1,
            });
        }
        actions
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn binding_table_has_no_duplicate_triggers() {
        for (i, (a, _)) in KEY_BINDINGS.iter().enumerate() {
            for (b, _) in &KEY_BINDINGS[i + 1..] {
                assert_ne!(a, b);
            }
        }
    }

    #[test]
    fn binding_table_covers_all_required_actions() {
        let has = |want: Action| KEY_BINDINGS.iter().any(|(_, a)| *a == want);
        for a in [
            Action::PanUp,
            Action::PanDown,
            Action::PanLeft,
            Action::PanRight,
            Action::ZoomIn,
            Action::ZoomOut,
            Action::Preset(0),
            Action::Preset(1),
            Action::Preset(2),
            Action::Close,
        ] {
            assert!(has(a), "missing binding for {a:?}");
        }
    }
}
