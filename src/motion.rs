//! Place each car along the spline once per frame, with banking on curves.
//!
//! The model is built with -Z as its forward axis (the glTF convention), so
//! we rotate it so local -Z aligns with the curve tangent.

use bevy::prelude::*;

use crate::physics::TrainState;
use crate::route::Route;
use crate::train::{Car, OFFSETS};

const BANK_LOOKAHEAD: f32 = 8.0;

/// Vertical offset of the model origin from the spline. Tuned so the model's
/// wheels land on the rail tops. The GLB's mesh-local bottom is ~1.5 units
/// above its origin, so we lift the entity slightly below the deck.
const TRAIN_LIFT: f32 = -1.5;

pub struct MotionPlugin;

impl Plugin for MotionPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, follow_curve.in_set(crate::SimStage::Motion));
    }
}

fn follow_curve(state: Res<TrainState>, route: Res<Route>, mut q: Query<(&Car, &mut Transform)>) {
    let length = route.spline.length();
    for (car, mut tf) in q.iter_mut() {
        let d = (state.dist - OFFSETS[car.index]).clamp(0.0, length);
        let p = route.spline.position_at_distance(d);
        let tan = route.spline.tangent_at_distance(d);

        // Rotate so local -Z (glTF forward) aligns with the tangent.
        let yaw = (-tan.x).atan2(-tan.z);

        // Banking: sample the tangent a few metres ahead, derive the signed
        // angular change, scale by speed^2 for a centripetal feel.
        let tan2 = route
            .spline
            .tangent_at_distance((d + BANK_LOOKAHEAD).min(length));
        let cross_y = tan.z * tan2.x - tan.x * tan2.z;
        let kappa = cross_y / BANK_LOOKAHEAD;
        let roll = (kappa * state.speed * state.speed * 0.010).clamp(-0.085, 0.085);

        let mut rot = Quat::from_rotation_y(yaw);
        if car.flip {
            rot *= Quat::from_rotation_y(std::f32::consts::PI);
            rot *= Quat::from_rotation_z(-roll);
        } else {
            rot *= Quat::from_rotation_z(roll);
        }

        tf.translation = Vec3::new(p.x, p.y + TRAIN_LIFT, p.z);
        tf.rotation = rot;
    }
}
