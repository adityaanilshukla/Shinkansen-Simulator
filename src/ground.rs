//! Flat ground plane under everything.
//!
//! TODO(ground-color): an earlier procedural-noise texture pass kept reading
//! as blue regardless of the ambient/sun setup. For now we draw the ground
//! as a plain warm-tan plane; revisit when we know what's actually tinting
//! it. Leads to investigate: linear-vs-sRGB conversion of the AmbientLight
//! `color` field, the sky dome's emissive contribution at low view angles,
//! and the lack of any toneMapping override on this material.

use bevy::prelude::*;

pub struct GroundPlugin;

impl Plugin for GroundPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_ground);
    }
}

fn spawn_ground(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.spawn(PbrBundle {
        mesh: meshes.add(Plane3d::default().mesh().size(40_000.0, 60_000.0)),
        material: materials.add(StandardMaterial {
            base_color: Color::srgb(0.66, 0.58, 0.42),
            perceptual_roughness: 0.95,
            metallic: 0.0,
            ..default()
        }),
        transform: Transform::from_xyz(-4200.0, 0.0, -14_500.0),
        ..default()
    });
}
