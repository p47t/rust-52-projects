use crate::envelope::Envelope;
use crate::length_counter::LengthCounter;

/// NTSC noise period lookup table.
const NOISE_PERIOD_TABLE: [u16; 16] = [
    4, 8, 16, 32, 64, 96, 128, 160, 202, 254, 380, 508, 762, 1016, 2034, 4068,
];

pub struct Noise {
    timer_period: u16,
    timer: u16,
    /// 15-bit linear feedback shift register.
    shift_register: u16,
    /// When true, use bit 6 for feedback (short mode); otherwise bit 1 (long mode).
    mode: bool,
    pub envelope: Envelope,
    pub length_counter: LengthCounter,
}

impl Default for Noise {
    fn default() -> Self {
        Self::new()
    }
}

impl Noise {
    pub fn new() -> Self {
        Self {
            timer_period: 0,
            timer: 0,
            shift_register: 1, // initial value
            mode: false,
            envelope: Envelope::new(),
            length_counter: LengthCounter::new(),
        }
    }

    /// Tick the noise timer (called at CPU/2 rate — every other CPU cycle).
    pub fn tick(&mut self) {
        if self.timer == 0 {
            self.timer = self.timer_period;
            self.clock_shift_register();
        } else {
            self.timer -= 1;
        }
    }

    fn clock_shift_register(&mut self) {
        let feedback_bit = if self.mode { 6 } else { 1 };
        let feedback = (self.shift_register & 1) ^ ((self.shift_register >> feedback_bit) & 1);
        self.shift_register >>= 1;
        self.shift_register |= feedback << 14;
    }

    /// Current output sample (0-15).
    pub fn output(&self) -> u8 {
        if !self.length_counter.is_active() {
            return 0;
        }
        if self.shift_register & 1 != 0 {
            return 0;
        }
        self.envelope.volume()
    }

    /// Write to registers $400C-$400F.
    /// `reg` is 0-3 relative to $400C.
    pub fn write_reg(&mut self, reg: u8, val: u8) {
        match reg {
            0 => {
                // $400C: envelope
                self.length_counter.halt = val & 0x20 != 0;
                self.envelope.write(val);
            }
            1 => {} // $400D: unused
            2 => {
                // $400E: mode + period
                self.mode = val & 0x80 != 0;
                self.timer_period = NOISE_PERIOD_TABLE[(val & 0x0F) as usize];
            }
            3 => {
                // $400F: length counter load
                self.length_counter.load((val >> 3) & 0x1F);
                self.envelope.start = true;
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_initial_shift_register() {
        let n = Noise::new();
        assert_eq!(n.shift_register, 1);
    }

    #[test]
    fn test_shift_register_changes() {
        let mut n = Noise::new();
        let initial = n.shift_register;
        n.clock_shift_register();
        // After one clock, shift register should change
        assert_ne!(n.shift_register, initial);
    }

    #[test]
    fn test_muted_when_disabled() {
        let n = Noise::new();
        assert_eq!(n.output(), 0); // length counter not active
    }
}
