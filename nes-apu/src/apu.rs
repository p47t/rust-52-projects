use crate::dmc::Dmc;
use crate::frame_counter::FrameCounter;
use crate::mixer::Mixer;
use crate::noise::Noise;
use crate::pulse::Pulse;
use crate::sweep::NegateMode;
use crate::triangle::Triangle;

/// NTSC CPU clock rate (~1.789773 MHz).
const CPU_FREQ: f64 = 1_789_773.0;

/// NES 2A03 Audio Processing Unit.
///
/// Pure DSP — generates audio samples without depending on any audio output library.
/// Call `tick()` once per CPU cycle, then `drain_samples()` each frame to retrieve
/// downsampled audio data.
pub struct Apu {
    pub pulse1: Pulse,
    pub pulse2: Pulse,
    pub triangle: Triangle,
    pub noise: Noise,
    pub dmc: Dmc,
    pub frame_counter: FrameCounter,
    mixer: Mixer,

    /// Total CPU cycles ticked.
    cycle: u64,
    /// Whether the current cycle is even (for pulse/noise half-rate clocking).
    even_cycle: bool,

    // Downsampling state
    sample_period: f64,  // CPU_FREQ / sample_rate
    sample_counter: f64, // accumulates until >= sample_period

    /// Output buffer of downsampled audio samples (mono, -1.0 to 1.0).
    sample_buffer: Vec<f32>,

    /// First-order high-pass filter state (~37Hz cutoff, removes DC offset).
    hpf_prev_in: f32,
    hpf_prev_out: f32,
    hpf_alpha: f32,
}

impl Apu {
    pub fn new(sample_rate: f64) -> Self {
        // First-order high-pass: alpha = RC / (RC + dt)
        // ~37Hz cutoff: RC = 1/(2*pi*37)
        let rc = 1.0 / (2.0 * std::f64::consts::PI * 37.0);
        let dt = 1.0 / sample_rate;
        let alpha = rc / (rc + dt);

        Self {
            pulse1: Pulse::new(NegateMode::OnesComplement),
            pulse2: Pulse::new(NegateMode::TwosComplement),
            triangle: Triangle::new(),
            noise: Noise::new(),
            dmc: Dmc::new(),
            frame_counter: FrameCounter::new(),
            mixer: Mixer::new(),
            cycle: 0,
            even_cycle: false,
            sample_period: CPU_FREQ / sample_rate,
            sample_counter: 0.0,
            sample_buffer: Vec::with_capacity((sample_rate / 30.0) as usize),
            hpf_prev_in: 0.0,
            hpf_prev_out: 0.0,
            hpf_alpha: alpha as f32,
        }
    }

    /// Tick one CPU cycle. Clocks channels and frame counter, produces downsampled output.
    pub fn tick(&mut self) {
        self.cycle += 1;
        self.even_cycle = !self.even_cycle;

        // Triangle and DMC tick every CPU cycle
        self.triangle.tick();
        self.dmc.tick();

        // Pulse, noise, and frame counter tick every other CPU cycle (APU rate)
        if self.even_cycle {
            self.pulse1.tick();
            self.pulse2.tick();
            self.noise.tick();

            // Frame counter sequencer (APU rate — every other CPU cycle)
            if let Some(event) = self.frame_counter.tick() {
                if event.quarter_frame {
                    self.clock_quarter_frame();
                }
                if event.half_frame {
                    self.clock_half_frame();
                }
            }
        }

        // Downsample: accumulate and emit a sample when counter wraps
        self.sample_counter += 1.0;
        if self.sample_counter >= self.sample_period {
            self.sample_counter -= self.sample_period;
            let sample = self.mix_output();
            self.sample_buffer.push(sample);
        }
    }

    fn clock_quarter_frame(&mut self) {
        self.pulse1.envelope.clock();
        self.pulse2.envelope.clock();
        self.triangle.clock_linear_counter();
        self.noise.envelope.clock();
    }

    fn clock_half_frame(&mut self) {
        self.pulse1.length_counter.clock();
        self.pulse2.length_counter.clock();
        self.triangle.length_counter.clock();
        self.noise.length_counter.clock();

        self.pulse1.clock_sweep();
        self.pulse2.clock_sweep();
    }

