//! Keyboard and mouse input. No touch controls.
//!
//! - `W`: throttle lever up one notch
//! - `S / Space`: throttle lever down one notch (brake)
//! - `A / D`: walk-mode strafe
//! - `Arrow keys`: orbit the camera (yaw + pitch). Hold `Shift` + ↑/↓ to zoom.
//! - `C`: flip view fore/aft
//! - `V`: reset camera
//! - `M`: toggle audio mute
//! - `Esc`: exit
//! - Drag with the mouse: orbit
//! - Wheel: zoom

use bevy::input::mouse::{MouseMotion, MouseWheel};
use bevy::prelude::*;

#[derive(Resource, Default)]
pub struct Controls {
    pub forward: bool,
    pub brake: bool,
    pub left: bool,
    pub right: bool,
    pub orbit_yaw: f32,
    pub orbit_pitch: f32,
    pub zoom: f32,
    pub dragging: bool,
}

impl Controls {
    pub fn new() -> Self {
        Self {
            zoom: 1.0,
            ..default()
        }
    }
}

pub struct InputPlugin;

impl Plugin for InputPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Controls::new())
            .add_systems(Update, (read_keys, read_mouse, read_wheel));
    }
}

fn read_keys(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mut controls: ResMut<Controls>,
    mut exit: EventWriter<AppExit>,
) {
    // W/A/S/D drive train throttle (handled in physics.rs) and walk-mode
    // movement (driver.rs). Arrow keys are camera-only.
    controls.forward = keys.pressed(KeyCode::KeyW);
    controls.brake = keys.any_pressed([KeyCode::KeyS, KeyCode::Space]);
    controls.left = keys.pressed(KeyCode::KeyA);
    controls.right = keys.pressed(KeyCode::KeyD);

    // Arrow keys orbit the camera at a fixed angular rate, matching what the
    // mouse drag does to `orbit_yaw` / `orbit_pitch`. Holding Shift turns
    // Up/Down into a zoom in/out instead (same effect as the wheel).
    let dt = time.delta_seconds().min(0.05);
    let shift = keys.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]);
    let yaw_rate = 1.4;
    let pitch_rate = 1.0;
    let zoom_rate = 0.9; // ln(zoom) units per second
    let mut dyaw = 0.0;
    let mut dpitch = 0.0;
    let mut dzoom_log = 0.0;
    if keys.pressed(KeyCode::ArrowLeft) {
        dyaw -= yaw_rate * dt;
    }
    if keys.pressed(KeyCode::ArrowRight) {
        dyaw += yaw_rate * dt;
    }
    if keys.pressed(KeyCode::ArrowUp) {
        if shift {
            dzoom_log += zoom_rate * dt;
        } else {
            dpitch += pitch_rate * dt;
        }
    }
    if keys.pressed(KeyCode::ArrowDown) {
        if shift {
            dzoom_log -= zoom_rate * dt;
        } else {
            dpitch -= pitch_rate * dt;
        }
    }
    if dyaw != 0.0 {
        controls.orbit_yaw += dyaw;
        if controls.orbit_yaw > std::f32::consts::PI {
            controls.orbit_yaw -= std::f32::consts::TAU;
        } else if controls.orbit_yaw < -std::f32::consts::PI {
            controls.orbit_yaw += std::f32::consts::TAU;
        }
    }
    if dpitch != 0.0 {
        controls.orbit_pitch = (controls.orbit_pitch + dpitch).clamp(-0.26, 0.85);
    }
    if dzoom_log != 0.0 {
        controls.zoom = (controls.zoom * dzoom_log.exp()).clamp(0.45, 2.4);
    }

    // V resets the orbit / zoom to the default chase view.
    if keys.just_pressed(KeyCode::KeyV) {
        controls.orbit_yaw = 0.0;
        controls.orbit_pitch = 0.0;
        controls.zoom = 1.0;
    }
    if keys.just_pressed(KeyCode::Escape) {
        exit.send(AppExit::Success);
    }
}

fn read_mouse(
    buttons: Res<ButtonInput<MouseButton>>,
    mut motion: EventReader<MouseMotion>,
    mut controls: ResMut<Controls>,
) {
    controls.dragging = buttons.pressed(MouseButton::Left);
    if !controls.dragging {
        motion.clear();
        return;
    }
    let mut dx = 0.0;
    let mut dy = 0.0;
    for ev in motion.read() {
        dx += ev.delta.x;
        dy += ev.delta.y;
    }
    controls.orbit_yaw -= dx * 0.0052;
    if controls.orbit_yaw > std::f32::consts::PI {
        controls.orbit_yaw -= std::f32::consts::TAU;
    } else if controls.orbit_yaw < -std::f32::consts::PI {
        controls.orbit_yaw += std::f32::consts::TAU;
    }
    controls.orbit_pitch = (controls.orbit_pitch + dy * 0.004).clamp(-0.26, 0.85);
}

fn read_wheel(mut wheel: EventReader<MouseWheel>, mut controls: ResMut<Controls>) {
    for ev in wheel.read() {
        let factor = if ev.y > 0.0 { 0.92 } else { 1.09 };
        controls.zoom = (controls.zoom * factor).clamp(0.45, 2.4);
    }
}
