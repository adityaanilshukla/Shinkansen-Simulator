//! Real Tokyo geometry around Tokyo Station.
//!
//! Loads the OSM dump baked into `src/osm_data.rs` and turns each building's
//! polygon footprint into an extruded prism at its actual height. Parks and
//! water polygons are rendered as flat colored patches. All buildings of one
//! class are merged into a single mesh so the scene stays at a handful of
//! draw calls regardless of how many buildings the source has.
//!
//! The buildings within this area replace the procedural city/houses; we
//! expose `OsmBbox` so those modules can skip placement inside.

use std::collections::HashMap;

use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};
use bevy::render::render_asset::RenderAssetUsages;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::render::texture::{
    ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor,
};

use crate::osm_data::{
    OSM_BUILDINGS, OSM_PARKS, OSM_POINTS, OSM_ROADS, OSM_TILE_BBOXES, OSM_WATER_LINES,
    OSM_WATER_POLYGONS,
};
use crate::route::CorridorMask;
use crate::water::WaterMask;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum BuildingClass {
    House,
    LowApt,
    Apt,
    Office,
    Skyscraper,
    Complex,
}

const CLASS_COUNT: usize = 6;
const VARIANT_COUNT: usize = 5;

/// Window bay and floor height in metres, used to compute UV scaling so a
/// window stays the same physical size on every building.
const BAY_M: f32 = 3.6;
const FLOOR_M: f32 = 3.3;
/// The texture itself is `COLS x ROWS` windows.
const TILE_COLS: u32 = 4;
const TILE_ROWS: u32 = 8;
const TILE_CELL: u32 = 32;
const TILE_W: u32 = TILE_COLS * TILE_CELL;
const TILE_H: u32 = TILE_ROWS * TILE_CELL;

const BUILDING_CLASSES: [BuildingClass; CLASS_COUNT] = [
    BuildingClass::House,
    BuildingClass::LowApt,
    BuildingClass::Apt,
    BuildingClass::Office,
    BuildingClass::Skyscraper,
    BuildingClass::Complex,
];

#[derive(Copy, Clone)]
struct FacadeSpec {
    style: FacadeStyle,
    wall: [u8; 3],
    glassy: bool,
    roughness: f32,
    metallic: f32,
}

#[derive(Copy, Clone, PartialEq, Eq)]
enum FacadeStyle {
    Punched,
    Curtain,
    Ribbon,
}

/// 5 facade specs per class — varied styles, colors, materials — so a row of
/// buildings on one street doesn't look like the same one cloned.
fn variants(class: BuildingClass) -> [FacadeSpec; VARIANT_COUNT] {
    use FacadeStyle::*;
    match class {
        // Houses: warm matte tones, mostly punched.
        BuildingClass::House => [
            FacadeSpec { style: Punched, wall: [217, 211, 192], glassy: false, roughness: 0.85, metallic: 0.0 },
            FacadeSpec { style: Punched, wall: [205, 188, 168], glassy: false, roughness: 0.88, metallic: 0.0 },
            FacadeSpec { style: Punched, wall: [180, 158, 128], glassy: false, roughness: 0.9,  metallic: 0.0 },
            FacadeSpec { style: Punched, wall: [196, 200, 196], glassy: false, roughness: 0.86, metallic: 0.0 },
            FacadeSpec { style: Punched, wall: [168, 178, 188], glassy: false, roughness: 0.85, metallic: 0.05 },
        ],
        // Low apartments.
        BuildingClass::LowApt => [
            FacadeSpec { style: Punched, wall: [197, 195, 188], glassy: false, roughness: 0.82, metallic: 0.05 },
            FacadeSpec { style: Punched, wall: [158, 162, 168], glassy: false, roughness: 0.84, metallic: 0.05 },
            FacadeSpec { style: Ribbon,  wall: [189, 176, 160], glassy: false, roughness: 0.82, metallic: 0.05 },
            FacadeSpec { style: Punched, wall: [173, 167, 152], glassy: false, roughness: 0.84, metallic: 0.05 },
            FacadeSpec { style: Ribbon,  wall: [134, 142, 150], glassy: false, roughness: 0.78, metallic: 0.1 },
        ],
        // Mid-rise apartments.
        BuildingClass::Apt => [
            FacadeSpec { style: Punched, wall: [162, 168, 173], glassy: false, roughness: 0.78, metallic: 0.1 },
            FacadeSpec { style: Ribbon,  wall: [111, 120, 132], glassy: false, roughness: 0.78, metallic: 0.1 },
            FacadeSpec { style: Ribbon,  wall: [156, 142, 122], glassy: false, roughness: 0.8,  metallic: 0.1 },
            FacadeSpec { style: Punched, wall: [188, 184, 173], glassy: false, roughness: 0.8,  metallic: 0.1 },
            FacadeSpec { style: Curtain, wall: [148, 156, 168], glassy: true,  roughness: 0.5,  metallic: 0.25 },
        ],
        // Offices.
        BuildingClass::Office => [
            FacadeSpec { style: Ribbon,  wall: [184, 181, 168], glassy: true,  roughness: 0.5,  metallic: 0.3 },
            FacadeSpec { style: Curtain, wall: [158, 182, 198], glassy: true,  roughness: 0.4,  metallic: 0.35 },
            FacadeSpec { style: Curtain, wall: [183, 168, 142], glassy: true,  roughness: 0.45, metallic: 0.3 },
            FacadeSpec { style: Ribbon,  wall: [128, 138, 148], glassy: true,  roughness: 0.45, metallic: 0.35 },
            FacadeSpec { style: Curtain, wall: [128, 168, 152], glassy: true,  roughness: 0.4,  metallic: 0.4 },
        ],
        // Skyscrapers.
        BuildingClass::Skyscraper => [
            FacadeSpec { style: Curtain, wall: [104, 130, 152], glassy: true,  roughness: 0.28, metallic: 0.55 },
            FacadeSpec { style: Curtain, wall: [88,  100, 116], glassy: true,  roughness: 0.25, metallic: 0.6  },
            FacadeSpec { style: Curtain, wall: [142, 168, 184], glassy: true,  roughness: 0.3,  metallic: 0.5  },
            FacadeSpec { style: Curtain, wall: [186, 168, 130], glassy: true,  roughness: 0.32, metallic: 0.5 },
            FacadeSpec { style: Ribbon,  wall: [108, 124, 138], glassy: true,  roughness: 0.32, metallic: 0.5 },
        ],
        // Stations / malls.
        BuildingClass::Complex => [
            FacadeSpec { style: Ribbon,  wall: [212, 209, 199], glassy: false, roughness: 0.78, metallic: 0.1 },
            FacadeSpec { style: Ribbon,  wall: [186, 176, 158], glassy: false, roughness: 0.8,  metallic: 0.1 },
            FacadeSpec { style: Punched, wall: [196, 192, 184], glassy: false, roughness: 0.8,  metallic: 0.1 },
            FacadeSpec { style: Ribbon,  wall: [152, 162, 172], glassy: true,  roughness: 0.55, metallic: 0.25 },
            FacadeSpec { style: Curtain, wall: [178, 168, 152], glassy: false, roughness: 0.78, metallic: 0.1 },
        ],
    }
}

pub struct OsmBuilding {
    pub start: u32,
    pub count: u32,
    pub class: BuildingClass,
    pub height: f32,
}

