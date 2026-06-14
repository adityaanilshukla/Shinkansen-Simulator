//! Real central-Tokyo arterials.
//!
//! Each road is a polyline of WGS84 points run through `geo()`, so streets
//! sit where they really do. All roads are merged into a single mesh for one
//! draw call.

use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};
use bevy::render::render_asset::RenderAssetUsages;

use crate::geo::geo;

const ROAD_Y: f32 = 0.22;
const ARTERIAL_HW: f32 = 6.5;

// Lifted directly from the original game's `road(...)` calls in
// reference/e8-shinkansen-tokyo.html, so the named arterials line up with
// the real Tokyo grid.
const ARTERIALS: &[&[(f32, f32)]] = &[
    // Chuo-dori (Ginza-Nihonbashi-Kanda-Ueno)
    &[
        (35.6712, 139.7650),
        (35.6766, 139.7715),
        (35.6840, 139.7745),
        (35.6905, 139.7728),
        (35.6983, 139.7731),
        (35.7065, 139.7740),
        (35.7115, 139.7745),
        (35.7148, 139.7770),
    ],
    // Showa-dori (Shuto Ueno line overhead)
    &[
        (35.6660, 139.7585),
        (35.6695, 139.7672),
        (35.6772, 139.7745),
        (35.6900, 139.7800),
        (35.7010, 139.7800),
        (35.7110, 139.7792),
    ],
    // Sotobori-dori (round the outer moat)
    &[
        (35.6665, 139.7585),
        (35.6705, 139.7510),
        (35.6755, 139.7405),
        (35.6860, 139.7298),
        (35.6918, 139.7370),
        (35.7015, 139.7448),
        (35.7005, 139.7555),
        (35.6998, 139.7648),
        (35.6985, 139.7720),
    ],
    // Yasukuni-dori
    &[
        (35.6905, 139.7000),
        (35.6928, 139.7180),
        (35.6925, 139.7340),
        (35.6952, 139.7510),
        (35.6958, 139.7578),
        (35.6952, 139.7660),
        (35.6948, 139.7758),
        (35.6960, 139.7855),
    ],
    // Hibiya-dori
    &[
        (35.6655, 139.7575),
        (35.6738, 139.7600),
        (35.6800, 139.7635),
        (35.6870, 139.7660),
        (35.6920, 139.7665),
    ],
    // Eitai-dori (over the Sumida)
    &[
        (35.6862, 139.7640),
        (35.6822, 139.7740),
        (35.6800, 139.7790),
        (35.6788, 139.7875),
        (35.6720, 139.7965),
    ],
    // Asakusa-dori (toward Skytree)
    &[
        (35.7110, 139.7775),
        (35.7115, 139.7910),
        (35.7108, 139.7970),
        (35.7100, 139.8075),
    ],
    // Kasuga-dori
    &[
        (35.7075, 139.7740),
        (35.7070, 139.7630),
        (35.7085, 139.7520),
        (35.7110, 139.7400),
        (35.7120, 139.7300),
    ],
    // Hongo-dori
    &[
        (35.6998, 139.7635),
        (35.7075, 139.7610),
        (35.7160, 139.7595),
        (35.7270, 139.7555),
        (35.7365, 139.7480),
        (35.7530, 139.7385),
    ],
    // Meiji-dori
    &[
        (35.6900, 139.7030),
        (35.7035, 139.7045),
        (35.7135, 139.7040),
        (35.7230, 139.7080),
        (35.7295, 139.7110),
        (35.7320, 139.7235),
        (35.7340, 139.7370),
        (35.7445, 139.7385),
        (35.7530, 139.7385),
    ],
    // Edo-dori
    &[
        (35.6840, 139.7745),
        (35.6940, 139.7855),
        (35.7050, 139.7935),
        (35.7105, 139.7975),
    ],
    // Kuramaebashi-dori
    &[
        (35.6995, 139.7720),
        (35.7010, 139.7830),
        (35.7020, 139.7935),
    ],
    // Route 122 corridor, north to Omiya
    &[
        (35.7530, 139.7385),
        (35.7640, 139.7300),
        (35.7780, 139.7232),
        (35.7900, 139.7215),
        (35.7975, 139.7205),
        (35.8160, 139.7140),
        (35.8350, 139.6905),
        (35.8600, 139.6700),
        (35.8850, 139.6450),
        (35.9060, 139.6260),
    ],
    // Nakasendo (Itabashi side)
    &[
        (35.7365, 139.7480),
        (35.7480, 139.7350),
        (35.7600, 139.7180),
        (35.7720, 139.7000),
        (35.7850, 139.6850),
    ],
    // Ring 7 (Kannana) arc, north
    &[
        (35.7340, 139.6800),
        (35.7400, 139.7050),
        (35.7430, 139.7300),
        (35.7400, 139.7560),
        (35.7320, 139.7800),
    ],
    // East-side connector by the river
    &[
        (35.6960, 139.8000),
        (35.7080, 139.7980),
        (35.7200, 139.7920),
        (35.7320, 139.7800),
    ],
];

