//! Density centres along the corridor.
//!
//! Both the city builder and the residential carpet read the same Gaussian
//! field to decide what should go where: tall glass towers near a cluster
//! peak, low blocks at the edges, single-family houses out in the tail.

use bevy::prelude::*;

use crate::geo::geo;

pub struct Cluster {
    pub center: Vec3,
    pub base: f32,
    pub radius: f32,
}

pub struct KeepOut {
    pub center: Vec3,
    pub radius: f32,
}

#[derive(Resource)]
pub struct Clusters {
    pub list: Vec<Cluster>,
    pub keep_outs: Vec<KeepOut>,
}

impl Clusters {
    pub fn boost_at(&self, x: f32, z: f32) -> f32 {
        self.list
            .iter()
            .map(|c| {
                let dx = x - c.center.x;
                let dz = z - c.center.z;
                c.base * (-(dx * dx + dz * dz) / (c.radius * c.radius)).exp()
            })
            .sum()
    }

    pub fn in_keep_out(&self, x: f32, z: f32) -> bool {
        self.keep_outs.iter().any(|k| {
            let dx = x - k.center.x;
            let dz = z - k.center.z;
            dx * dx + dz * dz < k.radius * k.radius
        })
    }
}

pub struct ClustersPlugin;

impl Plugin for ClustersPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Clusters {
            list: make_clusters(),
            keep_outs: make_keep_outs(),
        });
    }
}

fn make_clusters() -> Vec<Cluster> {
    [
        (35.683, 139.764, 165.0, 1000.0),   // Marunouchi / Otemachi
        (35.698, 139.772, 70.0, 667.0),     // Akihabara / Kanda
        (35.713, 139.777, 75.0, 667.0),     // Ueno
        (35.7295, 139.711, 100.0, 733.0),   // Ikebukuro
        (35.778, 139.721, 55.0, 600.0),     // Akabane
        (35.798, 139.712, 85.0, 667.0),     // Kawaguchi
        (35.894, 139.631, 130.0, 800.0),    // Saitama-Shintoshin
        (35.906, 139.626, 95.0, 800.0),     // Omiya
    ]
    .iter()
    .map(|&(la, lo, base, radius)| Cluster {
        center: geo(la, lo),
        base,
        radius,
    })
    .collect()
}

fn make_keep_outs() -> Vec<KeepOut> {
    vec![
        KeepOut {
            center: geo(35.6586, 139.7454),
            radius: 130.0,
        },
        KeepOut {
            center: geo(35.7101, 139.8107),
            radius: 170.0,
        },
        KeepOut {
            center: geo(35.6285, 139.7755),
            radius: 165.0,
        },
    ]
}
