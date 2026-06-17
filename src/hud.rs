//! Speed and direction HUD in the top-right corner.

use bevy::prelude::*;

use crate::driver::{GameMode, NearAction};
use crate::physics::{TrainState, V_MAX};
use crate::stations::Stations;

#[derive(Component)]
struct SpeedReadout;

#[derive(Component)]
struct DirReadout;

#[derive(Component)]
struct BarFill;

#[derive(Component)]
struct ActionPrompt;

#[derive(Component)]
struct NextStationReadout;

pub struct HudPlugin;

impl Plugin for HudPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, (build_hud, build_action_prompt))
            .add_systems(Update, (update_hud, update_action_prompt));
    }
}

fn build_action_prompt(mut commands: Commands) {
    commands.spawn((
        TextBundle::from_section(
            "",
            TextStyle {
                font_size: 16.0,
                color: Color::srgb(0.95, 0.96, 0.97),
                ..default()
            },
        )
        .with_style(Style {
            position_type: PositionType::Absolute,
            bottom: Val::Px(80.0),
            left: Val::Percent(50.0),
            margin: UiRect {
                left: Val::Px(-70.0),
                ..default()
            },
            padding: UiRect::all(Val::Px(10.0)),
            display: Display::None,
            ..default()
        })
        .with_background_color(Color::srgba(0.37, 0.23, 0.55, 0.85)),
        ActionPrompt,
    ));
}

fn update_action_prompt(
    mode: Res<GameMode>,
    mut q: Query<(&mut Text, &mut Style), With<ActionPrompt>>,
) {
    let Ok((mut text, mut style)) = q.get_single_mut() else {
        return;
    };
    let label = match mode.near_action {
        NearAction::StepOff => Some("STEP OFF  [E]"),
        NearAction::BoardCab => Some("BOARD CAB  [E]"),
        NearAction::None => None,
    };
    match label {
        Some(s) => {
            if let Some(section) = text.sections.get_mut(0) {
                section.value = s.to_string();
            }
            style.display = Display::Flex;
        }
        None => {
            style.display = Display::None;
        }
    }
}

fn build_hud(mut commands: Commands) {
    let panel = NodeBundle {
        style: Style {
            position_type: PositionType::Absolute,
            top: Val::Px(14.0),
            right: Val::Px(14.0),
            padding: UiRect::all(Val::Px(12.0)),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(4.0),
            ..default()
        },
        background_color: BackgroundColor(Color::srgba(0.05, 0.06, 0.09, 0.62)),
        border_radius: BorderRadius::all(Val::Px(14.0)),
        ..default()
    };
    let title_style = TextStyle {
        font_size: 12.0,
        color: Color::srgb(0.80, 0.75, 0.91),
        ..default()
    };
    let speed_style = TextStyle {
        font_size: 42.0,
        color: Color::srgb(0.95, 0.96, 0.97),
        ..default()
    };
    let unit_style = TextStyle {
        font_size: 13.0,
        color: Color::srgb(0.67, 0.69, 0.75),
        ..default()
    };
    let dir_style = TextStyle {
        font_size: 12.0,
        color: Color::srgb(0.96, 0.78, 0.26),
        ..default()
    };

    commands.spawn(panel).with_children(|p| {
        p.spawn(TextBundle::from_section("E5 HAYABUSA", title_style));
        p.spawn(NodeBundle {
            style: Style {
                column_gap: Val::Px(6.0),
                align_items: AlignItems::Baseline,
                ..default()
            },
            ..default()
        })
        .with_children(|row| {
            row.spawn((
                TextBundle::from_section("0", speed_style),
                SpeedReadout,
            ));
            row.spawn(TextBundle::from_section("km/h", unit_style));
        });

        // Speed bar background.
        p.spawn(NodeBundle {
            style: Style {
                width: Val::Px(118.0),
                height: Val::Px(5.0),
                ..default()
            },
            background_color: BackgroundColor(Color::srgb(0.16, 0.18, 0.22)),
            border_radius: BorderRadius::all(Val::Px(3.0)),
            ..default()
        })
        .with_children(|bar| {
            bar.spawn((
                NodeBundle {
                    style: Style {
                        width: Val::Px(0.0),
                        height: Val::Px(5.0),
                        ..default()
                    },
                    background_color: BackgroundColor(Color::srgb(0.37, 0.23, 0.55)),
                    border_radius: BorderRadius::all(Val::Px(3.0)),
                    ..default()
                },
                BarFill,
            ));
        });

        p.spawn((
            TextBundle::from_section("STOPPED", dir_style),
            DirReadout,
        ));
        p.spawn((
            TextBundle::from_section(
                "",
                TextStyle {
                    font_size: 12.0,
                    color: Color::srgb(0.78, 0.83, 0.95),
                    ..default()
                },
            ),
            NextStationReadout,
        ));
    });
}

