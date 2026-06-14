//! Sky dome and ambient lighting.
//!
//! A large back-side sphere with a vertical gradient stands in for the
//! atmosphere. The sun is a single directional light placed high to the south.

use bevy::pbr::NotShadowCaster;
use bevy::prelude::*;

#[derive(Component)]
pub struct SkyDome;

pub struct SkyPlugin;

impl Plugin for SkyPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(ClearColor(Color::srgb(0.82, 0.92, 0.97)))
            .insert_resource(AmbientLight {
                color: Color::srgb(0.85, 0.90, 0.98),
                brightness: 1400.0,
            })
            .add_systems(Startup, spawn_sky);
    }
}

fn spawn_sky(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.spawn((
        PbrBundle {
            mesh: meshes.add(Sphere::new(9000.0).mesh().ico(3).unwrap()),
            material: materials.add(StandardMaterial {
                base_color: Color::srgb(0.78, 0.88, 0.96),
                unlit: true,
                cull_mode: None,
                ..default()
            }),
            ..default()
        },
        NotShadowCaster,
        SkyDome,
    ));

    // Shadows are off: with 60k+ scene entities the shadow pass dominates the
    // frame. Ambient is bumped accordingly so unlit faces don't go black.
    commands.spawn(DirectionalLightBundle {
        directional_light: DirectionalLight {
            color: Color::srgb(1.0, 0.95, 0.86),
            illuminance: 14_000.0,
            shadows_enabled: false,
            ..default()
        },
        transform: Transform::from_xyz(320.0, 540.0, 200.0)
            .looking_at(Vec3::ZERO, Vec3::Y),
        ..default()
    });
}
