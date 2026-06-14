//! Elevated viaduct, rails, piers, catenary masts, and the contact wire.
//!
//! Geometry is **continuous quad-strip ribbons**. Per chunk we sample
//! `CHUNK_SIZE + 1` rings of `(position, normal)` along the spline; each
//! adjacent pair of rings emits four quads (top/bottom/inner/outer of the
//! cross-section). Adjacent quads inside a chunk share their boundary corner
//! positions exactly (same `Ring`), and adjacent chunks sample the same
//! `t` at their shared boundary, so gaps are impossible by construction.
//!
//! Piers and masts are filtered against both the procedural arterial mask
//! (`RoadMask`) and the OSM street mask (`OsmRoads`) so they never land on a
//! road. Masts are also suppressed near stations so they don't poke through
//! platform canopies.
//!
//! The catenary contact wire is rendered as a single thin ribbon along the
//! whole route at +5.15 m above the deck.

use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};
use bevy::render::render_asset::RenderAssetUsages;
use bevy::render::view::VisibilityRange;

use crate::roads::RoadMask;
use crate::route::{Route, DECK_Y};
use crate::stations::Stations;
use crate::tokyo::OsmRoads;

const SEG_COUNT: usize = 1600;
const CHUNK_SIZE: usize = 50;
const CHUNKS: usize = SEG_COUNT / CHUNK_SIZE;
const PIER_STEP: usize = 2;
const PIER_COUNT: usize = SEG_COUNT / PIER_STEP;
const WIRE_SAMPLES: usize = 781;
const VIS_DETAIL: f32 = 600.0;
const ROAD_AVOID_MARGIN: f32 = 2.5;
const STATION_MAST_SUPPRESS_M: f32 = 100.0;
const WIRE_DY: f32 = 5.15;
const WIRE_HW: f32 = 0.02;
const WIRE_HH: f32 = 0.02;
/// Number of small box segments used to discretise a parabolic arch span.
const ARCH_SEGMENTS: usize = 20;

/// One pier candidate position on the route. Built once at the start of
/// `spawn_track` so the two-pass pier placement can address by index.
#[derive(Copy, Clone)]
struct PierSample {
    chunk_index: usize,
    p_world: Vec3,
    blocked: bool,
}

/// Bitset for which faces of a ribbon to emit. INNER is the +nor side, OUTER
/// is the -nor side. For centred ribbons (nor_offset = 0) the labels are
/// arbitrary but consistent.
const FACE_TOP: u8 = 1;
const FACE_BOT: u8 = 2;
const FACE_INNER: u8 = 4;
const FACE_OUTER: u8 = 8;
const FACE_ALL: u8 = FACE_TOP | FACE_BOT | FACE_INNER | FACE_OUTER;

/// Cross-section of one ribbon. Y values are relative to the spline's Y level
/// (`DECK_Y` for the track ribbons, free for the wire).
#[derive(Copy, Clone)]
struct Ribbon {
    half_width: f32,
    top_y: f32,
    bot_y: f32,
    nor_offset: f32,
    faces: u8,
}

/// One sample along the curve. `p` is **chunk-local** for the track ribbons
/// (subtracted by chunk_center) and **world-space** for the wire.
#[derive(Copy, Clone)]
struct Ring {
    p: Vec3,
    nor: Vec3,
}

// Ribbon constructors -----------------------------------------------------

fn deck_ribbon() -> Ribbon {
    Ribbon { half_width: 5.2, top_y: 0.0, bot_y: -1.4, nor_offset: 0.0, faces: FACE_ALL }
}
fn slab_ribbon() -> Ribbon {
    // Slab bottom is +5 mm above deck top so the two faces don't z-fight; the
    // slab bottom face is also skipped because it's never visible.
    Ribbon {
        half_width: 1.5,
        top_y: 0.14,
        bot_y: 0.005,
        nor_offset: 0.0,
        faces: FACE_TOP | FACE_INNER | FACE_OUTER,
    }
}
fn wall_ribbon(side: f32) -> Ribbon {
    Ribbon { half_width: 0.175, top_y: 1.025, bot_y: -0.025, nor_offset: 5.0 * side, faces: FACE_ALL }
}
fn rail_ribbon(side: f32) -> Ribbon {
    Ribbon { half_width: 0.08, top_y: 0.38, bot_y: 0.14, nor_offset: 0.72 * side, faces: FACE_ALL }
}
fn wire_ribbon() -> Ribbon {
    Ribbon { half_width: WIRE_HW, top_y: WIRE_HH, bot_y: -WIRE_HH, nor_offset: 0.0, faces: FACE_ALL }
}