    fn mix_output(&mut self) -> f32 {
        let p1 = self.pulse1.output();
        let p2 = self.pulse2.output();
        let tri = self.triangle.output();
        let noise = self.noise.output();
        let dmc = self.dmc.output();

        let mixed = self.mixer.mix(p1, p2, tri, noise, dmc);

        // High-pass filter to remove DC offset (replicates the real NES ~37Hz HPF)
        let out = self.hpf_alpha * (self.hpf_prev_out + mixed - self.hpf_prev_in);
        self.hpf_prev_in = mixed;
        self.hpf_prev_out = out;
        out
    }

    /// Drain all accumulated audio samples. Call once per frame.
    pub fn drain_samples(&mut self) -> Vec<f32> {
        std::mem::take(&mut self.sample_buffer)
    }

    /// Returns true if any APU IRQ source is asserted.
    pub fn irq_pending(&self) -> bool {
        self.frame_counter.irq_flag || self.dmc.irq_flag
    }

    /// Read register ($4015 — APU status).
    pub fn read_register(&mut self, addr: u16) -> u8 {
        match addr {
            0x4015 => {
                let mut status = 0u8;
                if self.pulse1.length_counter.is_active() {
                    status |= 0x01;
                }
                if self.pulse2.length_counter.is_active() {
                    status |= 0x02;
                }
                if self.triangle.length_counter.is_active() {
                    status |= 0x04;
                }
                if self.noise.length_counter.is_active() {
                    status |= 0x08;
                }
                if self.dmc.bytes_remaining() > 0 {
                    status |= 0x10;
                }
                if self.frame_counter.irq_flag {
                    status |= 0x40;
                }
                if self.dmc.irq_flag {
                    status |= 0x80;
                }
                // Reading $4015 clears the frame counter IRQ flag
                self.frame_counter.irq_flag = false;
                status
            }
            _ => 0,
        }
    }

    /// Write register ($4000-$4013, $4015, $4017).
    pub fn write_register(&mut self, addr: u16, val: u8) {
        match addr {
            0x4000..=0x4003 => self.pulse1.write_reg((addr & 0x03) as u8, val),
            0x4004..=0x4007 => self.pulse2.write_reg((addr & 0x03) as u8, val),
            0x4008..=0x400B => self.triangle.write_reg((addr - 0x4008) as u8, val),
            0x400C..=0x400F => self.noise.write_reg((addr - 0x400C) as u8, val),
            0x4010..=0x4013 => self.dmc.write_reg((addr - 0x4010) as u8, val),
            0x4015 => {
                self.pulse1.length_counter.set_enabled(val & 0x01 != 0);
                self.pulse2.length_counter.set_enabled(val & 0x02 != 0);
                self.triangle.length_counter.set_enabled(val & 0x04 != 0);
                self.noise.length_counter.set_enabled(val & 0x08 != 0);
                self.dmc.set_enabled(val & 0x10 != 0);
            }
            0x4017 => {
                if let Some(event) = self.frame_counter.write(val) {
                    if event.quarter_frame {
                        self.clock_quarter_frame();
                    }
                    if event.half_frame {
                        self.clock_half_frame();
                    }
                }
            }
            _ => {}
        }
    }

    pub fn save_state(&self) -> Vec<u8> {
        use nes_cpu::state::*;
        let mut out = Vec::new();
        self.pulse1.save_state(&mut out);
        self.pulse2.save_state(&mut out);
        self.triangle.save_state(&mut out);
        self.noise.save_state(&mut out);
        self.dmc.save_state(&mut out);
        self.frame_counter.save_state(&mut out);
        write_u64(&mut out, self.cycle);
        write_bool(&mut out, self.even_cycle);
        write_f64(&mut out, self.sample_counter);
        // HPF state
        write_f32(&mut out, self.hpf_prev_in);
        write_f32(&mut out, self.hpf_prev_out);
        out
    }

    pub fn load_state(&mut self, data: &[u8]) {
        use nes_cpu::state::*;
        let mut cursor = data;
        self.pulse1.load_state(&mut cursor);
        self.pulse2.load_state(&mut cursor);
        self.triangle.load_state(&mut cursor);
        self.noise.load_state(&mut cursor);
        self.dmc.load_state(&mut cursor);
        self.frame_counter.load_state(&mut cursor);
        self.cycle = read_u64(&mut cursor);
        self.even_cycle = read_bool(&mut cursor);
        self.sample_counter = read_f64(&mut cursor);
        self.hpf_prev_in = read_f32(&mut cursor);
        self.hpf_prev_out = read_f32(&mut cursor);
        // Clear transient state
        self.sample_buffer.clear();
    }

