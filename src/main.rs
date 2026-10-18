mod voxel;
mod camera;
mod input;
mod ui;
mod terrain_gen;
mod state;
mod trees;
mod calendar;
mod environmental;

use bevy::prelude::*;
use leafwing_input_manager::prelude::*;
use crate::camera::{update_current_level, update_rts_camera, CurrentLevel, SliceSettings, spawn_camera};
use crate::input::PlayerAction;
use crate::state::{AppState, GamePhase, SettingsState};
use crate::terrain_gen::ErosionSettings;
use crate::ui::{spawn_level_ui, update_level_ui};
use crate::voxel::{update_level_mesh_culling, WorldChunkStorage, spawn_voxel_map};

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(InputManagerPlugin::<PlayerAction>::default())

        // Register States
        .init_state::<AppState>()
        .add_sub_state::<GamePhase>()
        .init_state::<SettingsState>()

        // Register Resources
        .init_resource::<CurrentLevel>()
        .init_resource::<SliceSettings>()
        .init_resource::<WorldChunkStorage>()
        .init_resource::<ErosionSettings>()

        // Global Camera
        .add_systems(Startup, spawn_camera)

        // Main Menu
        .add_systems(OnEnter(AppState::MainMenu), spawn_main_menu_placeholder)
        .add_systems(
            Update,
            handle_menu_input.run_if(in_state(AppState::MainMenu)),
        )

        // In game
        .add_systems(
            OnEnter(AppState::InGame),
            (
                spawn_voxel_map,
                spawn_test_light,
                spawn_level_ui,
            ),
        )
        .add_systems(
            Update,
            (
            update_rts_camera,
            (
                update_current_level,
                update_level_ui,
                update_level_mesh_culling,
                )
                .chain(),
            )
        .run_if(in_state(GamePhase::Playing)),
        )

        // Toggle pause/settings while in game
        .add_systems(
            Update,
            toggle_pause_input.run_if(in_state(AppState::InGame)),
        )
        .run();
}

fn toggle_pause_input(
    keys: Res<ButtonInput<KeyCode>>,
    phase: Res<State<GamePhase>>,
    mut next_phase: ResMut<NextState<GamePhase>>,
    settings: Res<State<SettingsState>>,
    mut next_settings: ResMut<NextState<SettingsState>>,
) {
    if keys.just_pressed(KeyCode::Escape) {
        if *settings.get() == SettingsState::Open {
            next_settings.set(SettingsState::Closed);
            return;
        }

        match phase.get() {
            GamePhase::Playing => next_phase.set(GamePhase::Paused),
            GamePhase::Paused => next_phase.set(GamePhase::Playing),
        }
    }
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

fn spawn_main_menu_placeholder(mut commands: Commands) {
    commands.spawn((
        DespawnOnExit(AppState::MainMenu),
        Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        },
        children![(
            Text::new("Press ENTER to start game"),
            TextFont {
                font_size: FontSize::Px(32.0),
                ..default()
            },
        )],
    ));
}

fn handle_menu_input(
    keys: Res<ButtonInput<KeyCode>>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    if keys.just_pressed(KeyCode::Enter) {
        next_state.set(AppState::InGame);
    }
}