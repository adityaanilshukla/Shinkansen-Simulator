//! Real Tohoku Shinkansen alignment out of Tokyo.
//!
//! The route follows the actual track: Tokyo, Ueno, then north past Akabane,
//! crossing the Arakawa, through Saitama-Shintoshin to Omiya, then the straight
//! run north toward Hasuda.

use bevy::prelude::*;
use std::collections::HashSet;

use crate::geo::geo;
use crate::spline::Spline;

/// The viaduct deck height in world units.
pub const DECK_Y: f32 = 14.0;

/// Spline curve resource owned by the route plugin and shared with everything
/// that places geometry along the track.
#[derive(Resource)]
pub struct Route {
    pub spline: Spline,
}

/// O(1) "is this point within ~36 units of the track" lookup, used by city and
/// tree placement so they don't spawn on top of the viaduct.
#[derive(Resource)]
pub struct CorridorMask {
    cells: HashSet<(i32, i32)>,
    cell_size: f32,
}

impl CorridorMask {
    pub fn near_track(&self, x: f32, z: f32) -> bool {
        let key = (
            (x / self.cell_size).round() as i32,
            (z / self.cell_size).round() as i32,
        );
        self.cells.contains(&key)
    }
}

const ROUTE_LATLON: &[(f32, f32)] = &[
    (35.6740, 139.7640),
    (35.6812, 139.7671),
    (35.6920, 139.7710),
    (35.6984, 139.7731),
    (35.7141, 139.7774),
    (35.7280, 139.7710),
    (35.7381, 139.7610),
    (35.7528, 139.7380),
    (35.7778, 139.7210),
    (35.7900, 139.7060),
    (35.8030, 139.6780),
    (35.8230, 139.6580),
    (35.8459, 139.6420),
    (35.8680, 139.6350),
    (35.8940, 139.6310),
    (35.9060, 139.6240),
    (35.9300, 139.6280),
    (35.9700, 139.6420),
    (36.0100, 139.6560),
    (36.0400, 139.6660),
];

pub fn build_route() -> Route {
    let points: Vec<Vec3> = ROUTE_LATLON
        .iter()
        .map(|&(lat, lon)| {
            let mut p = geo(lat, lon);
            p.y = DECK_Y;
            p
        })
        .collect();
    Route {
        spline: Spline::new(points, 0.5, 1600),
    }
}

fn build_corridor_mask(route: &Route) -> CorridorMask {
    // ~18 m radius. Covers the modelled viaduct deck (5 m half-width each
    // side) plus a route-approximation buffer (8-13 m) without leaving a
    // wide empty corridor through the city.
    let cell_size = 6.0;
    let radius_cells = 3;
    let r2 = radius_cells * radius_cells;
    let mut cells = HashSet::new();
    let n = 1600usize;
    for i in 0..=n {
        let p = route.spline.position(i as f32 / n as f32);
        let cx = (p.x / cell_size).round() as i32;
        let cz = (p.z / cell_size).round() as i32;
        for a in -radius_cells..=radius_cells {
            for b in -radius_cells..=radius_cells {
                if a * a + b * b <= r2 {
                    cells.insert((cx + a, cz + b));
                }
            }
        }
    }
    CorridorMask { cells, cell_size }
}

pub struct RoutePlugin;

impl Plugin for RoutePlugin {
    fn build(&self, app: &mut App) {
        let route = build_route();
        let mask = build_corridor_mask(&route);
        app.insert_resource(route).insert_resource(mask);
    }
}
