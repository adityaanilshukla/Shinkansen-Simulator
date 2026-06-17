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
const MAP_PAD: f32 = 11.0;
/// We rasterise the track + station dots at `RENDER_SCALE`× the displayed
/// resolution and let Bevy's default bilinear UI sampler downscale, so the
/// strokes don't read as a single chunky pixel ladder.
const RENDER_SCALE: u32 = 4;
const IMG_W: u32 = W * RENDER_SCALE;
const IMG_H: u32 = H * RENDER_SCALE;

const STATIONS: &[(f32, f32, &str)] = &[
    (35.6812, 139.7671, "TOKYO"),
    (35.7141, 139.7774, "UENO"),
    (35.9060, 139.6240, "OMIYA"),
];

/// World-space bounding box that the minimap projects onto its image. Sized
/// to the actual track + station extents at startup so the route fills the
/// panel instead of sitting in the right half with empty space on the left.
#[derive(Resource)]
struct MapBounds {
    x0: f32,
    x1: f32,
    z0: f32,
    z1: f32,
}

#[derive(Component)]
struct TrainDot;

pub struct MinimapPlugin;

impl Plugin for MinimapPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_minimap)
            .add_systems(Update, update_dot);
    }
}

fn compute_bounds(route: &Route) -> MapBounds {
    let mut x0 = f32::INFINITY;
    let mut x1 = f32::NEG_INFINITY;
    let mut z0 = f32::INFINITY;
    let mut z1 = f32::NEG_INFINITY;
    for s in 0..=400 {
        let p = route.spline.position(s as f32 / 400.0);
        x0 = x0.min(p.x);
        x1 = x1.max(p.x);
        z0 = z0.min(p.z);
        z1 = z1.max(p.z);
    }
    for &(lat, lon, _) in STATIONS {
        let g = geo(lat, lon);
        x0 = x0.min(g.x);
        x1 = x1.max(g.x);
        z0 = z0.min(g.z);
        z1 = z1.max(g.z);
    }
    // 5% margin so the route never touches the panel border.
    let mx = (x1 - x0) * 0.05;
    let mz = (z1 - z0) * 0.05;
    MapBounds {
        x0: x0 - mx,
        x1: x1 + mx,
        z0: z0 - mz,
        z1: z1 + mz,
    }
}

/// Returns panel-display pixel coordinates (0..W, 0..H). Image rasterisation
/// multiplies the result by `RENDER_SCALE` internally; UI placement uses it
/// as-is.
fn to_px(b: &MapBounds, x: f32, z: f32) -> (i32, i32) {
    let sc = ((W as f32 - MAP_PAD * 2.0) / (b.x1 - b.x0))
        .min((H as f32 - MAP_PAD * 2.0) / (b.z1 - b.z0));
    let ox = (W as f32 - (b.x1 - b.x0) * sc) * 0.5;
    let oz = (H as f32 - (b.z1 - b.z0) * sc) * 0.5;
    (
        (ox + (x - b.x0) * sc) as i32,
        (oz + (z - b.z0) * sc) as i32,
    )
}