pub struct OsmRing {
    pub start: u32,
    pub count: u32,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum RoadClass {
    /// motorway, trunk
    Major,
    /// primary
    Primary,
    /// secondary, tertiary
    Secondary,
    /// residential, unclassified
    Local,
}

impl RoadClass {
    /// Half-width in metres. Real-world dense Tokyo: residential lanes are
    /// ~3 m wide, arterials 10-14 m. These values give a believable network
    /// without crashing into building footprints.
    pub fn half_width(self) -> f32 {
        match self {
            RoadClass::Major => 6.0,
            RoadClass::Primary => 4.0,
            RoadClass::Secondary => 2.8,
            RoadClass::Local => 1.6,
        }
    }
}

pub struct OsmRoad {
    pub start: u32,
    pub count: u32,
    pub class: RoadClass,
}

/// World-space bounding rectangles of each OSM tile. The lamp placement reads
/// this so it doesn't drop lamps onto real-data streets that don't match the
/// hand-coded procedural arterials. `contains` is true if the point is inside
/// any tile.
#[derive(Resource)]
pub struct OsmBbox {
    rects: Vec<(f32, f32, f32, f32)>,
}

impl OsmBbox {
    pub fn contains(&self, x: f32, z: f32) -> bool {
        self.rects
            .iter()
            .any(|&(min_x, min_z, max_x, max_z)| x >= min_x && x <= max_x && z >= min_z && z <= max_z)
    }
}

/// Shared OSM road spatial mask, exposed so external systems (lamps, future
/// signage) can avoid placing things on streets we draw from the OSM data.
#[derive(Resource)]
pub struct OsmRoads {
    mask: OsmRoadMask,
}

impl OsmRoads {
    pub fn near(&self, x: f32, z: f32, margin: f32) -> bool {
        self.mask.near(x, z, margin)
    }
}

/// Shared OSM water mask — every river / canal / pond from the OSM tiles.
/// Trees consult this so they don't spawn in waterways the hand-coded
/// `WaterMask` doesn't know about.
#[derive(Resource)]
pub struct OsmWaters {
    mask: OsmWaterMask,
}

impl OsmWaters {
    pub fn in_water(&self, x: f32, z: f32) -> bool {
        self.mask.in_water(x, z)
    }
}

pub struct TokyoPlugin;

impl Plugin for TokyoPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(make_bbox())
            .insert_resource(OsmRoads {
                mask: OsmRoadMask::build(),
            })
            .insert_resource(OsmWaters {
                mask: OsmWaterMask::build(),
            })
            .add_systems(Startup, spawn_tokyo);
    }
}

fn make_bbox() -> OsmBbox {
    OsmBbox {
        rects: OSM_TILE_BBOXES.to_vec(),
    }
}

fn spawn_tokyo(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    corridor: Res<CorridorMask>,
    water: Res<WaterMask>,
    osm_roads: Res<OsmRoads>,
    osm_waters: Res<OsmWaters>,
) {
    let road_mask = &osm_roads.mask;
    let osm_water_mask = &osm_waters.mask;

    // 18 facade materials (6 classes × 3 variants) + one grey roof material.
    let mut facade_mats: [[Handle<StandardMaterial>; VARIANT_COUNT]; CLASS_COUNT] =
        std::array::from_fn(|_| std::array::from_fn(|_| Handle::default()));
    for (ci, &class) in BUILDING_CLASSES.iter().enumerate() {
        let specs = variants(class);
        for vi in 0..VARIANT_COUNT {
            let seed = 0xFAC0 ^ ((ci as u64) << 8) ^ (vi as u64);
            facade_mats[ci][vi] =
                make_facade_material(specs[vi], seed, &mut materials, &mut images);
        }
    }
    let roof_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.46, 0.47, 0.48),
        perceptual_roughness: 0.92,
        metallic: 0.0,
        ..default()
    });

    let mut buffers: Vec<Vec<MeshBuf>> = (0..CLASS_COUNT)
        .map(|_| (0..VARIANT_COUNT).map(|_| MeshBuf::new()).collect())
        .collect();
    let mut roof_buf = MeshBuf::new();

    let mut dropped_track = 0usize;
    let mut dropped_road = 0usize;
    let mut kept = 0usize;
    for b in OSM_BUILDINGS.iter() {
        let pts = ring_points(b.start, b.count);
        if pts.len() < 3 {
            continue;
        }
        if pts.iter().any(|p| corridor.near_track(p.x, p.y)) {
            dropped_track += 1;
            continue;
        }
        // TODO(buildings-on-roads): edge sampling catches most cases but
        // some long buildings still poke onto streets. Possibly need a
        // sub-metre rasterisation of the polygon, or shrink each footprint
        // by a 0.5 m inset before extruding.
        if polygon_touches_road(&pts, road_mask, -0.4) {
            dropped_road += 1;
            continue;
        }
        kept += 1;
        let variant = pick_variant(b);
        let buf = &mut buffers[b.class as usize][variant];
        extrude_sides(buf, &pts, b.height);

        // Architectural bands — share the facade material so they go in `buf`.
        if matches!(b.class, BuildingClass::Office | BuildingClass::Skyscraper)
            && b.height > 40.0
        {
            add_podium_base(buf, &pts, 4.0, 0.08);
        }
        if b.class == BuildingClass::Apt
            && b.height > 25.0
            && matches!(variant, 1 | 2 | 4)
        {
            add_balcony_bands(buf, &pts, b.height, 0.4, 14.4, 6.0);
        }
        if b.class == BuildingClass::Office && b.height > 8.0 {
            add_awning_band(buf, &pts, 3.5, 1.0, 0.3);
        }

        // TODO(house-roofs): every building gets a flat slab roof for now. An
        // earlier attempt at pitched gable/hip roofs on short houses looked
        // bad enough that it was pulled; this should be redone with proper
        // polygon-aware roof shells (handle L-shapes / U-shapes / concave
        // footprints, not just bbox-aligned prisms).
        extrude_roof(&mut roof_buf, &pts, b.height);
        // Cornice / parapet band on anything mid-rise or taller — gives each
        // building a visible top edge instead of a clean knife cut.
        if b.height > 18.0 {
            add_cornice(&mut roof_buf, &pts, b.height);
        }
        // Roof clutter (water tanks, AC boxes, vent fans, pipes, equipment)
        // on mid-rise and up. Deterministic from the building's OSM_POINTS
        // address so it stays the same run-to-run.
        if b.height > 16.0 {
            add_roof_clutter(&mut roof_buf, &pts, b.height, b.start);
        }
        // ~10% of skyscrapers get an antenna on top so the skyline isn't
        // flat at the top.
        if b.class == BuildingClass::Skyscraper
            && b.height > 80.0
            && (b.start.wrapping_mul(0xCAFE_BABE) % 10) == 0
        {
            add_antenna(&mut roof_buf, &pts, b.height);
        }
    }
    info!(
        "OSM buildings: kept {} (dropped {} on track, {} on road)",
        kept, dropped_track, dropped_road
    );

    for (ci, class_bufs) in buffers.into_iter().enumerate() {
        for (vi, buf) in class_bufs.into_iter().enumerate() {
            if buf.is_empty() {
                continue;
            }
            commands.spawn(PbrBundle {
                mesh: meshes.add(buf.into_mesh()),
                material: facade_mats[ci][vi].clone(),
                ..default()
            });
        }
    }
    if !roof_buf.is_empty() {
        commands.spawn(PbrBundle {
            mesh: meshes.add(roof_buf.into_mesh()),
            material: roof_mat,
            ..default()
        });
    }

    // Parks: flat green polygons just above ground.
    let park_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.37, 0.54, 0.32),
        perceptual_roughness: 0.95,
        ..default()
    });
    let mut park_buf = MeshBuf::new();
    for r in OSM_PARKS.iter() {
        let pts = ring_points(r.start, r.count);
        if pts.len() >= 3 {
            fill(&mut park_buf, &pts, 0.18);
        }
    }
    if !park_buf.is_empty() {
        commands.spawn(PbrBundle {
            mesh: meshes.add(park_buf.into_mesh()),
            material: park_mat,
            ..default()
        });
    }

    // OSM water polygons (moats, ponds, wide rivers).
    let water_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.27, 0.50, 0.67),
        perceptual_roughness: 0.18,
        metallic: 0.1,
        ..default()
    });
    let mut water_buf = MeshBuf::new();
    for r in OSM_WATER_POLYGONS.iter() {
        let pts = ring_points(r.start, r.count);
        if pts.len() >= 3 {
            fill(&mut water_buf, &pts, 0.32);
        }
    }
    // OSM waterway centerlines rendered as thick ribbons. These are small
    // streams and canals not in the hand-coded major rivers.
    let stream_hw: f32 = 3.5;
    for r in OSM_WATER_LINES.iter() {
        let pts = ring_points(r.start, r.count);
        if pts.len() >= 2 {
            unfiltered_ribbon(&mut water_buf, &pts, stream_hw, 0.32);
        }
    }
    if !water_buf.is_empty() {
        commands.spawn(PbrBundle {
            mesh: meshes.add(water_buf.into_mesh()),
            material: water_mat,
            ..default()
        });
    }

    // OSM highways. Per-class widths; raised above water; segments whose
    // midpoint falls in any water feature (hand-coded or OSM) are dropped.
    let road_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.235, 0.252, 0.278),
        perceptual_roughness: 0.95,
        ..default()
    });
    let road_y: f32 = 0.50;
    let mut road_buf = MeshBuf::new();
    for r in OSM_ROADS.iter() {
        let pts = ring_points(r.start, r.count);
        if pts.len() >= 2 {
            ribbon(
                &mut road_buf,
                &pts,
                r.class.half_width(),
                road_y,
                &water,
                osm_water_mask,
            );
        }
    }
    if !road_buf.is_empty() {
        commands.spawn(PbrBundle {
            mesh: meshes.add(road_buf.into_mesh()),
            material: road_mat,
            ..default()
        });
    }

    // Yellow centreline ribbon for Major + Primary roads, just above the
    // road surface. Plays the role of a painted lane divider at the resolution
    // we draw at.
    let lane_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.96, 0.78, 0.26),
        perceptual_roughness: 0.85,
        ..default()
    });
    let mut lane_buf = MeshBuf::new();
    for r in OSM_ROADS.iter() {
        if !matches!(r.class, RoadClass::Major | RoadClass::Primary) {
            continue;
        }
        let pts = ring_points(r.start, r.count);
        if pts.len() >= 2 {
            ribbon(&mut lane_buf, &pts, 0.12, road_y + 0.01, &water, osm_water_mask);
        }
    }
    if !lane_buf.is_empty() {
        commands.spawn(PbrBundle {
            mesh: meshes.add(lane_buf.into_mesh()),
            material: lane_mat,
            ..default()
        });
    }

    // Random trees inside park polygons. Rejection-sampled inside the
    // polygon bbox, density ~1 per 60 m^2.
    spawn_park_trees(&mut commands, &mut meshes, &mut materials);
}

