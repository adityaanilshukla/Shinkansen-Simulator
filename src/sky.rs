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
            // Dim and slightly warm ambient. Lower brightness lets the warm
            // directional sun dominate the look so surfaces no longer read
            // as blue-tinged.
            .insert_resource(AmbientLight {
                color: Color::srgb(0.98, 0.94, 0.86),
                brightness: 500.0,
            })
            .add_systems(Startup, spawn_sky)
            .add_systems(Update, follow_camera);
    }
}

/// The sky dome is a finite-radius sphere; if the camera leaves it the player
/// sees the dome as an actual ball in the distance. Snap its centre to the
/// camera each frame so it always wraps the view.
fn follow_camera(
    cam: Query<&Transform, (With<Camera3d>, Without<SkyDome>)>,
    mut sky: Query<&mut Transform, With<SkyDome>>,
) {
    let Ok(cam_tf) = cam.get_single() else {
        return;
    };
    let Ok(mut sky_tf) = sky.get_single_mut() else {
        return;
    };
    sky_tf.translation.x = cam_tf.translation.x;
    sky_tf.translation.z = cam_tf.translation.z;
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
