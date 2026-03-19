use bevy::prelude::*;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

use crate::emulation::AudioBuffer;

/// Keeps the cpal stream alive for the app's lifetime.
/// cpal::Stream is !Send on Windows, so this must be a non-send resource.
pub struct AudioStreamHolder(#[allow(dead_code)] cpal::Stream);

/// Exclusive startup system: creates a cpal output stream that drains from the shared AudioBuffer.
pub fn setup_audio(world: &mut World) {
    let buffer = world.resource::<AudioBuffer>().buffer.clone();

    let host = cpal::default_host();
    let device = match host.default_output_device() {
        Some(d) => d,
        None => {
            eprintln!("No audio output device found; audio disabled");
            return;
        }
    };

    let supported = match device.default_output_config() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to get audio config: {}; audio disabled", e);
            return;
        }
    };

    let channels = supported.channels() as usize;
    let config: cpal::StreamConfig = supported.into();

    let stream = device
        .build_output_stream(
            &config,
            move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                let mut buf = buffer.lock().unwrap();
                for frame in data.chunks_mut(channels) {
                    let sample = buf.pop_front().unwrap_or(0.0);
                    for s in frame.iter_mut() {
                        *s = sample;
                    }
                }
            },
            |err| eprintln!("Audio stream error: {}", err),
            None,
        )
        .expect("Failed to build audio stream");

    stream.play().expect("Failed to start audio stream");
    world.insert_non_send_resource(AudioStreamHolder(stream));
}
