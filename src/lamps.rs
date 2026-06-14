//! Street lamps along the arterials.
//!
//! Each lamp is a pole + a warm head with emissive material so they read at
//! distance. Sides alternate every other lamp. Placement skips anything that
//! would put a pole in water.

use bevy::prelude::*;
use bevy::render::view::VisibilityRange;

use crate::roads::RoadMask;
use crate::tokyo::{OsmBbox, OsmRoads};
use crate::water::WaterMask;

const LAMP_SPACING: f32 = 50.0;
const LAMP_HEIGHT: f32 = 7.2;
const SIDE_OFFSET: f32 = 8.5;
const FIRST_LAMP_AT: f32 = 18.0;
const VIS_RANGE: f32 = 700.0;

pub struct LampsPlugin;

impl Plugin for LampsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_lamps);
    }
}

fn spawn_lamps(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    roads: Res<RoadMask>,
    water: Res<WaterMask>,
    osm: Res<OsmBbox>,
    osm_roads: Res<OsmRoads>,
) {
    let pole_mesh = meshes.add(Cylinder {
        radius: 0.16,
        half_height: LAMP_HEIGHT * 0.5,
    });
    let head_mesh = meshes.add(Cuboid::new(0.66, 0.3, 1.05));

    let pole_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.27, 0.28, 0.30),
        perceptual_roughness: 0.6,
        metallic: 0.5,
        ..default()
    });
    let head_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.99, 0.95, 0.81),
        emissive: LinearRgba::new(1.0, 0.9, 0.63, 1.0),
        perceptual_roughness: 0.5,
        ..default()
    });

    for arterial in roads.arterials() {
        if arterial.len() < 2 {
            continue;
        }
        let mut acc = 0.0_f32;
        let mut next = FIRST_LAMP_AT;
        let mut side: f32 = 1.0;

        for i in 0..arterial.len() - 1 {
            let a = arterial[i];
            let b = arterial[i + 1];
            let seg_dx = b.x - a.x;
            let seg_dz = b.z - a.z;
            let seg_len = (seg_dx * seg_dx + seg_dz * seg_dz).sqrt();
            if seg_len < 0.5 {
                continue;
            }
            let ux = seg_dx / seg_len;
            let uz = seg_dz / seg_len;
            let nx = uz;
            let nz = -ux;

            while next <= acc + seg_len {
                let t = next - acc;
                let px = a.x + ux * t;
                let pz = a.z + uz * t;
                let off = SIDE_OFFSET * side;
                let lx = px + nx * off;
                let lz = pz + nz * off;

                // Inside the OSM extract the procedural arterials don't line
                // up with the actual streets, so lamps placed there would land
                // on buildings or the wrong side of a road. Real central Tokyo
                // gets no procedural lamps; the OSM area is its own world.
                if osm.contains(lx, lz) {
                    side = -side;
                    next += LAMP_SPACING;
                    continue;
                }
                // And anywhere the lamp would land on an OSM road surface
                // (including near the OSM bbox edge where the OSM data still
                // bleeds out a bit) — no lamp.
                if osm_roads.near(lx, lz, 1.5) {
                    side = -side;
                    next += LAMP_SPACING;
                    continue;
                }
                if !water.in_bay(lx, lz) && !water.near_river(lx, lz, 3.0) {
                    commands.spawn((
                        PbrBundle {
                            mesh: pole_mesh.clone(),
                            material: pole_mat.clone(),
                            transform: Transform::from_xyz(lx, LAMP_HEIGHT * 0.5, lz),
                            ..default()
                        },
                        VisibilityRange::abrupt(0.0, VIS_RANGE),
                    ));
                    commands.spawn((
                        PbrBundle {
                            mesh: head_mesh.clone(),
                            material: head_mat.clone(),
                            transform: Transform::from_xyz(lx, LAMP_HEIGHT, lz),
                            ..default()
                        },
                        VisibilityRange::abrupt(0.0, VIS_RANGE),
                    ));
                }

                side = -side;
                next += LAMP_SPACING;
            }
            acc += seg_len;
        }
    }
}
