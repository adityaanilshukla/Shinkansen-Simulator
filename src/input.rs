//! Keyboard and mouse input. No touch controls.
//!
//! - `W / Up`: throttle
//! - `S / Down / Space`: brake (and reverse after stopping)
//! - `A / D / Left / Right`: nudge orbit yaw
//! - `C`: flip view fore/aft
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
    keys: Res<ButtonInput<KeyCode>>,
    mut controls: ResMut<Controls>,
    mut exit: EventWriter<AppExit>,
) {
    controls.forward = keys.any_pressed([KeyCode::KeyW, KeyCode::ArrowUp]);
    controls.brake = keys.any_pressed([KeyCode::KeyS, KeyCode::ArrowDown, KeyCode::Space]);
    controls.left = keys.any_pressed([KeyCode::KeyA, KeyCode::ArrowLeft]);
    controls.right = keys.any_pressed([KeyCode::KeyD, KeyCode::ArrowRight]);

    if keys.just_pressed(KeyCode::KeyC) {
        // Flip the camera to look the other way down the train.
        if controls.orbit_yaw.abs() > std::f32::consts::FRAC_PI_2 {
            controls.orbit_yaw = 0.0;
        } else {
            controls.orbit_yaw = std::f32::consts::PI;
        }
        controls.orbit_pitch = 0.0;
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