/// Drop a believable number of trees inside each OSM park / garden polygon.
fn spawn_park_trees(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) {
    let trunk_mesh = meshes.add(Cylinder {
        radius: 0.32,
        half_height: 1.2,
    });
    let canopy_mesh = meshes.add(Sphere::new(1.0).mesh().ico(2).unwrap());
    let trunk_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.42, 0.31, 0.21),
        perceptual_roughness: 0.9,
        ..default()
    });
    let canopy_palette: Vec<Handle<StandardMaterial>> = [
        (0.31, 0.50, 0.30),
        (0.39, 0.55, 0.32),
        (0.45, 0.60, 0.35),
        (0.28, 0.46, 0.28),
        (0.34, 0.52, 0.33),
        (0.51, 0.64, 0.39),
    ]
    .iter()
    .map(|&(r, g, b)| {
        materials.add(StandardMaterial {
            base_color: Color::srgb(r, g, b),
            perceptual_roughness: 0.9,
            ..default()
        })
    })
    .collect();

    let mut rng = fastrand::Rng::with_seed(0x70A2_F00D);
    let mut placed = 0usize;
    for r in OSM_PARKS.iter() {
        let pts = ring_points(r.start, r.count);
        if pts.len() < 3 {
            continue;
        }
        let (mut min_x, mut min_z, mut max_x, mut max_z) =
            (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
        for p in &pts {
            if p.x < min_x {
                min_x = p.x;
            }
            if p.x > max_x {
                max_x = p.x;
            }
            if p.y < min_z {
                min_z = p.y;
            }
            if p.y > max_z {
                max_z = p.y;
            }
        }
        let bbox_area = (max_x - min_x) * (max_z - min_z);
        // Try this many candidate positions; only the ones inside the polygon
        // actually spawn a tree.
        let attempts = ((bbox_area / 60.0) as usize).clamp(4, 200);
        for _ in 0..attempts {
            let x = rng.f32() * (max_x - min_x) + min_x;
            let z = rng.f32() * (max_z - min_z) + min_z;
            if !point_in_poly(&pts, x, z) {
                continue;
            }
            let s = 0.85 + rng.f32() * 0.65;
            let trunk_y = 1.2 * s;
            commands.spawn(PbrBundle {
                mesh: trunk_mesh.clone(),
                material: trunk_mat.clone(),
                transform: Transform::from_xyz(x, trunk_y, z)
                    .with_scale(Vec3::splat(s)),
                ..default()
            });
            let rad = 2.0 + rng.f32() * 1.6;
            let height_scale = 0.9 + rng.f32() * 0.4;
            let canopy_mat = canopy_palette[rng.usize(..canopy_palette.len())].clone();
            commands.spawn(PbrBundle {
                mesh: canopy_mesh.clone(),
                material: canopy_mat,
                transform: Transform::from_xyz(x, trunk_y * 2.0 + rad * 0.6, z)
                    .with_scale(Vec3::new(rad, rad * height_scale, rad)),
                ..default()
            });
            placed += 1;
        }
    }
    if placed > 0 {
        info!("OSM park trees: spawned {}", placed);
    }
}

fn ring_points(start: u32, count: u32) -> Vec<Vec2> {
    let mut out = Vec::with_capacity(count as usize);
    for i in 0..count as usize {
        let (x, z) = OSM_POINTS[start as usize + i];
        out.push(Vec2::new(x, z));
    }
    out
}

/// Build the facade material for one (class, variant). Texture is a 128x256
/// RGBA tile in the style indicated by the spec; the sampler repeats so the
/// UV-scaled facade tiles cleanly across the building face.
fn make_facade_material(
    spec: FacadeSpec,
    seed: u64,
    materials: &mut Assets<StandardMaterial>,
    images: &mut Assets<Image>,
) -> Handle<StandardMaterial> {
    let tex = images.add(make_facade_texture(spec, seed));
    materials.add(StandardMaterial {
        base_color: Color::WHITE,
        base_color_texture: Some(tex),
        perceptual_roughness: spec.roughness,
        metallic: spec.metallic,
        ..default()
    })
}

