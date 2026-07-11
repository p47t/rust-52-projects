use anyhow::{anyhow, Result};
use wakeword_engine::{
    convert_channels_to_mono, resample_linear, AudioProcessing, Config,
    StreamConfig, NoiseSuppression, GainController2,
};


/// Audio preprocessor that handles channel mixing, resampling,
/// noise suppression (NS), and automatic gain control (AGC2) via WebRTC.
pub struct AudioPreprocessor {
    input_sample_rate: u32,
    apm: Option<AudioProcessing>,
    mic_samples_accumulator: Vec<f32>,
    apm_input_buffer: Vec<f32>,
}

impl AudioPreprocessor {
    pub fn new(input_sample_rate: u32, apm_enabled: bool) -> Result<Self> {
        let apm = if apm_enabled {
            let config = Config {
                noise_suppression: Some(NoiseSuppression::default()),
                gain_controller2: Some(GainController2::default()),
                ..Default::default()
            };
            
            let apm_engine = AudioProcessing::builder()
                .config(config)
                .capture_config(StreamConfig::new(16000, 1))
                .render_config(StreamConfig::new(16000, 1))
                .build();
            Some(apm_engine)
        } else {
            None
        };

        Ok(Self {
            input_sample_rate,
            apm,
            mic_samples_accumulator: Vec::new(),
            apm_input_buffer: Vec::new(),
        })
    }

    /// Feeds raw multi-channel samples captured from the microphone,
    /// performs mono channel-mixing and resampling to 16 kHz, and
    /// processes the audio using WebRTC noise suppression + AGC.
    ///
    /// Returns any processed 16 kHz samples that are ready.
    pub fn feed_and_process(&mut self, raw_samples: &[f32], channels: usize) -> Result<Vec<f32>> {
        if raw_samples.is_empty() {
            return Ok(Vec::new());
        }

        // 1. Channel Mixing: convert input channels to mono
        let mono_samples = convert_channels_to_mono(raw_samples, channels);
        self.mic_samples_accumulator.extend_from_slice(&mono_samples);

        // 2. Resampling: convert input rate to target 16,000 Hz
        let mic_samples_needed = (160.0 * (self.input_sample_rate as f32 / 16000.0)).ceil() as usize;
        while self.mic_samples_accumulator.len() >= mic_samples_needed {
            let chunk_to_resample = self.mic_samples_accumulator.drain(0..mic_samples_needed).collect::<Vec<_>>();
            let resampled_16k = resample_linear(&chunk_to_resample, self.input_sample_rate, 16000);
            self.apm_input_buffer.extend_from_slice(&resampled_16k);
        }

        // 3. WebRTC APM: Process 10ms frames (160 samples at 16 kHz)
        let apm_frame_size = 160;
        let mut processed_samples = Vec::new();

        while self.apm_input_buffer.len() >= apm_frame_size {
            let frame: Vec<f32> = self.apm_input_buffer.drain(0..apm_frame_size).collect();

            let processed_frame = if let Some(ref mut apm_engine) = self.apm {
                let mut dest = vec![0.0f32; apm_frame_size];
                apm_engine.process_capture_f32(&[&frame], &mut [&mut dest])
                    .map_err(|e| anyhow!("APM process error: {:?}", e))?;
                dest
            } else {
                frame
            };

            processed_samples.extend_from_slice(&processed_frame);
        }

        Ok(processed_samples)
    }

    /// Reset any leftover buffers
    pub fn reset(&mut self) {
        self.mic_samples_accumulator.clear();
        self.apm_input_buffer.clear();
    }
}
