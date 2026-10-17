mod voxel;
mod camera;
mod input;
mod ui;
mod terrain_gen;

use bevy::prelude::*;
use leafwing_input_manager::prelude::*;
use crate::camera::{update_current_level, update_rts_camera, CurrentLevel};
use crate::input::PlayerAction;
use crate::ui::{spawn_level_ui, update_level_ui};

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(InputManagerPlugin::<PlayerAction>::default())
        .insert_resource(CurrentLevel::default())
        .add_systems(Startup, voxel::spawn_voxel_map)
        .add_systems(Startup, camera::spawn_camera)
        .add_systems(Startup, spawn_test_light)
        .add_systems(Startup, spawn_level_ui)
        .add_systems(Update, update_rts_camera)
        .add_systems(Update, update_current_level)
        .add_systems(Update, update_level_ui.after(update_current_level))
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