/// Port of the reference's `facadeTexture(base, glassy, style)`. Three styles
/// (curtain / ribbon / punched), all painted on a 4x8-window tile so one
/// repeat covers 14.4 m horizontally and 26.4 m vertically.
fn make_facade_texture(spec: FacadeSpec, seed: u64) -> Image {
    let mut data = vec![0_u8; (TILE_W * TILE_H * 4) as usize];
    let pane = if spec.glassy {
        [38_u8, 52, 67]
    } else {
        [26_u8, 31, 38]
    };
    let lit = [236_u8, 223, 179];

    // Fill base wall colour.
    for y in 0..TILE_H {
        for x in 0..TILE_W {
            let off = ((y * TILE_W + x) * 4) as usize;
            data[off] = spec.wall[0];
            data[off + 1] = spec.wall[1];
            data[off + 2] = spec.wall[2];
            data[off + 3] = 255;
        }
    }

    let cs = TILE_CELL;
    let mut rng = fastrand::Rng::with_seed(seed);

    match spec.style {
        FacadeStyle::Curtain => {
            // Vertical pane between each column.
            for cc in 0..TILE_COLS {
                blend_rect(
                    &mut data, cc * cs + 3, 2, cs - 6, TILE_H - 4, pane, 0.82,
                );
            }
            // Faint horizontal floor lines.
            for rr in 0..TILE_ROWS {
                blend_rect(&mut data, 0, rr * cs + 1, TILE_W, 2, [0, 0, 0], 0.18);
            }
            // Occasional lit window.
            for rr in 0..TILE_ROWS {
                for cc in 0..TILE_COLS {
                    if rng.f32() < 0.05 {
                        blend_rect(
                            &mut data,
                            cc * cs + 4,
                            rr * cs + 5,
                            cs - 8,
                            cs - 12,
                            lit,
                            1.0,
                        );
                    }
                }
            }
        }
        FacadeStyle::Ribbon => {
            // Horizontal pane band on each floor.
            for rr in 0..TILE_ROWS {
                blend_rect(
                    &mut data, 0, rr * cs + 7, TILE_W, cs - 14, pane, 0.80,
                );
                // Sunlit lintel just above + shadowed sill just below each
                // band: reads as a recessed strip window.
                if rr * cs + 6 < TILE_H {
                    blend_rect(&mut data, 0, rr * cs + 6, TILE_W, 1, [250, 244, 220], 0.40);
                }
                blend_rect(&mut data, 0, rr * cs + cs - 8, TILE_W, 2, [0, 0, 0], 0.42);
            }
            // Faint vertical mullions.
            for cc in 0..TILE_COLS {
                blend_rect(&mut data, cc * cs + cs / 2 - 1, 0, 2, TILE_H, [0, 0, 0], 0.12);
            }
            for rr in 0..TILE_ROWS {
                for cc in 0..TILE_COLS {
                    if rng.f32() < 0.05 {
                        blend_rect(
                            &mut data,
                            cc * cs + 4,
                            rr * cs + 8,
                            cs - 8,
                            cs - 16,
                            lit,
                            1.0,
                        );
                    }
                }
            }
        }
        FacadeStyle::Punched => {
            for rr in 0..TILE_ROWS {
                for cc in 0..TILE_COLS {
                    let win_x = cc * cs + 6;
                    let win_y = rr * cs + 7;
                    let win_w = cs - 12;
                    let win_h = cs - 14;
                    let is_lit = rng.f32() < 0.06;
                    if is_lit {
                        blend_rect(&mut data, win_x, win_y, win_w, win_h, lit, 1.0);
                    } else {
                        // Reference: pane at alpha 0.6..0.88 for visual depth.
                        let a = 0.6 + rng.f32() * 0.28;
                        blend_rect(&mut data, win_x, win_y, win_w, win_h, pane, a);
                    }
                    // Window depth: shadow at the recess top + sunlit sill
                    // just below the opening. Visually inset.
                    if win_y > 0 {
                        blend_rect(&mut data, win_x, win_y - 1, win_w, 1, [250, 244, 220], 0.40);
                    }
                    blend_rect(&mut data, win_x, win_y, win_w, 2, [0, 0, 0], 0.50);
                    blend_rect(&mut data, win_x, win_y + win_h, win_w, 1, [255, 250, 220], 0.45);
                    blend_rect(&mut data, win_x, win_y + win_h - 2, win_w, 2, [0, 0, 0], 0.30);
                }
            }
            // Subtle floor lines between every row, like a slab edge.
            for rr in 1..TILE_ROWS {
                blend_rect(&mut data, 0, rr * cs, TILE_W, 1, [0, 0, 0], 0.18);
            }
        }
    }

    // Cornice band every 4 floors: a slightly darker stripe to break up the
    // vertical repetition on tall buildings.
    for rr in (4..TILE_ROWS).step_by(4) {
        blend_rect(&mut data, 0, rr * cs - 2, TILE_W, 4, [0, 0, 0], 0.25);
    }
    // Corner banding — thin vertical stripes at the tile edges so when the
    // texture tiles across a face there are subtle pilasters every 4 bays.
    blend_rect(&mut data, 0, 0, 2, TILE_H, [0, 0, 0], 0.2);
    blend_rect(&mut data, TILE_W - 2, 0, 2, TILE_H, [0, 0, 0], 0.2);

    let mut img = Image::new(
        Extent3d {
            width: TILE_W,
            height: TILE_H,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    );
    img.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::Repeat,
        address_mode_v: ImageAddressMode::Repeat,
        mag_filter: ImageFilterMode::Linear,
        min_filter: ImageFilterMode::Linear,
        ..default()
    });
    img
}

/// Source-over alpha blend a colour into a rectangle of the texture buffer.
fn blend_rect(data: &mut [u8], x: u32, y: u32, w: u32, h: u32, c: [u8; 3], a: f32) {
    let a = a.clamp(0.0, 1.0);
    let inv = 1.0 - a;
    for yy in y..y.saturating_add(h).min(TILE_H) {
        for xx in x..x.saturating_add(w).min(TILE_W) {
            let off = ((yy * TILE_W + xx) * 4) as usize;
            data[off] = (data[off] as f32 * inv + c[0] as f32 * a) as u8;
            data[off + 1] = (data[off + 1] as f32 * inv + c[1] as f32 * a) as u8;
            data[off + 2] = (data[off + 2] as f32 * inv + c[2] as f32 * a) as u8;
            data[off + 3] = 255;
        }
    }
}

/// Hash the building's address (its (start, count) in OSM_POINTS) to a stable
/// variant index. Same building → same variant across runs.
fn pick_variant(b: &OsmBuilding) -> usize {
    let h = (b.start as usize)
        .wrapping_mul(0x9E37_79B1)
        .wrapping_add((b.count as usize).wrapping_mul(0x85EB_CA77));
    h % VARIANT_COUNT
}

/// Podium base around a tall building: a slightly wider prism wrapping the
/// bottom `podium_h` metres. The expanded ring radiates each polygon vertex
/// outward from the centroid by `1 + expand`. Reuses the facade material by
/// writing into the wall buffer.
fn add_podium_base(buf: &mut MeshBuf, pts: &[Vec2], podium_h: f32, expand: f32) {
    let n = pts.len();
    if n < 3 {
        return;
    }
    let nf = n as f32;
    let mut cx = 0.0;
    let mut cz = 0.0;
    for p in pts {
        cx += p.x;
        cz += p.y;
    }
    cx /= nf;
    cz /= nf;
    let signed: f32 = (0..n)
        .map(|i| {
            let a = pts[i];
            let b = pts[(i + 1) % n];
            a.x * b.y - b.x * a.y
        })
        .sum::<f32>()
        * 0.5;
    let ccw = signed > 0.0;

    let expanded: Vec<Vec2> = pts
        .iter()
        .map(|p| Vec2::new(cx + (p.x - cx) * (1.0 + expand), cz + (p.y - cz) * (1.0 + expand)))
        .collect();

    // One quad per edge from y=0 to y=podium_h on the expanded ring.
    for i in 0..n {
        let a = expanded[i];
        let b = expanded[(i + 1) % n];
        let edge_dx = b.x - a.x;
        let edge_dz = b.y - a.y;
        let outward = if ccw {
            Vec3::new(edge_dz, 0.0, -edge_dx)
        } else {
            Vec3::new(-edge_dz, 0.0, edge_dx)
        }
        .normalize_or_zero();
        let bl = Vec3::new(a.x, 0.0, a.y);
        let br = Vec3::new(b.x, 0.0, b.y);
        let tr = Vec3::new(b.x, podium_h, b.y);
        let tl = Vec3::new(a.x, podium_h, a.y);
        let base = buf.positions.len() as u32;
        for p in [bl, br, tr, tl] {
            buf.positions.push([p.x, p.y, p.z]);
            buf.normals.push([outward.x, outward.y, outward.z]);
        }
        buf.uvs
            .extend_from_slice(&[[0.0, 0.5], [0.5, 0.5], [0.5, 0.0], [0.0, 0.0]]);
        buf.indices
            .extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }
}

