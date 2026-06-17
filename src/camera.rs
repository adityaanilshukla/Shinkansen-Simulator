//! Follow camera with two modes.
//!
//! - **Drive**: chase camera locked behind the leading nose, eased smoothly
//!   into curves. Mouse drag offsets yaw and pitch from the train tangent.
//! - **Walk**: third-person orbit around the driver figure.

use bevy::pbr::{FogFalloff, FogSettings};
use bevy::prelude::*;

use crate::driver::{Driver, GameMode};
use crate::input::Controls;
use crate::physics::TrainState;
use crate::route::Route;
use crate::train::{Car, CARS};

#[derive(Component)]
pub struct FollowCam;

#[derive(Resource, Default)]
struct CamSmoothing {
    pos: Vec3,
    look: Vec3,
}

pub struct CameraPlugin;

impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CamSmoothing>()
            .add_systems(Startup, spawn_camera)
            .add_systems(Update, update_camera.in_set(crate::SimStage::Camera));
    }
}

fn spawn_camera(mut commands: Commands) {
    commands.spawn((
        Camera3dBundle {
            projection: Projection::Perspective(PerspectiveProjection {
                fov: 55_f32.to_radians(),
                near: 0.5,
                far: 6_000.0,
                aspect_ratio: 1.0,
            }),
            transform: Transform::from_xyz(0.0, 60.0, 200.0).looking_at(Vec3::ZERO, Vec3::Y),
            ..default()
        },
        // Linear fog into the sky colour. Hides the VisibilityRange cut-off on
        // city/houses/trees so the world doesn't pop in or out of existence.
        FogSettings {
            color: Color::srgb(0.82, 0.92, 0.97),
            falloff: FogFalloff::Linear {
                start: 1500.0,
                end: 4500.0,
            },
            ..default()
        },
        FollowCam,
    ));
}

fn update_camera(
    time: Res<Time>,
    controls: Res<Controls>,
    state: Res<TrainState>,
    mode: Res<GameMode>,
    route: Res<Route>,
    cars: Query<(&Car, &Transform), (Without<FollowCam>, Without<Driver>)>,
    driver_q: Query<&Transform, (With<Driver>, Without<FollowCam>)>,
    mut cam: Query<(&mut Transform, &mut Projection), With<FollowCam>>,
    mut smooth: ResMut<CamSmoothing>,
) {
    let Ok((mut tf, mut proj)) = cam.get_single_mut() else {
        return;
    };
    let dt = time.delta_seconds().min(0.05);

    let (pivot, desired, look_target, fov) = if mode.walking {
        walk_view(&controls, &driver_q)
    } else {
        drive_view(&controls, &state, &route, &cars)
    };

    if smooth.pos == Vec3::ZERO {
        smooth.pos = desired;
        smooth.look = pivot;
    }
    let drag_fast = if controls.dragging { 12.0 } else { 3.2 };
    let look_fast = if controls.dragging { 14.0 } else { 4.5 };
    smooth.pos = smooth.pos.lerp(desired, 1.0 - (-drag_fast * dt).exp());
    smooth.look = smooth.look.lerp(look_target, 1.0 - (-look_fast * dt).exp());

    tf.translation = smooth.pos;
    tf.look_at(smooth.look, Vec3::Y);

    if let Projection::Perspective(ref mut p) = *proj {
        p.fov = fov;
    }
}

/// (pivot, desired_camera_pos, look_target, fov)
fn drive_view(
    controls: &Controls,
    state: &TrainState,
    route: &Route,
    cars: &Query<(&Car, &Transform), (Without<FollowCam>, Without<Driver>)>,
) -> (Vec3, Vec3, Vec3, f32) {
    let mut head_pos = Vec3::ZERO;
    let mut tail_pos = Vec3::ZERO;
    for (car, ctf) in cars.iter() {
        if car.index == 0 {
            head_pos = ctf.translation;
        }
        if car.index == CARS - 1 {
            tail_pos = ctf.translation;
        }
    }
    let head_tan = route.spline.tangent_at_distance(state.dist);
    let lead = (state.view_sign + 1.0) * 0.5;
    let pivot = tail_pos.lerp(head_pos, lead) + Vec3::Y * 0.34;
    let t2 = head_tan * state.view_sign;
    let yaw = t2.x.atan2(t2.z) + controls.orbit_yaw;
    let elev = (0.345 + controls.orbit_pitch).clamp(0.06, 1.25);
    let back = (42.0 + state.speed.abs() * 0.30) * controls.zoom;
    let desired = Vec3::new(
        pivot.x - yaw.sin() * elev.cos() * back,
        pivot.y + elev.sin() * back + 1.5,
        pivot.z - yaw.cos() * elev.cos() * back,
    );
    let ahead = 55.0
        * controls.orbit_yaw.cos().max(0.0)
        * (1.0 - controls.orbit_pitch).clamp(0.0, 1.0);
    let look_target = pivot + t2 * ahead + Vec3::Y * 2.5;
    let fov = (55.0 + state.speed.abs() * 0.11).to_radians();
    (pivot, desired, look_target, fov)
}

fn walk_view(
    controls: &Controls,
    driver_q: &Query<&Transform, (With<Driver>, Without<FollowCam>)>,
) -> (Vec3, Vec3, Vec3, f32) {
    let driver_pos = driver_q
        .get_single()
        .map(|t| t.translation)
        .unwrap_or(Vec3::ZERO);
    let pivot = driver_pos + Vec3::Y * 1.45;
    let yaw = controls.orbit_yaw;
    let elev = (0.16 + controls.orbit_pitch).clamp(-0.08, 1.2);
    let back = 7.5 * controls.zoom;
    let desired = Vec3::new(
        pivot.x - yaw.sin() * elev.cos() * back,
        pivot.y + elev.sin() * back + 0.2,
        pivot.z - yaw.cos() * elev.cos() * back,
    );
    (pivot, desired, pivot, 55_f32.to_radians())
}
