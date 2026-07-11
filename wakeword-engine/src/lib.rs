use anyhow::{anyhow, Result};
use hound::WavReader;
pub use oww_rs::config::SpeechUnlockType;
pub use oww_rs::oww::{OwwModel, OWW_MODEL_CHUNK_SIZE};
pub use sonora::{AudioProcessing, Config, StreamConfig};
pub use sonora::config::{NoiseSuppression, GainController2};

/// Initialize the WebRTC APM engine configured with Noise Suppression and AGC2.
pub fn init_apm(sample_rate: u32) -> Result<AudioProcessing> {
    let config = Config {
        noise_suppression: Some(NoiseSuppression::default()),
        gain_controller2: Some(GainController2::default()),
        ..Default::default()
    };
    
    let apm = AudioProcessing::builder()
        .config(config)
        .capture_config(StreamConfig::new(sample_rate, 1))
        .render_config(StreamConfig::new(sample_rate, 1))
        .build();
        
    Ok(apm)
}

/// Run WebRTC APM noise suppression and gain control on a float32 audio slice.
/// Operating in 10ms chunks (160 samples at 16kHz).
pub fn preprocess_audio(apm: &mut AudioProcessing, input: &[f32]) -> Result<Vec<f32>> {
    let frame_size = 160; // 10ms at 16kHz
    let mut output = Vec::with_capacity(input.len());
    let mut offset = 0;

    while offset + frame_size <= input.len() {
        let frame = &input[offset..offset + frame_size];
        let mut dest = vec![0.0f32; frame_size];
        apm.process_capture_f32(&[frame], &mut [&mut dest])
            .map_err(|e| anyhow!("APM error: {:?}", e))?;
        output.extend_from_slice(&dest);
        offset += frame_size;
    }
    
    // Append remaining samples without APM processing if they don't fill a 10ms frame
    if offset < input.len() {
        output.extend_from_slice(&input[offset..]);
    }

    Ok(output)
}

/// Convert multi-channel samples to mono by averaging channels.
pub fn convert_channels_to_mono(samples: &[f32], channels: usize) -> Vec<f32> {
    if channels == 1 {
        return samples.to_vec();
    }
    
    let mut mono = Vec::with_capacity(samples.len() / channels);
    for chunk in samples.chunks_exact(channels) {
        let sum: f32 = chunk.iter().sum();
        mono.push(sum / channels as f32);
    }
    mono
}

/// Resample audio from one sampling rate to another using linear interpolation.
pub fn resample_linear(input: &[f32], from_sample_rate: u32, to_sample_rate: u32) -> Vec<f32> {
    if from_sample_rate == to_sample_rate {
        return input.to_vec();
    }
    let ratio = to_sample_rate as f64 / from_sample_rate as f64;
    let new_length = (input.len() as f64 * ratio).round() as usize;
    let mut output = Vec::with_capacity(new_length);
    for i in 0..new_length {
        let pos = i as f64 / ratio;
        let idx = pos.floor() as usize;
        let frac = pos - idx as f64;
        if idx + 1 < input.len() {
            let sample = input[idx] * (1.0 - frac) as f32 + input[idx + 1] * frac as f32;
            output.push(sample);
        } else if idx < input.len() {
            output.push(input[idx]);
        }
    }
    output
}

