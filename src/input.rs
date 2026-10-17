use bevy::prelude::*;
use leafwing_input_manager::prelude::*;

#[derive(Actionlike, PartialEq, Eq, Clone, Copy, Hash, Debug, Reflect)]
pub enum PlayerAction {
    PanForward,
    PanBackward,
    PanLeft,
    PanRight,
    RotateLeft,
    RotateRight,
    PanDrag,
    OrbitDrag,
    Zoom,
    LayerUp,
    LayerDown,
    ToggleSlice,
}

pub fn default_camera_input_map() -> InputMap<PlayerAction> {
    let mut map = InputMap::default();

    // Keyboard panning
    map.insert(PlayerAction::PanForward, KeyCode::KeyW);
    map.insert(PlayerAction::PanForward, KeyCode::ArrowUp);
    map.insert(PlayerAction::PanBackward, KeyCode::KeyS);
    map.insert(PlayerAction::PanBackward, KeyCode::ArrowDown);
    map.insert(PlayerAction::PanLeft, KeyCode::KeyA);
    map.insert(PlayerAction::PanLeft, KeyCode::ArrowLeft);
    map.insert(PlayerAction::PanRight, KeyCode::KeyD);
    map.insert(PlayerAction::PanRight, KeyCode::ArrowRight);

    // Keyboard rotation
    map.insert(PlayerAction::RotateLeft, KeyCode::KeyQ);
    map.insert(PlayerAction::RotateRight, KeyCode::KeyE);

    // Mouse chords
    map.insert(PlayerAction::OrbitDrag, MouseButton::Right);
    map.insert(PlayerAction::PanDrag, MouseButton::Middle);

    // Layer Keys
    map.insert(PlayerAction::LayerUp, KeyCode::BracketRight);
    map.insert(PlayerAction::LayerDown, KeyCode::BracketLeft);
    map.insert(PlayerAction::ToggleSlice, KeyCode::KeyV);

    map
}

/*
Clearing a binding and setting new
input_map.clear_action(&CameraAction::PanForward);
input_map.insert(CameraAction::PanForward, new_key_code);
 */