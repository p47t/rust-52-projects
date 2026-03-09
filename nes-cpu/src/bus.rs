use std::cell::RefCell;
use std::rc::Rc;

use crate::ines::{INesRom, Mirroring};
use crate::mapper::Mapper;

/// CPU memory map address constants.
pub mod addr {
    pub const RAM_START: u16 = 0x0000;
    pub const RAM_END: u16 = 0x1FFF;
    pub const RAM_MASK: u16 = 0x07FF;

    pub const IO_START: u16 = 0x2000;
    pub const IO_END: u16 = 0x3FFF;
    pub const IO_MASK: u16 = 0x0007;

    pub const APU_IO_END: u16 = 0x5FFF;

    pub const OAM_DMA: u16 = 0x4014;
    pub const JOYPAD1: u16 = 0x4016;
    pub const JOYPAD2: u16 = 0x4017;

    pub const CARTRIDGE_START: u16 = 0x6000;
}

/// Trait for I/O devices mapped into the CPU address space.
/// PPU registers ($2000-$3FFF), OAM DMA ($4014), and joypads ($4016/$4017) route through this.
pub trait BusIo {
    fn read(&mut self, addr: u16) -> u8;
    fn write(&mut self, addr: u16, val: u8);
}

pub struct Bus {
    ram: [u8; 2048],
    pub mapper: Option<Rc<RefCell<Box<dyn Mapper>>>>,
    pub io: Option<Box<dyn BusIo>>,
}

impl Default for Bus {
    fn default() -> Self {
        Self::new()
    }
}

impl Bus {
    pub fn new() -> Self {
        Self {
            ram: [0u8; 2048],
            mapper: None,
            io: None,
        }
    }

    /// Convenience constructor for standalone CPU tests (mapper 0 only).
    pub fn from_rom(rom: INesRom) -> Self {
        let mapper = SimpleNrom {
            prg_rom: rom.prg_rom,
            prg_ram: [0u8; 8192],
            mirroring: rom.mirroring,
        };
        Self {
            ram: [0u8; 2048],
            mapper: Some(Rc::new(RefCell::new(Box::new(mapper) as Box<dyn Mapper>))),
            io: None,
        }
    }

    pub fn read(&mut self, addr: u16) -> u8 {
        match addr {
            addr::RAM_START..=addr::RAM_END => self.ram[(addr & addr::RAM_MASK) as usize],
            addr::IO_START..=addr::IO_END => {
                if let Some(io) = &mut self.io {
                    io.read(addr::IO_START + (addr & addr::IO_MASK))
                } else {
                    0xFF
                }
            }
            addr::JOYPAD1 | addr::JOYPAD2 => {
                if let Some(io) = &mut self.io {
                    io.read(addr)
                } else {
                    0xFF
                }
            }
            addr::CARTRIDGE_START..=0xFFFF => {
                if let Some(m) = &self.mapper {
                    m.borrow().read_prg(addr)
                } else {
                    0xFF
                }
            }
            _ => 0xFF, // open bus
        }
    }

    /// Read without side effects — skips I/O registers to avoid PPU/APU side effects.
    /// Used for logging and disassembly.
    pub fn peek(&self, addr: u16) -> u8 {
        match addr {
            addr::RAM_START..=addr::RAM_END => self.ram[(addr & addr::RAM_MASK) as usize],
            addr::IO_START..=addr::APU_IO_END => 0xFF, // I/O — don't trigger side effects
            _ => {
                if let Some(m) = &self.mapper {
                    m.borrow().read_prg(addr)
                } else {
                    0xFF
                }
            }
        }
    }

    pub fn write(&mut self, addr: u16, val: u8) {
        match addr {
            addr::RAM_START..=addr::RAM_END => self.ram[(addr & addr::RAM_MASK) as usize] = val,
            addr::IO_START..=addr::IO_END => {
                if let Some(io) = &mut self.io {
                    io.write(addr::IO_START + (addr & addr::IO_MASK), val);
                }
            }
            addr::OAM_DMA => {
                if let Some(io) = &mut self.io {
                    io.write(addr::OAM_DMA, val);
                }
            }
            addr::JOYPAD1 => {
                if let Some(io) = &mut self.io {
                    io.write(addr, val);
                }
            }
            addr::CARTRIDGE_START..=0xFFFF => {
                if let Some(m) = &self.mapper {
                    m.borrow_mut().write_prg(addr, val);
                }
            }
            _ => {}
        }
    }
}

/// Minimal mapper 0 (NROM) for standalone nes-cpu tests.
/// Only handles PRG; CHR stubs return 0 since CPU tests don't exercise the PPU.
struct SimpleNrom {
    prg_rom: Vec<u8>,
    prg_ram: [u8; 8192],
    mirroring: Mirroring,
}

impl Mapper for SimpleNrom {
    fn read_prg(&self, addr: u16) -> u8 {
        match addr {
            0x6000..=0x7FFF => self.prg_ram[(addr - 0x6000) as usize],
            0x8000..=0xFFFF => {
                let offset = (addr - 0x8000) as usize % self.prg_rom.len();
                self.prg_rom[offset]
            }
            _ => 0,
        }
    }

    fn write_prg(&mut self, addr: u16, val: u8) {
        if let 0x6000..=0x7FFF = addr {
            self.prg_ram[(addr - 0x6000) as usize] = val;
        }
    }

    fn read_chr(&self, _addr: u16) -> u8 {
        0
    }

    fn write_chr(&mut self, _addr: u16, _val: u8) {}

    fn mirroring(&self) -> Mirroring {
        self.mirroring
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_bus(prg_rom: Vec<u8>) -> Bus {
        let rom = INesRom {
            prg_rom,
            chr_rom: Vec::new(),
            mapper: 0,
            mirroring: Mirroring::Horizontal,
            has_battery: false,
        };
        Bus::from_rom(rom)
    }

    #[test]
    fn test_ram_mirror() {
        let mut bus = make_bus(vec![0u8; 0x4000]);
        bus.write(0x0000, 0xAB);
        assert_eq!(bus.read(0x0000), 0xAB);
        assert_eq!(bus.read(0x0800), 0xAB); // mirror
        assert_eq!(bus.read(0x1000), 0xAB); // mirror
        assert_eq!(bus.read(0x1800), 0xAB); // mirror
    }

    #[test]
    fn test_rom_mirror() {
        let mut rom = vec![0u8; 0x4000]; // 16KB
        rom[0] = 0x4C; // JMP at offset 0 (maps to 0x8000 and 0xC000)
        let mut bus = make_bus(rom);
        assert_eq!(bus.read(0x8000), 0x4C);
        assert_eq!(bus.read(0xC000), 0x4C); // mirrored
    }

    #[test]
    fn test_prg_ram() {
        let mut bus = make_bus(vec![0u8; 0x4000]);
        bus.write(0x6000, 0x42);
        assert_eq!(bus.read(0x6000), 0x42);
        bus.write(0x7FFF, 0xAB);
        assert_eq!(bus.read(0x7FFF), 0xAB);
    }
}