// Plugin ------------------------------------------------------------------

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
    roads: Res<RoadMask>,
    osm_roads: Res<OsmRoads>,
    stations: Res<Stations>,
) {
    let length = route.spline.length();

    // Materials -----------------------------------------------------------
    // Dark weathered concrete for the viaduct surfaces, near-black for the
    // rails so they read distinctly against the deck.
    let concrete = materials.add(StandardMaterial {
        base_color: Color::srgb(0.42, 0.43, 0.45),
        perceptual_roughness: 0.9,
        ..default()
    });
    let concrete2 = materials.add(StandardMaterial {
        base_color: Color::srgb(0.36, 0.37, 0.39),
        perceptual_roughness: 0.92,
        ..default()
    });
    let slab_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.30, 0.30, 0.32),
        perceptual_roughness: 0.92,
        ..default()
    });
    let rail_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.07, 0.07, 0.09),
        perceptual_roughness: 0.4,
        metallic: 0.7,
        ..default()
    });
    let mast_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.24, 0.26, 0.30),
        perceptual_roughness: 0.6,
        metallic: 0.4,
        ..default()
    });
    let wire_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.188, 0.204, 0.227),
        perceptual_roughness: 0.5,
        metallic: 0.6,
        ..default()
    });

    let mast_mesh = meshes.add(Cylinder { radius: 0.15, half_height: 2.9 });
    let arm_mesh = meshes.add(Cuboid::new(0.12, 0.12, 5.6));

    // Pre-compute per-chunk centres so arch geometry (which can land in any
    // chunk) can address the right buffer.
    let chunk_centers: Vec<Vec3> = (0..CHUNKS)
        .map(|ci| {
            let mid_t = (ci * CHUNK_SIZE + CHUNK_SIZE / 2) as f32 / SEG_COUNT as f32;
            route.spline.position(mid_t)
        })
        .collect();

    // Hoisted per-chunk pier/cap buffers so pass 2 can write arch geometry
    // into any chunk it needs.
    let mut pier_bufs: Vec<MeshBuf> = (0..CHUNKS).map(|_| MeshBuf::new()).collect();
    let mut cap_bufs: Vec<MeshBuf> = (0..CHUNKS).map(|_| MeshBuf::new()).collect();

    // Pre-sample every pier candidate so we can detect runs of blocked
    // positions for arch spans.
    let samples: Vec<PierSample> = (0..PIER_COUNT)
        .map(|s| {
            let global_i = s * PIER_STEP;
            let chunk_index = (global_i / CHUNK_SIZE).min(CHUNKS - 1);
            let t = global_i as f32 / SEG_COUNT as f32;
            let p_world = route.spline.position(t);
            PierSample {
                chunk_index,
                p_world,
                blocked: blocked(&osm_roads, &roads, p_world),
            }
        })
        .collect();

    // Chunks: emit deck/slab/walls/rails immediately (these don't span
    // chunks), piers/caps go into the hoisted buffers.
    for ci in 0..CHUNKS {
        let chunk_center = chunk_centers[ci];
        let rings = build_chunk_rings(&route, ci, chunk_center);

        let mut deck = MeshBuf::new();
        let mut slab = MeshBuf::new();
        let mut walls = MeshBuf::new();
        let mut rails = MeshBuf::new();

        emit_ribbon(&mut deck, &rings, &deck_ribbon());
        emit_ribbon(&mut slab, &rings, &slab_ribbon());
        emit_ribbon(&mut walls, &rings, &wall_ribbon(1.0));
        emit_ribbon(&mut walls, &rings, &wall_ribbon(-1.0));
        emit_ribbon(&mut rails, &rings, &rail_ribbon(1.0));
        emit_ribbon(&mut rails, &rings, &rail_ribbon(-1.0));

        spawn_chunk(&mut commands, &mut meshes, deck, concrete.clone(), chunk_center);
        spawn_chunk(&mut commands, &mut meshes, slab, slab_mat.clone(), chunk_center);
        spawn_chunk(&mut commands, &mut meshes, walls, concrete2.clone(), chunk_center);
        spawn_chunk(&mut commands, &mut meshes, rails, rail_mat.clone(), chunk_center);
    }

    // Pass 1: emit centre piers at every clear sample.
    for s in &samples {
        if s.blocked {
            continue;
        }
        let p_local = s.p_world - chunk_centers[s.chunk_index];
        add_centre_pier(
            &mut pier_bufs[s.chunk_index],
            &mut cap_bufs[s.chunk_index],
            p_local,
        );
    }

    // Pass 2: walk the blocked array, build a parabolic arc parallel to the
    // track for every run of consecutive blocked positions that has clear
    // abutments on both sides.
    let mut i = 0;
    while i < samples.len() {
        if !samples[i].blocked {
            i += 1;
            continue;
        }
        let mut j = i;
        while j < samples.len() && samples[j].blocked {
            j += 1;
        }
        if i > 0 && j < samples.len() {
            let prev = i - 1;
            let next = j;
            add_arch_span(
                &samples,
                &chunk_centers,
                prev,
                next,
                &mut pier_bufs,
                &mut cap_bufs,
            );
        }
        i = j;
    }

    // Spawn the pier/cap chunks now that both passes are done.
    for ci in 0..CHUNKS {
        let chunk_center = chunk_centers[ci];
        let piers = std::mem::replace(&mut pier_bufs[ci], MeshBuf::new());
        let caps = std::mem::replace(&mut cap_bufs[ci], MeshBuf::new());
        spawn_chunk(&mut commands, &mut meshes, piers, concrete2.clone(), chunk_center);
        spawn_chunk(&mut commands, &mut meshes, caps, concrete.clone(), chunk_center);
    }

    // Masts + arms (individual entities so they range-cull) ---------------
    let station_dists: Vec<f32> = stations.list.iter().map(|s| s.dist).collect();
    for i in (0..SEG_COUNT).step_by(PIER_STEP) {
        let t = (i as f32 + 0.5) / SEG_COUNT as f32;
        let arc = t * length;
        if station_dists
            .iter()
            .any(|&d| (arc - d).abs() < STATION_MAST_SUPPRESS_M)
        {
            continue;
        }
        let p = route.spline.position(t);
        let tan = route.spline.tangent(t);
        let nor = Vec3::new(tan.z, 0.0, -tan.x).normalize_or_zero();
        let mast_pos = Vec3::new(p.x + nor.x * 5.4, DECK_Y + 2.9, p.z + nor.z * 5.4);
        if osm_roads.near(mast_pos.x, mast_pos.z, ROAD_AVOID_MARGIN)
            || roads.near(mast_pos.x, mast_pos.z, ROAD_AVOID_MARGIN)
        {
            continue;
        }
        let rot_yaw = Quat::from_rotation_y(tan.x.atan2(tan.z));
        // The arm box is long along its local Z (5.6 m). The yaw rotates local
        // +Z to the tangent direction — that would make the arm point along
        // the track, which is wrong. An extra +90° around Y rotates local +Z
        // to the curve normal, so the arm crosses the track from the mast
        // toward the centreline.
        let arm_rot = rot_yaw * Quat::from_rotation_y(std::f32::consts::FRAC_PI_2);
        commands.spawn((
            PbrBundle {
                mesh: mast_mesh.clone(),
                material: mast_mat.clone(),
                transform: Transform::from_translation(mast_pos),
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
                .with_rotation(arm_rot),
                ..default()
            },
            VisibilityRange::abrupt(0.0, VIS_DETAIL),
        ));
    }

    // Catenary contact wire — one big mesh along the full spline.
    build_wire(&mut commands, &mut meshes, &route, wire_mat);
}

