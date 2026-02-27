use crate::ines::{INesRom, Mirroring};

#[allow(dead_code)] // mirroring/mapper stored for future PPU/mapper support
pub struct Bus {
    ram: [u8; 2048],
    prg_rom: Vec<u8>,
    pub mirroring: Mirroring,
    pub mapper: u8,
}

impl Bus {
    pub fn from_rom(rom: INesRom) -> Self {
        Self {
            ram: [0u8; 2048],
            prg_rom: rom.prg_rom,
            mirroring: rom.mirroring,
            mapper: rom.mapper,
        }
    }

    pub fn read(&self, addr: u16) -> u8 {
        match addr {
            0x0000..=0x1FFF => self.ram[(addr & 0x07FF) as usize],
            0x8000..=0xFFFF => {
                let offset = (addr - 0x8000) as usize % self.prg_rom.len();
                self.prg_rom[offset]
            }
            _ => 0xFF, // open bus
        }
    }

    pub fn write(&mut self, addr: u16, val: u8) {
        if let 0x0000..=0x1FFF = addr {
            self.ram[(addr & 0x07FF) as usize] = val;
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
        let bus = make_bus(rom);
        assert_eq!(bus.read(0x8000), 0x4C);
        assert_eq!(bus.read(0xC000), 0x4C); // mirrored
    }
}
