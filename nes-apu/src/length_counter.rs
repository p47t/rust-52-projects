/// Length counter lookup table (32 entries, indexed by the upper 5 bits of register writes).
const LENGTH_TABLE: [u8; 32] = [
    10, 254, 20, 2, 40, 4, 80, 6, 160, 8, 60, 10, 14, 12, 26, 14, 12, 16, 24, 18, 48, 20, 96, 22,
    192, 24, 72, 26, 16, 28, 32, 30,
];

pub struct LengthCounter {
    pub counter: u8,
    pub halt: bool,
    pub enabled: bool,
}

impl Default for LengthCounter {
    fn default() -> Self {
        Self::new()
    }
}

impl LengthCounter {
    pub fn new() -> Self {
        Self {
            counter: 0,
            halt: false,
            enabled: false,
        }
    }

    /// Clock the length counter (called at half-frame rate).
    pub fn clock(&mut self) {
        if !self.halt && self.counter > 0 {
            self.counter -= 1;
        }
    }

    /// Load a new length value from the lookup table.
    pub fn load(&mut self, index: u8) {
        if self.enabled {
            self.counter = LENGTH_TABLE[index as usize];
        }
    }

    /// Returns true if the channel should produce output.
    pub fn is_active(&self) -> bool {
        self.counter > 0
    }

    /// Called when the channel enable bit is cleared in $4015.
    pub fn disable(&mut self) {
        self.enabled = false;
        self.counter = 0;
    }

    /// Called when the channel enable bit is set in $4015.
    pub fn enable(&mut self) {
        self.enabled = true;
    }

    pub fn save_state(&self, out: &mut Vec<u8>) {
        use nes_cpu::state::*;
        write_u8(out, self.counter);
        write_bool(out, self.halt);
        write_bool(out, self.enabled);
    }

    pub fn load_state(&mut self, cursor: &mut &[u8]) {
        use nes_cpu::state::*;
        self.counter = read_u8(cursor);
        self.halt = read_bool(cursor);
        self.enabled = read_bool(cursor);
    }

    /// Set enabled state from $4015 write. Disabling clears the counter.
    pub fn set_enabled(&mut self, enabled: bool) {
        if enabled {
            self.enable();
        } else {
            self.disable();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_from_table() {
        let mut lc = LengthCounter::new();
        lc.enable();
        lc.load(0); // index 0 = 10
        assert_eq!(lc.counter, 10);
        lc.load(1); // index 1 = 254
        assert_eq!(lc.counter, 254);
    }

    #[test]
    fn test_clock_decrements() {
        let mut lc = LengthCounter::new();
        lc.enable();
        lc.load(0); // 10
        lc.clock();
        assert_eq!(lc.counter, 9);
    }

    #[test]
    fn test_halt_prevents_clock() {
        let mut lc = LengthCounter::new();
        lc.enable();
        lc.load(0);
        lc.halt = true;
        lc.clock();
        assert_eq!(lc.counter, 10); // unchanged
    }

    #[test]
    fn test_disable_clears_counter() {
        let mut lc = LengthCounter::new();
        lc.enable();
        lc.load(0);
        assert!(lc.is_active());
        lc.disable();
        assert!(!lc.is_active());
        assert_eq!(lc.counter, 0);
    }
}
