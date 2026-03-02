use crate::ines::{INesRom, Mirroring};

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

    pub const PRG_RAM_START: u16 = 0x6000;
    pub const PRG_RAM_END: u16 = 0x7FFF;

    pub const PRG_ROM_START: u16 = 0x8000;
}

/// Trait for I/O devices mapped into the CPU address space.
/// PPU registers ($2000-$3FFF) and OAM DMA ($4014) route through this.
pub trait BusIo {
    fn read(&mut self, addr: u16) -> u8;
    fn write(&mut self, addr: u16, val: u8);
}

pub struct Bus {
    ram: [u8; 2048],
    prg_ram: [u8; 8192], // $6000-$7FFF (battery/work RAM, blargg test output)
    prg_rom: Vec<u8>,
    pub mirroring: Mirroring,
    pub mapper: u8,
    pub io: Option<Box<dyn BusIo>>,
}

impl Bus {
    pub fn from_rom(rom: INesRom) -> Self {
        Self {
            ram: [0u8; 2048],
            prg_ram: [0u8; 8192],
            prg_rom: rom.prg_rom,
            mirroring: rom.mirroring,
            mapper: rom.mapper,
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
            addr::PRG_RAM_START..=addr::PRG_RAM_END => {
                self.prg_ram[(addr - addr::PRG_RAM_START) as usize]
            }
            addr::PRG_ROM_START..=0xFFFF => {
                let offset = (addr - addr::PRG_ROM_START) as usize % self.prg_rom.len();
                self.prg_rom[offset]
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
            addr::PRG_RAM_START..=addr::PRG_RAM_END => {
                self.prg_ram[(addr - addr::PRG_RAM_START) as usize]
            }
            _ => {
                if addr >= addr::PRG_ROM_START {
                    let offset = (addr - addr::PRG_ROM_START) as usize % self.prg_rom.len();
                    self.prg_rom[offset]
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
            addr::PRG_RAM_START..=addr::PRG_RAM_END => {
                self.prg_ram[(addr - addr::PRG_RAM_START) as usize] = val
            }
            _ => {}
        }
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
