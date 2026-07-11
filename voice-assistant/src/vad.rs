/// Encapsulates voice activity detection (VAD) and recording buffer state.
/// Tracks RMS volume levels to decide when the user has finished speaking.
pub struct VadRecorder {
    recording_samples: Vec<f32>,
    silence_frames_count: usize,
    total_frames_count: usize,
    vad_threshold: f32,
    silence_limit_frames: usize,
    max_recording_frames: usize,
    min_recording_frames: usize,
}

impl VadRecorder {
    pub fn new(vad_threshold: f32, silence_duration_sec: f32, max_duration_sec: f32) -> Self {
        // Frames are 10ms (0.010s)
        let silence_limit_frames = (silence_duration_sec / 0.010).round() as usize;
        let max_recording_frames = (max_duration_sec / 0.010).round() as usize;
        let min_recording_frames = 100; // Minimum 1.0 second recording to avoid micro-silence cutoffs

        Self {
            recording_samples: Vec::new(),
            silence_frames_count: 0,
            total_frames_count: 0,
            vad_threshold,
            silence_limit_frames,
            max_recording_frames,
            min_recording_frames,
        }
    }

    /// Reset recorder state for a new recording session
    pub fn reset(&mut self) {
        self.recording_samples.clear();
        self.silence_frames_count = 0;
        self.total_frames_count = 0;
    }

    /// Processes a single 10ms preprocessed frame.
    /// Returns a tuple `(is_finished, current_rms)` indicating:
    /// - `is_finished`: Whether recording should stop (due to silence VAD or timeout)
    /// - `current_rms`: The Root Mean Square energy of the frame
    pub fn process_frame(&mut self, frame: &[f32]) -> (bool, f32) {
        self.recording_samples.extend_from_slice(frame);
        self.total_frames_count += 1;

        // Compute Root Mean Square (RMS) for energy-based VAD
        let mut sum_squares = 0.0f32;
        for &sample in frame {
            sum_squares += sample * sample;
        }
        let rms = (sum_squares / frame.len() as f32).sqrt();

        // Increment silence frames if energy is below threshold, reset if voice is active
        if rms < self.vad_threshold {
            self.silence_frames_count += 1;
        } else {
            self.silence_frames_count = 0;
        }

        // Check completion criteria
        let is_silent = self.silence_frames_count >= self.silence_limit_frames 
            && self.total_frames_count >= self.min_recording_frames;
        let hit_max = self.total_frames_count >= self.max_recording_frames;

        (is_silent || hit_max, rms)
    }

    /// Access the captured audio samples
    pub fn samples(&self) -> &[f32] {
        &self.recording_samples
    }
}
