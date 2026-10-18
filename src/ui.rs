use bevy::prelude::*;
use crate::camera::{CurrentLevel, SliceSettings};
use crate::state::AppState;

#[derive(Component)]
pub struct LevelText;

pub fn spawn_level_ui(mut commands: Commands) {
    commands
        .spawn((
            DespawnOnExit(AppState::InGame),
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(16.0),
                left: Val::Px(16.0),
                padding: UiRect::axes(Val::Px(12.0), Val::Px(8.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.1, 0.1, 0.1, 0.85)),
            BorderColor::all(Color::srgba(0.3, 0.3, 0.3, 1.0)),
        ))
        .with_children(|parent| {
           parent.spawn((
               Text::new("Level: 0"),
               TextFont {
                   font_size: FontSize::Px(18.0),
                   ..default()
               },
               TextColor(Color::srgb(0.9, 0.9, 0.9)),
               LevelText,
               ));
        });
}

pub fn update_level_ui(
    level: Res<CurrentLevel>,
    slice: Res<SliceSettings>,
    mut query: Query<&mut Text, With<LevelText>>,
) {
    if !level.is_changed() && !slice.is_changed() {
        return;
    }

    for mut text in &mut query {
        let status = if slice.enabled { "ON" } else { "OFF" };
        **text = format!("Level: {} [Slice: {} (V)]", level.0, status);
    }
}