use crate::length_counter::LengthCounter;

/// 32-step triangle waveform sequence.
const TRIANGLE_SEQUENCE: [u8; 32] = [
    15, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12,
    13, 14, 15,
];

pub struct Triangle {
    pub timer_period: u16,
    timer: u16,
    sequencer_pos: u8,
    pub length_counter: LengthCounter,
    pub linear_counter: u8,
    pub linear_counter_reload_value: u8,
    pub linear_counter_reload_flag: bool,
    pub control_flag: bool, // also serves as length counter halt
}

impl Default for Triangle {
    fn default() -> Self {
        Self::new()
    }
}

impl Triangle {
    pub fn new() -> Self {
        Self {
            timer_period: 0,
            timer: 0,
            sequencer_pos: 0,
            length_counter: LengthCounter::new(),
            linear_counter: 0,
            linear_counter_reload_value: 0,
            linear_counter_reload_flag: false,
            control_flag: false,
        }
    }

    /// Tick the triangle timer (called every CPU cycle — NOT halved like pulse/noise).
    pub fn tick(&mut self) {
        if self.timer == 0 {
            self.timer = self.timer_period;
            // Only advance sequencer if both counters are non-zero
            if self.linear_counter > 0 && self.length_counter.is_active() {
                self.sequencer_pos = (self.sequencer_pos + 1) % 32;
            }
        } else {
            self.timer -= 1;
        }
    }

    /// Current output sample (0-15).
    ///
    /// On real hardware, the triangle DAC always outputs the current sequencer
    /// position. The length counter and linear counter only gate sequencer
    /// advancement (handled in `tick()`), they do not silence the output.
    pub fn output(&self) -> u8 {
        TRIANGLE_SEQUENCE[self.sequencer_pos as usize]
    }

    /// Clock the linear counter (called at quarter-frame rate).
    pub fn clock_linear_counter(&mut self) {
        if self.linear_counter_reload_flag {
            self.linear_counter = self.linear_counter_reload_value;
        } else if self.linear_counter > 0 {
            self.linear_counter -= 1;
        }
        if !self.control_flag {
            self.linear_counter_reload_flag = false;
        }
    }

    pub fn save_state(&self, out: &mut Vec<u8>) {
        use nes_cpu::state::*;
        write_u16(out, self.timer_period);
        write_u16(out, self.timer);
        write_u8(out, self.sequencer_pos);
        self.length_counter.save_state(out);
        write_u8(out, self.linear_counter);
        write_u8(out, self.linear_counter_reload_value);
        write_bool(out, self.linear_counter_reload_flag);
        write_bool(out, self.control_flag);
    }

    pub fn load_state(&mut self, cursor: &mut &[u8]) {
        use nes_cpu::state::*;
        self.timer_period = read_u16(cursor);
        self.timer = read_u16(cursor);
        self.sequencer_pos = read_u8(cursor);
        self.length_counter.load_state(cursor);
        self.linear_counter = read_u8(cursor);
        self.linear_counter_reload_value = read_u8(cursor);
        self.linear_counter_reload_flag = read_bool(cursor);
        self.control_flag = read_bool(cursor);
    }

    /// Write to registers $4008-$400B.
    /// `reg` is 0-3 relative to $4008.
    pub fn write_reg(&mut self, reg: u8, val: u8) {
        match reg {
            0 => {
                // $4008: control flag / linear counter reload value
                self.control_flag = val & 0x80 != 0;
                self.length_counter.halt = self.control_flag;
                self.linear_counter_reload_value = val & 0x7F;
            }
            1 => {} // $4009: unused
            2 => {
                // $400A: timer low
                self.timer_period = (self.timer_period & 0x0700) | val as u16;
            }
            3 => {
                // $400B: timer high + length counter load
                self.timer_period = (self.timer_period & 0x00FF) | ((val as u16 & 0x07) << 8);
                self.length_counter.load((val >> 3) & 0x1F);
                self.linear_counter_reload_flag = true;
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_triangle_sequence() {
        assert_eq!(TRIANGLE_SEQUENCE[0], 15);
        assert_eq!(TRIANGLE_SEQUENCE[15], 0);
        assert_eq!(TRIANGLE_SEQUENCE[16], 0);
        assert_eq!(TRIANGLE_SEQUENCE[31], 15);
    }

    #[test]
    fn test_output_advances() {
        let mut t = Triangle::new();
        t.length_counter.enable();
        t.write_reg(0, 0xFF); // control=true, linear reload=127
        t.write_reg(2, 0x00); // timer low
        t.write_reg(3, 0x08); // timer high=0, length load
        t.linear_counter = 10;
        // With timer_period=0, each tick advances the sequencer
        assert_eq!(t.output(), TRIANGLE_SEQUENCE[0]);
        t.tick(); // timer wraps, sequencer advances to 1
        assert_eq!(t.output(), TRIANGLE_SEQUENCE[1]);
    }

    #[test]
    fn test_gated_by_linear_counter() {
        let mut t = Triangle::new();
        t.length_counter.enable();
        t.write_reg(3, 0x08); // load length
        t.linear_counter = 0;
        let pos_before = t.sequencer_pos;
        t.tick();
        // Sequencer should NOT advance when linear counter is 0
        assert_eq!(t.sequencer_pos, pos_before);
        // But output should still reflect current sequencer position (not silenced)
        assert_eq!(t.output(), TRIANGLE_SEQUENCE[pos_before as usize]);
    }
}
