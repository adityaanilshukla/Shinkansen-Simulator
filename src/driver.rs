//! Driver figure and walk-around mode at stations.
//!
//! While the train is stopped near a station, pressing E steps the player out
//! as a small box-figure. Movement on the platform uses WASD relative to the
//! camera yaw. The figure is confined to the two platforms either side of the
//! train. Pressing E again near either nose boards back into the cab.

use bevy::prelude::*;

use crate::input::Controls;
use crate::physics::TrainState;
use crate::route::Route;
use crate::stations::Stations;
use crate::train::MODEL_HALF_LENGTH;

/// World-space distance (m) from the train centre to a station platform's
/// centre at which "STEP OFF" lights up.
const STATION_PROX: f32 = 110.0;
/// Distance (m) from the nose/tail at which "BOARD CAB" lights up.
const BOARD_PROX: f32 = 9.0;
/// Walk speed in m/s.
const WALK_SPEED: f32 = 4.6;
/// Half-extent of the platform along the curve tangent.
const PLATFORM_HALF: f32 = 88.0;
/// Half-width between the train and the platform edge.
const TRAIN_HALF_WIDTH: f32 = 1.95;
/// Limit how far the driver can stray onto the platform deck.
const PLATFORM_WALL: f32 = 8.2;
/// Driver Y above the platform deck and above the trackbed.
const PLATFORM_STAND: f32 = 1.0;
const TRACKBED_STAND: f32 = 0.18;

/// Active mode for the run loop. The action variant tracks what the contextual
/// E-key prompt currently offers.
#[derive(Resource, Default)]
pub struct GameMode {
    pub walking: bool,
    pub active_station: usize,
    pub near_action: NearAction,
}

#[derive(Default, PartialEq, Eq, Clone, Copy)]
pub enum NearAction {
    #[default]
    None,
    StepOff,
    BoardCab,
}

#[derive(Component)]
pub struct Driver;

pub struct DriverPlugin;

impl Plugin for DriverPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GameMode>()
            .add_systems(Startup, spawn_driver)
            .add_systems(Update, walk_loop);
    }
}

fn spawn_driver(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let navy = materials.add(StandardMaterial {
        base_color: Color::srgb(0.13, 0.19, 0.31),
        perceptual_roughness: 0.8,
        ..default()
    });
    let skin = materials.add(StandardMaterial {
        base_color: Color::srgb(0.85, 0.66, 0.52),
        perceptual_roughness: 0.7,
        ..default()
    });
    let white = materials.add(StandardMaterial {
        base_color: Color::srgb(0.91, 0.92, 0.93),
        perceptual_roughness: 0.6,
        ..default()
    });

    let body = commands
        .spawn((
            SpatialBundle {
                visibility: Visibility::Hidden,
                ..default()
            },
            Driver,
        ))
        .id();

    let limb = |meshes: &mut Assets<Mesh>, w: f32, h: f32, x: f32, y: f32| {
        PbrBundle {
            mesh: meshes.add(Cuboid::new(w, h, w + 0.02)),
            material: navy.clone(),
            transform: Transform::from_xyz(x, y - h * 0.5, 0.0),
            ..default()
        }
    };

    let parts = [
        limb(&mut meshes, 0.15, 0.74, -0.11, 0.78),
        limb(&mut meshes, 0.15, 0.74, 0.11, 0.78),
        limb(&mut meshes, 0.12, 0.58, -0.30, 1.32),
        limb(&mut meshes, 0.12, 0.58, 0.30, 1.32),
    ];

    for p in parts {
        let id = commands.spawn(p).id();
        commands.entity(body).add_child(id);
    }

    let torso = commands
        .spawn(PbrBundle {
            mesh: meshes.add(Cuboid::new(0.46, 0.60, 0.26)),
            material: navy.clone(),
            transform: Transform::from_xyz(0.0, 1.08, 0.0),
            ..default()
        })
        .id();
    let head = commands
        .spawn(PbrBundle {
            mesh: meshes.add(Sphere::new(0.145).mesh().ico(2).unwrap()),
            material: skin,
            transform: Transform::from_xyz(0.0, 1.53, 0.0),
            ..default()
        })
        .id();
    let band = commands
        .spawn(PbrBundle {
            mesh: meshes.add(Cylinder {
                radius: 0.158,
                half_height: 0.015,
            }),
            material: white,
            transform: Transform::from_xyz(0.0, 1.60, 0.0),
            ..default()
        })
        .id();
    let cap = commands
        .spawn(PbrBundle {
            mesh: meshes.add(Cylinder {
                radius: 0.155,
                half_height: 0.04,
            }),
            material: navy,
            transform: Transform::from_xyz(0.0, 1.655, 0.0),
            ..default()
        })
        .id();

    commands
        .entity(body)
        .push_children(&[torso, head, band, cap]);
}

