//! Elevated viaduct, rails, piers, and catenary.
//!
//! Instead of one entity per 0.6 m of track (which gave ~13 k entities for the
//! viaduct alone), the deck/slab/walls/rails/piers/caps are merged into chunks
//! of `CHUNK_SIZE` segments. Each chunk is a single mesh entity positioned at
//! the chunk's centroid, so `VisibilityRange` can cull distant track properly.

use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};
use bevy::render::render_asset::RenderAssetUsages;
use bevy::render::view::VisibilityRange;

use crate::route::{Route, DECK_Y};

const SEG_COUNT: usize = 1600;
const CHUNK_SIZE: usize = 50;
const CHUNKS: usize = SEG_COUNT / CHUNK_SIZE;
const VIS_DETAIL: f32 = 600.0;

pub struct TrackPlugin;

impl Plugin for TrackPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_track);
    }
}

fn spawn_track(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    route: Res<Route>,
) {
    let length = route.spline.length();
    let seg_len = length / SEG_COUNT as f32;

    let concrete = materials.add(StandardMaterial {
        base_color: Color::srgb(0.72, 0.73, 0.74),
        perceptual_roughness: 0.85,
        ..default()
    });
    let concrete2 = materials.add(StandardMaterial {
        base_color: Color::srgb(0.64, 0.65, 0.67),
        perceptual_roughness: 0.9,
        ..default()
    });
    let slab_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.79, 0.80, 0.81),
        perceptual_roughness: 0.8,
        ..default()
    });
    let rail_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.49, 0.51, 0.53),
        perceptual_roughness: 0.35,
        metallic: 0.85,
        ..default()
    });
    let mast_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.31, 0.33, 0.36),
        perceptual_roughness: 0.6,
        metallic: 0.4,
        ..default()
    });

    let mast_mesh = meshes.add(Cylinder {
        radius: 0.15,
        half_height: 2.9,
    });
    let arm_mesh = meshes.add(Cuboid::new(0.12, 0.12, 5.6));

    for ci in 0..CHUNKS {
        let start = ci * CHUNK_SIZE;
        let end = (ci + 1) * CHUNK_SIZE;

        let mid_t = ((start + end) as f32 * 0.5) / SEG_COUNT as f32;
        let chunk_center = route.spline.position(mid_t);

        let mut deck = MeshBuf::new();
        let mut slab = MeshBuf::new();
        let mut walls = MeshBuf::new();
        let mut rails = MeshBuf::new();
        let mut piers = MeshBuf::new();
        let mut caps = MeshBuf::new();

        for i in start..end {
            let t = (i as f32 + 0.5) / SEG_COUNT as f32;
            let p_world = route.spline.position(t);
            let p = p_world - chunk_center;
            let tan = route.spline.tangent(t);
            let nor = Vec3::new(tan.z, 0.0, -tan.x).normalize_or_zero();
            let rot = Quat::from_rotation_y(tan.x.atan2(tan.z));

            // Box length overlap: pad each segment by `seg_len + OVERLAP` so
            // consecutive boxes meaningfully overlap on curves, where their
            // orientations rotate and the unpadded ends would diverge.
            let overlap_deck = seg_len + 2.5;
            let overlap_wall = seg_len + 2.5;
            let overlap_rail = seg_len + 2.0;

            deck.append_box(
                p + Vec3::new(0.0, -0.7, 0.0),
                rot,
                Vec3::new(10.4, 1.4, overlap_deck),
            );
            slab.append_box(
                p + Vec3::new(0.0, 0.07, 0.0),
                rot,
                Vec3::new(3.0, 0.14, overlap_deck),
            );

            let wleft = p + nor * 5.0 + Vec3::Y * 0.5;
            let wright = p - nor * 5.0 + Vec3::Y * 0.5;
            walls.append_box(wleft, rot, Vec3::new(0.35, 1.05, overlap_wall));
            walls.append_box(wright, rot, Vec3::new(0.35, 1.05, overlap_wall));

            let rleft = p + nor * 0.72 + Vec3::Y * 0.26;
            let rright = p - nor * 0.72 + Vec3::Y * 0.26;
            rails.append_box(rleft, rot, Vec3::new(0.16, 0.24, overlap_rail));
            rails.append_box(rright, rot, Vec3::new(0.16, 0.24, overlap_rail));

            if i % 2 == 0 {
                piers.append_box(
                    p + Vec3::new(0.0, 5.9 - DECK_Y, 0.0),
                    Quat::IDENTITY,
                    Vec3::new(2.6, 11.8, 2.6),
                );
                caps.append_box(
                    p + Vec3::new(0.0, 12.35 - DECK_Y, 0.0),
                    Quat::IDENTITY,
                    Vec3::new(8.0, 1.1, 3.4),
                );
            }
        }

        spawn_chunk(&mut commands, &mut meshes, deck, concrete.clone(), chunk_center);
        spawn_chunk(&mut commands, &mut meshes, slab, slab_mat.clone(), chunk_center);
        spawn_chunk(&mut commands, &mut meshes, walls, concrete2.clone(), chunk_center);
        spawn_chunk(&mut commands, &mut meshes, rails, rail_mat.clone(), chunk_center);
        spawn_chunk(&mut commands, &mut meshes, piers, concrete2.clone(), chunk_center);
        spawn_chunk(&mut commands, &mut meshes, caps, concrete.clone(), chunk_center);
    }

    // Catenary masts + arms stay as individual entities (the cylinder is hard
    // to merge cheaply) but only render up close.
    for i in 0..SEG_COUNT {
        if i % 2 != 0 {
            continue;
        }
        let t = (i as f32 + 0.5) / SEG_COUNT as f32;
        let p = route.spline.position(t);
        let tan = route.spline.tangent(t);
        let nor = Vec3::new(tan.z, 0.0, -tan.x).normalize_or_zero();
        let rot = Quat::from_rotation_y(tan.x.atan2(tan.z));

        commands.spawn((
            PbrBundle {
                mesh: mast_mesh.clone(),
                material: mast_mat.clone(),
                transform: Transform::from_xyz(
                    p.x + nor.x * 5.4,
                    DECK_Y + 2.9,
                    p.z + nor.z * 5.4,
                ),
                ..default()
            },
            VisibilityRange::abrupt(0.0, VIS_DETAIL),
        ));
        commands.spawn((
            PbrBundle {
                mesh: arm_mesh.clone(),
                material: mast_mat.clone(),
                transform: Transform::from_xyz(
                    p.x + nor.x * 2.7,
                    DECK_Y + 5.35,
                    p.z + nor.z * 2.7,
                )
                .with_rotation(rot),
                ..default()
            },
            VisibilityRange::abrupt(0.0, VIS_DETAIL),
        ));
    }
}

