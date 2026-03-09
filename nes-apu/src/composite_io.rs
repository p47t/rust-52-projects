use std::cell::RefCell;
use std::rc::Rc;

use nes_cpu::bus::BusIo;
use nes_joypad::composite_io::CompositeBusIo;

use crate::apu::Apu;

/// Composite I/O that adds APU register routing on top of the joypad+PPU composite.
///
/// Address routing:
/// - Reads  $4015         → APU status
/// - Writes $4000-$4013   → APU channel registers
/// - Writes $4015         → APU channel enable
/// - Writes $4017         → APU frame counter (reads still go to joypad2 via inner)
/// - Everything else      → inner CompositeBusIo (joypad + PPU)
pub struct ApuCompositeBusIo {
    pub inner: CompositeBusIo,
    pub apu: Rc<RefCell<Apu>>,
}

impl BusIo for ApuCompositeBusIo {
    fn read(&mut self, addr: u16) -> u8 {
        match addr {
            0x4015 => self.apu.borrow_mut().read_register(addr),
            _ => self.inner.read(addr),
        }
    }

    fn write(&mut self, addr: u16, val: u8) {
        match addr {
            0x4000..=0x4013 | 0x4015 => {
                self.apu.borrow_mut().write_register(addr, val);
            }
            0x4017 => {
                // $4017 write → APU frame counter
                self.apu.borrow_mut().write_register(addr, val);
            }
            _ => self.inner.write(addr, val),
        }
    }
}
