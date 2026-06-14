//! Three identifiable Tokyo landmarks: Tokyo Tower, Skytree, Mt. Fuji.
//!
//! These are stylised: a stack of cones/cylinders rather than accurate
//! lattice models. They sit at the real WGS84 positions so they line up with
//! the rest of the world.

use bevy::prelude::*;

use crate::geo::geo;

pub struct LandmarksPlugin;

impl Plugin for LandmarksPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, (spawn_tokyo_tower, spawn_skytree, spawn_fuji));
    }
}

fn spawn_tokyo_tower(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let pos = geo(35.6586, 139.7454);
    let red = materials.add(StandardMaterial {
        base_color: Color::srgb(0.84, 0.32, 0.18),
        perceptual_roughness: 0.6,
        metallic: 0.3,
        ..default()
    });
    let cream = materials.add(StandardMaterial {
        base_color: Color::srgb(0.91, 0.89, 0.85),
        perceptual_roughness: 0.7,
        ..default()
    });

    let scale = 1.36;
    let group = commands
        .spawn((
            SpatialBundle::from_transform(
                Transform::from_xyz(pos.x, 0.0, pos.z).with_scale(Vec3::splat(scale)),
            ),
        ))
        .id();

    let lower = commands
        .spawn(PbrBundle {
            mesh: meshes.add(Cylinder::new(20.0, 65.0)),
            material: red.clone(),
            transform: Transform::from_xyz(0.0, 65.0, 0.0),
            ..default()
        })
        .id();
    let deck1 = commands
        .spawn(PbrBundle {
            mesh: meshes.add(Cylinder::new(10.0, 3.5)),
            material: cream.clone(),
            transform: Transform::from_xyz(0.0, 124.0, 0.0),
            ..default()
        })
        .id();
    let upper = commands
        .spawn(PbrBundle {
            mesh: meshes.add(Cylinder::new(5.0, 40.0)),
            material: red.clone(),
            transform: Transform::from_xyz(0.0, 168.0, 0.0),
            ..default()
        })
        .id();
    let deck2 = commands
        .spawn(PbrBundle {
            mesh: meshes.add(Cylinder::new(4.6, 2.5)),
            material: cream,
            transform: Transform::from_xyz(0.0, 205.0, 0.0),
            ..default()
        })
        .id();
    let antenna = commands
        .spawn(PbrBundle {
            mesh: meshes.add(Cylinder::new(0.7, 19.0)),
            material: red,
            transform: Transform::from_xyz(0.0, 226.0, 0.0),
            ..default()
        })
        .id();

    commands
        .entity(group)
        .push_children(&[lower, deck1, upper, deck2, antenna]);
}

fn spawn_skytree(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let pos = geo(35.7101, 139.8107);
    let shaft = materials.add(StandardMaterial {
        base_color: Color::srgb(0.84, 0.87, 0.89),
        perceptual_roughness: 0.6,
        metallic: 0.3,
        ..default()
    });
    let deck = materials.add(StandardMaterial {
        base_color: Color::srgb(0.94, 0.95, 0.96),
        perceptual_roughness: 0.5,
        metallic: 0.2,
        ..default()
    });

    let scale = 1.62;
    let group = commands
        .spawn(SpatialBundle::from_transform(
            Transform::from_xyz(pos.x, 0.0, pos.z).with_scale(Vec3::splat(scale)),
        ))
        .id();

    let main = commands
        .spawn(PbrBundle {
            mesh: meshes.add(Cylinder::new(13.0, 150.0)),
            material: shaft.clone(),
            transform: Transform::from_xyz(0.0, 150.0, 0.0),
            ..default()
        })
        .id();
    let d1 = commands
        .spawn(PbrBundle {
            mesh: meshes.add(Cylinder::new(13.0, 4.5)),
            material: deck.clone(),
            transform: Transform::from_xyz(0.0, 206.0, 0.0),
            ..default()
        })
        .id();
    let d2 = commands
        .spawn(PbrBundle {
            mesh: meshes.add(Cylinder::new(9.0, 4.5)),
            material: deck,
            transform: Transform::from_xyz(0.0, 268.0, 0.0),
            ..default()
        })
        .id();
    let spire = commands
        .spawn(PbrBundle {
            mesh: meshes.add(Cylinder::new(1.3, 45.0)),
            material: shaft,
            transform: Transform::from_xyz(0.0, 345.0, 0.0),
            ..default()
        })
        .id();

    commands
        .entity(group)
        .push_children(&[main, d1, d2, spire]);
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
