use crate::envelope::Envelope;
use crate::length_counter::LengthCounter;
use crate::sweep::{NegateMode, Sweep};

/// Duty cycle waveform sequences (8 steps each).
const DUTY_TABLE: [[u8; 8]; 4] = [
    [0, 0, 0, 0, 0, 0, 0, 1], // 12.5%
    [0, 0, 0, 0, 0, 0, 1, 1], // 25%
    [0, 0, 0, 0, 1, 1, 1, 1], // 50%
    [1, 1, 1, 1, 1, 1, 0, 0], // 75% (inverted 25%)
];

pub struct Pulse {
    pub duty: u8,
    pub timer_period: u16,
    timer: u16,
    sequencer_pos: u8,
    pub envelope: Envelope,
    pub length_counter: LengthCounter,
    pub sweep: Sweep,
}

impl Pulse {
    pub fn new(negate_mode: NegateMode) -> Self {
        Self {
            duty: 0,
            timer_period: 0,
            timer: 0,
            sequencer_pos: 0,
            envelope: Envelope::new(),
            length_counter: LengthCounter::new(),
            sweep: Sweep::new(negate_mode),
        }
    }

    /// Tick the pulse timer (called at CPU/2 rate — every other CPU cycle).
    pub fn tick(&mut self) {
        if self.timer == 0 {
            self.timer = self.timer_period;
            self.sequencer_pos = (self.sequencer_pos + 1) % 8;
        } else {
            self.timer -= 1;
        }
    }

    /// Current output sample (0-15).
    pub fn output(&self) -> u8 {
        if !self.length_counter.is_active() {
            return 0;
        }
        if self.sweep.muting(self.timer_period) {
            return 0;
        }
        if DUTY_TABLE[self.duty as usize][self.sequencer_pos as usize] == 0 {
            return 0;
        }
        self.envelope.volume()
    }

    /// Write to registers $4000-$4003 (or $4004-$4007 for pulse 2).
    /// `reg` is 0-3 relative to the channel base.
    pub fn write_reg(&mut self, reg: u8, val: u8) {
        match reg {
            0 => {
                self.duty = (val >> 6) & 0x03;
                self.length_counter.halt = val & 0x20 != 0;
                self.envelope.write(val);
            }
            1 => {
                self.sweep.write(val);
            }
            2 => {
                // Timer low 8 bits
                self.timer_period = (self.timer_period & 0x0700) | val as u16;
            }
            3 => {
                // Timer high 3 bits + length counter load
                self.timer_period = (self.timer_period & 0x00FF) | ((val as u16 & 0x07) << 8);
                self.length_counter.load((val >> 3) & 0x1F);
                self.sequencer_pos = 0;
                self.envelope.start = true;
            }
            _ => {}
        }
    }

    /// Clock sweep, updating timer_period if needed.
    pub fn clock_sweep(&mut self) {
        if let Some(new_period) = self.sweep.clock(self.timer_period) {
            self.timer_period = new_period;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_duty_select() {
        let mut p = Pulse::new(NegateMode::TwosComplement);
        p.length_counter.enable();
        p.write_reg(0, 0xBF); // duty=2 (50%), constant vol=15
        p.write_reg(2, 0x80); // timer low = 128 (period >= 8 to avoid muting)
        p.write_reg(3, 0x08); // timer high=0, length load
                              // With 50% duty, 4 of 8 steps produce output
        let mut nonzero = 0;
        for _ in 0..8 {
            if p.output() > 0 {
                nonzero += 1;
            }
            p.sequencer_pos = (p.sequencer_pos + 1) % 8;
        }
        assert_eq!(nonzero, 4);
    }

    #[test]
    fn test_muted_when_period_low() {
        let mut p = Pulse::new(NegateMode::TwosComplement);
        p.length_counter.enable();
        p.write_reg(0, 0xBF);
        p.write_reg(3, 0x08);
        p.timer_period = 3; // < 8, should be muted
        assert_eq!(p.output(), 0);
    }
}
