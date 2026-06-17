//! Procedural audio.
//!
//! A brown-noise rumble mixed with a thin pink texture and a 50 Hz traction
//! hum stands in for wheels-on-rail + airflow + EMU motors. Volume scales
//! with the train's speed; pitch stays close to fixed so the bed sounds like
//! the same train growing louder, not a different one. The horn is a
//! seamlessly-looping chord played for as long as H is held.

use bevy::audio::{AudioSink, PlaybackMode, Volume};
use bevy::prelude::*;

use crate::input::Controls;
use crate::physics::{TrainState, V_MAX};

/// Maps `Controls.zoom` (0.45..=2.4, smaller = closer) to a 1/x volume factor:
/// closer camera → louder, further camera → quieter. Capped at 1.0 so even
/// cab-close view never blows out the mix, and floored so far-out audio is
/// faint rather than silent.
fn zoom_volume_factor(zoom: f32) -> f32 {
    (0.7 / zoom.max(0.1)).clamp(0.12, 1.0)
}

#[derive(Component)]
struct RunningBed;

#[derive(Component)]
struct HornVoice;

#[derive(Resource)]
struct HornHandle(Handle<AudioSource>);

/// Master mute. Toggled by the player with M; while true every sink in the
/// scene is silenced and the horn refuses to spawn.
#[derive(Resource, Default)]
pub struct AudioMute(pub bool);

pub struct AudioPlugin;

impl Plugin for AudioPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<AudioMute>()
            .add_systems(Startup, setup_audio)
            .add_systems(
                Update,
                (toggle_mute, update_bed, drive_horn, update_horn_volume),
            );
    }
}

fn toggle_mute(
    keys: Res<ButtonInput<KeyCode>>,
    mut mute: ResMut<AudioMute>,
    horn_voices: Query<&AudioSink, With<HornVoice>>,
) {
    if keys.just_pressed(KeyCode::KeyM) {
        mute.0 = !mute.0;
        // Snap any currently-sounding horn voice off immediately; the bed gets
        // re-set every frame by `update_bed` so it doesn't need a nudge.
        if mute.0 {
            for sink in &horn_voices {
                sink.set_volume(0.0);
            }
        }
    }
}

fn setup_audio(mut commands: Commands, mut sources: ResMut<Assets<AudioSource>>) {
    let noise_bytes = generate_noise_wav();
    let horn_bytes = generate_horn_wav();

    let noise_handle = sources.add(AudioSource {
        bytes: noise_bytes.into(),
    });
    let horn_handle = sources.add(AudioSource {
        bytes: horn_bytes.into(),
    });

    commands.spawn((
        AudioBundle {
            source: noise_handle,
            settings: PlaybackSettings {
                mode: PlaybackMode::Loop,
                volume: Volume::new(0.0),
                speed: 1.0,
                ..default()
            },
        },
        RunningBed,
    ));

    commands.insert_resource(HornHandle(horn_handle));
}

fn update_bed(
    state: Res<TrainState>,
    mute: Res<AudioMute>,
    controls: Res<Controls>,
    sinks: Query<&AudioSink, With<RunningBed>>,
) {
    let Ok(sink) = sinks.get_single() else {
        return;
    };
    if mute.0 {
        sink.set_volume(0.0);
        return;
    }
    let frac = (state.speed.abs() / V_MAX).clamp(0.0, 1.0);
    // Quiet idle rumble that swells with speed and with how close the camera
    // is to the train. Pitch stays nearly constant so the texture reads as the
    // same train running faster, not as a chipmunk-on-rails.
    // Sized so that even at cab-close zoom (`zoom_volume_factor` = 1.0) the
    // bed maxes out at roughly what the previous tuning produced when fully
    // zoomed out. The player asked for that level to be the loudest possible.
    let base_vol = 0.0035 + frac * 0.018;
    sink.set_volume(base_vol * zoom_volume_factor(controls.zoom));
    sink.set_speed(0.92 + frac * 0.18);
}

fn drive_horn(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    horn: Res<HornHandle>,
    mute: Res<AudioMute>,
    voices: Query<Entity, With<HornVoice>>,
) {
    if keys.just_pressed(KeyCode::KeyH) && !mute.0 {
        commands.spawn((
            AudioBundle {
                source: horn.0.clone(),
                settings: PlaybackSettings {
                    mode: PlaybackMode::Loop,
                    // Spawn quiet; `update_horn_volume` retunes it on the next
                    // frame based on zoom. Prevents a half-frame pop on press.
                    volume: Volume::new(0.02),
                    speed: 1.0,
                    ..default()
                },
            },
            HornVoice,
        ));
    }
    if keys.just_released(KeyCode::KeyH) {
        for entity in &voices {
            commands.entity(entity).despawn();
        }
    }
}