    /// Returns a pending DMC sample fetch address, if any.
    pub fn dmc_sample_request(&mut self) -> Option<u16> {
        self.dmc.sample_request.take()
    }

    /// Load a byte fetched by the system into the DMC sample buffer.
    pub fn dmc_load_sample(&mut self, val: u8) {
        self.dmc.load_sample(val);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        let apu = Apu::new(44100.0);
        assert!(apu.sample_buffer.is_empty());
        assert!(!apu.irq_pending());
    }

    #[test]
    fn test_produces_samples() {
        let mut apu = Apu::new(44100.0);
        // Tick enough cycles for one frame (~29780 CPU cycles per NTSC frame)
        for _ in 0..29780 {
            apu.tick();
        }
        let samples = apu.drain_samples();
        // Should produce approximately 44100/60 ≈ 735 samples per frame
        assert!(samples.len() > 700 && samples.len() < 800);
    }

    #[test]
    fn test_status_register() {
        let mut apu = Apu::new(44100.0);
        // Enable pulse 1
        apu.write_register(0x4015, 0x01);
        // Write to pulse 1 to set length counter
        apu.write_register(0x4003, 0x08); // length counter load

        let status = apu.read_register(0x4015);
        assert!(status & 0x01 != 0); // pulse 1 active
    }

    #[test]
    fn test_register_dispatch() {
        let mut apu = Apu::new(44100.0);
        // Write to pulse 1 duty
        apu.write_register(0x4000, 0xBF); // duty=2, constant vol=15
        assert_eq!(apu.pulse1.duty, 2);

        // Write to triangle
        apu.write_register(0x4008, 0xFF);
        assert!(apu.triangle.control_flag);

        // Write to noise
        apu.write_register(0x400C, 0x3F);
        assert!(apu.noise.length_counter.halt);

        // Write to frame counter (5-step mode)
        apu.write_register(0x4017, 0x80);
        assert_eq!(apu.frame_counter.mode, crate::frame_counter::Mode::FiveStep);
    }

    // ---------------------------------------------------------------
    // blargg apu_test 1: len_ctr — length counter behavior
    // ---------------------------------------------------------------

    /// Helper: enable a channel, load a length value, tick some cycles.
    fn setup_pulse1_with_length(apu: &mut Apu, length_index: u8) {
        apu.write_register(0x4015, 0x01); // enable pulse 1
        apu.write_register(0x4000, 0x9F); // duty=2, NO halt, constant vol=15
        apu.write_register(0x4002, 0x80); // timer low (period >= 8)
        apu.write_register(0x4003, length_index << 3); // length load
    }

    #[test]
    fn test_len_ctr_load_and_status() {
        // blargg 1-len_ctr, code 2: length counter load + $4015 status
        let mut apu = Apu::new(44100.0);
        setup_pulse1_with_length(&mut apu, 0x01); // index 1 = 254 half-frames
        let status = apu.read_register(0x4015);
        assert!(
            status & 0x01 != 0,
            "pulse 1 should be active after length load"
        );
    }

    #[test]
    fn test_len_ctr_expires_over_time() {
        // blargg 1-len_ctr, code 3: length counter decrements and eventually expires
        let mut apu = Apu::new(44100.0);
        apu.write_register(0x4017, 0x40); // 4-step mode, inhibit IRQ (no immediate clock)
        setup_pulse1_with_length(&mut apu, 0x03); // index 3 = 2 half-frames

        // Length = 2: needs 2 half-frame clocks to expire.
        // In 4-step mode, half-frames at APU cycles 7457 and 14915.
        // Tick enough CPU cycles to reach the second half-frame:
        // 14915 APU cycles * 2 = 29830 CPU cycles, plus margin.
        for _ in 0..29840 {
            apu.tick();
        }
        let status = apu.read_register(0x4015);
        assert_eq!(
            status & 0x01,
            0,
            "pulse 1 should have expired after 2 half-frames"
        );
    }

