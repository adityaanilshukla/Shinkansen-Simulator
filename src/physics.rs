//! Train physics: accelerator, brake, aero drag, rolling resistance, and the
//! brake-through-zero latch that lets the player reverse.
//!
//! All quantities are in metres and seconds.

use bevy::prelude::*;

use crate::driver::GameMode;
use crate::input::Controls;
use crate::route::Route;
use crate::train::MODEL_HALF_LENGTH;

/// 300 km/h, the E8 service maximum.
pub const V_MAX: f32 = 300.0 / 3.6;

const ACCEL: f32 = 6.2;
const BRAKE: f32 = 7.5;
const DRAG: f32 = 0.000_12;
const ROLL: f32 = 0.20;

/// Live driving state. `dist` is metres along the curve from t=0.
#[derive(Resource)]
pub struct TrainState {
    pub dist: f32,
    pub speed: f32,
    pub latch_f: bool,
    pub latch_b: bool,
    pub view_sign: f32,
}

impl TrainState {
    fn initial(route: &Route) -> Self {
        // The rear of the model is ~`2 * MODEL_HALF_LENGTH` behind the front;
        // leave a small margin so it doesn't poke off the start of the track.
        let d_min = 2.0 * MODEL_HALF_LENGTH + 10.0;
        let dist = (d_min + 240.0).min(route.spline.length() - 25.0);
        Self {
            dist,
            speed: 0.0,
            latch_f: false,
            latch_b: false,
            view_sign: 1.0,
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
    controls: Res<Controls>,
    route: Res<Route>,
    mode: Res<GameMode>,
    mut state: ResMut<TrainState>,
) {
    if mode.walking {
        // Train is parked while the player is on the platform.
        state.speed = 0.0;
        return;
    }
    let dt = time.delta_seconds().min(0.05);
    let p_sign = state.speed.signum();

    if controls.forward && !state.latch_f {
        state.speed += ACCEL * dt;
    }
    if controls.brake && !state.latch_b {
        state.speed -= BRAKE * dt;
    }

    // Aero drag is quadratic; rolling resistance is constant.
    let v = state.speed;
    state.speed -= v * v.abs() * DRAG * dt;
    let roll = ROLL * dt;
    if state.speed.abs() <= roll && !controls.forward && !controls.brake {
        state.speed = 0.0;
    } else if state.speed != 0.0 {
        state.speed -= state.speed.signum() * roll;
    }

    // Brake-through-zero: hitting the brake while moving forward stops at zero
    // and latches. Release the brake to clear the latch, then press again to
    // reverse. Same in the other direction with the accelerator.
    if p_sign > 0.0 && state.speed <= 0.0 && controls.brake {
        state.speed = 0.0;
        state.latch_b = true;
    }
    if p_sign < 0.0 && state.speed >= 0.0 && controls.forward {
        state.speed = 0.0;
        state.latch_f = true;
    }
    if !controls.brake {
        state.latch_b = false;
    }
    if !controls.forward {
        state.latch_f = false;
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

    if state.speed > 0.8 {
        state.view_sign += (1.0 - state.view_sign) * (dt * 1.6).min(1.0);
    }
    if state.speed < -0.8 {
        state.view_sign += (-1.0 - state.view_sign) * (dt * 1.6).min(1.0);
    }
}
