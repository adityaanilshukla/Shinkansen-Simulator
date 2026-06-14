//! Procedural city: tens of thousands of buildings driven by the cluster
//! density field in `clusters.rs`. Tall glass towers near the peaks, short
//! matte blocks at the edges. Footprints avoid water, the viaduct, roads,
//! and the landmark keep-out zones.

use bevy::prelude::*;
use bevy::render::view::VisibilityRange;

use crate::clusters::Clusters;
use crate::roads::RoadMask;
use crate::route::CorridorMask;
use crate::water::WaterMask;

const GRID: i32 = 40;
const VIS_RANGE: f32 = 2500.0;
const BOUNDS_X: (i32, i32) = (-13_400, 5_000);
const BOUNDS_Z: (i32, i32) = (-31_200, 2_200);

pub struct CityPlugin;

impl Plugin for CityPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_city);
    }
}

fn spawn_city(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    water: Res<WaterMask>,
    corridor: Res<CorridorMask>,
    roads: Res<RoadMask>,
    clusters: Res<Clusters>,
) {
    let palette = make_palette(&mut materials);
    let unit_box = meshes.add(Cuboid::new(1.0, 1.0, 1.0));

    let mut rng = fastrand::Rng::with_seed(0x5E3A8C);

    for gx in (BOUNDS_X.0..=BOUNDS_X.1).step_by(GRID as usize) {
        for gz in (BOUNDS_Z.0..=BOUNDS_Z.1).step_by(GRID as usize) {
            let x = gx as f32 + (rng.f32() * 22.0 - 11.0);
            let z = gz as f32 + (rng.f32() * 22.0 - 11.0);

            if water.in_bay(x, z) || water.near_river(x, z, 9.0) {
                continue;
            }
            if corridor.near_track(x, z) {
                continue;
            }
            if clusters.in_keep_out(x, z) {
                continue;
            }

            let boost = clusters.boost_at(x, z);
            if boost < 6.0 {
                continue;
            }
            if rng.f32() < 0.32 {
                continue;
            }

            let (w, d, h, kind) = footprint(boost, &mut rng);
            if kind == Kind::Block && rng.f32() < 0.6 {
                continue;
            }
            let clearance = w.max(d) * 0.5 + 1.5;
            if roads.near(x, z, clearance) {
                continue;
            }

            let yaw = rng.f32() * std::f32::consts::TAU;
            let mat = if kind == Kind::Tower || boost > 60.0 {
                palette.glass[rng.usize(..palette.glass.len())].clone()
            } else {
                palette.matte[rng.usize(..palette.matte.len())].clone()
            };

            commands.spawn((
                PbrBundle {
                    mesh: unit_box.clone(),
                    material: mat,
                    transform: Transform::from_xyz(x, h * 0.5, z)
                        .with_rotation(Quat::from_rotation_y(yaw))
                        .with_scale(Vec3::new(w, h, d)),
                    ..default()
                },
                VisibilityRange::abrupt(0.0, VIS_RANGE),
            ));
        }
    }
}

#[derive(PartialEq, Eq)]
enum Kind {
    Block,
    Midrise,
    Slab,
    Tower,
}

fn footprint(boost: f32, rng: &mut fastrand::Rng) -> (f32, f32, f32, Kind) {
    if boost > 40.0 && rng.f32() < 0.34 {
        let w = 14.0 + rng.f32() * 16.0;
        let d = 14.0 + rng.f32() * 16.0;
        let h = ((40.0 + boost) * (1.1 + rng.f32() * 1.8)).min(270.0);
        (w, d, h, Kind::Tower)
    } else if boost > 22.0 {
        let w = 12.0 + rng.f32() * 14.0;
        let d = 12.0 + rng.f32() * 9.0;
        let h = 24.0 + rng.f32() * 40.0 + boost * 0.2;
        (w, d, h, Kind::Slab)
    } else if boost > 10.0 {
        let w = 10.0 + rng.f32() * 12.0;
        let d = 10.0 + rng.f32() * 7.0;
        let h = 11.0 + rng.f32() * 18.0 + boost * 0.2;
        (w, d, h, Kind::Midrise)
    } else {
        let w = 8.0 + rng.f32() * 7.0;
        let d = 8.0 + rng.f32() * 7.0;
        let h = 8.0 + rng.f32() * 9.0;
        (w, d, h, Kind::Block)
    }
}

struct Palette {
    matte: Vec<Handle<StandardMaterial>>,
    glass: Vec<Handle<StandardMaterial>>,
}

fn make_palette(materials: &mut Assets<StandardMaterial>) -> Palette {
    let matte_colors = [
        (0.81, 0.83, 0.85),
        (0.78, 0.74, 0.65),
        (0.56, 0.57, 0.59),
        (0.90, 0.91, 0.89),
        (0.66, 0.57, 0.49),
        (0.73, 0.64, 0.54),
        (0.44, 0.47, 0.52),
        (0.60, 0.63, 0.66),
    ];
    let glass_colors = [
        (0.62, 0.71, 0.78),
        (0.36, 0.42, 0.47),
        (0.50, 0.68, 0.62),
        (0.56, 0.66, 0.72),
        (0.72, 0.66, 0.56),
        (0.68, 0.75, 0.81),
    ];
    Palette {
        matte: matte_colors
            .iter()
            .map(|&(r, g, b)| {
                materials.add(StandardMaterial {
                    base_color: Color::srgb(r, g, b),
                    perceptual_roughness: 0.88,
                    ..default()
                })
            })
            .collect(),
        glass: glass_colors
            .iter()
            .map(|&(r, g, b)| {
                materials.add(StandardMaterial {
                    base_color: Color::srgb(r, g, b),
                    perceptual_roughness: 0.3,
                    metallic: 0.45,
                    ..default()
                })
            })
            .collect(),
    }
}