fn walk_loop(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    controls: Res<Controls>,
    state: Res<TrainState>,
    route: Res<Route>,
    stations: Res<Stations>,
    mut mode: ResMut<GameMode>,
    mut driver_q: Query<(&mut Transform, &mut Visibility), With<Driver>>,
) {
    let Ok((mut tf, mut vis)) = driver_q.get_single_mut() else {
        return;
    };

    let head_dist = state.dist;
    let tail_dist = (state.dist - 2.0 * MODEL_HALF_LENGTH).max(0.0);
    let center_dist = (state.dist - MODEL_HALF_LENGTH).max(0.0);
    let head_pos = route.spline.position_at_distance(head_dist);
    let tail_pos = route.spline.position_at_distance(tail_dist);
    let center_pos = route.spline.position_at_distance(center_dist);

    // Decide what the contextual E action is right now.
    mode.near_action = if mode.walking {
        let d_head = xz_distance(tf.translation, head_pos);
        let d_tail = xz_distance(tf.translation, tail_pos);
        if d_head.min(d_tail) < BOARD_PROX {
            NearAction::BoardCab
        } else {
            NearAction::None
        }
    } else if state.speed.abs() < 0.5 {
        // Compare in world space: which station is the train centre closest to?
        let mut best: Option<(usize, f32)> = None;
        for (i, s) in stations.list.iter().enumerate() {
            let d = xz_distance(center_pos, s.pos);
            if best.map_or(true, |(_, bd)| d < bd) {
                best = Some((i, d));
            }
        }
        if let Some((idx, d)) = best.filter(|(_, d)| *d < STATION_PROX) {
            mode.active_station = idx;
            let _ = d;
            NearAction::StepOff
        } else {
            NearAction::None
        }
    } else {
        NearAction::None
    };

    if keys.just_pressed(KeyCode::KeyE) {
        match mode.near_action {
            NearAction::StepOff => {
                let s = stations.list[mode.active_station];
                let pos = head_pos + s.normal * 3.6;
                tf.translation = Vec3::new(pos.x, s.pos.y + PLATFORM_STAND, pos.z);
                *vis = Visibility::Visible;
                mode.walking = true;
            }
            NearAction::BoardCab => {
                *vis = Visibility::Hidden;
                mode.walking = false;
            }
            NearAction::None => {}
        }
    }

    if !mode.walking {
        return;
    }

    let dt = time.delta_seconds().min(0.05);
    let yaw = controls.orbit_yaw;
    let mx = controls.right as i32 as f32 - controls.left as i32 as f32;
    let mz = controls.forward as i32 as f32 - controls.brake as i32 as f32;
    let move_v = Vec3::new(yaw.sin() * mz - yaw.cos() * mx, 0.0, yaw.cos() * mz + yaw.sin() * mx);
    let move_v = if move_v.length() > 1e-3 {
        move_v.normalize() * WALK_SPEED * dt
    } else {
        Vec3::ZERO
    };
    tf.translation += move_v;

    // Confine to the active station's platforms.
    let s = stations.list[mode.active_station];
    let rel = tf.translation - s.pos;
    let u = rel.dot(s.tangent).clamp(-PLATFORM_HALF, PLATFORM_HALF);
    let mut v = rel.dot(s.normal);

    // Train footprint along the platform, derived from the live head/tail
    // positions projected onto this station's tangent.
    let train_u0 = (tail_pos - s.pos).dot(s.tangent) - 5.0;
    let train_u1 = (head_pos - s.pos).dot(s.tangent) + 5.0;
    if u > train_u0 && u < train_u1 && v.abs() < TRAIN_HALF_WIDTH {
        v = if v >= 0.0 {
            TRAIN_HALF_WIDTH
        } else {
            -TRAIN_HALF_WIDTH
        };
    }
    v = v.clamp(-PLATFORM_WALL, PLATFORM_WALL);

    let on_platform = v.abs() > TRAIN_HALF_WIDTH;
    let stand_y = if on_platform {
        s.pos.y + PLATFORM_STAND
    } else {
        s.pos.y + TRACKBED_STAND
    };
    let confined = s.pos + s.tangent * u + s.normal * v;
    tf.translation = Vec3::new(confined.x, stand_y, confined.z);

    // Face the direction of motion.
    if move_v.length_squared() > 1e-6 {
        let face = move_v.x.atan2(move_v.z);
        tf.rotation = Quat::from_rotation_y(face);
    }
}

fn xz_distance(a: Vec3, b: Vec3) -> f32 {
    let dx = a.x - b.x;
    let dz = a.z - b.z;
    (dx * dx + dz * dz).sqrt()
}

