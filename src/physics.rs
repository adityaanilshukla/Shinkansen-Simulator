//! Train physics: accelerator, brake, aero drag, rolling resistance, and the
//! brake-through-zero latch that lets the player reverse.
//!
//! All quantities are in metres and seconds.

use bevy::prelude::*;

use crate::driver::GameMode;
use crate::route::Route;
use crate::train::MODEL_HALF_LENGTH;

/// 300 km/h, the E8 service maximum.
pub const V_MAX: f32 = 300.0 / 3.6;

const ACCEL: f32 = 6.2;
const BRAKE: f32 = 7.5;
const DRAG: f32 = 0.000_12;
const ROLL: f32 = 0.20;

/// Maximum throttle notch in either direction (power or brake).
pub const THROTTLE_NOTCHES: i32 = 4;

/// Live driving state. `dist` is metres along the curve from t=0.
#[derive(Resource)]
pub struct TrainState {
    pub dist: f32,
    pub speed: f32,
    pub view_sign: f32,
    /// Discrete throttle lever in `[-THROTTLE_NOTCHES, +THROTTLE_NOTCHES]`.
    /// Positive notches apply power in `forward_dir`; negative notches apply
    /// brake force proportional to magnitude.
    pub throttle_level: i32,
    /// Direction the train treats as forward: +1 default, -1 after pressing
    /// C. Reversing also flips the camera (via `view_sign`) and zeros the
    /// throttle lever.
    pub forward_dir: f32,
}

impl TrainState {
    fn initial(route: &Route) -> Self {
        let d_min = 2.0 * MODEL_HALF_LENGTH + 10.0;
        let dist = (d_min + 240.0).min(route.spline.length() - 25.0);
        Self {
            dist,
            speed: 0.0,
            view_sign: 1.0,
            throttle_level: 0,
            forward_dir: 1.0,
        }
    }
}

pub struct PhysicsPlugin;

impl Plugin for PhysicsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, init_train_state)
            .add_systems(Update, step_physics);
    }
}

fn init_train_state(mut commands: Commands, route: Res<Route>) {
    commands.insert_resource(TrainState::initial(&route));
}

fn step_physics(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    route: Res<Route>,
    mode: Res<GameMode>,
    mut state: ResMut<TrainState>,
) {
    if mode.walking {
        // Train parks while the player is on the platform; also re-zero the
        // throttle lever so we don't auto-accelerate after boarding.
        state.speed = 0.0;
        state.throttle_level = 0;
        return;
    }
    let dt = time.delta_seconds().min(0.05);

    // Throttle lever: W/Up steps the notch up, S/Down steps it down.
    if keys.just_pressed(KeyCode::KeyW) || keys.just_pressed(KeyCode::ArrowUp) {
        state.throttle_level = (state.throttle_level + 1).min(THROTTLE_NOTCHES);
    }
    if keys.just_pressed(KeyCode::KeyS) || keys.just_pressed(KeyCode::ArrowDown) {
        state.throttle_level = (state.throttle_level - 1).max(-THROTTLE_NOTCHES);
    }

    // C reverses the train (and the camera) when nearly stopped. Returning
    // the lever to 0 forces a deliberate re-engage.
    if keys.just_pressed(KeyCode::KeyC) && state.speed.abs() < 1.0 {
        state.forward_dir = -state.forward_dir;
        state.throttle_level = 0;
        // Snap the camera target immediately; the smoothed view_sign update
        // below will ease it over a frame or two.
        state.view_sign = state.forward_dir;
    }

    // Apply the lever. Positive notches accelerate in forward_dir; negative
    // notches reduce |speed|. The lever's magnitude / NOTCHES is the fraction
    // of ACCEL or BRAKE applied.
    let level = state.throttle_level as f32 / THROTTLE_NOTCHES as f32;
    if level > 0.0 {
        state.speed += ACCEL * dt * level * state.forward_dir;
    } else if level < 0.0 {
        let brake = BRAKE * dt * -level;
        if state.speed.abs() < brake {
            state.speed = 0.0;
        } else {
            state.speed -= state.speed.signum() * brake;
        }
    }

    // Aero drag (quadratic) + rolling resistance (constant).
    let v = state.speed;
    state.speed -= v * v.abs() * DRAG * dt;
    let roll = ROLL * dt;
    if state.speed.abs() <= roll && state.throttle_level == 0 {
        state.speed = 0.0;
    } else if state.speed != 0.0 {
        state.speed -= state.speed.signum() * roll;
    }

    state.speed = state.speed.clamp(-V_MAX, V_MAX);
    state.dist += state.speed * dt;

    let d_min = 2.0 * MODEL_HALF_LENGTH + 10.0;
    let d_max = route.spline.length() - 25.0;
    if state.dist < d_min {
        state.dist = d_min;
        if state.speed < 0.0 {
            state.speed = 0.0;
        }
    }
    if state.dist > d_max {
        state.dist = d_max;
        if state.speed > 0.0 {
            state.speed = 0.0;
        }
    }

    // view_sign tracks forward_dir so the camera looks ahead of the train
    // regardless of speed. C flips it instantly above; this smooths the
    // remainder.
    let target = state.forward_dir;
    state.view_sign += (target - state.view_sign) * (dt * 4.0).min(1.0);
}
