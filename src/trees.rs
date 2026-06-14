//! Trees scattered around the corridor.
//!
//! A simple trunk + canopy pair per tree. Placement avoids water and the
//! viaduct itself; everywhere else is fair game. A handful of conifers among
//! the broadleafs adds variety on the horizon.

use bevy::prelude::*;
use bevy::render::view::VisibilityRange;

use crate::route::CorridorMask;
use crate::water::WaterMask;

const TREE_COUNT: usize = 1_200;
const VIS_RANGE: f32 = 1100.0;
const BOUNDS_X: (f32, f32) = (-13_300.0, 4_900.0);
const BOUNDS_Z: (f32, f32) = (-30_800.0, 1_900.0);

pub struct TreesPlugin;

impl Plugin for TreesPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_trees);
    }
}

fn spawn_trees(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    water: Res<WaterMask>,
    corridor: Res<CorridorMask>,
) {
    let trunk_mesh = meshes.add(Cylinder {
        radius: 0.32,
        half_height: 1.2,
    });
    let broadleaf_mesh = meshes.add(Sphere::new(1.0).mesh().ico(2).unwrap());
    let conifer_mesh = meshes.add(Cone {
        radius: 1.0,
        height: 1.0,
    });

    let trunk_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.42, 0.31, 0.21),
        perceptual_roughness: 0.9,
        ..default()
    });
    let canopy_palette: Vec<Handle<StandardMaterial>> = [
        (0.31, 0.49, 0.29),
        (0.37, 0.55, 0.31),
        (0.42, 0.60, 0.33),
        (0.25, 0.42, 0.25),
        (0.47, 0.64, 0.36),
    ]
    .iter()
    .map(|&(r, g, b)| {
        materials.add(StandardMaterial {
            base_color: Color::srgb(r, g, b),
            perceptual_roughness: 0.9,
            ..default()
        })
    })
    .collect();
    let conifer_palette: Vec<Handle<StandardMaterial>> = [
        (0.22, 0.39, 0.25),
        (0.18, 0.34, 0.22),
        (0.26, 0.42, 0.27),
    ]
    .iter()
    .map(|&(r, g, b)| {
        materials.add(StandardMaterial {
            base_color: Color::srgb(r, g, b),
            perceptual_roughness: 0.9,
            ..default()
        })
    })
    .collect();

    let mut rng = fastrand::Rng::with_seed(0xF5C842);
    let mut placed = 0usize;
    let mut attempts = 0usize;

    while placed < TREE_COUNT && attempts < TREE_COUNT * 8 {
        attempts += 1;
        let x = rng.f32() * (BOUNDS_X.1 - BOUNDS_X.0) + BOUNDS_X.0;
        let z = rng.f32() * (BOUNDS_Z.1 - BOUNDS_Z.0) + BOUNDS_Z.0;

        if water.in_bay(x, z) || water.near_river(x, z, 4.0) {
            continue;
        }
        if corridor.near_track(x, z) {
            continue;
        }

        let s = 0.8 + rng.f32() * 0.7;
        let trunk_y = 1.2 * s;
        commands.spawn((
            PbrBundle {
                mesh: trunk_mesh.clone(),
                material: trunk_mat.clone(),
                transform: Transform::from_xyz(x, trunk_y, z)
                    .with_scale(Vec3::new(s, s, s)),
                ..default()
            },
            VisibilityRange::abrupt(0.0, VIS_RANGE),
        ));

        if rng.f32() < 0.18 {
            let r = 1.7 + rng.f32() * 1.1;
            let h = 5.0 + rng.f32() * 4.5;
            let mat = conifer_palette[rng.usize(..conifer_palette.len())].clone();
            commands.spawn((
                PbrBundle {
                    mesh: conifer_mesh.clone(),
                    material: mat,
                    transform: Transform::from_xyz(x, trunk_y * 2.0, z)
                        .with_scale(Vec3::new(r, h, r)),
                    ..default()
                },
                VisibilityRange::abrupt(0.0, VIS_RANGE),
            ));
        } else {
            let r = 2.2 + rng.f32() * 1.7;
            let mat = canopy_palette[rng.usize(..canopy_palette.len())].clone();
            commands.spawn((
                PbrBundle {
                    mesh: broadleaf_mesh.clone(),
                    material: mat,
                    transform: Transform::from_xyz(x, trunk_y * 2.0 + r * 0.6, z)
                        .with_scale(Vec3::new(r, r * (0.9 + rng.f32() * 0.3), r)),
                    ..default()
                },
                VisibilityRange::abrupt(0.0, VIS_RANGE),
            ));
        }

        placed += 1;
    }
}
