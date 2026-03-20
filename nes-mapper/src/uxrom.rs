use nes_cpu::ines::{INesRom, Mirroring};
use nes_cpu::mapper::Mapper;
use nes_cpu::state::*;

/// Mapper 2 (UxROM): Switchable 16KB PRG bank at $8000, fixed last bank at $C000.
/// CHR-RAM only (no CHR-ROM banking).
pub struct UxRom {
    prg_rom: Vec<u8>,
    chr_ram: Vec<u8>,
    prg_bank: usize,
    prg_bank_count: usize,
    mirroring: Mirroring,
}

impl UxRom {
    pub fn new(rom: &INesRom) -> Self {
        let prg_bank_count = rom.prg_rom.len() / 0x4000;
        Self {
            prg_rom: rom.prg_rom.clone(),
            chr_ram: vec![0u8; 8192],
            prg_bank: 0,
            prg_bank_count,
            mirroring: rom.mirroring,
        }
    }
}

impl Mapper for UxRom {
    fn read_prg(&self, addr: u16) -> u8 {
        match addr {
            0x8000..=0xBFFF => {
                let offset = self.prg_bank * 0x4000 + (addr - 0x8000) as usize;
                self.prg_rom[offset]
            }
            0xC000..=0xFFFF => {
                let offset = (self.prg_bank_count - 1) * 0x4000 + (addr - 0xC000) as usize;
                self.prg_rom[offset]
            }
            _ => 0,
        }
    }

    fn write_prg(&mut self, addr: u16, val: u8) {
        if addr >= 0x8000 {
            self.prg_bank = (val as usize) % self.prg_bank_count;
        }
    }

    fn read_chr(&self, addr: u16) -> u8 {
        self.chr_ram[(addr & 0x1FFF) as usize]
    }

    fn write_chr(&mut self, addr: u16, val: u8) {
        self.chr_ram[(addr & 0x1FFF) as usize] = val;
    }

    fn mirroring(&self) -> Mirroring {
        self.mirroring
    }

    fn save_state(&self) -> Vec<u8> {
        let mut out = Vec::new();
        write_bytes(&mut out, &self.chr_ram);
        write_u8(&mut out, self.prg_bank as u8);
        out
    }

    fn load_state(&mut self, data: &[u8]) {
        let mut cursor = data;
        let chr = read_bytes(&mut cursor);
        self.chr_ram.copy_from_slice(&chr);
        self.prg_bank = read_u8(&mut cursor) as usize;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_uxrom(banks: usize) -> UxRom {
        let prg_size = banks * 0x4000;
        let mut prg_rom = vec![0u8; prg_size];
        // Fill each bank with its bank number
        for bank in 0..banks {
            for i in 0..0x4000 {
                prg_rom[bank * 0x4000 + i] = bank as u8;
            }
        }
        let rom = INesRom {
            prg_rom,
            chr_rom: Vec::new(),
            mapper: 2,
            mirroring: Mirroring::Vertical,
            has_battery: false,
        };
        UxRom::new(&rom)
    }

    #[test]
    fn initial_bank_zero() {
        let m = make_uxrom(8);
        assert_eq!(m.read_prg(0x8000), 0); // bank 0
    }

    #[test]
    fn fixed_last_bank() {
        let m = make_uxrom(8);
        assert_eq!(m.read_prg(0xC000), 7); // last bank (7)
    }

    #[test]
    fn bank_switch() {
        let mut m = make_uxrom(8);
        m.write_prg(0x8000, 3); // switch to bank 3
        assert_eq!(m.read_prg(0x8000), 3);
        assert_eq!(m.read_prg(0xC000), 7); // last bank unchanged
    }

    #[test]
    fn bank_wraps() {
        let mut m = make_uxrom(4);
        m.write_prg(0x8000, 5); // 5 % 4 = 1
        assert_eq!(m.read_prg(0x8000), 1);
    }

    #[test]
    fn chr_ram() {
        let mut m = make_uxrom(2);
        m.write_chr(0x0000, 0x42);
        assert_eq!(m.read_chr(0x0000), 0x42);
    }
}