    #[test]
    fn test_len_ctr_five_step_immediate_clock() {
        // blargg 1-len_ctr, code 4: writing $80 to $4017 clocks length immediately
        let mut apu = Apu::new(44100.0);
        setup_pulse1_with_length(&mut apu, 0x03); // index 3 = 2 half-frames

        // Writing $80 to $4017 (5-step mode) should immediately clock half-frame
        apu.write_register(0x4017, 0x80);
        // Length was 2, now should be 1 after immediate half-frame clock
        assert_eq!(apu.pulse1.length_counter.counter, 1);
    }

    #[test]
    fn test_len_ctr_four_step_no_immediate_clock() {
        // blargg 1-len_ctr, code 5: writing $00 to $4017 should NOT clock immediately
        let mut apu = Apu::new(44100.0);
        setup_pulse1_with_length(&mut apu, 0x03); // index 3 = 2

        apu.write_register(0x4017, 0x00); // 4-step mode, no immediate clock
        assert_eq!(
            apu.pulse1.length_counter.counter, 2,
            "should not have clocked"
        );
    }

    #[test]
    fn test_len_ctr_disable_clears() {
        // blargg 1-len_ctr, code 6: disabling via $4015 clears length counter
        let mut apu = Apu::new(44100.0);
        setup_pulse1_with_length(&mut apu, 0x01); // index 1 = 254
        assert!(apu.pulse1.length_counter.is_active());

        apu.write_register(0x4015, 0x00); // disable all channels
        assert!(!apu.pulse1.length_counter.is_active());
        assert_eq!(apu.pulse1.length_counter.counter, 0);

        // Re-enabling should NOT restore the counter
        apu.write_register(0x4015, 0x01);
        assert_eq!(apu.pulse1.length_counter.counter, 0);
    }

    #[test]
    fn test_len_ctr_disabled_prevents_reload() {
        // blargg 1-len_ctr, code 7: when disabled, length can't reload
        let mut apu = Apu::new(44100.0);
        // Do NOT enable pulse 1 via $4015
        apu.write_register(0x4000, 0xBF);
        apu.write_register(0x4002, 0x80);
        apu.write_register(0x4003, 0x08); // try to load length
        assert_eq!(
            apu.pulse1.length_counter.counter, 0,
            "length should not load when channel is disabled"
        );
    }

    #[test]
    fn test_len_ctr_halt_suspends_clock() {
        // blargg 1-len_ctr, code 8: halt bit suspends length counter clocking
        let mut apu = Apu::new(44100.0);
        apu.write_register(0x4015, 0x01);
        apu.write_register(0x4000, 0xBF | 0x20); // halt bit set
        apu.write_register(0x4002, 0x80);
        apu.write_register(0x4003, 0x18); // length load (index 3 = 2)
        let initial = apu.pulse1.length_counter.counter;
        assert!(initial > 0);

        // 5-step mode immediate clock — should NOT decrement because halt is set
        apu.write_register(0x4017, 0x80);
        assert_eq!(apu.pulse1.length_counter.counter, initial);
    }

    // ---------------------------------------------------------------
    // blargg apu_test 2: len_table — verify all 32 length table entries
    // ---------------------------------------------------------------

    #[test]
    fn test_len_table_all_entries() {
        let expected: [u8; 32] = [
            10, 254, 20, 2, 40, 4, 80, 6, 160, 8, 60, 10, 14, 12, 26, 14, 12, 16, 24, 18, 48, 20,
            96, 22, 192, 24, 72, 26, 16, 28, 32, 30,
        ];
        let mut apu = Apu::new(44100.0);
        apu.write_register(0x4015, 0x01); // enable pulse 1
        apu.write_register(0x4000, 0xBF);
        apu.write_register(0x4002, 0x80);

        for (i, &exp) in expected.iter().enumerate() {
            apu.write_register(0x4003, (i as u8) << 3);
            assert_eq!(
                apu.pulse1.length_counter.counter, exp,
                "length table index {} should be {}",
                i, exp
            );
        }
    }

    // ---------------------------------------------------------------
    // blargg apu_test 3: irq_flag — frame IRQ flag behavior
    // ---------------------------------------------------------------