/// Balcony / spandrel bands at regular floor intervals. Each band is a
/// thin horizontal flange sticking out `depth` from the wall.
fn add_balcony_bands(
    buf: &mut MeshBuf,
    pts: &[Vec2],
    height: f32,
    depth: f32,
    spacing: f32,
    start_y: f32,
) {
    let n = pts.len();
    if n < 3 {
        return;
    }
    let signed: f32 = (0..n)
        .map(|i| {
            let a = pts[i];
            let b = pts[(i + 1) % n];
            a.x * b.y - b.x * a.y
        })
        .sum::<f32>()
        * 0.5;
    let ccw = signed > 0.0;
    let thick = 0.2_f32;
    let mut y = start_y;
    while y < height - 1.0 {
        for i in 0..n {
            let a = pts[i];
            let b = pts[(i + 1) % n];
            let edge_dx = b.x - a.x;
            let edge_dz = b.y - a.y;
            let outward = if ccw {
                Vec3::new(edge_dz, 0.0, -edge_dx)
            } else {
                Vec3::new(-edge_dz, 0.0, edge_dx)
            }
            .normalize_or_zero();
            // Wall corners (inner) and outer corners offset by outward * depth.
            let ai_low = Vec3::new(a.x, y, a.y);
            let bi_low = Vec3::new(b.x, y, b.y);
            let ao_low = ai_low + outward * depth;
            let bo_low = bi_low + outward * depth;
            let ai_hi = Vec3::new(a.x, y + thick, a.y);
            let bi_hi = Vec3::new(b.x, y + thick, b.y);
            let ao_hi = ai_hi + outward * depth;
            let bo_hi = bi_hi + outward * depth;
            // Top, outer-front, bottom — endcaps skipped.
            push_band_quad(buf, ai_hi, bi_hi, bo_hi, ao_hi, [0.0, 1.0, 0.0]);
            push_band_quad(
                buf,
                ao_low,
                bo_low,
                bo_hi,
                ao_hi,
                [outward.x, outward.y, outward.z],
            );
            push_band_quad(buf, ao_low, ai_low, bi_low, bo_low, [0.0, -1.0, 0.0]);
        }
        y += spacing;
    }
}

/// Single horizontal awning slab around the perimeter at `y`. Same shape as
/// one balcony band; reused for the ground-floor awning on commercial
/// buildings.
fn add_awning_band(buf: &mut MeshBuf, pts: &[Vec2], y: f32, depth: f32, thick: f32) {
    let n = pts.len();
    if n < 3 {
        return;
    }
    let signed: f32 = (0..n)
        .map(|i| {
            let a = pts[i];
            let b = pts[(i + 1) % n];
            a.x * b.y - b.x * a.y
        })
        .sum::<f32>()
        * 0.5;
    let ccw = signed > 0.0;
    for i in 0..n {
        let a = pts[i];
        let b = pts[(i + 1) % n];
        let edge_dx = b.x - a.x;
        let edge_dz = b.y - a.y;
        let outward = if ccw {
            Vec3::new(edge_dz, 0.0, -edge_dx)
        } else {
            Vec3::new(-edge_dz, 0.0, edge_dx)
        }
        .normalize_or_zero();
        let ai_low = Vec3::new(a.x, y, a.y);
        let bi_low = Vec3::new(b.x, y, b.y);
        let ao_low = ai_low + outward * depth;
        let bo_low = bi_low + outward * depth;
        let ai_hi = Vec3::new(a.x, y + thick, a.y);
        let bi_hi = Vec3::new(b.x, y + thick, b.y);
        let ao_hi = ai_hi + outward * depth;
        let bo_hi = bi_hi + outward * depth;
        push_band_quad(buf, ai_hi, bi_hi, bo_hi, ao_hi, [0.0, 1.0, 0.0]);
        push_band_quad(
            buf,
            ao_low,
            bo_low,
            bo_hi,
            ao_hi,
            [outward.x, outward.y, outward.z],
        );
        push_band_quad(buf, ao_low, ai_low, bi_low, bo_low, [0.0, -1.0, 0.0]);
    }
}

/// Helper for bands — emit a quad with a hand-provided normal (the cross-
/// product approach in `add_quad` could give the wrong sign on degenerate
/// strips). UV pinned to a small wall texel.
fn push_band_quad(buf: &mut MeshBuf, a: Vec3, b: Vec3, c: Vec3, d: Vec3, normal: [f32; 3]) {
    let base = buf.positions.len() as u32;
    for p in [a, b, c, d] {
        buf.positions.push([p.x, p.y, p.z]);
        buf.normals.push(normal);
    }
    buf.uvs
        .extend_from_slice(&[[0.1, 0.5], [0.4, 0.5], [0.4, 0.3], [0.1, 0.3]]);
    buf.indices
        .extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
}

/// A thin band around the building's top, slightly wider than the wall. Reads
/// as a parapet or cornice and visually caps off the building.
fn add_cornice(buf: &mut MeshBuf, pts: &[Vec2], wall_height: f32) {
    let n = pts.len();
    if n < 3 {
        return;
    }
    // Compute polygon centroid + outward "expand" vector for each vertex.
    let mut centroid = Vec2::ZERO;
    for p in pts {
        centroid += *p;
    }
    centroid /= n as f32;
    let band_top = wall_height + 1.0;
    let band_bot = wall_height - 0.1;
    let outset = 0.4;
    let mut outer: Vec<Vec2> = Vec::with_capacity(n);
    for &p in pts {
        let dir = (p - centroid).normalize_or_zero();
        outer.push(p + dir * outset);
    }
    // Top ring + outer side strip.
    for i in 0..n {
        let a_out = outer[i];
        let b_out = outer[(i + 1) % n];
        let tl = Vec3::new(a_out.x, band_top, a_out.y);
        let tr = Vec3::new(b_out.x, band_top, b_out.y);
        let bl = Vec3::new(a_out.x, band_bot, a_out.y);
        let br = Vec3::new(b_out.x, band_bot, b_out.y);
        // Outer face.
        add_quad(buf, bl, br, tr, tl);
        // Top face (flat).
        let a_in = pts[i];
        let b_in = pts[(i + 1) % n];
        let tl_in = Vec3::new(a_in.x, band_top, a_in.y);
        let tr_in = Vec3::new(b_in.x, band_top, b_in.y);
        add_quad(buf, tl_in, tr_in, tr, tl);
    }
}

