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
}

impl Apu {
    pub fn new(sample_rate: f64) -> Self {
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
        }
    }

    /// Tick one CPU cycle. Clocks channels and frame counter, produces downsampled output.
    pub fn tick(&mut self) {
        self.cycle += 1;
        self.even_cycle = !self.even_cycle;

        // Triangle and DMC tick every CPU cycle
        self.triangle.tick();
        self.dmc.tick();

        // Pulse and noise tick every other CPU cycle
        if self.even_cycle {
            self.pulse1.tick();
            self.pulse2.tick();
            self.noise.tick();
        }

        // Frame counter tick
        if let Some(event) = self.frame_counter.tick() {
            if event.quarter_frame {
                self.clock_quarter_frame();
            }
            if event.half_frame {
                self.clock_half_frame();
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

    fn mix_output(&self) -> f32 {
        let p1 = self.pulse1.output();
        let p2 = self.pulse2.output();
        let tri = self.triangle.output();
        let noise = self.noise.output();
        let dmc = self.dmc.output();

        // Mixer produces 0.0-~1.0 range, center around 0 for audio output
        let mixed = self.mixer.mix(p1, p2, tri, noise, dmc);
        mixed * 2.0 - 1.0 // convert to -1.0..1.0 range
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
            0x4017 => self.frame_counter.write(val),
            _ => {}
        }
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
}