    #[test]
    fn test_irq_flag_not_set_in_mode_40() {
        // Code 2: flag shouldn't be set in $4017 mode $40 (4-step + irq inhibit)
        let mut apu = Apu::new(44100.0);
        apu.write_register(0x4017, 0x40);
        // Tick through a full 4-step sequence (~29830 CPU cycles)
        for _ in 0..29840 {
            apu.tick();
        }
        let status = apu.read_register(0x4015);
        assert_eq!(status & 0x40, 0, "IRQ flag should not be set with inhibit");
    }

    #[test]
    fn test_irq_flag_not_set_in_mode_80() {
        // Code 3: flag shouldn't be set in $4017 mode $80 (5-step)
        let mut apu = Apu::new(44100.0);
        apu.write_register(0x4017, 0x80);
        for _ in 0..37300 {
            apu.tick();
        }
        let status = apu.read_register(0x4015);
        assert_eq!(
            status & 0x40,
            0,
            "IRQ flag should not be set in 5-step mode"
        );
    }

    #[test]
    fn test_irq_flag_set_in_mode_00() {
        // Code 4: flag should be set in $4017 mode $00 (4-step, no inhibit)
        let mut apu = Apu::new(44100.0);
        apu.write_register(0x4017, 0x00);
        // Tick through a full 4-step sequence
        for _ in 0..29840 {
            apu.tick();
        }
        let status = apu.read_register(0x4015);
        assert!(status & 0x40 != 0, "IRQ flag should be set in 4-step mode");
    }

    #[test]
    fn test_irq_flag_cleared_by_read() {
        // Code 5: reading $4015 clears the frame IRQ flag
        let mut apu = Apu::new(44100.0);
        apu.write_register(0x4017, 0x00);
        for _ in 0..29840 {
            apu.tick();
        }
        let status1 = apu.read_register(0x4015);
        assert!(status1 & 0x40 != 0, "flag should be set first");

        let status2 = apu.read_register(0x4015);
        assert_eq!(status2 & 0x40, 0, "flag should be cleared after read");
    }

    #[test]
    fn test_irq_flag_write_00_80_no_effect() {
        // Code 6: writing $00 or $80 to $4017 should not clear existing flag
        let mut apu = Apu::new(44100.0);
        apu.frame_counter.irq_flag = true;

        apu.write_register(0x4017, 0x00);
        assert!(apu.frame_counter.irq_flag, "$00 should not clear IRQ flag");

        apu.write_register(0x4017, 0x80);
        assert!(apu.frame_counter.irq_flag, "$80 should not clear IRQ flag");
    }

    #[test]
    fn test_irq_flag_write_40_c0_clears() {
        // Code 7: writing $40 or $C0 to $4017 (irq_inhibit set) should clear flag
        let mut apu = Apu::new(44100.0);
        apu.frame_counter.irq_flag = true;
        apu.write_register(0x4017, 0x40);
        assert!(!apu.frame_counter.irq_flag, "$40 should clear IRQ flag");

        apu.frame_counter.irq_flag = true;
        apu.write_register(0x4017, 0xC0);
        assert!(!apu.frame_counter.irq_flag, "$C0 should clear IRQ flag");
    }

    // ---------------------------------------------------------------
    // blargg apu_test 7: dmc_basics — DMC functionality
    // ---------------------------------------------------------------

    #[test]
    fn test_dmc_enable_disable_status() {
        // DMC enable/disable reflected in $4015
        let mut apu = Apu::new(44100.0);
        apu.write_register(0x4012, 0x00); // sample addr
        apu.write_register(0x4013, 0x01); // sample length = 17

        apu.write_register(0x4015, 0x10); // enable DMC
        let status = apu.read_register(0x4015);
        assert!(status & 0x10 != 0, "DMC should show active in $4015");

        apu.write_register(0x4015, 0x00); // disable DMC
        let status = apu.read_register(0x4015);
        assert_eq!(status & 0x10, 0, "DMC should show inactive after disable");
    }

    #[test]
    fn test_dmc_irq_flag_cleared_on_4015_write() {
        let mut apu = Apu::new(44100.0);
        apu.dmc.irq_flag = true;
        assert!(apu.irq_pending());

        apu.write_register(0x4015, 0x00); // any write to $4015 clears DMC IRQ
        assert!(!apu.dmc.irq_flag);
    }

    #[test]
    fn test_dmc_direct_load() {
        let mut apu = Apu::new(44100.0);
        apu.write_register(0x4011, 0x55); // direct load = 0x55
        assert_eq!(apu.dmc.output(), 0x55);
    }

