use bevy::input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll};
use bevy::prelude::*;
use crate::input::{default_camera_input_map, PlayerAction};
use leafwing_input_manager::prelude::*;

pub fn spawn_camera(mut commands: Commands) {
    let chunk_center = Vec3::new(16.0, 15.0, 16.0);

    commands.spawn((
        Camera3d::default(),
        Transform::default(),
        RtsCamera::default(),
        ActionState::<PlayerAction>::default(),
        default_camera_input_map(),
        AmbientLight {
            color: Color::srgb(0.8, 0.85, 1.0),
            brightness: 300.0,
            ..default()
        },
        ));
}

pub fn update_rts_camera(
    time: Res<Time>,
    mouse_motion: Res<AccumulatedMouseMotion>,
    mouse_scroll: Res<AccumulatedMouseScroll>,
    mut query: Query<(&ActionState<PlayerAction>, &mut RtsCamera, &mut Transform)>,
) {
    for (actions, mut cam, mut transform) in &mut query {
        let dt = time.delta_secs();

        if actions.pressed(&PlayerAction::OrbitDrag) {
            cam.yaw -= mouse_motion.delta.x * cam.orbit_sensitivity;
            cam.pitch += mouse_motion.delta.y * cam.orbit_sensitivity;
        }

        if actions.pressed(&PlayerAction::RotateLeft) {
            cam.yaw -= 1.5 * dt;
        }
        if actions.pressed(&PlayerAction::RotateRight) {
            cam.yaw += 1.5 * dt;
        }

        // Clamp pitch to avoid flipping
        cam.pitch = cam.pitch.clamp(0.2, std::f32::consts::FRAC_PI_2 - 0.05);

        // Zoom with mouse wheel
        if mouse_scroll.delta.y != 0.0 {
            cam.radius -= mouse_scroll.delta.y * cam.zoom_speed;
            cam.radius = cam.radius.clamp(10.0, 150.0);
        }

        // Panning
        let mut move_dir = Vec2::ZERO;

        if actions.pressed(&PlayerAction::PanForward) { move_dir.y += 1.0; }
        if actions.pressed(&PlayerAction::PanBackward) { move_dir.y -= 1.0; }
        if actions.pressed(&PlayerAction::PanLeft) { move_dir.x -= 1.0; }
        if actions.pressed(&PlayerAction::PanRight) { move_dir.x += 1.0; }

        if actions.pressed(&PlayerAction::PanDrag) {
            move_dir.x -= mouse_motion.delta.x * 0.1;
            move_dir.y += mouse_motion.delta.y * 0.1;
        }

        if move_dir != Vec2::ZERO {
            move_dir = move_dir.normalize();

            // project onto ground plan
            let forward = Vec3::new(-cam.yaw.sin(), 0.0, -cam.yaw.cos()).normalize();
            let right = Vec3::new(cam.yaw.cos(), 0.0, -cam.yaw.sin()).normalize();

            let delta_pan = (forward * move_dir.y + right * move_dir.x) * cam.pan_speed * dt;
            cam.target += delta_pan;
        }

        let x = cam.radius * cam.pitch.cos() * cam.yaw.sin();
        let y = cam.radius * cam.pitch.sin();
        let z = cam.radius * cam.pitch.cos() * cam.yaw.cos();

        transform.translation = cam.target + Vec3::new(x, y, z);
        transform.look_at(cam.target, Vec3::Y);
    }
}

pub fn update_current_level(
    mut level: ResMut<CurrentLevel>,
    action_query: Query<&ActionState<PlayerAction>>,
) {
    for action in action_query.iter() {
        if action.just_pressed(&PlayerAction::LayerUp) {
            level.0 += 1;
        }

        if action.just_pressed(&PlayerAction::LayerDown) {
            level.0 -= 1;
        }
    }
}

#[derive(Resource)]
pub struct CurrentLevel(pub i32);

impl Default for CurrentLevel {
    fn default() -> Self {
        Self(0)
    }
}

#[derive(Component)]
pub struct RtsCamera {
    pub target: Vec3,
    // Distance from target
    pub radius: f32,
    // Horizontal rotation in radians
    pub yaw: f32,
    // Vertical tilt in radians
    pub pitch: f32,
    pub pan_speed: f32,
    pub zoom_speed: f32,
    pub orbit_sensitivity: f32,
}

impl Default for RtsCamera {
    fn default() -> Self {
        Self {
            target: Vec3::new(16.0, 15.0, 16.0),
            radius: 50.0,
            yaw: std::f32::consts::FRAC_PI_4, // 45 degrees
            pitch: std::f32::consts::FRAC_PI_3, // 60 degree downward
            pan_speed: 30.0,
            zoom_speed: 5.0,
            orbit_sensitivity: 0.003,
        }
    }
}