fn spawn_chunk(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    buf: MeshBuf,
    material: Handle<StandardMaterial>,
    center: Vec3,
) {
    if buf.is_empty() {
        return;
    }
    // No `VisibilityRange` on the viaduct chunks: with their centroids ~1.25 km
    // apart, distance-based culling would pop adjacent chunks in at different
    // times and leave a visible gap. The merged geometry is small (~6k tris
    // per chunk) and fog already fades the far distance.
    commands.spawn(PbrBundle {
        mesh: meshes.add(buf.into_mesh()),
        material,
        transform: Transform::from_translation(center),
        ..default()
    });
}

/// Accumulator for chunk geometry. Each box is expanded into 6 flat-shaded
/// faces with their own normals so adjacent boxes don't smooth into each other.
struct MeshBuf {
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    uvs: Vec<[f32; 2]>,
    indices: Vec<u32>,
}

impl MeshBuf {
    fn new() -> Self {
        Self {
            positions: Vec::new(),
            normals: Vec::new(),
            uvs: Vec::new(),
            indices: Vec::new(),
        }
    }

    fn is_empty(&self) -> bool {
        self.positions.is_empty()
    }

    fn append_box(&mut self, center: Vec3, rot: Quat, size: Vec3) {
        let hx = size.x * 0.5;
        let hy = size.y * 0.5;
        let hz = size.z * 0.5;
        let faces: [(Vec3, [Vec3; 4]); 6] = [
            (
                Vec3::Y,
                [
                    Vec3::new(-hx, hy, -hz),
                    Vec3::new(hx, hy, -hz),
                    Vec3::new(hx, hy, hz),
                    Vec3::new(-hx, hy, hz),
                ],
            ),
            (
                Vec3::NEG_Y,
                [
                    Vec3::new(-hx, -hy, hz),
                    Vec3::new(hx, -hy, hz),
                    Vec3::new(hx, -hy, -hz),
                    Vec3::new(-hx, -hy, -hz),
                ],
            ),
            (
                Vec3::X,
                [
                    Vec3::new(hx, -hy, -hz),
                    Vec3::new(hx, -hy, hz),
                    Vec3::new(hx, hy, hz),
                    Vec3::new(hx, hy, -hz),
                ],
            ),
            (
                Vec3::NEG_X,
                [
                    Vec3::new(-hx, -hy, hz),
                    Vec3::new(-hx, -hy, -hz),
                    Vec3::new(-hx, hy, -hz),
                    Vec3::new(-hx, hy, hz),
                ],
            ),
            (
                Vec3::Z,
                [
                    Vec3::new(hx, -hy, hz),
                    Vec3::new(-hx, -hy, hz),
                    Vec3::new(-hx, hy, hz),
                    Vec3::new(hx, hy, hz),
                ],
            ),
            (
                Vec3::NEG_Z,
                [
                    Vec3::new(-hx, -hy, -hz),
                    Vec3::new(hx, -hy, -hz),
                    Vec3::new(hx, hy, -hz),
                    Vec3::new(-hx, hy, -hz),
                ],
            ),
        ];

        for (face_normal, corners) in faces {
            let base = self.positions.len() as u32;
            let world_n = rot * face_normal;
            for v in corners {
                let p = center + rot * v;
                self.positions.push([p.x, p.y, p.z]);
                self.normals.push([world_n.x, world_n.y, world_n.z]);
            }
            self.uvs
                .extend_from_slice(&[[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]]);
            self.indices
                .extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
        }
    }

    fn into_mesh(self) -> Mesh {
        let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, self.positions);
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, self.normals);
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, self.uvs);
        mesh.insert_indices(Indices::U32(self.indices));
        mesh
    }
}
