//! Three identifiable Tokyo landmarks: Tokyo Tower, Skytree, Mt. Fuji.
//!
//! Tokyo Tower and Skytree are real GLBs loaded from `assets/`, auto-
//! rescaled at startup to match their real-world heights; Fuji is still a
//! stylised cone pair. All three sit at their real WGS84 positions so they
//! line up with the rest of the world.

use bevy::prelude::*;
use bevy::render::primitives::Aabb;

use crate::geo::geo;

/// Tag attached to a freshly-spawned landmark GLB. Carries the real-world
/// total height in metres; `rescale_landmark` measures the loaded model's
/// vertical extent, scales to hit that height, drops the base onto y=0, and
/// then removes the tag.
#[derive(Component)]
struct AutoRescaleHeight(f32);

pub struct LandmarksPlugin;

impl Plugin for LandmarksPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, (spawn_tokyo_tower, spawn_skytree, spawn_fuji))
            .add_systems(Update, rescale_landmark);
    }
}

fn spawn_tokyo_tower(mut commands: Commands, asset_server: Res<AssetServer>) {
    let pos = geo(35.6586, 139.7454);
    commands.spawn((
        SceneBundle {
            scene: asset_server.load("tokyo_tower.glb#Scene0"),
            transform: Transform::from_xyz(pos.x, 0.0, pos.z),
            ..default()
        },
        // Real-world Tokyo Tower height including the antenna mast.
        AutoRescaleHeight(333.0),
    ));
}

fn spawn_skytree(mut commands: Commands, asset_server: Res<AssetServer>) {
    let pos = geo(35.7101, 139.8107);
    commands.spawn((
        SceneBundle {
            scene: asset_server.load("tokyo_skytree.glb#Scene0"),
            transform: Transform::from_xyz(pos.x, 0.0, pos.z),
            ..default()
        },
        AutoRescaleHeight(634.0),
    ));
}

/// Walks each tagged landmark's scene hierarchy and unions every mesh's
/// world-space Aabb (because the GLB hierarchy can stack arbitrary
/// translations between the root and any single mesh — we can't just look
/// at one mesh's local Aabb). Once the model is loaded, picks a uniform
/// scale that maps the union's height to the tag's target metres, shifts
/// the root so the base lands on y=0, and removes the tag.
fn rescale_landmark(
    mut commands: Commands,
    leaves: Query<(&Aabb, &GlobalTransform)>,
    children: Query<&Children>,
    mut roots: Query<(Entity, &mut Transform, &AutoRescaleHeight)>,
) {
    for (root, mut transform, target) in &mut roots {
        let mut min_y = f32::INFINITY;
        let mut max_y = f32::NEG_INFINITY;
        let mut count = 0;
        collect_world_y(root, &leaves, &children, &mut min_y, &mut max_y, &mut count);
        if count == 0 {
            // Scene mesh entities aren't ready yet; try again next frame.
            continue;
        }
        let model_height = max_y - min_y;
        if model_height <= 0.001 {
            continue;
        }
        let s = target.0 / model_height;
        transform.scale = Vec3::splat(s);
        // The world Y measurements above were taken with the root at
        // (pos.x, 0, pos.z) and scale=1, so they match root-local Y. After
        // multiplying everything by s around the root's origin, the lowest
        // vertex sits at s * min_y; shift the root up by -s * min_y so it
        // hits exactly y=0.
        transform.translation.y = -s * min_y;
        commands.entity(root).remove::<AutoRescaleHeight>();
    }
}

fn collect_world_y(
    entity: Entity,
    leaves: &Query<(&Aabb, &GlobalTransform)>,
    children: &Query<&Children>,
    min_y: &mut f32,
    max_y: &mut f32,
    count: &mut usize,
) {
    if let Ok((aabb, gt)) = leaves.get(entity) {
        let affine = gt.affine();
        let cx = aabb.center.x;
        let cy = aabb.center.y;
        let cz = aabb.center.z;
        let hx = aabb.half_extents.x;
        let hy = aabb.half_extents.y;
        let hz = aabb.half_extents.z;
        for sx in [-1.0_f32, 1.0] {
            for sy in [-1.0_f32, 1.0] {
                for sz in [-1.0_f32, 1.0] {
                    let local = Vec3::new(cx + sx * hx, cy + sy * hy, cz + sz * hz);
                    let world = affine.transform_point3(local);
                    *min_y = min_y.min(world.y);
                    *max_y = max_y.max(world.y);
                    *count += 1;
                }
            }
        }
    }
    if let Ok(kids) = children.get(entity) {
        for &c in kids.iter() {
            collect_world_y(c, leaves, children, min_y, max_y, count);
        }
    }
}

fn spawn_fuji(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let body = materials.add(StandardMaterial {
        base_color: Color::srgb(0.56, 0.64, 0.72),
        perceptual_roughness: 1.0,
        ..default()
    });
    let cap = materials.add(StandardMaterial {
        base_color: Color::srgb(0.96, 0.97, 0.98),
        perceptual_roughness: 0.9,
        ..default()
    });
    commands.spawn(PbrBundle {
        mesh: meshes.add(Cone {
            radius: 4500.0,
            height: 2280.0,
        }),
        material: body,
        transform: Transform::from_xyz(-14_160.0, 1140.0, 5530.0),
        ..default()
    });
    commands.spawn(PbrBundle {
        mesh: meshes.add(Cone {
            radius: 1860.0,
            height: 960.0,
        }),
        material: cap,
        transform: Transform::from_xyz(-14_160.0, 1800.0, 5530.0),
        ..default()
    });
}