/// Sample `CHUNK_SIZE + 1` rings for chunk `ci`, stored in `chunk_center`-local
/// coordinates. The inclusive range is important: chunk N's last ring is at
/// the same `t` as chunk N+1's first ring, so the shared boundary corner
/// lands at identical world positions in both chunks → seamless join.
fn build_chunk_rings(route: &Route, ci: usize, chunk_center: Vec3) -> Vec<Ring> {
    let mut rings = Vec::with_capacity(CHUNK_SIZE + 1);
    for k in 0..=CHUNK_SIZE {
        let global_i = ci * CHUNK_SIZE + k;
        let t = global_i as f32 / SEG_COUNT as f32;
        let p_world = route.spline.position(t);
        let tan = route.spline.tangent(t);
        let nor = Vec3::new(tan.z, 0.0, -tan.x).normalize_or_zero();
        rings.push(Ring { p: p_world - chunk_center, nor });
    }
    rings
}

/// Emit a continuous quad-strip ribbon along `rings`. Winding for each face
/// is **verified** to produce outward normals under Bevy 0.14's default
/// `FrontFace::Ccw` + `Face::Back` cull. Do not paraphrase the corner orders.
fn emit_ribbon(buf: &mut MeshBuf, rings: &[Ring], r: &Ribbon) {
    if rings.len() < 2 {
        return;
    }
    for w in 0..rings.len() - 1 {
        let r0 = &rings[w];
        let r1 = &rings[w + 1];
        let c0 = r0.p + r0.nor * r.nor_offset;
        let c1 = r1.p + r1.nor * r.nor_offset;

        let tl0 = c0 + r0.nor * r.half_width + Vec3::Y * r.top_y;
        let tr0 = c0 - r0.nor * r.half_width + Vec3::Y * r.top_y;
        let bl0 = c0 + r0.nor * r.half_width + Vec3::Y * r.bot_y;
        let br0 = c0 - r0.nor * r.half_width + Vec3::Y * r.bot_y;
        let tl1 = c1 + r1.nor * r.half_width + Vec3::Y * r.top_y;
        let tr1 = c1 - r1.nor * r.half_width + Vec3::Y * r.top_y;
        let bl1 = c1 + r1.nor * r.half_width + Vec3::Y * r.bot_y;
        let br1 = c1 - r1.nor * r.half_width + Vec3::Y * r.bot_y;

        if r.faces & FACE_TOP != 0 {
            buf.add_quad(tl0, tr0, tr1, tl1);
        }
        if r.faces & FACE_BOT != 0 {
            buf.add_quad(bl0, bl1, br1, br0);
        }
        if r.faces & FACE_INNER != 0 {
            buf.add_quad(bl0, tl0, tl1, bl1);
        }
        if r.faces & FACE_OUTER != 0 {
            buf.add_quad(br0, br1, tr1, tr0);
        }
    }
}