/// Load, resample, and preprocess WAV audio files to be ready for openWakeWord inference.
pub fn get_processed_samples(path: &str, apm_enabled: bool) -> Result<Vec<f32>> {
    let mut reader = WavReader::open(path)?;
    let spec = reader.spec();

    let raw_samples;
    match spec.sample_format {
        hound::SampleFormat::Float => {
            let samples: Vec<f32> = reader.samples::<f32>().filter_map(Result::ok).collect();
            raw_samples = convert_channels_to_mono(&samples, spec.channels as usize);
        }
        hound::SampleFormat::Int => {
            let max_val = (1 << (spec.bits_per_sample - 1)) as f32;
            let samples: Vec<f32> = reader.samples::<i32>()
                .filter_map(Result::ok)
                .map(|s| s as f32 / max_val)
                .collect();
            raw_samples = convert_channels_to_mono(&samples, spec.channels as usize);
        }
    }

    // Resample to 16,000 Hz if necessary
    let resampled = if spec.sample_rate != 16000 {
        resample_linear(&raw_samples, spec.sample_rate, 16000)
    } else {
        raw_samples
    };

    // Preprocess with WebRTC APM if enabled
    let processed_samples = if apm_enabled {
        let mut apm = init_apm(16000)?;
        preprocess_audio(&mut apm, &resampled)?
    } else {
        resampled
    };

    Ok(processed_samples)
}

/// Helper function to process a single WAV file and count detections
pub fn process_wav_file(path: &str, oww_model: &mut OwwModel, apm_enabled: bool) -> Result<u32> {
    let processed_samples = get_processed_samples(path, apm_enabled)?;
    let chunk_size = OWW_MODEL_CHUNK_SIZE;
    let total_samples = processed_samples.len();
    let mut offset = 0;
    let mut detections = 0;

    while offset + chunk_size <= total_samples {
        let chunk = &processed_samples[offset..offset + chunk_size];
        let triggered = oww_model.detection(chunk.to_vec());
        if triggered.detected {
            detections += 1;
        }
        offset += chunk_size;
    }
    Ok(detections)
}

/// Trait representing an audio processing stage in the pipeline
pub trait AudioStep: Send {
    /// Process a slice of input samples, returning any samples ready for the next stage
    fn process(&mut self, input: &[f32]) -> Result<Vec<f32>>;
    /// Reset any internal state (e.g. buffers, filters)
    fn reset(&mut self) -> Result<()>;
}

/// A stage that averages multi-channel inputs to mono
pub struct ChannelMixer {
    pub channels: usize,
}

impl AudioStep for ChannelMixer {
    fn process(&mut self, input: &[f32]) -> Result<Vec<f32>> {
        Ok(convert_channels_to_mono(input, self.channels))
    }
    fn reset(&mut self) -> Result<()> {
        Ok(())
    }
}

/// A stateful linear resampler designed for clean streaming audio processing
pub struct LinearResampler {
    pub from_rate: u32,
    pub to_rate: u32,
    pub leftover: Vec<f32>,
    pub next_input_idx: f64,
}

impl AudioStep for LinearResampler {
    fn process(&mut self, input: &[f32]) -> Result<Vec<f32>> {
        if self.from_rate == self.to_rate {
            return Ok(input.to_vec());
        }
        self.leftover.extend_from_slice(input);
        
        let ratio = self.to_rate as f64 / self.from_rate as f64;
        let mut output = Vec::new();
        
        while self.next_input_idx + 1.0 < self.leftover.len() as f64 {
            let idx = self.next_input_idx.floor() as usize;
            let frac = self.next_input_idx - idx as f64;
            let sample = self.leftover[idx] * (1.0 - frac) as f32 + self.leftover[idx + 1] * frac as f32;
            output.push(sample);
            self.next_input_idx += 1.0 / ratio;
        }
        
        let consumed = self.next_input_idx.floor() as usize;
        if consumed > 0 {
            self.leftover.drain(0..consumed);
            self.next_input_idx -= consumed as f64;
        }
        
        Ok(output)
    }
    
    fn reset(&mut self) -> Result<()> {
        self.leftover.clear();
        self.next_input_idx = 0.0;
        Ok(())
    }
}

/// A low-cost first-order high pass IIR filter suitable for microcontrollers
pub struct HighPassFilterStep {
    pub alpha: f32,
    pub prev_input: f32,
    pub prev_output: f32,
}

