//! WGS84 latitude/longitude to world-space conversion.
//!
//! The world origin sits near the Imperial Palace. Distances are scaled so 1
//! world unit is roughly 3.33 metres on the ground. Positive X runs east,
//! positive Z runs south. Y is up and not derived here.

use bevy::math::Vec3;

const LAT0: f32 = 35.680;
const LON0: f32 = 139.739;
const X_PER_DEG_LON: f32 = 90_440.0;
const Z_PER_DEG_LAT: f32 = 110_900.0;

/// Convert a (latitude, longitude) pair into world-space coordinates at y = 0.
pub fn geo(lat: f32, lon: f32) -> Vec3 {
    Vec3::new(
        (lon - LON0) * X_PER_DEG_LON,
        0.0,
        -(lat - LAT0) * Z_PER_DEG_LAT,
    )
}
