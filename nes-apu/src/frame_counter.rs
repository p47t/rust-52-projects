/// Frame counter mode: 4-step (with optional IRQ) or 5-step (no IRQ).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    FourStep,
    FiveStep,
}

/// Events produced by the frame counter each step.
#[derive(Clone, Copy, Debug)]
pub struct FrameEvent {
    /// Clock envelopes and triangle linear counter.
    pub quarter_frame: bool,
    /// Clock length counters and sweep units.
    pub half_frame: bool,
    /// Fire IRQ (4-step mode only, when not inhibited).
    pub irq: bool,
}

/// APU frame counter / sequencer. Divides APU cycles into ~240Hz quarter-frame
/// and ~120Hz half-frame clocks for the envelope, sweep, and length counter units.
///
/// The frame counter is clocked once per APU cycle (every other CPU cycle).
/// Step boundaries are specified in APU cycles.
///
/// 4-step mode (mode bit = 0):
///   3729  — quarter frame
///   7457  — quarter + half frame
///   11186 — quarter frame
///   14914 — IRQ flag set (no clocks)
///   14915 — quarter + half frame, IRQ flag set, sequence resets
///   0     — IRQ flag set (post-reset, one more cycle)
///
/// 5-step mode (mode bit = 1):
///   3729  — quarter frame
///   7457  — quarter + half frame
///   11186 — quarter frame
///   14915 — (empty step)
///   18641 — quarter + half frame, sequence resets
///   No IRQ in 5-step mode. Immediate half+quarter clock on write.
pub struct FrameCounter {
    pub mode: Mode,
    pub irq_inhibit: bool,
    pub irq_flag: bool,
    /// APU cycle counter within the current frame sequence.
    cycle: u16,
    /// After 4-step sequence resets, IRQ flag gets set one more cycle.
    new_sequence_irq: bool,
}

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
            new_sequence_irq: false,
        }
    }

    pub fn save_state(&self, out: &mut Vec<u8>) {
        use nes_cpu::state::*;
        write_u8(out, if self.mode == Mode::FourStep { 0 } else { 1 });
        write_bool(out, self.irq_inhibit);
        write_bool(out, self.irq_flag);
        write_u16(out, self.cycle);
        write_bool(out, self.new_sequence_irq);
    }

    pub fn load_state(&mut self, cursor: &mut &[u8]) {
        use nes_cpu::state::*;
        self.mode = if read_u8(cursor) == 0 {
            Mode::FourStep
        } else {
            Mode::FiveStep
        };
        self.irq_inhibit = read_bool(cursor);
        self.irq_flag = read_bool(cursor);
        self.cycle = read_u16(cursor);
        self.new_sequence_irq = read_bool(cursor);
    }

    /// Write $4017 frame counter register.
    ///
    /// Returns `Some(event)` if an immediate clock should fire (5-step mode).
    /// The caller is responsible for applying the returned event to channels.
    pub fn write(&mut self, val: u8) -> Option<FrameEvent> {
        self.mode = if val & 0x80 != 0 {
            Mode::FiveStep
        } else {
            Mode::FourStep
        };
        self.irq_inhibit = val & 0x40 != 0;
        if self.irq_inhibit {
            self.irq_flag = false;
        }
        // Reset sequencer
        self.cycle = 0;
        self.new_sequence_irq = false;

        // 5-step mode immediately clocks all units on write
        if self.mode == Mode::FiveStep {
            Some(FrameEvent {
                quarter_frame: true,
                half_frame: true,
                irq: false,
            })
        } else {
            None
        }
    }

    /// Tick one APU cycle. Called every other CPU cycle (at CPU/2 rate).
    /// Returns frame events if any step boundary is crossed.
    pub fn tick(&mut self) -> Option<FrameEvent> {
        // Handle post-reset IRQ for mode 0 (third consecutive IRQ cycle)
        if self.new_sequence_irq {
            self.new_sequence_irq = false;
            if !self.irq_inhibit {
                self.irq_flag = true;
            }
        }

        self.cycle += 1;

        match self.mode {
            Mode::FourStep => self.tick_four_step(),
            Mode::FiveStep => self.tick_five_step(),
        }
    }

    fn tick_four_step(&mut self) -> Option<FrameEvent> {
        match self.cycle {
            3729 => Some(FrameEvent {
                quarter_frame: true,
                half_frame: false,
                irq: false,
            }),
            7457 => Some(FrameEvent {
                quarter_frame: true,
                half_frame: true,
                irq: false,
            }),
            11186 => Some(FrameEvent {
                quarter_frame: true,
                half_frame: false,
                irq: false,
            }),
            14914 => {
                // IRQ flag only — no quarter/half frame clocks yet
                let irq = !self.irq_inhibit;
                if irq {
                    self.irq_flag = true;
                }
                if irq {
                    Some(FrameEvent {
                        quarter_frame: false,
                        half_frame: false,
                        irq: true,
                    })
                } else {
                    None
                }
            }
            14915 => {
                // Quarter + half frame clocks, IRQ flag set, sequence resets
                let irq = !self.irq_inhibit;
                if irq {
                    self.irq_flag = true;
                }
                self.cycle = 0;
                // Schedule one more IRQ flag set on the next tick (post-reset)
                self.new_sequence_irq = !self.irq_inhibit;
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
            3729 => Some(FrameEvent {
                quarter_frame: true,
                half_frame: false,
                irq: false,
            }),
            7457 => Some(FrameEvent {
                quarter_frame: true,
                half_frame: true,
                irq: false,
            }),
            11186 => Some(FrameEvent {
                quarter_frame: true,
                half_frame: false,
                irq: false,
            }),
            14915 => None, // step 4 is empty in 5-step mode
            18641 => {
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

        let mut events = Vec::new();
        // Tick through one full 4-step sequence (14915 APU cycles)
        for _ in 0..14916 {
            if let Some(e) = fc.tick() {
                events.push(e);
            }
        }
        // Should have 5 events:
        // 3729: quarter, 7457: quarter+half, 11186: quarter,
        // 14914: IRQ only, 14915: quarter+half+IRQ
        assert_eq!(events.len(), 5);
        assert!(events[0].quarter_frame && !events[0].half_frame && !events[0].irq);
        assert!(events[1].quarter_frame && events[1].half_frame && !events[1].irq);
        assert!(events[2].quarter_frame && !events[2].half_frame && !events[2].irq);
        // IRQ-only event at 14914
        assert!(!events[3].quarter_frame && !events[3].half_frame && events[3].irq);
        // Full event at 14915
        assert!(events[4].quarter_frame && events[4].half_frame && events[4].irq);
        // IRQ flag should be set
        assert!(fc.irq_flag);
    }

    #[test]
    fn test_four_step_irq_spans_three_cycles() {
        let mut fc = FrameCounter::new();
        fc.mode = Mode::FourStep;
        fc.irq_inhibit = false;

        // Tick to cycle 14914 (first IRQ)
        for _ in 0..14914 {
            fc.tick();
        }
        assert!(fc.irq_flag); // Set at cycle 14914

        fc.irq_flag = false; // Clear it
        fc.tick(); // cycle 14915: IRQ set again + clocks + reset
        assert!(fc.irq_flag);

        fc.irq_flag = false; // Clear it
        fc.tick(); // cycle 1 after reset: post-reset IRQ
        assert!(fc.irq_flag); // Third consecutive IRQ
    }

    #[test]
    fn test_irq_inhibit() {
        let mut fc = FrameCounter::new();
        fc.mode = Mode::FourStep;
        fc.irq_inhibit = true;

        for _ in 0..14916 {
            if let Some(e) = fc.tick() {
                assert!(!e.irq);
            }
        }
        assert!(!fc.irq_flag);
    }

    #[test]
    fn test_five_step_no_irq() {
        let mut fc = FrameCounter::new();
        // Use write() to enter 5-step mode
        let event = fc.write(0x80);
        // Should produce an immediate clock event
        assert!(event.is_some());
        let e = event.unwrap();
        assert!(e.quarter_frame && e.half_frame && !e.irq);

        for _ in 0..18642 {
            if let Some(e) = fc.tick() {
                assert!(!e.irq);
            }
        }
        assert!(!fc.irq_flag);
    }

    #[test]
    fn test_five_step_sequence() {
        let mut fc = FrameCounter::new();
        fc.mode = Mode::FiveStep;

        let mut events = Vec::new();
        for _ in 0..18642 {
            if let Some(e) = fc.tick() {
                events.push(e);
            }
        }
        // 4 events: 3729 quarter, 7457 quarter+half, 11186 quarter, 18641 quarter+half
        // (14915 is empty)
        assert_eq!(events.len(), 4);
        assert!(events[0].quarter_frame && !events[0].half_frame);
        assert!(events[1].quarter_frame && events[1].half_frame);
        assert!(events[2].quarter_frame && !events[2].half_frame);
        assert!(events[3].quarter_frame && events[3].half_frame);
    }

    #[test]
    fn test_write_clears_irq_with_inhibit() {
        let mut fc = FrameCounter::new();
        fc.irq_flag = true;

        // Writing with bit 6 set (irq_inhibit) should clear the flag
        fc.write(0x40);
        assert!(!fc.irq_flag);
    }

    #[test]
    fn test_write_preserves_irq_without_inhibit() {
        let mut fc = FrameCounter::new();
        fc.irq_flag = true;

        // Writing $00 (mode 0, no inhibit) should NOT clear the flag
        fc.write(0x00);
        assert!(fc.irq_flag);

        // Writing $80 (mode 1, no inhibit) should NOT clear the flag
        fc.irq_flag = true;
        fc.write(0x80);
        assert!(fc.irq_flag);
    }
}