    #[test]
    fn test_dmc_irq_disabled_clears_flag() {
        // Writing to $4010 with bit 7 clear should clear DMC IRQ flag
        let mut apu = Apu::new(44100.0);
        apu.dmc.irq_flag = true;
        apu.write_register(0x4010, 0x00); // irq_enabled = false
        assert!(!apu.dmc.irq_flag);
    }

    #[test]
    fn test_dmc_restart_on_enable() {
        // Enabling DMC when bytes_remaining == 0 should restart playback
        let mut apu = Apu::new(44100.0);
        apu.write_register(0x4012, 0x02); // sample addr = $C080
        apu.write_register(0x4013, 0x01); // sample length = 17
        apu.write_register(0x4015, 0x10); // enable DMC
        assert_eq!(apu.dmc.bytes_remaining(), 17);
    }

    #[test]
    fn test_dmc_no_restart_when_already_playing() {
        // Enabling DMC when bytes_remaining > 0 should NOT restart
        let mut apu = Apu::new(44100.0);
        apu.write_register(0x4012, 0x00);
        apu.write_register(0x4013, 0x02); // length = 33
        apu.write_register(0x4015, 0x10); // enable, starts with 33
        assert_eq!(apu.dmc.bytes_remaining(), 33);

        // Simulate some bytes consumed
        apu.dmc.load_sample(0xFF);
        let remaining_after = apu.dmc.bytes_remaining();
        assert!(remaining_after < 33);

        // Re-enable: should NOT restart since bytes_remaining > 0
        apu.write_register(0x4015, 0x10);
        assert_eq!(apu.dmc.bytes_remaining(), remaining_after);
    }

    // ---------------------------------------------------------------
    // blargg apu_test 8: dmc_rates — verify all 16 DMC rate entries
    // ---------------------------------------------------------------

    #[test]
    fn test_dmc_rate_table() {
        let expected: [u16; 16] = [
            428, 380, 340, 320, 286, 254, 226, 214, 190, 160, 142, 128, 106, 84, 72, 54,
        ];
        let mut apu = Apu::new(44100.0);
        for (i, &exp) in expected.iter().enumerate() {
            apu.write_register(0x4010, i as u8);
            assert_eq!(
                apu.dmc.timer_period, exp,
                "DMC rate index {} should be {}",
                i, exp
            );
        }
    }

    // ---------------------------------------------------------------
    // Additional correctness: all 4 channels share length counter behavior
    // ---------------------------------------------------------------

    #[test]
    fn test_all_channels_length_counter_status() {
        let mut apu = Apu::new(44100.0);
        apu.write_register(0x4015, 0x0F); // enable pulse1, pulse2, triangle, noise
        apu.write_register(0x4003, 0x08); // pulse1 length
        apu.write_register(0x4007, 0x08); // pulse2 length
        apu.write_register(0x400B, 0x08); // triangle length
        apu.write_register(0x400F, 0x08); // noise length

        let status = apu.read_register(0x4015);
        assert_eq!(status & 0x0F, 0x0F, "all 4 channels should be active");

        // Disable all
        apu.write_register(0x4015, 0x00);
        let status = apu.read_register(0x4015);
        assert_eq!(status & 0x0F, 0x00, "all 4 channels should be inactive");
    }

    #[test]
    fn test_five_step_clocks_all_channels() {
        // Writing $80 to $4017 should clock length counters for ALL channels
        let mut apu = Apu::new(44100.0);
        apu.write_register(0x4015, 0x0F);
        // Load length index 3 = 2 for all channels
        apu.write_register(0x4003, 0x18); // pulse1
        apu.write_register(0x4007, 0x18); // pulse2
        apu.write_register(0x400B, 0x18); // triangle
        apu.write_register(0x400F, 0x18); // noise

        apu.write_register(0x4017, 0x80); // 5-step: immediate clock

        // All should have decremented from 2 to 1
        assert_eq!(apu.pulse1.length_counter.counter, 1);
        assert_eq!(apu.pulse2.length_counter.counter, 1);
        assert_eq!(apu.triangle.length_counter.counter, 1);
        assert_eq!(apu.noise.length_counter.counter, 1);
    }
}