/// Procedurally rasterises a small isoceles triangle pointing straight up
/// (apex at the top, base at the bottom) with the cabin yellow colour. We
/// rotate this around its centre each frame to indicate travel direction.
fn build_arrow_image() -> Image {
    const W: u32 = 9;
    const H: u32 = 11;
    let mut data = vec![0u8; (W * H * 4) as usize];
    let apex_x = (W - 1) as f32 * 0.5;
    let base_half = (W - 1) as f32 * 0.5;
    for y in 0..H {
        let frac = y as f32 / (H - 1) as f32;
        let half_w = base_half * frac;
        let x_min = apex_x - half_w - 0.5;
        let x_max = apex_x + half_w + 0.5;
        for x in 0..W {
            if (x as f32) >= x_min && (x as f32) <= x_max {
                let i = ((y * W + x) * 4) as usize;
                data[i] = 245;
                data[i + 1] = 199;
                data[i + 2] = 66;
                data[i + 3] = 255;
            }
        }
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

fn build_track_image(route: &Route, bounds: &MapBounds) -> Image {
    let count = (IMG_W * IMG_H) as usize;
    let mut data = vec![0u8; count * 4];
    for i in 0..count {
        let off = i * 4;
        data[off] = 13;
        data[off + 1] = 15;
        data[off + 2] = 22;
        data[off + 3] = 158;
    }

    // Track polyline at high res. Sample more densely than before because we
    // now have RENDER_SCALE× more pixels to fill — the old 400 segments
    // would leave visible gaps when projected to 624×992.
    let line_radius = (RENDER_SCALE as i32) * 3 / 4 + 1;
    let mut prev: Option<(i32, i32)> = None;
    for s in 0..=1600 {
        let p = route.spline.position(s as f32 / 1600.0);
        let cur = scale_px(to_px(bounds, p.x, p.z));
        if let Some(prv) = prev {
            line(&mut data, prv, cur, line_radius, [46, 139, 87, 255]);
        }
        prev = Some(cur);
    }

    let dot_radius = (RENDER_SCALE as i32) * 7 / 4;
    for &(lat, lon, _) in STATIONS {
        let g = geo(lat, lon);
        let p = scale_px(to_px(bounds, g.x, g.z));
        dot(&mut data, p, dot_radius, [255, 255, 255, 255]);
    }

    Image::new(
        Extent3d {
            width: IMG_W,
            height: IMG_H,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    )
}

#[inline]
fn scale_px((x, y): (i32, i32)) -> (i32, i32) {
    (
        x * RENDER_SCALE as i32 + RENDER_SCALE as i32 / 2,
        y * RENDER_SCALE as i32 + RENDER_SCALE as i32 / 2,
    )
}

fn put(data: &mut [u8], x: i32, y: i32, color: [u8; 4]) {
    if x < 0 || y < 0 || x >= IMG_W as i32 || y >= IMG_H as i32 {
        return;
    }
    let i = ((y as u32 * IMG_W + x as u32) * 4) as usize;
    data[i] = color[0];
    data[i + 1] = color[1];
    data[i + 2] = color[2];
    data[i + 3] = color[3];
}

fn line(
    data: &mut [u8],
    (x0, y0): (i32, i32),
    (x1, y1): (i32, i32),
    radius: i32,
    color: [u8; 4],
) {
    let dx = (x1 - x0).abs();
    let dy = -(y1 - y0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;
    let mut x = x0;
    let mut y = y0;
    loop {
        // Stamp a filled disc at every Bresenham step so the line keeps a
        // consistent visible thickness after bilinear downscale.
        for dy_off in -radius..=radius {
            for dx_off in -radius..=radius {
                if dx_off * dx_off + dy_off * dy_off <= radius * radius {
                    put(data, x + dx_off, y + dy_off, color);
                }
            }
        }
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
    let bounds = compute_bounds(&route);
    let handle = images.add(build_track_image(&route, &bounds));
    let arrow_handle = images.add(build_arrow_image());

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

            // Station name labels, anchored at the station dot on the map.
            let label_style = TextStyle {
                font_size: 11.0,
                color: Color::srgb(0.97, 0.97, 0.97),
                ..default()
            };
            for &(lat, lon, name) in STATIONS {
                let g = geo(lat, lon);
                let (px, py) = to_px(&bounds, g.x, g.z);
                p.spawn(
                    TextBundle::from_section(name, label_style.clone()).with_style(Style {
                        position_type: PositionType::Absolute,
                        left: Val::Px(px as f32 + 6.0),
                        top: Val::Px(py as f32 - 7.0),
                        ..default()
                    }),
                );
            }

            // Train marker: a procedurally-rasterised triangle pointing up
            // by default, rotated each frame by `update_dot` to align with
            // the current travel direction.
            p.spawn((
                ImageBundle {
                    style: Style {
                        position_type: PositionType::Absolute,
                        width: Val::Px(9.0),
                        height: Val::Px(11.0),
                        ..default()
                    },
                    image: UiImage::new(arrow_handle),
                    ..default()
                },
                TrainDot,
            ));
        });

    commands.insert_resource(bounds);
}

fn update_dot(
    state: Res<TrainState>,
    route: Res<Route>,
    bounds: Res<MapBounds>,
    mut q: Query<(&mut Style, &mut Transform), With<TrainDot>>,
) {
    let p = route.spline.position_at_distance(state.dist);
    let (px, py) = to_px(&bounds, p.x, p.z);
    let tan = route.spline.tangent_at_distance(state.dist);
    // Travel sign: while actually moving use `speed`, otherwise show whatever
    // direction the train is currently *facing* (forward_dir).
    let sign = if state.speed.abs() > 0.1 {
        state.speed.signum()
    } else {
        state.forward_dir.signum()
    };
    // Map world (X, Z) -> minimap (left, down). The arrow's natural
    // orientation is straight up the screen, i.e. (sx, sy) = (0, -1).
    // We want it to point in (tan.x, tan.z) * sign instead.
    let sx = tan.x * sign;
    let sy = tan.z * sign;
    let ang = sx.atan2(-sy);

    if let Ok((mut s, mut tf)) = q.get_single_mut() {
        // Centre the 9x11 arrow image on the train's projected position.
        s.left = Val::Px(px as f32 - 4.5);
        s.top = Val::Px(py as f32 - 5.5);
        tf.rotation = Quat::from_rotation_z(ang);
    }
}