fn update_hud(
    state: Res<TrainState>,
    stations: Res<Stations>,
    mut speed_q: Query<
        &mut Text,
        (
            With<SpeedReadout>,
            Without<DirReadout>,
            Without<NextStationReadout>,
        ),
    >,
    mut dir_q: Query<
        &mut Text,
        (
            With<DirReadout>,
            Without<SpeedReadout>,
            Without<NextStationReadout>,
        ),
    >,
    mut next_q: Query<
        &mut Text,
        (
            With<NextStationReadout>,
            Without<SpeedReadout>,
            Without<DirReadout>,
        ),
    >,
    mut bar_q: Query<&mut Style, With<BarFill>>,
) {
    let kmh = (state.speed.abs() * 3.6).round() as i32;
    if let Ok(mut t) = speed_q.get_single_mut() {
        if let Some(section) = t.sections.get_mut(0) {
            section.value = kmh.to_string();
        }
    }
    if let Ok(mut t) = dir_q.get_single_mut() {
        if let Some(section) = t.sections.get_mut(0) {
            let dir = if state.speed > 0.5 {
                "FORWARD"
            } else if state.speed < -0.5 {
                "REVERSE"
            } else if state.forward_dir < 0.0 {
                "STOPPED REV"
            } else {
                "STOPPED FWD"
            };
            let lever = match state.throttle_level {
                0 => "IDLE".to_string(),
                n if n > 0 => format!("PWR +{}", n),
                n => format!("BRK -{}", n.abs()),
            };
            section.value = format!("{}  |  {}", dir, lever);
        }
    }
    if let Ok(mut t) = next_q.get_single_mut() {
        if let Some(section) = t.sections.get_mut(0) {
            section.value = next_station_text(&stations, state.dist, state.forward_dir);
        }
    }
    if let Ok(mut s) = bar_q.get_single_mut() {
        let frac = (state.speed.abs() / V_MAX).clamp(0.0, 1.0);
        s.width = Val::Px(118.0 * frac);
    }
}

/// Decides the panel string from the train's arc-distance and travel
/// direction:
///
/// - `AT <name>` when the train arrow is at-or-just-past the station marker
///   (signed distance in travel direction is in `(-PAST_THRESHOLD, 0]`).
/// - `TO <name>  ·  X km` for the nearest station ahead otherwise.
/// - `END OF LINE` when there's nothing further in front.
fn next_station_text(stations: &Stations, dist: f32, forward_dir: f32) -> String {
    /// How far past a station's centre we keep showing `AT` before flipping
    /// to `TO next`. Roughly one platform length, so the status reads "AT
    /// TOKYO" for the full duration the train is alongside the platform.
    const PAST_THRESHOLD: f32 = 100.0;
    let dir = if forward_dir >= 0.0 { 1.0 } else { -1.0 };

    // Did we just cross (or are sitting at) any station centre?
    for s in &stations.list {
        let signed = (s.dist - dist) * dir;
        if signed <= 0.0 && signed > -PAST_THRESHOLD {
            return format!("AT {}", s.name);
        }
    }

    // Otherwise, nearest station strictly ahead in the travel direction.
    let mut best: Option<(&'static str, f32)> = None;
    for s in &stations.list {
        let signed = (s.dist - dist) * dir;
        if signed <= 0.0 {
            continue;
        }
        match best {
            Some((_, b)) if signed >= b => {}
            _ => best = Some((s.name, signed)),
        }
    }
    match best {
        Some((name, d)) if d >= 1000.0 => format!("TO {}   {:.1} km", name, d / 1000.0),
        Some((name, d)) => format!("TO {}   {:.0} m", name, d),
        None => "END OF LINE".to_string(),
    }
}
