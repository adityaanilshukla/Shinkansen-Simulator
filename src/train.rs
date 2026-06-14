//! Load the E5 Shinkansen GLB and attach a `Car` tag so the motion system
//! drives it along the route.
//!
//! The model is 3 rigid cars in one mesh hierarchy (front cab + middle car +
//! rear cab, ~79.5 m end to end). It's treated as one entity. On straight
//! track that looks correct; on Tokyo's tight bends the rigid body can't
//! follow the curve, which is the trade-off of using a single-piece model.
//! Splitting the GLB into per-car files in Blender is the way to recover
//! curve-following.

use bevy::prelude::*;

/// Number of independently-positioned car entities. The current model is one
/// rigid 3-car block, so this is 1.
pub const CARS: usize = 1;

/// Half the model's total length in metres. Place the entity at
/// `state.dist - MODEL_HALF_LENGTH` so the model's nose lands at `state.dist`.
pub const MODEL_HALF_LENGTH: f32 = 39.7;

/// Per-car offset from the head distance. With one rigid model, the only
/// entry is the model's half-length.
pub const OFFSETS: [f32; CARS] = [MODEL_HALF_LENGTH];

/// Tag for the train entity. `flip` is preserved for future per-car splits;
/// the single rigid model never flips.
#[derive(Component, Clone, Copy)]
pub struct Car {
    pub index: usize,
    pub flip: bool,
}

pub struct TrainPlugin;

impl Plugin for TrainPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_train);
    }
}

fn spawn_train(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.spawn((
        SceneBundle {
            scene: asset_server.load("train.glb#Scene0"),
            ..default()
        },
        Car {
            index: 0,
            flip: false,
        },
    ));
}
