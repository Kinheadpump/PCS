//! Input: maps keys and mouse to `Action` values through a binding table (TECHNICAL_DESIGN §4.4).
//! This module and `render` are the only places that read `macroquad` input.

use crate::tools::Tool;
use macroquad::prelude::{
    KeyCode, MouseButton, is_key_down, is_key_pressed, is_mouse_button_down,
    is_mouse_button_pressed, is_mouse_button_released, mouse_position, mouse_wheel,
};
use sim::Speed;

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
    /// Show or hide the debug lines (tick, hash, FPS, map).
    ToggleDebug,
    /// Pause, or return to the speed before the pause.
    TogglePause,
    /// Set the game speed.
    SetSpeed(Speed),
    /// Left click at a screen position (select).
    LeftClick(f32, f32),
    /// Left button released at a screen position (end of a drag).
    LeftRelease(f32, f32),
    /// Right click at a screen position (debug move order, or cancel a drag).
    RightClick(f32, f32),
    /// Choose a map tool.
    SelectTool(Tool),
    /// Show or hide the work panel.
    ToggleWork,
    /// Show or hide the rooms overlay.
    ToggleRooms,
    /// Show or hide the temperature overlay.
    ToggleTemp,
    /// Choose the building of the build tool by its number (0-based, `buildings.ron` order).
    PickBuilding(usize),
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
    (Trigger::Pressed(KeyCode::F1), Action::Preset(0)),
    (Trigger::Pressed(KeyCode::F2), Action::Preset(1)),
    (Trigger::Pressed(KeyCode::F3), Action::Preset(2)),
    (Trigger::Pressed(KeyCode::G), Action::ToggleGrid),
    (Trigger::Pressed(KeyCode::F4), Action::ToggleDebug),
    (Trigger::Pressed(KeyCode::Space), Action::TogglePause),
    (Trigger::Pressed(KeyCode::Q), Action::SetSpeed(Speed::X1)),
    (Trigger::Pressed(KeyCode::E), Action::SetSpeed(Speed::X3)),
    (Trigger::Pressed(KeyCode::R), Action::SetSpeed(Speed::X6)),
    (
        Trigger::Pressed(KeyCode::V),
        Action::SelectTool(Tool::Select),
    ),
    (Trigger::Pressed(KeyCode::T), Action::SelectTool(Tool::Cut)),
    (Trigger::Pressed(KeyCode::M), Action::SelectTool(Tool::Mine)),
    (Trigger::Pressed(KeyCode::H), Action::SelectTool(Tool::Haul)),
    (
        Trigger::Pressed(KeyCode::Z),
        Action::SelectTool(Tool::Stockpile),
    ),
    (
        Trigger::Pressed(KeyCode::C),
        Action::SelectTool(Tool::Cancel),
    ),
    (
        Trigger::Pressed(KeyCode::B),
        Action::SelectTool(Tool::Build),
    ),
    (
        Trigger::Pressed(KeyCode::X),
        Action::SelectTool(Tool::Deconstruct),
    ),
    (Trigger::Pressed(KeyCode::O), Action::ToggleRooms),
    (Trigger::Pressed(KeyCode::Y), Action::ToggleTemp),
    (Trigger::Pressed(KeyCode::Key1), Action::PickBuilding(0)),
    (Trigger::Pressed(KeyCode::Key2), Action::PickBuilding(1)),
    (Trigger::Pressed(KeyCode::Key3), Action::PickBuilding(2)),
    (Trigger::Pressed(KeyCode::Key4), Action::PickBuilding(3)),
    (Trigger::Pressed(KeyCode::Key5), Action::PickBuilding(4)),
    (Trigger::Pressed(KeyCode::Key6), Action::PickBuilding(5)),
    (Trigger::Pressed(KeyCode::Key7), Action::PickBuilding(6)),
    (Trigger::Pressed(KeyCode::P), Action::ToggleWork),
    (Trigger::Pressed(KeyCode::Escape), Action::Close),
];

