use anyhow::{anyhow, Result};
use wakeword_engine::{OwwModel, SpeechUnlockType, OWW_MODEL_CHUNK_SIZE};

/// Encapsulates the openWakeWord model and details of feeding
/// preprocessed samples to detect wakewords in 1280-sample chunks.
pub struct WakewordDetector {
    oww_model: OwwModel,
    oww_chunk_buffer: Vec<f32>,
}

impl WakewordDetector {
    pub fn new(model_type: SpeechUnlockType, threshold: f32) -> Result<Self> {
        let oww_model = OwwModel::new(model_type, threshold)
            .map_err(|e| anyhow!("Failed to initialize openWakeWord model: {:?}", e))?;
        
        Ok(Self {
            oww_model,
            oww_chunk_buffer: Vec::new(),
        })
    }

    /// Feeds 16 kHz preprocessed mono samples and checks for wake words.
    /// Returns true if a wake word was detected.
    pub fn feed_and_detect(&mut self, samples: &[f32]) -> bool {
        if samples.is_empty() {
            return false;
        }

        self.oww_chunk_buffer.extend_from_slice(samples);
        let mut triggered = false;

        // openWakeWord operates on chunks of 1280 samples
        while self.oww_chunk_buffer.len() >= OWW_MODEL_CHUNK_SIZE {
            let chunk: Vec<f32> = self.oww_chunk_buffer.drain(0..OWW_MODEL_CHUNK_SIZE).collect();
            let result = self.oww_model.detection(chunk);
            if result.detected {
                triggered = true;
            }
        }

        triggered
    }

    /// Reset any accumulated samples
    pub fn reset(&mut self) {
        self.oww_chunk_buffer.clear();
    }
}
