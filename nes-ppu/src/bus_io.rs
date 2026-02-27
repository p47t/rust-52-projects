use std::cell::{Cell, RefCell};
use std::rc::Rc;

use nes_cpu::bus::BusIo;

use crate::ppu::Ppu;

/// Bridges the PPU to the CPU address bus via the BusIo trait.
/// Implements catch-up ticking: before any PPU register access,
/// the PPU is ticked forward to match the current CPU cycle.
///
/// Timing offsets model real hardware's CPU-PPU phase relationship:
/// - **Reads**: For $2002, uses split catch-up (target-1) to handle
///   VBlank suppression race conditions accurately.
/// - **Writes**: Catch up to full target. NMI cancellation for writes
///   that disable NMI near VBlank set is handled by age-based logic
///   in PPU's update_nmi (freshly-set NMIs can be cancelled).
pub struct PpuBusIo {
    pub ppu: Rc<RefCell<Ppu>>,
    /// Shared CPU cycle counter — written by System during cpu.step() via
    /// the Cpu.cycles field, mirrored here so BusIo can catch up the PPU.
    pub cpu_cycles: Rc<Cell<u64>>,
    /// PPU cycles already ticked.
    pub ppu_cycles: Rc<Cell<u64>>,
}

impl PpuBusIo {
    /// Tick the PPU forward to the given absolute PPU cycle.
    fn catch_up_to(&self, target_ppu: u64) {
        let current_ppu = self.ppu_cycles.get();
        if target_ppu > current_ppu {
            let ticks = target_ppu - current_ppu;
            let mut ppu = self.ppu.borrow_mut();
            for _ in 0..ticks {
                ppu.tick();
            }
            self.ppu_cycles.set(target_ppu);
        }
    }
}

impl BusIo for PpuBusIo {
    fn read(&mut self, addr: u16) -> u8 {
        let target = self.cpu_cycles.get() * 3;

        if addr == 0x2002 {
            // Split catch-up for $2002: tick to target-1, then handle
            // VBlank suppression race before ticking the final cycle.
            let split_target = target.saturating_sub(1);
            self.catch_up_to(split_target);

            let mut ppu = self.ppu.borrow_mut();

            // Check for VBlank suppression: if PPU is at (241, 0), the
            // next tick would set VBlank. Reading $2002 now creates a race
            // that suppresses VBlank from ever being set this frame.
            if ppu.scanline == 241 && ppu.dot == 0 {
                ppu.suppress_vbl = true;
            }

            let val = ppu.read_register(addr);

            // Tick the final cycle
            ppu.tick();
            self.ppu_cycles.set(target);

            val
        } else {
            self.catch_up_to(target);
            self.ppu.borrow_mut().read_register(addr)
        }
    }

    fn write(&mut self, addr: u16, val: u8) {
        let target = self.cpu_cycles.get() * 3;
        self.catch_up_to(target);
        self.ppu.borrow_mut().write_register(addr, val);
    }
}
