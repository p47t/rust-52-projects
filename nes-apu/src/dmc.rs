/// NTSC DMC rate lookup table (in CPU cycles).
const DMC_RATE_TABLE: [u16; 16] = [
    428, 380, 340, 320, 286, 254, 226, 214, 190, 160, 142, 128, 106, 84, 72, 54,
];

/// Delta Modulation Channel — plays 1-bit DPCM samples from memory.
pub struct Dmc {
    pub irq_enabled: bool,
    pub loop_flag: bool,
    rate_index: u8,
    timer: u16,
    pub timer_period: u16,

    // Sample address and length (from registers)
    sample_address: u16,
    sample_length: u16,

    // Current playback state
    current_address: u16,
    bytes_remaining: u16,

    // Output unit
    pub output_level: u8,
    shift_register: u8,
    bits_remaining: u8,
    silence_flag: bool,

    // Sample buffer (fetched from CPU memory)
    sample_buffer: Option<u8>,

    /// Set when the DMC needs to fetch a byte from CPU memory.
    /// The system should read from this address and call `load_sample()`.
    pub sample_request: Option<u16>,

    pub irq_flag: bool,
    pub enabled: bool,
}

impl Default for Dmc {
    fn default() -> Self {
        Self::new()
    }
}

impl Dmc {
    pub fn new() -> Self {
        Self {
            irq_enabled: false,
            loop_flag: false,
            rate_index: 0,
            timer: 0,
            timer_period: DMC_RATE_TABLE[0],
            sample_address: 0xC000,
            sample_length: 1,
            current_address: 0xC000,
            bytes_remaining: 0,
            output_level: 0,
            shift_register: 0,
            bits_remaining: 0,
            silence_flag: true,
            sample_buffer: None,
            sample_request: None,
            irq_flag: false,
            enabled: false,
        }
    }

    /// Tick the DMC timer (called every CPU cycle).
    pub fn tick(&mut self) {
        if self.timer == 0 {
            self.timer = self.timer_period;
            self.clock_output();
        } else {
            self.timer -= 1;
        }

        // Request a sample byte if buffer is empty and bytes remain
        if self.sample_buffer.is_none() && self.bytes_remaining > 0 && self.sample_request.is_none()
        {
            self.sample_request = Some(self.current_address);
        }
    }

    fn clock_output(&mut self) {
        if self.bits_remaining == 0 {
            // Start new output cycle
            if let Some(byte) = self.sample_buffer.take() {
                self.silence_flag = false;
                self.shift_register = byte;
            } else {
                self.silence_flag = true;
            }
            self.bits_remaining = 8;
        }

        if !self.silence_flag {
            // Bit 0 of shift register: 1 = increment, 0 = decrement output level
            if self.shift_register & 1 != 0 {
                if self.output_level <= 125 {
                    self.output_level += 2;
                }
            } else if self.output_level >= 2 {
                self.output_level -= 2;
            }
            self.shift_register >>= 1;
        }

        self.bits_remaining -= 1;
    }

    /// Load a sample byte fetched from CPU memory.
    pub fn load_sample(&mut self, val: u8) {
        self.sample_buffer = Some(val);
        self.sample_request = None;
        self.current_address = self.current_address.wrapping_add(1) | 0x8000;
        self.bytes_remaining -= 1;

        if self.bytes_remaining == 0 {
            if self.loop_flag {
                self.restart();
            } else if self.irq_enabled {
                self.irq_flag = true;
            }
        }
    }

    fn restart(&mut self) {
        self.current_address = self.sample_address;
        self.bytes_remaining = self.sample_length;
    }

    pub fn save_state(&self, out: &mut Vec<u8>) {
        use nes_cpu::state::*;
        write_bool(out, self.irq_enabled);
        write_bool(out, self.loop_flag);
        write_u8(out, self.rate_index);
        write_u16(out, self.timer);
        write_u16(out, self.timer_period);
        write_u16(out, self.sample_address);
        write_u16(out, self.sample_length);
        write_u16(out, self.current_address);
        write_u16(out, self.bytes_remaining);
        write_u8(out, self.output_level);
        write_u8(out, self.shift_register);
        write_u8(out, self.bits_remaining);
        write_bool(out, self.silence_flag);
        write_bool(out, self.sample_buffer.is_some());
        write_u8(out, self.sample_buffer.unwrap_or(0));
        write_bool(out, self.irq_flag);
        write_bool(out, self.enabled);
    }

    pub fn load_state(&mut self, cursor: &mut &[u8]) {
        use nes_cpu::state::*;
        self.irq_enabled = read_bool(cursor);
        self.loop_flag = read_bool(cursor);
        self.rate_index = read_u8(cursor);
        self.timer = read_u16(cursor);
        self.timer_period = read_u16(cursor);
        self.sample_address = read_u16(cursor);
        self.sample_length = read_u16(cursor);
        self.current_address = read_u16(cursor);
        self.bytes_remaining = read_u16(cursor);
        self.output_level = read_u8(cursor);
        self.shift_register = read_u8(cursor);
        self.bits_remaining = read_u8(cursor);
        self.silence_flag = read_bool(cursor);
        let has_buf = read_bool(cursor);
        let buf_val = read_u8(cursor);
        self.sample_buffer = if has_buf { Some(buf_val) } else { None };
        self.irq_flag = read_bool(cursor);
        self.enabled = read_bool(cursor);
        self.sample_request = None;
    }

    /// Write to registers $4010-$4013.
    /// `reg` is 0-3 relative to $4010.
    pub fn write_reg(&mut self, reg: u8, val: u8) {
        match reg {
            0 => {
                // $4010: flags + rate
                self.irq_enabled = val & 0x80 != 0;
                self.loop_flag = val & 0x40 != 0;
                self.rate_index = val & 0x0F;
                self.timer_period = DMC_RATE_TABLE[self.rate_index as usize];
                if !self.irq_enabled {
                    self.irq_flag = false;
                }
            }
            1 => {
                // $4011: direct load (7-bit)
                self.output_level = val & 0x7F;
            }
            2 => {
                // $4012: sample address = %11AAAAAA.AA000000 = $C000 + A * 64
                self.sample_address = 0xC000 + (val as u16) * 64;
            }
            3 => {
                // $4013: sample length = %LLLL.LLLL0001 = L * 16 + 1
                self.sample_length = (val as u16) * 16 + 1;
            }
            _ => {}
        }
    }

    /// Enable/disable the DMC channel (called from $4015 write).
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        self.irq_flag = false;
        if !enabled {
            self.bytes_remaining = 0;
        } else if self.bytes_remaining == 0 {
            self.restart();
        }
    }

    /// Current output level (0-127).
    pub fn output(&self) -> u8 {
        self.output_level
    }

    /// Returns the number of sample bytes remaining.
    pub fn bytes_remaining(&self) -> u16 {
        self.bytes_remaining
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_direct_load() {
        let mut d = Dmc::new();
        d.write_reg(1, 0x40);
        assert_eq!(d.output_level, 0x40);
    }

    #[test]
    fn test_sample_address() {
        let mut d = Dmc::new();
        d.write_reg(2, 0x00);
        assert_eq!(d.sample_address, 0xC000);
        d.write_reg(2, 0x01);
        assert_eq!(d.sample_address, 0xC040);
    }

    #[test]
    fn test_sample_length() {
        let mut d = Dmc::new();
        d.write_reg(3, 0x00);
        assert_eq!(d.sample_length, 1);
        d.write_reg(3, 0x01);
        assert_eq!(d.sample_length, 17);
    }
}
