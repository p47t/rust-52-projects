/// Sweep unit for pulse channels. Adjusts the pulse timer period up or down.
/// Pulse 1 and Pulse 2 use different negation modes.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum NegateMode {
    /// Pulse 1: ones' complement (complement then no adjustment)
    OnesComplement,
    /// Pulse 2: twos' complement (complement then add 1, i.e. standard negation)
    TwosComplement,
}

pub struct Sweep {
    pub enabled: bool,
    pub period: u8,
    pub negate: bool,
    pub shift: u8,
    pub reload: bool,
    pub negate_mode: NegateMode,
    divider: u8,
}

impl Sweep {
    pub fn new(negate_mode: NegateMode) -> Self {
        Self {
            enabled: false,
            period: 0,
            negate: false,
            shift: 0,
            reload: false,
            negate_mode,
            divider: 0,
        }
    }

    pub fn save_state(&self, out: &mut Vec<u8>) {
        use nes_cpu::state::*;
        write_bool(out, self.enabled);
        write_u8(out, self.period);
        write_bool(out, self.negate);
        write_u8(out, self.shift);
        write_bool(out, self.reload);
        write_u8(out, self.divider);
    }

    pub fn load_state(&mut self, cursor: &mut &[u8]) {
        use nes_cpu::state::*;
        self.enabled = read_bool(cursor);
        self.period = read_u8(cursor);
        self.negate = read_bool(cursor);
        self.shift = read_u8(cursor);
        self.reload = read_bool(cursor);
        self.divider = read_u8(cursor);
    }

    /// Write the sweep register ($4001/$4005).
    pub fn write(&mut self, val: u8) {
        self.enabled = val & 0x80 != 0;
        self.period = (val >> 4) & 0x07;
        self.negate = val & 0x08 != 0;
        self.shift = val & 0x07;
        self.reload = true;
    }

    /// Compute the target period given the current timer period.
    pub fn target_period(&self, current_period: u16) -> u16 {
        let change = current_period >> self.shift;
        if self.negate {
            let neg = match self.negate_mode {
                NegateMode::OnesComplement => change.wrapping_add(1),
                NegateMode::TwosComplement => change,
            };
            current_period.wrapping_sub(neg)
        } else {
            current_period.wrapping_add(change)
        }
    }

    /// Returns true if the sweep would mute the channel.
    pub fn muting(&self, current_period: u16) -> bool {
        current_period < 8 || self.target_period(current_period) > 0x7FF
    }

    /// Clock the sweep unit (called at half-frame rate). Returns new timer period if changed.
    pub fn clock(&mut self, current_period: u16) -> Option<u16> {
        let mut new_period = None;

        if self.divider == 0 && self.enabled && !self.muting(current_period) && self.shift > 0 {
            let target = self.target_period(current_period);
            new_period = Some(target);
        }

        if self.divider == 0 || self.reload {
            self.divider = self.period;
            self.reload = false;
        } else {
            self.divider -= 1;
        }

        new_period
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_target_period_add() {
        let mut s = Sweep::new(NegateMode::OnesComplement);
        s.shift = 2;
        s.negate = false;
        // current=400, shift=2 → change=100, target=500
        assert_eq!(s.target_period(400), 500);
    }

    #[test]
    fn test_target_period_negate_ones() {
        let mut s = Sweep::new(NegateMode::OnesComplement);
        s.shift = 1;
        s.negate = true;
        // current=400, shift=1 → change=200, ones' comp subtract = 400 - 201 = 199
        assert_eq!(s.target_period(400), 199);
    }

    #[test]
    fn test_target_period_negate_twos() {
        let mut s = Sweep::new(NegateMode::TwosComplement);
        s.shift = 1;
        s.negate = true;
        // current=400, shift=1 → change=200, twos' comp subtract = 400 - 200 = 200
        assert_eq!(s.target_period(400), 200);
    }

    #[test]
    fn test_muting_low_period() {
        let s = Sweep::new(NegateMode::OnesComplement);
        assert!(s.muting(7)); // < 8
        assert!(!s.muting(8));
    }
}
