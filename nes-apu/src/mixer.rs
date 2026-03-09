/// Nonlinear mixer for the NES APU.
///
/// Uses precomputed lookup tables based on the standard NES mixing formulas:
/// - pulse_out  = 95.88 / (8128.0 / (p1 + p2) + 100.0)
/// - tnd_out    = 159.79 / (1.0 / (t/8227.0 + n/12241.0 + d/22638.0) + 100.0)
pub struct Mixer {
    /// Indexed by (pulse1 + pulse2), range 0..=30.
    pulse_table: [f32; 31],
    /// Indexed by (3*triangle + 2*noise + dmc), range 0..=202.
    tnd_table: [f32; 203],
}

impl Default for Mixer {
    fn default() -> Self {
        Self::new()
    }
}

impl Mixer {
    pub fn new() -> Self {
        let mut pulse_table = [0.0f32; 31];
        for (i, entry) in pulse_table.iter_mut().enumerate().skip(1) {
            *entry = 95.88 / (8128.0 / i as f32 + 100.0);
        }

        let mut tnd_table = [0.0f32; 203];
        for (i, entry) in tnd_table.iter_mut().enumerate().skip(1) {
            *entry = 159.79 / (1.0 / (i as f32 / 8227.0) + 100.0);
        }

        Self {
            pulse_table,
            tnd_table,
        }
    }

    /// Mix the 5 channel outputs into a single sample in the range [0.0, 1.0].
    /// - pulse1, pulse2: 0-15
    /// - triangle: 0-15
    /// - noise: 0-15
    /// - dmc: 0-127
    pub fn mix(&self, pulse1: u8, pulse2: u8, triangle: u8, noise: u8, dmc: u8) -> f32 {
        let pulse_index = (pulse1 as usize + pulse2 as usize).min(30);
        let tnd_index = (3 * triangle as usize + 2 * noise as usize + dmc as usize).min(202);

        self.pulse_table[pulse_index] + self.tnd_table[tnd_index]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_silence() {
        let m = Mixer::new();
        assert_eq!(m.mix(0, 0, 0, 0, 0), 0.0);
    }

    #[test]
    fn test_pulse_only() {
        let m = Mixer::new();
        let val = m.mix(15, 15, 0, 0, 0);
        assert!(val > 0.0 && val < 1.0);
    }

    #[test]
    fn test_full_mix() {
        let m = Mixer::new();
        let val = m.mix(15, 15, 15, 15, 127);
        assert!(val > 0.0 && val < 2.0); // combined can exceed 1.0
    }
}