/// Mouse buttons that drag the map. The right button is not here: it gives move orders.
pub const DRAG_BUTTONS: &[MouseButton] = &[MouseButton::Middle];

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
        if is_mouse_button_pressed(MouseButton::Left) {
            actions.push(Action::LeftClick(mouse.0, mouse.1));
        }
        if is_mouse_button_released(MouseButton::Left) {
            actions.push(Action::LeftRelease(mouse.0, mouse.1));
        }
        if is_mouse_button_pressed(MouseButton::Right) {
            actions.push(Action::RightClick(mouse.0, mouse.1));
        }
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
            Action::ToggleDebug,
            Action::TogglePause,
            Action::SetSpeed(Speed::X1),
            Action::SetSpeed(Speed::X3),
            Action::SetSpeed(Speed::X6),
            Action::Close,
            Action::ToggleWork,
            Action::SelectTool(Tool::Select),
            Action::SelectTool(Tool::Cut),
            Action::SelectTool(Tool::Mine),
            Action::SelectTool(Tool::Haul),
            Action::SelectTool(Tool::Stockpile),
            Action::SelectTool(Tool::Cancel),
            Action::SelectTool(Tool::Build),
            Action::SelectTool(Tool::Deconstruct),
            Action::ToggleRooms,
            Action::PickBuilding(0),
            Action::PickBuilding(6),
        ] {
            assert!(has(a), "missing binding for {a:?}");
        }
    }

    #[test]
    fn preset_keys_are_function_keys_and_speed_keys_are_q_e_r() {
        let key_of = |want: Action| {
            KEY_BINDINGS
                .iter()
                .find(|(_, a)| *a == want)
                .map(|(t, _)| *t)
        };
        assert_eq!(
            key_of(Action::Preset(0)),
            Some(Trigger::Pressed(KeyCode::F1))
        );
        assert_eq!(
            key_of(Action::Preset(2)),
            Some(Trigger::Pressed(KeyCode::F3))
        );
        assert_eq!(
            key_of(Action::SetSpeed(Speed::X6)),
            Some(Trigger::Pressed(KeyCode::R))
        );
        assert_eq!(
            key_of(Action::TogglePause),
            Some(Trigger::Pressed(KeyCode::Space))
        );
    }

    fn key_letter(code: KeyCode) -> Option<char> {
        match code {
            KeyCode::V => Some('V'),
            KeyCode::T => Some('T'),
            KeyCode::M => Some('M'),
            KeyCode::H => Some('H'),
            KeyCode::Z => Some('Z'),
            KeyCode::C => Some('C'),
            KeyCode::B => Some('B'),
            KeyCode::X => Some('X'),
            _ => None,
        }
    }

    #[test]
    fn tool_keys_match_the_labels_in_the_toolbar() {
        for tool in Tool::ALL {
            let binding = KEY_BINDINGS
                .iter()
                .find(|(_, a)| *a == Action::SelectTool(tool))
                .map(|(t, _)| *t);
            let Some(Trigger::Pressed(code)) = binding else {
                panic!("no key for {tool:?}");
            };
            assert_eq!(key_letter(code), Some(tool.key_label()));
        }
    }

    #[test]
    fn tool_keys_do_not_collide_with_pan_or_speed_keys() {
        // WASD pans and Q/E/R set the speed, so no tool uses these keys.
        for (trigger, action) in KEY_BINDINGS {
            if matches!(
                action,
                Action::SelectTool(_) | Action::ToggleWork | Action::ToggleRooms
            ) {
                let (Trigger::Pressed(k) | Trigger::Held(k)) = trigger;
                for pan in [
                    KeyCode::W,
                    KeyCode::A,
                    KeyCode::S,
                    KeyCode::D,
                    KeyCode::Q,
                    KeyCode::E,
                    KeyCode::R,
                    KeyCode::G,
                ] {
                    assert_ne!(*k, pan);
                }
            }
        }
    }

    #[test]
    fn right_button_does_not_drag() {
        assert!(!DRAG_BUTTONS.contains(&MouseButton::Right));
    }
}
