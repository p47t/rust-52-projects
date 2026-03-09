/// Envelope unit shared by Pulse and Noise channels.
/// Generates a 4-bit volume that either stays constant or decays over time.
pub struct Envelope {
    pub start: bool,
    pub loop_flag: bool,
    pub constant_volume: bool,
    pub divider_period: u8,
    divider: u8,
    decay_level: u8,
}

impl Default for Envelope {
    fn default() -> Self {
        Self::new()
    }
}

impl Envelope {
    pub fn new() -> Self {
        Self {
            start: false,
            loop_flag: false,
            constant_volume: false,
            divider_period: 0,
            divider: 0,
            decay_level: 0,
        }
    }

    /// Clock the envelope (called at quarter-frame rate).
    pub fn clock(&mut self) {
        if self.start {
            self.start = false;
            self.decay_level = 15;
            self.divider = self.divider_period;
        } else if self.divider == 0 {
            self.divider = self.divider_period;
            if self.decay_level > 0 {
                self.decay_level -= 1;
            } else if self.loop_flag {
                self.decay_level = 15;
            }
        } else {
            self.divider -= 1;
        }
    }

    /// Current volume output (0-15).
    pub fn volume(&self) -> u8 {
        if self.constant_volume {
            self.divider_period
        } else {
            self.decay_level
        }
    }

    /// Write bits from register 0 of a channel ($4000/$4004/$400C).
    pub fn write(&mut self, val: u8) {
        self.constant_volume = val & 0x10 != 0;
        self.loop_flag = val & 0x20 != 0;
        self.divider_period = val & 0x0F;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_constant_volume() {
        let mut env = Envelope::new();
        env.write(0x1A); // constant=true, period=0xA
        assert_eq!(env.volume(), 0x0A);
    }

    #[test]
    fn test_decay() {
        let mut env = Envelope::new();
        env.write(0x05); // constant=false, period=5
        env.start = true;
        env.clock(); // start: decay=15, divider=5
        assert_eq!(env.volume(), 15);
        for _ in 0..6 {
            env.clock(); // divider counts down 5..0, then decay=14
        }
        assert_eq!(env.volume(), 14);
    }

    #[test]
    fn test_loop() {
        let mut env = Envelope::new();
        env.write(0x20); // loop=true, constant=false, period=0
        env.start = true;
        env.clock(); // start: decay=15, divider=0
                     // Each clock: divider is 0, so decay decrements each time
        for _ in 0..15 {
            env.clock();
        }
        assert_eq!(env.volume(), 0);
        env.clock(); // loop wraps to 15
        assert_eq!(env.volume(), 15);
    }
}