fn blocked(osm_roads: &OsmRoads, roads: &RoadMask, p: Vec3) -> bool {
    osm_roads.near(p.x, p.z, ROAD_AVOID_MARGIN) || roads.near(p.x, p.z, ROAD_AVOID_MARGIN)
}

/// Centre pier: a wider square base footing + a tall square column + the wide
/// cap that sits under the deck. Three boxes give it more sculpted weight
/// than the single rectangle we had before.
fn add_centre_pier(piers: &mut MeshBuf, caps: &mut MeshBuf, p_local: Vec3) {
    // Base footing: short and wide, sitting at ground.
    append_box(
        piers,
        p_local + Vec3::new(0.0, 0.4 - DECK_Y, 0.0),
        Vec3::new(3.8, 0.8, 3.8),
    );
    // Main column: square, full height under the deck.
    append_box(
        piers,
        p_local + Vec3::new(0.0, 5.9 - DECK_Y, 0.0),
        Vec3::new(2.6, 10.4, 2.6),
    );
    // Cap: wide flat plank that the deck sits on.
    append_box(
        caps,
        p_local + Vec3::new(0.0, 12.35 - DECK_Y, 0.0),
        Vec3::new(8.0, 1.1, 3.4),
    );
}

/// Parabolic arch span between two clear abutment piers, parallel to the
/// track. The arc curves up from cap-top level to a peak ~rise metres above,
/// then back down. Discretised as `ARCH_SEGMENTS` short oriented boxes that
/// form a curving beam. Geometry lands in the chunk that contains the arc
/// midpoint so the chunk transform places it correctly in world space.
fn add_arch_span(
    samples: &[PierSample],
    chunk_centers: &[Vec3],
    prev: usize,
    next: usize,
    _pier_bufs: &mut [MeshBuf],
    cap_bufs: &mut [MeshBuf],
) {
    let a = samples[prev].p_world;
    let b = samples[next].p_world;
    let chord = b - a;
    let chord_len = chord.length();
    if chord_len < 1.0 {
        return;
    }
    // Spring the arch from partway down the pier columns so the rise has
    // visible vertical room without ever poking into the deck bottom
    // (y = DECK_Y - 1.4 = 12.6).
    let spring_y = 7.0;
    let max_apex = DECK_Y - 1.6; // ~0.2 m of clearance under the deck bottom
    let max_rise = max_apex - spring_y; // 5.4 m
    let rise = (chord_len * 0.08).clamp(3.0, max_rise);

    let host_chunk = samples[(prev + next) / 2].chunk_index;
    let host_center = chunk_centers[host_chunk];

    // Sample the parabola.
    let arc: Vec<Vec3> = (0..=ARCH_SEGMENTS)
        .map(|k| {
            let u = k as f32 / ARCH_SEGMENTS as f32;
            let on_chord = a + chord * u;
            let h = 4.0 * rise * u * (1.0 - u);
            Vec3::new(on_chord.x, spring_y + h, on_chord.z)
        })
        .collect();

    // The arch beam itself: one oriented box per arc segment.
    for k in 0..ARCH_SEGMENTS {
        let p0 = arc[k];
        let p1 = arc[k + 1];
        let mid = (p0 + p1) * 0.5;
        let dir = p1 - p0;
        let seg_len = dir.length();
        if seg_len < 1e-4 {
            continue;
        }
        let dir = dir / seg_len;
        let yaw = Quat::from_rotation_y(dir.x.atan2(dir.z));
        let horiz = (dir.x * dir.x + dir.z * dir.z).sqrt().max(1e-4);
        let pitch = Quat::from_axis_angle(Vec3::X, -dir.y.atan2(horiz));
        let rot = yaw * pitch;

        // Box size: width across-track 1.0, vertical 1.1, length along arc.
        // Local frame after rotation: local Z = arc tangent, local Y = up-ish,
        // local X = cross-track. So size = (across, vertical, length).
        let size = Vec3::new(1.0, 1.1, seg_len + 0.04);
        let centre_local = mid - host_center;
        append_oriented_box(&mut cap_bufs[host_chunk], centre_local, rot, size);
    }
}

