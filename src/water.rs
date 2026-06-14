//! Sumida River, Arakawa River, and Tokyo Bay.
//!
//! Rivers are flat ribbons built from a centreline polyline plus a half-width.
//! The bay is a single polygon mesh closed off to the south-east. The same
//! polylines are also exposed as a `WaterMask` resource so the city and tree
//! placement can avoid spawning on water.

use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};
use bevy::render::render_asset::RenderAssetUsages;

use crate::geo::geo;

const RIVER_Y: f32 = 0.35;

const SUMIDA: &[(f32, f32)] = &[
    (35.788, 139.720),
    (35.772, 139.762),
    (35.748, 139.806),
    (35.727, 139.803),
    (35.711, 139.801),
    (35.700, 139.794),
    (35.691, 139.789),
    (35.683, 139.787),
    (35.672, 139.781),
    (35.664, 139.776),
    (35.655, 139.769),
];
const SUMIDA_HW: f32 = 87.0;

const ARAKAWA: &[(f32, f32)] = &[
    (35.800, 139.628),
    (35.796, 139.660),
    (35.790, 139.696),
    (35.782, 139.726),
    (35.768, 139.760),
    (35.748, 139.792),
    (35.728, 139.820),
];
const ARAKAWA_HW: f32 = 183.0;

const BAY: &[(f32, f32)] = &[
    (35.612, 139.748),
    (35.629, 139.755),
    (35.638, 139.758),
    (35.646, 139.763),
    (35.654, 139.769),
    (35.660, 139.776),
    (35.655, 139.787),
    (35.649, 139.800),
    (35.645, 139.818),
    (35.643, 139.835),
];

#[derive(Resource)]
pub struct WaterMask {
    sumida: Vec<Vec3>,
    arakawa: Vec<Vec3>,
    bay: Vec<Vec3>,
}

impl WaterMask {
    pub fn near_river(&self, x: f32, z: f32, margin: f32) -> bool {
        near_polyline(&self.sumida, SUMIDA_HW, x, z, margin)
            || near_polyline(&self.arakawa, ARAKAWA_HW, x, z, margin)
    }

    pub fn in_bay(&self, x: f32, z: f32) -> bool {
        point_in_poly(&self.bay, x, z)
    }
}

fn build_mask() -> WaterMask {
    let sumida: Vec<Vec3> = SUMIDA.iter().map(|&(la, lo)| geo(la, lo)).collect();
    let arakawa: Vec<Vec3> = ARAKAWA.iter().map(|&(la, lo)| geo(la, lo)).collect();
    let mut bay: Vec<Vec3> = BAY.iter().map(|&(la, lo)| geo(la, lo)).collect();
    bay.push(Vec3::new(9333.0, 0.0, 4333.0));
    bay.push(Vec3::new(9333.0, 0.0, 9333.0));
    bay.push(Vec3::new(500.0, 0.0, 9333.0));
    WaterMask {
        sumida,
        arakawa,
        bay,
    }
}

pub struct WaterPlugin;

impl Plugin for WaterPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(build_mask())
            .add_systems(Startup, spawn_water);
    }
}

fn spawn_water(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mask: Res<WaterMask>,
) {
    let water = materials.add(StandardMaterial {
        base_color: Color::srgb(0.23, 0.49, 0.66),
        perceptual_roughness: 0.16,
        metallic: 0.1,
        double_sided: true,
        cull_mode: None,
        ..default()
    });

    commands.spawn(PbrBundle {
        mesh: meshes.add(ribbon_mesh(&mask.sumida, SUMIDA_HW)),
        material: water.clone(),
        ..default()
    });
    commands.spawn(PbrBundle {
        mesh: meshes.add(ribbon_mesh(&mask.arakawa, ARAKAWA_HW)),
        material: water.clone(),
        ..default()
    });
    commands.spawn(PbrBundle {
        mesh: meshes.add(bay_mesh(&mask.bay)),
        material: water,
        ..default()
    });
}

fn ribbon_mesh(pts: &[Vec3], hw: f32) -> Mesh {
    let n = pts.len();
    let mut pos = Vec::with_capacity(n * 2);
    for i in 0..n {
        let a = pts[i.saturating_sub(1)];
        let b = pts[(i + 1).min(n - 1)];
        let dir = (b - a).normalize_or_zero();
        let nor = Vec3::new(dir.z, 0.0, -dir.x).normalize_or_zero();
        let p = pts[i];
        pos.push([p.x + nor.x * hw, RIVER_Y, p.z + nor.z * hw]);
        pos.push([p.x - nor.x * hw, RIVER_Y, p.z - nor.z * hw]);
    }
    let mut idx = Vec::with_capacity((n - 1) * 6);
    for s in 0..n - 1 {
        let k = (s * 2) as u32;
        idx.extend_from_slice(&[k, k + 1, k + 2, k + 1, k + 3, k + 2]);
    }
    build_mesh(pos, idx)
}

fn bay_mesh(poly: &[Vec3]) -> Mesh {
    let pos: Vec<[f32; 3]> = poly.iter().map(|p| [p.x, RIVER_Y, p.z]).collect();
    let mut idx: Vec<u32> = Vec::new();
    for i in 1..(poly.len() - 1) as u32 {
        idx.extend_from_slice(&[0, i, i + 1]);
    }
    build_mesh(pos, idx)
}

fn build_mesh(pos: Vec<[f32; 3]>, idx: Vec<u32>) -> Mesh {
    let n = pos.len();
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, pos);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0, 1.0, 0.0]; n]);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, vec![[0.0, 0.0]; n]);
    mesh.insert_indices(Indices::U32(idx));
    mesh
}

fn near_polyline(pts: &[Vec3], hw: f32, x: f32, z: f32, m: f32) -> bool {
    let r2 = (hw + m) * (hw + m);
    for i in 0..pts.len() - 1 {
        let a = pts[i];
        let b = pts[i + 1];
        let ax = b.x - a.x;
        let az = b.z - a.z;
        let denom = (ax * ax + az * az).max(1e-6);
        let t = (((x - a.x) * ax + (z - a.z) * az) / denom).clamp(0.0, 1.0);
        let dx = x - (a.x + ax * t);
        let dz = z - (a.z + az * t);
        if dx * dx + dz * dz < r2 {
            return true;
        }
    }
    false
}

fn point_in_poly(poly: &[Vec3], x: f32, z: f32) -> bool {
    let mut inside = false;
    let n = poly.len();
    let mut j = n - 1;
    for i in 0..n {
        let xi = poly[i].x;
        let zi = poly[i].z;
        let xj = poly[j].x;
        let zj = poly[j].z;
        if (zi > z) != (zj > z) && x < (xj - xi) * (z - zi) / (zj - zi) + xi {
            inside = !inside;
        }
        j = i;
    }
    inside
}