impl HighPassFilterStep {
    pub fn new(cutoff_hz: f32, sample_rate: u32) -> Self {
        let dt = 1.0 / sample_rate as f32;
        let rc = 1.0 / (2.0 * std::f32::consts::PI * cutoff_hz);
        let alpha = rc / (rc + dt);
        Self {
            alpha,
            prev_input: 0.0,
            prev_output: 0.0,
        }
    }
}

impl AudioStep for HighPassFilterStep {
    fn process(&mut self, input: &[f32]) -> Result<Vec<f32>> {
        let mut output = Vec::with_capacity(input.len());
        for &x in input {
            let y = self.alpha * (self.prev_output + x - self.prev_input);
            output.push(y);
            self.prev_input = x;
            self.prev_output = y;
        }
        Ok(output)
    }
    
    fn reset(&mut self) -> Result<()> {
        self.prev_input = 0.0;
        self.prev_output = 0.0;
        Ok(())
    }
}

/// A simple gain stage multiplier
pub struct GainApplier {
    pub factor: f32,
}

impl AudioStep for GainApplier {
    fn process(&mut self, input: &[f32]) -> Result<Vec<f32>> {
        Ok(input.iter().map(|&s| s * self.factor).collect())
    }
    fn reset(&mut self) -> Result<()> {
        Ok(())
    }
}

/// WebRTC Audio Processing Module (APM) wrapper stage
pub struct WebRtcApmStep {
    pub apm: AudioProcessing,
    pub buffer: Vec<f32>,
    pub frame_size: usize,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum NoiseSuppressionLevel {
    Low,
    Moderate,
    High,
    VeryHigh,
}

impl WebRtcApmStep {
    pub fn new(sample_rate: u32, ns_level: NoiseSuppressionLevel, agc2_enabled: bool) -> Result<Self> {
        let level = match ns_level {
            NoiseSuppressionLevel::Low => sonora::config::NoiseSuppressionLevel::Low,
            NoiseSuppressionLevel::Moderate => sonora::config::NoiseSuppressionLevel::Moderate,
            NoiseSuppressionLevel::High => sonora::config::NoiseSuppressionLevel::High,
            NoiseSuppressionLevel::VeryHigh => sonora::config::NoiseSuppressionLevel::VeryHigh,
        };
        
        let ns = sonora::config::NoiseSuppression {
            level,
            analyze_linear_aec_output_when_available: false,
        };
        
        let gain = if agc2_enabled {
            Some(sonora::config::GainController2::default())
        } else {
            None
        };
        
        let config = Config {
            noise_suppression: Some(ns),
            gain_controller2: gain,
            ..Default::default()
        };
        
        let apm = AudioProcessing::builder()
            .config(config)
            .capture_config(StreamConfig::new(sample_rate, 1))
            .render_config(StreamConfig::new(sample_rate, 1))
            .build();
            
        let frame_size = (sample_rate / 100) as usize; // 10ms frame
        
        Ok(Self {
            apm,
            buffer: Vec::new(),
            frame_size,
        })
    }
}

impl AudioStep for WebRtcApmStep {
    fn process(&mut self, input: &[f32]) -> Result<Vec<f32>> {
        self.buffer.extend_from_slice(input);
        let mut output = Vec::new();
        
        while self.buffer.len() >= self.frame_size {
            let frame: Vec<f32> = self.buffer.drain(0..self.frame_size).collect();
            let mut dest = vec![0.0f32; self.frame_size];
            self.apm.process_capture_f32(&[&frame], &mut [&mut dest])
                .map_err(|e| anyhow!("APM process error: {:?}", e))?;
            output.extend(dest);
        }
        
        Ok(output)
    }
    
