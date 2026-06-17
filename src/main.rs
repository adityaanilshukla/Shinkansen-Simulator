//! E5 Shinkansen, Tokyo Drive (Rust/bevy port).
//!
//! This file is just the wiring: window setup, then every plugin in the order
//! they need to register their resources.

mod audio;
mod camera;
mod driver;
mod geo;
mod ground;
mod hud;
mod input;
mod lamps;
mod landmarks;
mod minimap;
mod motion;
mod osm_data;
mod physics;
mod roads;
mod route;
mod sky;
mod spline;
mod stations;
mod tokyo;
mod track;
mod train;
mod trees;
mod water;

use bevy::prelude::*;

/// Frame order for the simulation pipeline. Physics writes `TrainState.dist`,
/// motion uses it to place the train cars, and camera reads the resulting
/// transforms to follow them. Without this chain, Bevy is free to interleave
/// the three systems each frame and the camera sometimes ends up reading
/// stale car transforms — visible as jitter at high speed.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum SimStage {
    Physics,
    Motion,
    Camera,
}

fn main() {
    App::new()
        .add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "E5 Shinkansen | Tokyo Drive".into(),
                        resolution: (1280.0, 800.0).into(),
                        ..default()
                    }),
                    ..default()
                })
                .set(AssetPlugin {
                    // Resolve assets relative to the project root regardless of
                    // where the binary is launched from. CARGO_MANIFEST_DIR is
                    // embedded at compile time.
                    file_path: concat!(env!("CARGO_MANIFEST_DIR"), "/assets").to_string(),
                    ..default()
                }),
        )
        .add_plugins((
            route::RoutePlugin,
            sky::SkyPlugin,
            ground::GroundPlugin,
            water::WaterPlugin,
            landmarks::LandmarksPlugin,
            // StationsPlugin must come before TrackPlugin: spawn_track reads
            // Res<Stations>.list to suppress catenary masts inside station
            // envelopes, and Stations is populated in its Startup system.
            stations::StationsPlugin,
            roads::RoadsPlugin,
            tokyo::TokyoPlugin,
            track::TrackPlugin,
            lamps::LampsPlugin,
            trees::TreesPlugin,
            train::TrainPlugin,
        ))
        .add_plugins((
            input::InputPlugin,
            driver::DriverPlugin,
            physics::PhysicsPlugin,
            motion::MotionPlugin,
            camera::CameraPlugin,
            hud::HudPlugin,
            minimap::MinimapPlugin,
            audio::AudioPlugin,
        ))
        .configure_sets(
            Update,
            (SimStage::Physics, SimStage::Motion, SimStage::Camera).chain(),
        )
        .run();
}