/// Rotated box. Each corner is pushed through `rot` before being added to
/// `center`. The six face windings are chosen so `(b−a) × (d−a)` points
/// outward, matching Bevy's CCW-front + back-cull defaults — derived face by
/// face below.
fn append_oriented_box(buf: &mut MeshBuf, center: Vec3, rot: Quat, size: Vec3) {
    let hx = size.x * 0.5;
    let hy = size.y * 0.5;
    let hz = size.z * 0.5;
    let pos = |x: f32, y: f32, z: f32| center + rot * Vec3::new(x, y, z);
    let c000 = pos(-hx, -hy, -hz);
    let c100 = pos(hx, -hy, -hz);
    let c110 = pos(hx, hy, -hz);
    let c010 = pos(-hx, hy, -hz);
    let c001 = pos(-hx, -hy, hz);
    let c101 = pos(hx, -hy, hz);
    let c111 = pos(hx, hy, hz);
    let c011 = pos(-hx, hy, hz);
    emit_box_faces(buf, c000, c100, c110, c010, c001, c101, c111, c011);
}

/// Axis-aligned box. Same outward-facing winding as `append_oriented_box`.
fn append_box(buf: &mut MeshBuf, center: Vec3, size: Vec3) {
    let hx = size.x * 0.5;
    let hy = size.y * 0.5;
    let hz = size.z * 0.5;
    let c000 = center + Vec3::new(-hx, -hy, -hz);
    let c100 = center + Vec3::new(hx, -hy, -hz);
    let c110 = center + Vec3::new(hx, hy, -hz);
    let c010 = center + Vec3::new(-hx, hy, -hz);
    let c001 = center + Vec3::new(-hx, -hy, hz);
    let c101 = center + Vec3::new(hx, -hy, hz);
    let c111 = center + Vec3::new(hx, hy, hz);
    let c011 = center + Vec3::new(-hx, hy, hz);
    emit_box_faces(buf, c000, c100, c110, c010, c001, c101, c111, c011);
}

