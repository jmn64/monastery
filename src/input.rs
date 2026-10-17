use bevy::prelude::*;
use leafwing_input_manager::prelude::*;

#[derive(Actionlike, PartialEq, Eq, Clone, Copy, Hash, Debug, Reflect)]
pub enum CameraAction {
    PanForward,
    PanBackward,
    PanLeft,
    PanRight,
    RotateLeft,
    RotateRight,
    PanDrag,
    OrbitDrag,
    Zoom,
}

pub fn default_camera_input_map() -> InputMap<CameraAction> {
    let mut map = InputMap::default();

    // Keyboard panning
    map.insert(CameraAction::PanForward, KeyCode::KeyW);
    map.insert(CameraAction::PanForward, KeyCode::ArrowUp);
    map.insert(CameraAction::PanBackward, KeyCode::KeyS);
    map.insert(CameraAction::PanBackward, KeyCode::ArrowDown);
    map.insert(CameraAction::PanLeft, KeyCode::KeyA);
    map.insert(CameraAction::PanLeft, KeyCode::ArrowLeft);
    map.insert(CameraAction::PanRight, KeyCode::KeyD);
    map.insert(CameraAction::PanRight, KeyCode::ArrowRight);

    // Keyboard rotation
    map.insert(CameraAction::RotateLeft, KeyCode::KeyQ);
    map.insert(CameraAction::RotateRight, KeyCode::KeyE);

    // Mouse chords
    map.insert(CameraAction::OrbitDrag, MouseButton::Right);
    map.insert(CameraAction::PanDrag, MouseButton::Middle);

    map
}

/*
Clearing a binding and setting new
input_map.clear_action(&CameraAction::PanForward);
input_map.insert(CameraAction::PanForward, new_key_code);
 */