    fn reset(&mut self) -> Result<()> {
        self.buffer.clear();
        Ok(())
    }
}

/// Device computational profiles for configuring the pipeline complexity
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DeviceProfile {
    /// Ultra-lightweight, bypassing heavy WebRTC APM and using simple filters
    LowPower,
    /// Lightweight WebRTC Noise Suppression, disabled AGC2
    Standard,
    /// Aggressive WebRTC Noise Suppression and AGC2 enabled
    HighPerformance,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DetectionResult {
    pub detected: bool,
    pub probability: f32,
}

/// A highly customizable, stateful audio processing and inference pipeline
pub struct WakewordPipeline {
    pub steps: Vec<Box<dyn AudioStep>>,
    pub detector: OwwModel,
    pub detector_chunk_size: usize,
    pub detector_buffer: Vec<f32>,
}

impl WakewordPipeline {
    pub fn new(
        profile: DeviceProfile,
        threshold: f32,
        model_type: SpeechUnlockType,
        input_sample_rate: u32,
        input_channels: usize,
    ) -> Result<Self> {
        let detector = OwwModel::new(model_type, threshold)
            .map_err(|e| anyhow!("Failed to initialize detector: {:?}", e))?;
            
        let mut steps: Vec<Box<dyn AudioStep>> = Vec::new();
        
        // 1. Channel Mix (if multi-channel)
        if input_channels > 1 {
            steps.push(Box::new(ChannelMixer { channels: input_channels }));
        }
        
        // 2. Resampling (if input rate is not 16000)
        if input_sample_rate != 16000 {
            steps.push(Box::new(LinearResampler {
                from_rate: input_sample_rate,
                to_rate: 16000,
                leftover: Vec::new(),
                next_input_idx: 0.0,
            }));
        }
        
        // 3. Profile-specific Preprocessing steps
        match profile {
            DeviceProfile::LowPower => {
                // Low power profile uses low-cost High-Pass Filter and Gain Applier, bypassing WebRTC APM
                steps.push(Box::new(HighPassFilterStep::new(80.0, 16000)));
                steps.push(Box::new(GainApplier { factor: 1.0 }));
            }
            DeviceProfile::Standard => {
                // Standard profile uses lightweight WebRTC Noise Suppression, disabling AGC2 to save CPU
                steps.push(Box::new(WebRtcApmStep::new(
                    16000,
                    NoiseSuppressionLevel::Moderate,
                    false,
                )?));
            }
            DeviceProfile::HighPerformance => {
                // High performance profile uses aggressive WebRTC Noise Suppression and AGC2
                steps.push(Box::new(WebRtcApmStep::new(
                    16000,
                    NoiseSuppressionLevel::High,
                    true,
                )?));
            }
        }
        
        Ok(Self {
            steps,
            detector,
            detector_chunk_size: OWW_MODEL_CHUNK_SIZE,
            detector_buffer: Vec::new(),
        })
    }
    
    /// Process incoming audio samples. Returns a list of detection results.
    /// Can accept input buffers of any size.
    pub fn process(&mut self, input: &[f32]) -> Result<Vec<DetectionResult>> {
        let mut current_buffer = input.to_vec();
        
        // Run through each audio step
        for step in &mut self.steps {
            current_buffer = step.process(&current_buffer)?;
        }
        
        // Accumulate processed 16kHz samples for the detector
        self.detector_buffer.extend_from_slice(&current_buffer);
        
        let mut detections = Vec::new();
        
        // The detector consumes chunks of size OWW_MODEL_CHUNK_SIZE (1280 samples)
        while self.detector_buffer.len() >= self.detector_chunk_size {
            let chunk: Vec<f32> = self.detector_buffer.drain(0..self.detector_chunk_size).collect();
            let result = self.detector.detection(chunk);
            detections.push(DetectionResult {
                detected: result.detected,
                probability: result.probability,
            });
        }
        
        Ok(detections)
    }
    
    /// Reset the pipeline state
    pub fn reset(&mut self) -> Result<()> {
        for step in &mut self.steps {
            step.reset()?;
        }
        self.detector_buffer.clear();
        Ok(())
    }
}