/// Heavily detailed rooftop equipment: water tanks, AC units, ventilation
/// fans, pipe stacks, electrical boxes. Each feature has its own independent
/// probability roll so most buildings end up with 3+ pieces. All deterministic
/// from the building seed so the same building looks the same every run.
fn add_roof_clutter(buf: &mut MeshBuf, pts: &[Vec2], roof_y: f32, seed: u32) {
    let n = pts.len();
    if n < 3 {
        return;
    }
    let nf = n as f32;
    let mut cx = 0.0;
    let mut cz = 0.0;
    let mut xmin = f32::INFINITY;
    let mut xmax = f32::NEG_INFINITY;
    let mut zmin = f32::INFINITY;
    let mut zmax = f32::NEG_INFINITY;
    for p in pts {
        cx += p.x;
        cz += p.y;
        if p.x < xmin { xmin = p.x; }
        if p.x > xmax { xmax = p.x; }
        if p.y < zmin { zmin = p.y; }
        if p.y > zmax { zmax = p.y; }
    }
    cx /= nf;
    cz /= nf;

    // Shoelace area — drop small buildings entirely so we don't spill
    // clutter outside tight footprints.
    let signed: f32 = (0..n)
        .map(|i| {
            let a = pts[i];
            let b = pts[(i + 1) % n];
            a.x * b.y - b.x * a.y
        })
        .sum::<f32>()
        * 0.5;
    let area = signed.abs();
    const AREA_MIN: f32 = 200.0;
    if area < AREA_MIN {
        return;
    }

    // Independent hash channels per feature so the rolls aren't correlated.
    let h = |k: u32| -> u32 {
        seed.wrapping_mul(0x9E37_79B1)
            .wrapping_add(k.wrapping_mul(0x85EB_CA77))
            .wrapping_add(0xC2B2_AE3D)
    };
    // Pick a position inside the polygon bbox, shrunk by the feature's
    // half-radius so the feature stays on the roof. Fall back to centroid
    // for L-shapes / concave footprints where the bbox pick lands outside.
    let place = |hh: u32, feat_r: f32| -> (f32, f32) {
        let inset = feat_r + 0.5;
        let xlo = (xmin + inset).min(cx);
        let xhi = (xmax - inset).max(cx);
        let zlo = (zmin + inset).min(cz);
        let zhi = (zmax - inset).max(cz);
        let u = (hh % 1000) as f32 / 1000.0;
        let v = ((hh >> 10) % 1000) as f32 / 1000.0;
        let x = xlo + (xhi - xlo) * u;
        let z = zlo + (zhi - zlo) * v;
        if point_in_poly(pts, x, z) {
            (x, z)
        } else {
            (cx, cz)
        }
    };
    let box_radius = |w: f32, d: f32| -> f32 { 0.5 * (w * w + d * d).sqrt() };

    // 1) Water tank (hex prism). ~80% chance, taller and wider.
    let h1 = h(1);
    if h1 % 10 < 8 {
        let r = 1.4 + (h1 % 8) as f32 * 0.18;
        let height = 2.5 + (h1 % 6) as f32 * 0.4;
        let (px, pz) = place(h1, r);
        add_hex_prism(buf, px, pz, roof_y, r, height);
    }
    // 2) Big AC / mechanical block. ~90% chance.
    let h2 = h(2);
    if h2 % 10 < 9 {
        let w = 2.3 + (h2 % 7) as f32 * 0.3;
        let d = 1.9 + (h2 % 5) as f32 * 0.3;
        let ht = 1.6 + (h2 % 4) as f32 * 0.4;
        let (px, pz) = place(h2, box_radius(w, d));
        add_rooftop_box(buf, px, pz, roof_y, w, ht, d);
    }
    // 3) Second AC unit. ~70% chance.
    let h3 = h(3);
    if h3 % 10 < 7 {
        let w = 1.7 + (h3 % 5) as f32 * 0.2;
        let d = 1.4 + (h3 % 4) as f32 * 0.2;
        let ht = 1.3 + (h3 % 3) as f32 * 0.3;
        let (px, pz) = place(h3, box_radius(w, d));
        add_rooftop_box(buf, px, pz, roof_y, w, ht, d);
    }
    // 4) Ventilation fan (squat hex prism). ~80% chance, bigger radius so it
    // actually reads at distance.
    let h4 = h(4);
    if h4 % 10 < 8 {
        let r = 1.0 + (h4 % 5) as f32 * 0.18;
        let height = 0.5 + (h4 % 4) as f32 * 0.18;
        let (px, pz) = place(h4, r);
        add_hex_prism(buf, px, pz, roof_y, r, height);
    }
    // 5) Vent / exhaust pipe (tall hex prism). ~80% chance, fat enough to
    // see from a distance.
    let h5 = h(5);
    if h5 % 10 < 8 {
        let r = 0.40 + (h5 % 4) as f32 * 0.12;
        let height = 2.5 + (h5 % 5) as f32 * 0.45;
        let (px, pz) = place(h5, r);
        add_hex_prism(buf, px, pz, roof_y, r, height);
    }
    // 6) Tall electrical / equipment box. ~70% chance.
    let h6 = h(6);
    if h6 % 10 < 7 {
        let w = 0.9 + (h6 % 4) as f32 * 0.2;
        let d = 0.7 + (h6 % 3) as f32 * 0.2;
        let ht = 1.4 + (h6 % 4) as f32 * 0.3;
        let (px, pz) = place(h6, box_radius(w, d));
        add_rooftop_box(buf, px, pz, roof_y, w, ht, d);
    }
    // 7) Small low utility box. ~60% chance.
    let h7 = h(7);
    if h7 % 10 < 6 {
        let s = 0.8 + (h7 % 4) as f32 * 0.15;
        let ht = 0.6 + (h7 % 3) as f32 * 0.2;
        let (px, pz) = place(h7, box_radius(s, s));
        add_rooftop_box(buf, px, pz, roof_y, s, ht, s);
    }
}

/// Hexagonal prism, axis-aligned, as a rooftop water tank. Winding is
/// chosen so each side's normal points radially outward and the top fan
/// points +Y — both required for Bevy's CCW front-face / back-cull defaults.
fn add_hex_prism(buf: &mut MeshBuf, cx: f32, cz: f32, base_y: f32, radius: f32, height: f32) {
    let sides = 6;
    let top_y = base_y + height;
    let centre_top = Vec3::new(cx, top_y, cz);
    let step = std::f32::consts::TAU / sides as f32;
    for i in 0..sides {
        let a0 = i as f32 * step;
        let a1 = (i + 1) as f32 * step;
        let (s0, c0) = a0.sin_cos();
        let (s1, c1) = a1.sin_cos();
        let b0 = Vec3::new(cx + radius * c0, base_y, cz + radius * s0);
        let b1 = Vec3::new(cx + radius * c1, base_y, cz + radius * s1);
        let t0 = Vec3::new(cx + radius * c0, top_y, cz + radius * s0);
        let t1 = Vec3::new(cx + radius * c1, top_y, cz + radius * s1);
        // Side face: outward normal. Winding b0 → t0 → t1 → b1 puts the
        // tangent (b1−b0) and the vertical (t0−b0) so (b−a)×(d−a) = +radial.
        add_quad(buf, b0, t0, t1, b1);
        // Top fan: outward normal +Y. t0 → centre → t1.
        add_tri(buf, t0, centre_top, t1);
    }
}

/// Plain rooftop box: w along x, h along y, d along z. Bottom is skipped
/// (sits on roof). Side winding chosen for outward normals.
fn add_rooftop_box(buf: &mut MeshBuf, cx: f32, cz: f32, base_y: f32, w: f32, h: f32, d: f32) {
    let hx = w * 0.5;
    let hd = d * 0.5;
    let top_y = base_y + h;
    let sw = Vec3::new(cx - hx, base_y, cz - hd);
    let se = Vec3::new(cx + hx, base_y, cz - hd);
    let ne = Vec3::new(cx + hx, base_y, cz + hd);
    let nw = Vec3::new(cx - hx, base_y, cz + hd);
    let tsw = Vec3::new(cx - hx, top_y, cz - hd);
    let tse = Vec3::new(cx + hx, top_y, cz - hd);
    let tne = Vec3::new(cx + hx, top_y, cz + hd);
    let tnw = Vec3::new(cx - hx, top_y, cz + hd);
    // For (b-a) × (d-a) to point in the outward direction, the four side
    // windings need to go up first then around.
    add_quad(buf, sw, tsw, tse, se); // -Z face, normal -Z
    add_quad(buf, se, tse, tne, ne); // +X face, normal +X
    add_quad(buf, ne, tne, tnw, nw); // +Z face, normal +Z
    add_quad(buf, nw, tnw, tsw, sw); // -X face, normal -X
    add_quad(buf, tsw, tnw, tne, tse); // top, normal +Y
}

