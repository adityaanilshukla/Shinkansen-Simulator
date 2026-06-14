//! Top-left minimap panel.
//!
//! The track and station markers are rasterised once into an RGBA image at
//! startup; the train position is a small yellow dot whose absolute UI offset
//! is recomputed each frame.

use bevy::prelude::*;
use bevy::render::render_asset::RenderAssetUsages;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

use crate::geo::geo;
use crate::physics::TrainState;
use crate::route::Route;

const W: u32 = 156;
const H: u32 = 248;
const X0: f32 = -13_300.0;
const X1: f32 = 5_000.0;
const Z0: f32 = -31_000.0;
const Z1: f32 = 2_000.0;

const STATIONS: &[(f32, f32)] = &[
    (35.6812, 139.7671),
    (35.7141, 139.7774),
    (35.9060, 139.6240),
];

#[derive(Component)]
struct TrainDot;

pub struct MinimapPlugin;

impl Plugin for MinimapPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_minimap)
            .add_systems(Update, update_dot);
    }
}

fn to_px(x: f32, z: f32) -> (i32, i32) {
    let sc = ((W as f32 - 22.0) / (X1 - X0)).min((H as f32 - 22.0) / (Z1 - Z0));
    let ox = (W as f32 - (X1 - X0) * sc) * 0.5;
    let oz = (H as f32 - (Z1 - Z0) * sc) * 0.5;
    (
        (ox + (x - X0) * sc) as i32,
        (oz + (z - Z0) * sc) as i32,
    )
}

fn build_track_image(route: &Route) -> Image {
    let count = (W * H) as usize;
    let mut data = vec![0u8; count * 4];
    for i in 0..count {
        let off = i * 4;
        data[off] = 13;
        data[off + 1] = 15;
        data[off + 2] = 22;
        data[off + 3] = 158;
    }

    let mut prev: Option<(i32, i32)> = None;
    for s in 0..=220 {
        let p = route.spline.position(s as f32 / 220.0);
        let cur = to_px(p.x, p.z);
        if let Some(prv) = prev {
            line(&mut data, prv, cur, [46, 139, 87, 255]);
        }
        prev = Some(cur);
    }

    for &(lat, lon) in STATIONS {
        let g = geo(lat, lon);
        let p = to_px(g.x, g.z);
        dot(&mut data, p, 3, [255, 255, 255, 255]);
    }

    Image::new(
        Extent3d {
            width: W,
            height: H,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    )
}

fn put(data: &mut [u8], x: i32, y: i32, color: [u8; 4]) {
    if x < 0 || y < 0 || x >= W as i32 || y >= H as i32 {
        return;
    }
    let i = ((y as u32 * W + x as u32) * 4) as usize;
    data[i] = color[0];
    data[i + 1] = color[1];
    data[i + 2] = color[2];
    data[i + 3] = color[3];
}

fn line(data: &mut [u8], (x0, y0): (i32, i32), (x1, y1): (i32, i32), color: [u8; 4]) {
    let dx = (x1 - x0).abs();
    let dy = -(y1 - y0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;
    let mut x = x0;
    let mut y = y0;
    loop {
        put(data, x, y, color);
        put(data, x + 1, y, color);
        put(data, x, y + 1, color);
        if x == x1 && y == y1 {
            break;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x += sx;
        }
        if e2 <= dx {
            err += dx;
            y += sy;
        }
    }
}

fn dot(data: &mut [u8], (cx, cy): (i32, i32), r: i32, color: [u8; 4]) {
    for dy in -r..=r {
        for dx in -r..=r {
            if dx * dx + dy * dy <= r * r {
                put(data, cx + dx, cy + dy, color);
            }
        }
    }
}

fn spawn_minimap(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    route: Res<Route>,
) {
    let handle = images.add(build_track_image(&route));

    commands
        .spawn(NodeBundle {
            style: Style {
                position_type: PositionType::Absolute,
                top: Val::Px(14.0),
                left: Val::Px(14.0),
                width: Val::Px(W as f32),
                height: Val::Px(H as f32),
                ..default()
            },
            border_radius: BorderRadius::all(Val::Px(14.0)),
            ..default()
        })
        .with_children(|p| {
            p.spawn(ImageBundle {
                style: Style {
                    position_type: PositionType::Absolute,
                    width: Val::Px(W as f32),
                    height: Val::Px(H as f32),
                    ..default()
                },
                image: UiImage::new(handle),
                ..default()
            });
            p.spawn((
                NodeBundle {
                    style: Style {
                        position_type: PositionType::Absolute,
                        width: Val::Px(7.0),
                        height: Val::Px(7.0),
                        ..default()
                    },
                    background_color: BackgroundColor(Color::srgb(0.96, 0.78, 0.26)),
                    border_radius: BorderRadius::all(Val::Px(3.5)),
                    ..default()
                },
                TrainDot,
            ));
        });
}

fn update_dot(
    state: Res<TrainState>,
    route: Res<Route>,
    mut q: Query<&mut Style, With<TrainDot>>,
) {
    let p = route.spline.position_at_distance(state.dist);
    let (px, py) = to_px(p.x, p.z);
    if let Ok(mut s) = q.get_single_mut() {
        s.left = Val::Px(px as f32 - 3.5);
        s.top = Val::Px(py as f32 - 3.5);
    }
}