#[derive(Resource)]
pub struct RoadMask {
    lines: Vec<Vec<Vec3>>,
    hw: f32,
}

#[allow(dead_code)]
impl RoadMask {
    /// Returns true if (x, z) lies within `margin` of any road centerline.
    pub fn near(&self, x: f32, z: f32, margin: f32) -> bool {
        let r = self.hw + margin;
        let r2 = r * r;
        for line in &self.lines {
            for i in 0..line.len() - 1 {
                let a = line[i];
                let b = line[i + 1];
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
        }
        false
    }

    pub fn arterials(&self) -> impl Iterator<Item = &[Vec3]> {
        self.lines.iter().map(Vec::as_slice)
    }

    pub fn half_width(&self) -> f32 {
        self.hw
    }
}

pub struct RoadsPlugin;

impl Plugin for RoadsPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(build_mask())
            .add_systems(Startup, spawn_roads);
    }
}

fn build_mask() -> RoadMask {
    let lines: Vec<Vec<Vec3>> = ARTERIALS
        .iter()
        .map(|arterial| arterial.iter().map(|&(la, lo)| geo(la, lo)).collect())
        .collect();
    RoadMask {
        lines,
        hw: ARTERIAL_HW,
    }
}

fn spawn_roads(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let mut pos: Vec<[f32; 3]> = Vec::new();
    let mut idx: Vec<u32> = Vec::new();

    for arterial in ARTERIALS {
        let pts: Vec<Vec3> = arterial.iter().map(|&(la, lo)| geo(la, lo)).collect();
        push_ribbon(&pts, ARTERIAL_HW, ROAD_Y, &mut pos, &mut idx);
    }

    let n = pos.len();
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, pos);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0, 1.0, 0.0]; n]);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, vec![[0.0, 0.0]; n]);
    mesh.insert_indices(Indices::U32(idx));

    let mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.235, 0.252, 0.278),
        perceptual_roughness: 0.95,
        ..default()
    });

    commands.spawn(PbrBundle {
        mesh: meshes.add(mesh),
        material: mat,
        ..default()
    });
}

fn push_ribbon(pts: &[Vec3], hw: f32, y: f32, pos: &mut Vec<[f32; 3]>, idx: &mut Vec<u32>) {
    let n = pts.len();
    let start = pos.len() as u32;
    for i in 0..n {
        let a = pts[i.saturating_sub(1)];
        let b = pts[(i + 1).min(n - 1)];
        let dir = (b - a).normalize_or_zero();
        let nor = Vec3::new(dir.z, 0.0, -dir.x).normalize_or_zero();
        let p = pts[i];
        pos.push([p.x + nor.x * hw, y, p.z + nor.z * hw]);
        pos.push([p.x - nor.x * hw, y, p.z - nor.z * hw]);
    }
    for s in 0..(n as u32 - 1) {
        let k = start + s * 2;
        idx.extend_from_slice(&[k, k + 1, k + 2, k + 1, k + 3, k + 2]);
    }
}