/// Tall thin spike on top of the building centre — reads as a skyscraper
/// antenna at distance.
fn add_antenna(buf: &mut MeshBuf, pts: &[Vec2], top_y: f32) {
    let n = pts.len() as f32;
    let mut cx = 0.0;
    let mut cz = 0.0;
    for p in pts {
        cx += p.x;
        cz += p.y;
    }
    cx /= n;
    cz /= n;
    let r = 0.4_f32;
    let h = top_y + 18.0;
    let sw = Vec3::new(cx - r, top_y, cz - r);
    let se = Vec3::new(cx + r, top_y, cz - r);
    let ne = Vec3::new(cx + r, top_y, cz + r);
    let nw = Vec3::new(cx - r, top_y, cz + r);
    let apex = Vec3::new(cx, h, cz);
    add_tri(buf, sw, se, apex);
    add_tri(buf, se, ne, apex);
    add_tri(buf, ne, nw, apex);
    add_tri(buf, nw, sw, apex);
}

fn add_quad(buf: &mut MeshBuf, a: Vec3, b: Vec3, c: Vec3, d: Vec3) {
    let normal = (b - a).cross(d - a).normalize_or_zero();
    let base = buf.positions.len() as u32;
    for v in [a, b, c, d] {
        buf.positions.push([v.x, v.y, v.z]);
        buf.normals.push([normal.x, normal.y, normal.z]);
        buf.uvs.push([0.0, 0.0]);
    }
    buf.indices
        .extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
}

fn add_tri(buf: &mut MeshBuf, a: Vec3, b: Vec3, c: Vec3) {
    let normal = (b - a).cross(c - a).normalize_or_zero();
    let base = buf.positions.len() as u32;
    for v in [a, b, c] {
        buf.positions.push([v.x, v.y, v.z]);
        buf.normals.push([normal.x, normal.y, normal.z]);
        buf.uvs.push([0.0, 0.0]);
    }
    buf.indices.extend_from_slice(&[base, base + 1, base + 2]);
}

/// Side walls only. UVs are scaled so the tile (4 bays × 8 floors at 3.6 m and
/// 3.3 m respectively) keeps window cells at one window per ~3.6 m horizontal
/// and per ~3.3 m vertical, regardless of building dimensions.
fn extrude_sides(buf: &mut MeshBuf, pts: &[Vec2], height: f32) {
    let n = pts.len();
    let signed: f32 = (0..n)
        .map(|i| {
            let a = pts[i];
            let b = pts[(i + 1) % n];
            a.x * b.y - b.x * a.y
        })
        .sum::<f32>()
        * 0.5;
    let ccw = signed > 0.0;

    // Texture tile covers TILE_ROWS * FLOOR_M metres vertically. Anything
    // shorter than half a tile (~13 m) gets a minimum so the texture isn't
    // squashed into a band.
    let v_max = (height / (TILE_ROWS as f32 * FLOOR_M)).max(0.25);

    for i in 0..n {
        let a = pts[i];
        let b = pts[(i + 1) % n];
        let edge_dx = b.x - a.x;
        let edge_dz = b.y - a.y;
        let edge_len = (edge_dx * edge_dx + edge_dz * edge_dz).sqrt();
        let u_max = (edge_len / (TILE_COLS as f32 * BAY_M)).max(0.25);

        let raw = if ccw {
            Vec3::new(edge_dz, 0.0, -edge_dx)
        } else {
            Vec3::new(-edge_dz, 0.0, edge_dx)
        };
        let normal = raw.normalize_or_zero();

        let base = buf.positions.len() as u32;
        buf.positions.push([a.x, 0.0, a.y]);
        buf.positions.push([b.x, 0.0, b.y]);
        buf.positions.push([b.x, height, b.y]);
        buf.positions.push([a.x, height, a.y]);
        for _ in 0..4 {
            buf.normals.push([normal.x, normal.y, normal.z]);
        }
        buf.uvs.push([0.0, v_max]); // a-low (bottom of texture)
        buf.uvs.push([u_max, v_max]); // b-low
        buf.uvs.push([u_max, 0.0]); // b-high
        buf.uvs.push([0.0, 0.0]); // a-high
        buf.indices
            .extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }
}

/// Roof face. Earcut-triangulated polygon at the building's height. Goes into
/// its own buffer so the roof can use a separate (untextured grey) material.
fn extrude_roof(buf: &mut MeshBuf, pts: &[Vec2], height: f32) {
    let n = pts.len();
    let signed: f32 = (0..n)
        .map(|i| {
            let a = pts[i];
            let b = pts[(i + 1) % n];
            a.x * b.y - b.x * a.y
        })
        .sum::<f32>()
        * 0.5;
    let ccw = signed > 0.0;
    let flat: Vec<f64> = pts.iter().flat_map(|p| [p.x as f64, p.y as f64]).collect();
    let tris = match earcutr::earcut(&flat, &[], 2) {
        Ok(t) => t,
        Err(_) => return,
    };
    let base = buf.positions.len() as u32;
    for p in pts {
        buf.positions.push([p.x, height, p.y]);
        buf.normals.push([0.0, 1.0, 0.0]);
        buf.uvs.push([0.0, 0.0]);
    }
    if ccw {
        for c in tris.chunks(3) {
            buf.indices
                .extend_from_slice(&[base + c[0] as u32, base + c[1] as u32, base + c[2] as u32]);
        }
    } else {
        for c in tris.chunks(3) {
            buf.indices
                .extend_from_slice(&[base + c[0] as u32, base + c[2] as u32, base + c[1] as u32]);
        }
    }
}

/// A flat ribbon along a polyline (roads). Segments whose midpoint sits in
/// either the hand-coded `WaterMask` (Sumida, Arakawa, Tokyo Bay) or the
/// per-tile `OsmWaterMask` (every other river / canal / pond) are skipped,
/// so river-crossing roads stop at the bank rather than sit on the surface.
fn ribbon(
    buf: &mut MeshBuf,
    pts: &[Vec2],
    hw: f32,
    y: f32,
    water: &WaterMask,
    osm_water: &OsmWaterMask,
) {
    push_ribbon_verts(buf, pts, hw, y);
    let n = pts.len();
    let base = buf.positions.len() as u32 - (n as u32) * 2;
    for i in 0..n - 1 {
        let a = pts[i];
        let b = pts[i + 1];
        let mid_x = (a.x + b.x) * 0.5;
        let mid_z = (a.y + b.y) * 0.5;
        if water.in_bay(mid_x, mid_z)
            || water.near_river(mid_x, mid_z, 0.0)
            || osm_water.in_water(mid_x, mid_z)
        {
            continue;
        }
        let k = base + (i as u32) * 2;
        buf.indices
            .extend_from_slice(&[k, k + 1, k + 2, k + 1, k + 3, k + 2]);
    }
}

/// Ribbon without the water cutoff, used for the OSM stream/canal centerlines
/// themselves (we want them drawn as water, not omitted).
fn unfiltered_ribbon(buf: &mut MeshBuf, pts: &[Vec2], hw: f32, y: f32) {
    push_ribbon_verts(buf, pts, hw, y);
    let n = pts.len();
    let base = buf.positions.len() as u32 - (n as u32) * 2;
    for i in 0..n as u32 - 1 {
        let k = base + i * 2;
        buf.indices
            .extend_from_slice(&[k, k + 1, k + 2, k + 1, k + 3, k + 2]);
    }
}

fn push_ribbon_verts(buf: &mut MeshBuf, pts: &[Vec2], hw: f32, y: f32) {
    let n = pts.len();
    for i in 0..n {
        let a = pts[i.saturating_sub(1)];
        let b = pts[(i + 1).min(n - 1)];
        let dir = (b - a).normalize_or_zero();
        let nor = Vec2::new(dir.y, -dir.x);
        let p = pts[i];
        buf.positions.push([p.x + nor.x * hw, y, p.y + nor.y * hw]);
        buf.positions.push([p.x - nor.x * hw, y, p.y - nor.y * hw]);
        buf.normals.push([0.0, 1.0, 0.0]);
        buf.normals.push([0.0, 1.0, 0.0]);
        buf.uvs.push([0.0, 0.0]);
        buf.uvs.push([0.0, 0.0]);
    }
}

