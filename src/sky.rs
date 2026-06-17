//! Sky colour and lighting.
//!
//! No geometry: the camera's `ClearColor` plus the linear fog in `camera.rs`
//! give us the sky look. The sun is a single directional light placed high
//! to the south; ambient is warmed to keep shadowed faces from reading blue.

use bevy::prelude::*;

pub struct SkyPlugin;

impl Plugin for SkyPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(ClearColor(Color::srgb(0.82, 0.92, 0.97)))
            // Dim and slightly warm ambient. Lower brightness lets the warm
            // directional sun dominate the look so surfaces no longer read
            // as blue-tinged.
            .insert_resource(AmbientLight {
                color: Color::srgb(0.98, 0.94, 0.86),
                brightness: 500.0,
            })
            .add_systems(Startup, spawn_sun);
    }
}

fn spawn_sun(mut commands: Commands) {
    // Shadows are off: with 60k+ scene entities the shadow pass dominates the
    // frame. Ambient is bumped accordingly so unlit faces don't go black.
    commands.spawn(DirectionalLightBundle {
        directional_light: DirectionalLight {
            color: Color::srgb(1.0, 0.95, 0.86),
            illuminance: 14_000.0,
            shadows_enabled: false,
            ..default()
        },
        transform: Transform::from_xyz(320.0, 540.0, 200.0).looking_at(Vec3::ZERO, Vec3::Y),
        ..default()
    });
}