/// Shared face emitter for both append_box / append_oriented_box. Each
/// winding was verified by computing `(b−a) × (d−a)` and confirming the
/// outward normal direction (+Y for top, +X for right, etc.).
#[allow(clippy::too_many_arguments)]
fn emit_box_faces(
    buf: &mut MeshBuf,
    c000: Vec3, c100: Vec3, c110: Vec3, c010: Vec3,
    c001: Vec3, c101: Vec3, c111: Vec3, c011: Vec3,
) {
    buf.add_quad(c010, c011, c111, c110); // +Y top
    buf.add_quad(c000, c100, c101, c001); // −Y bottom
    buf.add_quad(c100, c110, c111, c101); // +X right
    buf.add_quad(c001, c011, c010, c000); // −X left
    buf.add_quad(c001, c101, c111, c011); // +Z front
    buf.add_quad(c000, c010, c110, c100); // −Z back
}

fn build_wire(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    route: &Route,
    material: Handle<StandardMaterial>,
) {
    let mut rings = Vec::with_capacity(WIRE_SAMPLES);
    for w in 0..WIRE_SAMPLES {
        let t = w as f32 / (WIRE_SAMPLES - 1) as f32;
        let mut p = route.spline.position(t);
        p.y += WIRE_DY;
        let tan = route.spline.tangent(t);
        let nor = Vec3::new(tan.z, 0.0, -tan.x).normalize_or_zero();
        rings.push(Ring { p, nor });
    }
    let mut buf = MeshBuf::new();
    emit_ribbon(&mut buf, &rings, &wire_ribbon());
    if buf.is_empty() {
        return;
    }
    commands.spawn(PbrBundle {
        mesh: meshes.add(buf.into_mesh()),
        material,
        ..default()
    });
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
    commands.spawn(PbrBundle {
        mesh: meshes.add(buf.into_mesh()),
        material,
        transform: Transform::from_translation(center),
        ..default()
    });
}

// Mesh buffer -------------------------------------------------------------

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

    /// Append a flat-shaded quad. Normal is `(b-a) × (d-a)`, which under the
    /// verified ribbon windings points outward from the strip.
    fn add_quad(&mut self, a: Vec3, b: Vec3, c: Vec3, d: Vec3) {
        let n = (b - a).cross(d - a).normalize_or_zero();
        let base = self.positions.len() as u32;
        for p in [a, b, c, d] {
            self.positions.push([p.x, p.y, p.z]);
        }
        for _ in 0..4 {
            self.normals.push([n.x, n.y, n.z]);
        }
        self.uvs
            .extend_from_slice(&[[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]]);
        self.indices
            .extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
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
