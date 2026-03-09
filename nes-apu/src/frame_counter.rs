/// Frame counter mode: 4-step (with optional IRQ) or 5-step (no IRQ).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    FourStep,
    FiveStep,
}

/// Events produced by the frame counter each step.
#[derive(Clone, Copy)]
pub struct FrameEvent {
    /// Clock envelopes and triangle linear counter.
    pub quarter_frame: bool,
    /// Clock length counters and sweep units.
    pub half_frame: bool,
    /// Fire IRQ (4-step mode only, when not inhibited).
    pub irq: bool,
}

/// APU frame counter / sequencer. Divides CPU cycles into ~240Hz quarter-frame
/// and ~120Hz half-frame clocks for the envelope, sweep, and length counter units.
pub struct FrameCounter {
    pub mode: Mode,
    pub irq_inhibit: bool,
    pub irq_flag: bool,
    /// CPU cycle counter within the current frame sequence.
    cycle: u16,
    /// If true, immediately clock on reset.
    pending_immediate_clock: bool,
}

// NTSC frame counter step boundaries (in CPU cycles from frame start).
// 4-step mode: steps at 3729, 7457, 11186, 14915 (and IRQ at 14914)
// 5-step mode: steps at 3729, 7457, 11186, 14915, 18641
const STEP_4: [u16; 4] = [3729, 7457, 11186, 14914];
const STEP_5: [u16; 5] = [3729, 7457, 11186, 14915, 18641];

impl Default for FrameCounter {
    fn default() -> Self {
        Self::new()
    }
}

impl FrameCounter {
    pub fn new() -> Self {
        Self {
            mode: Mode::FourStep,
            irq_inhibit: false,
            irq_flag: false,
            cycle: 0,
            pending_immediate_clock: false,
        }
    }

    /// Write $4017 frame counter register.
    pub fn write(&mut self, val: u8) {
        self.mode = if val & 0x80 != 0 {
            Mode::FiveStep
        } else {
            Mode::FourStep
        };
        self.irq_inhibit = val & 0x40 != 0;
        if self.irq_inhibit {
            self.irq_flag = false;
        }
        // Reset is delayed by 3-4 CPU cycles; we simplify to immediate reset
        self.cycle = 0;
        // 5-step mode immediately clocks on write
        if self.mode == Mode::FiveStep {
            self.pending_immediate_clock = true;
        }
    }

    /// Tick one CPU cycle. Returns frame events if any step boundary is crossed.
    pub fn tick(&mut self) -> Option<FrameEvent> {
        // Handle immediate clock from 5-step mode write
        if self.pending_immediate_clock {
            self.pending_immediate_clock = false;
            return Some(FrameEvent {
                quarter_frame: true,
                half_frame: true,
                irq: false,
            });
        }

        self.cycle += 1;

        match self.mode {
            Mode::FourStep => self.tick_four_step(),
            Mode::FiveStep => self.tick_five_step(),
        }
    }

    fn tick_four_step(&mut self) -> Option<FrameEvent> {
        match self.cycle {
            c if c == STEP_4[0] => Some(FrameEvent {
                quarter_frame: true,
                half_frame: false,
                irq: false,
            }),
            c if c == STEP_4[1] => Some(FrameEvent {
                quarter_frame: true,
                half_frame: true,
                irq: false,
            }),
            c if c == STEP_4[2] => Some(FrameEvent {
                quarter_frame: true,
                half_frame: false,
                irq: false,
            }),
            c if c == STEP_4[3] => {
                let irq = !self.irq_inhibit;
                if irq {
                    self.irq_flag = true;
                }
                self.cycle = 0;
                Some(FrameEvent {
                    quarter_frame: true,
                    half_frame: true,
                    irq,
                })
            }
            _ => None,
        }
    }

    fn tick_five_step(&mut self) -> Option<FrameEvent> {
        match self.cycle {
            c if c == STEP_5[0] => Some(FrameEvent {
                quarter_frame: true,
                half_frame: false,
                irq: false,
            }),
            c if c == STEP_5[1] => Some(FrameEvent {
                quarter_frame: true,
                half_frame: true,
                irq: false,
            }),
            c if c == STEP_5[2] => Some(FrameEvent {
                quarter_frame: true,
                half_frame: false,
                irq: false,
            }),
            c if c == STEP_5[3] => None, // step 4 is empty in 5-step mode
            c if c == STEP_5[4] => {
                self.cycle = 0;
                Some(FrameEvent {
                    quarter_frame: true,
                    half_frame: true,
                    irq: false, // 5-step mode never generates IRQ
                })
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_four_step_sequence() {
        let mut fc = FrameCounter::new();
        fc.mode = Mode::FourStep;
        fc.irq_inhibit = false;

        // Tick to first quarter frame
        let mut events = Vec::new();
        for _ in 0..STEP_4[3] + 1 {
            if let Some(e) = fc.tick() {
                events.push(e);
            }
        }
        // Should have 4 events: 3 quarter frames + 1 final (quarter+half+irq)
        assert_eq!(events.len(), 4);
        assert!(events[0].quarter_frame && !events[0].half_frame);
        assert!(events[1].quarter_frame && events[1].half_frame);
        assert!(events[2].quarter_frame && !events[2].half_frame);
        assert!(events[3].quarter_frame && events[3].half_frame && events[3].irq);
    }

    #[test]
    fn test_irq_inhibit() {
        let mut fc = FrameCounter::new();
        fc.mode = Mode::FourStep;
        fc.irq_inhibit = true;

        for _ in 0..STEP_4[3] + 1 {
            if let Some(e) = fc.tick() {
                assert!(!e.irq);
            }
        }
        assert!(!fc.irq_flag);
    }

    #[test]
    fn test_five_step_no_irq() {
        let mut fc = FrameCounter::new();
        fc.write(0x80); // 5-step mode

        // Skip the immediate clock event
        let _ = fc.tick();

        for _ in 0..STEP_5[4] + 1 {
            if let Some(e) = fc.tick() {
                assert!(!e.irq);
            }
        }
    }
}
