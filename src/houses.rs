//! Single-family houses carpeting the corridor between the dense city cores.
//!
//! Each house is a wall box plus a hip or gable roof. Placement is on a coarse
//! grid with jitter, skipping anything we'd build over (water, viaduct, roads,
//! landmark keep-outs) and the cluster cores where the city already has tall
//! buildings.

use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};
use bevy::render::render_asset::RenderAssetUsages;
use bevy::render::view::VisibilityRange;

use crate::clusters::Clusters;
use crate::roads::RoadMask;
use crate::route::CorridorMask;
use crate::water::WaterMask;

const GRID: i32 = 36;
const VIS_RANGE: f32 = 1400.0;
const BOUNDS_X: (i32, i32) = (-13_300, 4_900);
const BOUNDS_Z: (i32, i32) = (-30_800, 2_000);
/// Houses only appear where the cluster boost is below this threshold; above
/// it the city builder is placing taller blocks.
const HOUSE_BOOST_MAX: f32 = 16.0;

pub struct HousesPlugin;

impl Plugin for HousesPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_houses);
    }
}

fn spawn_houses(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    water: Res<WaterMask>,
    corridor: Res<CorridorMask>,
    roads: Res<RoadMask>,
    clusters: Res<Clusters>,
) {
    let wall_mesh = meshes.add(Cuboid::new(1.0, 1.0, 1.0));
    let gable_mesh = meshes.add(gable_roof_mesh());
    let hip_mesh = meshes.add(hip_roof_mesh());

    let wall_palette: Vec<Handle<StandardMaterial>> = [
        (0.92, 0.90, 0.85),
        (0.95, 0.94, 0.90),
        (0.85, 0.80, 0.71),
        (0.82, 0.86, 0.89),
        (0.78, 0.80, 0.81),
        (0.91, 0.87, 0.78),
        (0.73, 0.76, 0.78),
        (0.80, 0.72, 0.61),
    ]
    .iter()
    .map(|&(r, g, b)| {
        materials.add(StandardMaterial {
            base_color: Color::srgb(r, g, b),
            perceptual_roughness: 0.82,
            ..default()
        })
    })
    .collect();
    let roof_palette: Vec<Handle<StandardMaterial>> = [
        (0.29, 0.33, 0.38),
        (0.22, 0.26, 0.29),
        (0.42, 0.33, 0.30),
        (0.33, 0.38, 0.44),
        (0.27, 0.28, 0.31),
        (0.48, 0.42, 0.36),
    ]
    .iter()
    .map(|&(r, g, b)| {
        materials.add(StandardMaterial {
            base_color: Color::srgb(r, g, b),
            perceptual_roughness: 0.78,
            double_sided: true,
            cull_mode: None,
            ..default()
        })
    })
    .collect();

    let mut rng = fastrand::Rng::with_seed(0xC8_BC_A6_F2);

    for gx in (BOUNDS_X.0..=BOUNDS_X.1).step_by(GRID as usize) {
        for gz in (BOUNDS_Z.0..=BOUNDS_Z.1).step_by(GRID as usize) {
            let x = gx as f32 + (rng.f32() * 16.0 - 8.0);
            let z = gz as f32 + (rng.f32() * 16.0 - 8.0);

            if water.in_bay(x, z) || water.near_river(x, z, 6.0) {
                continue;
            }
            if corridor.near_track(x, z) {
                continue;
            }
            if clusters.in_keep_out(x, z) {
                continue;
            }
            // Skip dense cores; the city handles those.
            if clusters.boost_at(x, z) > HOUSE_BOOST_MAX {
                continue;
            }
            // Random thinning so the carpet doesn't read as a perfect grid.
            if rng.f32() < 0.35 {
                continue;
            }

            let fw = 6.5 + rng.f32() * 5.5;
            let fd = 7.0 + rng.f32() * 6.0;
            let two_storey = rng.f32() < 0.5;
            let wall_h = if two_storey { 6.0 } else { 3.3 } + rng.f32() * 0.8;

            let clearance = fw.max(fd) * 0.5 + 1.0;
            if roads.near(x, z, clearance) {
                continue;
            }

            let yaw = rng.f32() * std::f32::consts::TAU;
            let rot = Quat::from_rotation_y(yaw);

            let wall_mat = wall_palette[rng.usize(..wall_palette.len())].clone();
            commands.spawn((
                PbrBundle {
                    mesh: wall_mesh.clone(),
                    material: wall_mat,
                    transform: Transform::from_xyz(x, wall_h * 0.5, z)
                        .with_rotation(rot)
                        .with_scale(Vec3::new(fw, wall_h, fd)),
                    ..default()
                },
                VisibilityRange::abrupt(0.0, VIS_RANGE),
            ));

            let hip = rng.f32() < 0.62;
            let rise = if hip {
                fw.max(fd) * (0.36 + rng.f32() * 0.14)
            } else {
                fw * (0.42 + rng.f32() * 0.18)
            };
            let eaves = if hip { 1.30 } else { 1.34 };
            let roof_mat = roof_palette[rng.usize(..roof_palette.len())].clone();
            let roof_mesh = if hip { hip_mesh.clone() } else { gable_mesh.clone() };
            commands.spawn((
                PbrBundle {
                    mesh: roof_mesh,
                    material: roof_mat,
                    transform: Transform::from_xyz(x, wall_h, z)
                        .with_rotation(rot)
                        .with_scale(Vec3::new(fw * eaves, rise, fd * eaves)),
                    ..default()
                },
                VisibilityRange::abrupt(0.0, VIS_RANGE),
            ));
        }
    }
}

