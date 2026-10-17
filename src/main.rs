mod voxel;
mod camera;
mod input;

use bevy::prelude::*;
use leafwing_input_manager::prelude::*;
use crate::camera::update_rts_camera;
use crate::input::CameraAction;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(InputManagerPlugin::<CameraAction>::default())
        .add_systems(Startup, voxel::spawn_voxel_map)
        .add_systems(Startup, camera::spawn_camera)
        .add_systems(Startup, spawn_test_light)
        .add_systems(Update, update_rts_camera)
        .run();
}

fn spawn_test_light(mut commands: Commands) {
    let chunk_center = Vec3::new(16.0, 15.0, 16.0);

    commands.spawn((
        DirectionalLight {
            illuminance: 800.0,
            ..default()
        },
        Transform::from_xyz(40.0, 60.0, 20.0).looking_at(chunk_center, Vec3::Y),
        ));
}