/// Hold the horn at a flat, low playback volume regardless of zoom. The
/// player asked for a constant level, so the horn doesn't look at
/// `Controls.zoom` at all.
fn update_horn_volume(mute: Res<AudioMute>, sinks: Query<&AudioSink, With<HornVoice>>) {
    let vol = if mute.0 { 0.0 } else { 0.03 };
    for sink in &sinks {
        sink.set_volume(vol);
    }
}

/// Two-second loopable bed: brown noise (the dominant rumble), a thin pink
/// noise layer (air / texture), and a 50 Hz traction hum, slowly amplitude-
/// modulated by a 0.7 Hz LFO so the rumble breathes rather than reading as
/// flat static.
fn generate_noise_wav() -> Vec<u8> {
    let sr: u32 = 22_050;
    let n = (sr as usize) * 2;
    let mut samples: Vec<i16> = Vec::with_capacity(n);

    // Paul Kellet pink-noise filter state.
    let mut b0 = 0.0_f32;
    let mut b1 = 0.0_f32;
    let mut b2 = 0.0_f32;
    // Leaky-integrator brown-noise state.
    let mut brown = 0.0_f32;

    let mut rng = fastrand::Rng::with_seed(0xAA_BB_CC_DD);
    for i in 0..n {
        let w = rng.f32() * 2.0 - 1.0;

        b0 = 0.99765 * b0 + w * 0.099_046_0;
        b1 = 0.96300 * b1 + w * 0.296_516_4;
        b2 = 0.57000 * b2 + w * 1.052_691_3;
        let pink = (b0 + b1 + b2 + w * 0.1848) * 0.18;

        // Leak 0.985 → ~53 Hz cutoff, giving a sub-bass rumble.
        brown = brown * 0.985 + w * 0.04;

        let t = i as f32 / sr as f32;
        let hum = 0.05 * (t * 50.0 * std::f32::consts::TAU).sin();
        let lfo = 0.85 + 0.15 * (t * 0.7 * std::f32::consts::TAU).sin();

        let s = (brown * 1.5 + pink * 0.30) * lfo + hum;

        let v = (s * 22_000.0).clamp(-32_767.0, 32_767.0);
        samples.push(v as i16);
    }
    pcm_to_wav(&samples, sr)
}

/// Four-tone horn chord (185 + detuned 185.5 + 233 + 92.5 sub-octave),
/// dropped a full octave below the original Three.js demo so the horn reads
/// as a deep warning sound.
///
/// Every frequency here is chosen so that `f * duration` is an integer with
/// `duration = 2.0 s`. That means each sine completes a whole number of
/// cycles in the clip, sample[0] and the natural next-sample after sample[n]
/// share the same phase, and `PlaybackMode::Loop` produces a continuous
/// drone with no audible seam. No fade is applied for that same reason -
/// fading was what made the loop boundary audible as a pulse before.
fn generate_horn_wav() -> Vec<u8> {
    let sr: u32 = 22_050;
    let duration = 2.0_f32;
    let n = (sr as f32 * duration) as usize;
    let mut samples: Vec<i16> = Vec::with_capacity(n);
    // 185.0 * 2.0 = 370, 185.5 * 2.0 = 371, 233.0 * 2.0 = 466,
    // 92.5 * 2.0 = 185 — all integer cycles per loop.
    let freqs: [(f32, f32); 4] = [
        (185.0, 0.30),
        (185.5, 0.30),
        (233.0, 0.28),
        (92.5, 0.55),
    ];
    for i in 0..n {
        let t = i as f32 / sr as f32;
        let mut s = 0.0_f32;
        for &(f, amp) in &freqs {
            s += amp * (t * f * std::f32::consts::TAU).sin();
        }
        let v = s * 0.45;
        let clipped = (v * 30_000.0).clamp(-32_767.0, 32_767.0);
        samples.push(clipped as i16);
    }
    pcm_to_wav(&samples, sr)
}

/// Wrap mono 16-bit PCM samples in a minimal RIFF/WAVE header so bevy_audio
/// can decode them through its normal pipeline.
fn pcm_to_wav(samples: &[i16], sample_rate: u32) -> Vec<u8> {
    let data_size = (samples.len() * 2) as u32;
    let chunk_size = 36 + data_size;
    let mut out = Vec::with_capacity(44 + data_size as usize);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&chunk_size.to_le_bytes());
    out.extend_from_slice(b"WAVE");
    out.extend_from_slice(b"fmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&sample_rate.to_le_bytes());
    out.extend_from_slice(&(sample_rate * 2).to_le_bytes());
    out.extend_from_slice(&2u16.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_size.to_le_bytes());
    for s in samples {
        out.extend_from_slice(&s.to_le_bytes());
    }
    out
}