/// Triangular-prism gable, unit-sized. Ridge runs along the Z axis at y = 1.
fn gable_roof_mesh() -> Mesh {
    let v: Vec<[f32; 3]> = vec![
        [-0.5, 0.0, -0.5],
        [-0.5, 0.0, 0.5],
        [0.0, 1.0, 0.5],
        [-0.5, 0.0, -0.5],
        [0.0, 1.0, 0.5],
        [0.0, 1.0, -0.5],
        [0.5, 0.0, 0.5],
        [0.5, 0.0, -0.5],
        [0.0, 1.0, -0.5],
        [0.5, 0.0, 0.5],
        [0.0, 1.0, -0.5],
        [0.0, 1.0, 0.5],
        [-0.5, 0.0, -0.5],
        [0.0, 1.0, -0.5],
        [0.5, 0.0, -0.5],
        [0.5, 0.0, 0.5],
        [0.0, 1.0, 0.5],
        [-0.5, 0.0, 0.5],
    ];
    let i: Vec<u32> = (0..18).collect();
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, v);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, vec![[0.0_f32, 0.0]; 18]);
    mesh.insert_indices(Indices::U32(i));
    mesh.compute_smooth_normals();
    mesh
}

/// Four-sided pyramid (hip roof) rising to an apex at (0, 1, 0).
fn hip_roof_mesh() -> Mesh {
    let v: Vec<[f32; 3]> = vec![
        [-0.5, 0.0, -0.5],
        [0.5, 0.0, -0.5],
        [0.0, 1.0, 0.0],
        [0.5, 0.0, -0.5],
        [0.5, 0.0, 0.5],
        [0.0, 1.0, 0.0],
        [0.5, 0.0, 0.5],
        [-0.5, 0.0, 0.5],
        [0.0, 1.0, 0.0],
        [-0.5, 0.0, 0.5],
        [-0.5, 0.0, -0.5],
        [0.0, 1.0, 0.0],
    ];
    let i: Vec<u32> = (0..12).collect();
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, v);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, vec![[0.0_f32, 0.0]; 12]);
    mesh.insert_indices(Indices::U32(i));
    mesh.compute_smooth_normals();
    mesh
}
