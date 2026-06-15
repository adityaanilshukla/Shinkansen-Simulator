//! Procedural audio.
//!
//! A brown-noise rumble mixed with a thin pink texture and a 50 Hz traction
//! hum stands in for wheels-on-rail + airflow + EMU motors. Volume scales
//! with the train's speed; pitch stays close to fixed so the bed sounds like
//! the same train growing louder, not a different one. The horn is a
//! seamlessly-looping chord played for as long as H is held.

use bevy::audio::{AudioSink, PlaybackMode, Volume};
use bevy::prelude::*;

use crate::physics::{TrainState, V_MAX};

#[derive(Component)]
struct RunningBed;

#[derive(Component)]
struct HornVoice;

#[derive(Resource)]
struct HornHandle(Handle<AudioSource>);

pub struct AudioPlugin;

impl Plugin for AudioPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_audio)
            .add_systems(Update, (update_bed, drive_horn));
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

fn update_bed(state: Res<TrainState>, sinks: Query<&AudioSink, With<RunningBed>>) {
    let Ok(sink) = sinks.get_single() else {
        return;
    };
    let frac = (state.speed.abs() / V_MAX).clamp(0.0, 1.0);
    // Always-on quiet idle rumble; swells with speed. Pitch stays nearly
    // constant so the texture reads as the same train running faster, not
    // as a chipmunk-on-rails.
    sink.set_volume(0.08 + frac * 0.34);
    sink.set_speed(0.92 + frac * 0.18);
}

fn drive_horn(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    horn: Res<HornHandle>,
    voices: Query<Entity, With<HornVoice>>,
) {
    if keys.just_pressed(KeyCode::KeyH) {
        commands.spawn((
            AudioBundle {
                source: horn.0.clone(),
                settings: PlaybackSettings {
                    mode: PlaybackMode::Loop,
                    volume: Volume::new(0.7),
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

/// Four-tone horn chord (370 sine + detuned 370 + 466 + 185 octave) rendered
/// at constant amplitude with a 5 ms fade at each end so the bevy_audio loop
/// seam doesn't pop. Played in `PlaybackMode::Loop` for as long as H is held.
fn generate_horn_wav() -> Vec<u8> {
    let sr: u32 = 22_050;
    let duration = 1.4_f32;
    let n = (sr as f32 * duration) as usize;
    let mut samples: Vec<i16> = Vec::with_capacity(n);
    let freqs: [(f32, f32); 4] = [
        (370.0, 0.30),
        (370.5, 0.30),
        (466.0, 0.30),
        (185.0, 0.50),
    ];
    let fade = 0.005_f32;
    for i in 0..n {
        let t = i as f32 / sr as f32;
        let mut s = 0.0_f32;
        for &(f, amp) in &freqs {
            s += amp * (t * f * std::f32::consts::TAU).sin();
        }
        let env = if t < fade {
            t / fade
        } else if t > duration - fade {
            (duration - t) / fade
        } else {
            1.0
        };
        let v = s * env * 0.45;
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