/// Sample polygon vertices and edges (every ~2 m) and return true if any
/// sample sits inside the road footprint plus `margin`.
fn polygon_touches_road(pts: &[Vec2], mask: &OsmRoadMask, margin: f32) -> bool {
    let n = pts.len();
    for i in 0..n {
        let a = pts[i];
        let b = pts[(i + 1) % n];
        if mask.near(a.x, a.y, margin) {
            return true;
        }
        let dist = (b - a).length();
        let steps = (dist / 2.0).max(1.0) as usize;
        for s in 1..steps {
            let t = s as f32 / steps as f32;
            let p = a + (b - a) * t;
            if mask.near(p.x, p.y, margin) {
                return true;
            }
        }
    }
    false
}

/// Tests for points inside any OSM water polygon or within `line_hw` of any
/// OSM waterway centerline. Linear scan; the count (~270 features) is small
/// enough that a spatial hash isn't worth it.
struct OsmWaterMask {
    polygons: Vec<Vec<Vec2>>,
    lines: Vec<Vec<Vec2>>,
    line_hw2: f32,
}

impl OsmWaterMask {
    fn build() -> Self {
        let polygons: Vec<Vec<Vec2>> = OSM_WATER_POLYGONS
            .iter()
            .map(|r| ring_points(r.start, r.count))
            .collect();
        let lines: Vec<Vec<Vec2>> = OSM_WATER_LINES
            .iter()
            .map(|r| ring_points(r.start, r.count))
            .collect();
        // Slightly larger than the rendered stream ribbon (3.5 m hw) so the
        // road filter cuts at the visible water edge.
        let line_hw = 4.5_f32;
        Self {
            polygons,
            lines,
            line_hw2: line_hw * line_hw,
        }
    }

    fn in_water(&self, x: f32, z: f32) -> bool {
        for poly in &self.polygons {
            if point_in_poly(poly, x, z) {
                return true;
            }
        }
        for line in &self.lines {
            for i in 0..line.len() - 1 {
                let a = line[i];
                let b = line[i + 1];
                let dx = b.x - a.x;
                let dz = b.y - a.y;
                let denom = (dx * dx + dz * dz).max(1e-6);
                let t = (((x - a.x) * dx + (z - a.y) * dz) / denom).clamp(0.0, 1.0);
                let qx = a.x + dx * t;
                let qz = a.y + dz * t;
                let ddx = qx - x;
                let ddz = qz - z;
                if ddx * ddx + ddz * ddz < self.line_hw2 {
                    return true;
                }
            }
        }
        false
    }
}

fn point_in_poly(poly: &[Vec2], x: f32, z: f32) -> bool {
    let mut inside = false;
    let n = poly.len();
    if n < 3 {
        return false;
    }
    let mut j = n - 1;
    for i in 0..n {
        let xi = poly[i].x;
        let zi = poly[i].y;
        let xj = poly[j].x;
        let zj = poly[j].y;
        if (zi > z) != (zj > z) && x < (xj - xi) * (z - zi) / (zj - zi) + xi {
            inside = !inside;
        }
        j = i;
    }
    inside
}

/// Spatial hash of OSM road segments for building-overlap testing. Cells are
/// 40 m square; building queries look at a 3x3 cell neighbourhood and
/// segment-test only the roads that touch those cells.
struct OsmRoadMask {
    cells: HashMap<(i32, i32), Vec<u32>>,
    cell_size: f32,
}

impl OsmRoadMask {
    fn build() -> Self {
        let cell_size = 40.0;
        let sample_step = cell_size * 0.4;
        let mut cells: HashMap<(i32, i32), Vec<u32>> = HashMap::new();
        for (road_idx, road) in OSM_ROADS.iter().enumerate() {
            let mut last_cell: Option<(i32, i32)> = None;
            for i in 0..(road.count as usize).saturating_sub(1) {
                let a = OSM_POINTS[road.start as usize + i];
                let b = OSM_POINTS[road.start as usize + i + 1];
                let dx = b.0 - a.0;
                let dz = b.1 - a.1;
                let seg_len = (dx * dx + dz * dz).sqrt();
                let steps = ((seg_len / sample_step).ceil().max(1.0)) as usize;
                for s in 0..=steps {
                    let t = s as f32 / steps as f32;
                    let x = a.0 + dx * t;
                    let z = a.1 + dz * t;
                    let cell = ((x / cell_size).round() as i32, (z / cell_size).round() as i32);
                    if last_cell != Some(cell) {
                        cells
                            .entry(cell)
                            .or_insert_with(Vec::new)
                            .push(road_idx as u32);
                        last_cell = Some(cell);
                    }
                }
            }
        }
        Self { cells, cell_size }
    }

    /// `margin` is added to the road's class half-width. Negative margins
    /// allow buildings to overhang the rendered road edge slightly (the
    /// typical edge-of-street case).
    fn near(&self, x: f32, z: f32, margin: f32) -> bool {
        let cx = (x / self.cell_size).round() as i32;
        let cz = (z / self.cell_size).round() as i32;
        let mut last: u32 = u32::MAX;
        for da in -1..=1 {
            for db in -1..=1 {
                let Some(idxs) = self.cells.get(&(cx + da, cz + db)) else {
                    continue;
                };
                for &road_idx in idxs {
                    if road_idx == last {
                        continue;
                    }
                    last = road_idx;
                    let road = &OSM_ROADS[road_idx as usize];
                    let hw = (road.class.half_width() + margin).max(0.0);
                    let r2 = hw * hw;
                    for i in 0..(road.count as usize).saturating_sub(1) {
                        let (ax, az) = OSM_POINTS[road.start as usize + i];
                        let (bx, bz) = OSM_POINTS[road.start as usize + i + 1];
                        let dx = bx - ax;
                        let dz = bz - az;
                        let denom = (dx * dx + dz * dz).max(1e-6);
                        let t = (((x - ax) * dx + (z - az) * dz) / denom).clamp(0.0, 1.0);
                        let qx = ax + dx * t;
                        let qz = az + dz * t;
                        let ddx = qx - x;
                        let ddz = qz - z;
                        if ddx * ddx + ddz * ddz < r2 {
                            return true;
                        }
                    }
                }
            }
        }
        false
    }
}

/// A flat colored patch (parks, ponds) at the given y.
fn fill(buf: &mut MeshBuf, pts: &[Vec2], y: f32) {
    let n = pts.len();
    let signed: f32 = (0..n)
        .map(|i| {
            let a = pts[i];
            let b = pts[(i + 1) % n];
            a.x * b.y - b.x * a.y
        })
        .sum::<f32>()
        * 0.5;
    let ccw = signed > 0.0;
    let flat: Vec<f64> = pts.iter().flat_map(|p| [p.x as f64, p.y as f64]).collect();
    let tris = match earcutr::earcut(&flat, &[], 2) {
        Ok(t) => t,
        Err(_) => return,
    };
    let base = buf.positions.len() as u32;
    for p in pts {
        buf.positions.push([p.x, y, p.y]);
        buf.normals.push([0.0, 1.0, 0.0]);
        buf.uvs.push([0.0, 0.0]);
    }
    if ccw {
        for c in tris.chunks(3) {
            buf.indices
                .extend_from_slice(&[base + c[0] as u32, base + c[1] as u32, base + c[2] as u32]);
        }
    } else {
        for c in tris.chunks(3) {
            buf.indices
                .extend_from_slice(&[base + c[0] as u32, base + c[2] as u32, base + c[1] as u32]);
        }
    }
}

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

    fn into_mesh(self) -> Mesh {
        let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, self.positions);
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, self.normals);
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, self.uvs);
        mesh.insert_indices(Indices::U32(self.indices));
        mesh
    }
}
