use std::cell::RefCell;
use std::rc::Rc;

use nes_cpu::bus::addr;
use nes_cpu::bus::BusIo;
use nes_ppu::bus_io::PpuBusIo;

use crate::joypad::Joypad;

/// Composite I/O dispatcher that routes CPU bus addresses to the appropriate
/// device: PPU registers ($2000–$2007, $4014) or joypads ($4016, $4017).
pub struct CompositeBusIo {
    pub ppu_io: PpuBusIo,
    pub joypad1: Rc<RefCell<Joypad>>,
    pub joypad2: Rc<RefCell<Joypad>>,
}

impl BusIo for CompositeBusIo {
    fn read(&mut self, addr: u16) -> u8 {
        match addr {
            addr::JOYPAD1 => self.joypad1.borrow_mut().read_bit(),
            addr::JOYPAD2 => self.joypad2.borrow_mut().read_bit(),
            _ => self.ppu_io.read(addr),
        }
    }

    fn write(&mut self, addr: u16, val: u8) {
        match addr {
            addr::JOYPAD1 => {
                // Strobe write latches both controllers simultaneously
                self.joypad1.borrow_mut().write_strobe(val);
                self.joypad2.borrow_mut().write_strobe(val);
            }
            _ => self.ppu_io.write(addr, val),
        }
    }
}
