//! Station platforms at Tokyo, Ueno, and Omiya.
//!
//! Each station is a short series of platform decks stepped along the route
//! curve so the platform follows bends instead of shooting off straight.

use bevy::prelude::*;

use crate::geo::geo;
use crate::route::Route;

const PLAT_HALF: f32 = 96.0;
const PLAT_SEGMENTS: usize = 16;

const STATIONS: &[(f32, f32, &str)] = &[
    (35.6812, 139.7671, "TOKYO"),
    (35.7141, 139.7774, "UENO"),
    (35.9060, 139.6240, "OMIYA"),
];

/// Surveyed once at startup. Fields are read by the (deferred) walk mode and
/// by the station-proximity HUD, so they're kept on the resource.
#[allow(dead_code)]
#[derive(Clone, Copy)]
pub struct StationInfo {
    pub t: f32,
    pub dist: f32,
    pub pos: Vec3,
    pub tangent: Vec3,
    pub normal: Vec3,
    pub name: &'static str,
}

#[derive(Resource, Default)]
pub struct Stations {
    pub list: Vec<StationInfo>,
}

pub struct StationsPlugin;

impl Plugin for StationsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Stations>()
            .add_systems(Startup, spawn_stations);
    }
}

pub fn spawn_stations(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    route: Res<Route>,
    mut stations: ResMut<Stations>,
) {
    let plat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.85, 0.83, 0.78),
        perceptual_roughness: 0.9,
        ..default()
    });
    let roof = materials.add(StandardMaterial {
        base_color: Color::srgb(0.29, 0.33, 0.38),
        perceptual_roughness: 0.6,
        metallic: 0.3,
        ..default()
    });
    let edge = materials.add(StandardMaterial {
        base_color: Color::srgb(0.96, 0.78, 0.26),
        perceptual_roughness: 0.7,
        ..default()
    });
    let face = materials.add(StandardMaterial {
        base_color: Color::srgb(0.64, 0.65, 0.67),
        perceptual_roughness: 0.9,
        ..default()
    });
    let col_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.55, 0.57, 0.6),
        perceptual_roughness: 0.5,
        metallic: 0.5,
        ..default()
    });
    let pier_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.64, 0.65, 0.67),
        perceptual_roughness: 0.9,
        ..default()
    });

    let length = route.spline.length();
    let half_u = PLAT_HALF / length;
    let seg_u = (2.0 * half_u) / PLAT_SEGMENTS as f32;
    let seg_len = (2.0 * PLAT_HALF / PLAT_SEGMENTS as f32) * 1.06;

    let deck_mesh = meshes.add(Cuboid::new(6.6, 1.1, seg_len));
    let roof_mesh = meshes.add(Cuboid::new(7.4, 0.28, seg_len));
    let edge_mesh = meshes.add(Cuboid::new(0.5, 0.06, seg_len));
    let face_mesh = meshes.add(Cuboid::new(0.4, 2.6, seg_len));
    let pier_mesh = meshes.add(Cuboid::new(1.3, 13.9, 1.3));
    let col_mesh = meshes.add(Cylinder {
        radius: 0.16,
        half_height: 2.05,
    });

    for &(lat, lon, name) in STATIONS {
        let g = geo(lat, lon);
        let t = find_t_near(&route, g.x, g.z);
        let p = route.spline.position(t);
        let tan = route.spline.tangent(t);
        let nor = Vec3::new(tan.z, 0.0, -tan.x).normalize_or_zero();

        let dist = route.spline.distance_at_t(t);
        stations.list.push(StationInfo {
            t,
            dist,
            pos: p,
            tangent: tan,
            normal: nor,
            name,
        });

        for side in [-1.0_f32, 1.0_f32] {
            for i in 0..PLAT_SEGMENTS {
                let u = (t - half_u + (i as f32 + 0.5) * seg_u).clamp(0.0, 1.0);
                let pp = route.spline.position(u);
                let tt = route.spline.tangent(u);
                let nn = Vec3::new(tt.z, 0.0, -tt.x).normalize_or_zero();
                let q = align_yaw(tt);

                let base = pp + nn * (side * 5.15);
                spawn_seg(&mut commands, &deck_mesh, &plat, base + Vec3::Y * 0.45, q);
                spawn_seg(&mut commands, &roof_mesh, &roof, base + Vec3::Y * 5.1, q);
                spawn_seg(
                    &mut commands,
                    &edge_mesh,
                    &edge,
                    pp + nn * (side * 2.1) + Vec3::Y * 1.04,
                    q,
                );
                spawn_seg(
                    &mut commands,
                    &face_mesh,
                    &face,
                    pp + nn * (side * 8.2) - Vec3::Y * 1.4,
                    q,
                );
            }

            for k in 0..9 {
                let u = (t - half_u + (k as f32 / 8.0) * 2.0 * half_u).clamp(0.0, 1.0);
                let pp = route.spline.position(u);
                let tt = route.spline.tangent(u);
                let nn = Vec3::new(tt.z, 0.0, -tt.x).normalize_or_zero();

                spawn_seg(
                    &mut commands,
                    &col_mesh,
                    &col_mat,
                    pp + nn * (side * 8.0) + Vec3::Y * 3.0,
                    Quat::IDENTITY,
                );
                spawn_seg(
                    &mut commands,
                    &pier_mesh,
                    &pier_mat,
                    pp + nn * (side * 8.0) - Vec3::Y * 7.05,
                    Quat::IDENTITY,
                );
            }
        }
    }
}

fn spawn_seg(
    commands: &mut Commands,
    mesh: &Handle<Mesh>,
    mat: &Handle<StandardMaterial>,
    pos: Vec3,
    rot: Quat,
) {
    commands.spawn(PbrBundle {
        mesh: mesh.clone(),
        material: mat.clone(),
        transform: Transform::from_translation(pos).with_rotation(rot),
        ..default()
    });
}

fn align_yaw(tan: Vec3) -> Quat {
    Quat::from_rotation_y(tan.x.atan2(tan.z))
}

/// Sample t values to find the one whose curve point is closest to (x, z).
fn find_t_near(route: &Route, x: f32, z: f32) -> f32 {
    let mut best_t = 0.0;
    let mut best_d = f32::MAX;
    for i in 0..1400 {
        let t = i as f32 / 1400.0;
        let p = route.spline.position(t);
        let dx = p.x - x;
        let dz = p.z - z;
        let d = dx * dx + dz * dz;
        if d < best_d {
            best_d = d;
            best_t = t;
        }
    }
    best_t
}

