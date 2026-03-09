use nes_cpu::ines::{INesRom, Mirroring};
use nes_cpu::mapper::Mapper;

/// Mapper 7 (AxROM): 32KB PRG bank switching + single-screen mirroring control.
/// CHR-RAM only.
pub struct AxRom {
    prg_rom: Vec<u8>,
    chr_ram: Vec<u8>,
    prg_bank: usize,
    prg_bank_count: usize,
    mirroring: Mirroring,
}

impl AxRom {
    pub fn new(rom: &INesRom) -> Self {
        let prg_bank_count = rom.prg_rom.len() / 0x8000;
        Self {
            prg_rom: rom.prg_rom.clone(),
            chr_ram: vec![0u8; 8192],
            prg_bank: 0,
            prg_bank_count,
            mirroring: Mirroring::SingleScreenLower,
        }
    }
}

impl Mapper for AxRom {
    fn read_prg(&self, addr: u16) -> u8 {
        match addr {
            0x8000..=0xFFFF => {
                let offset = self.prg_bank * 0x8000 + (addr - 0x8000) as usize;
                self.prg_rom[offset]
            }
            _ => 0,
        }
    }

    fn write_prg(&mut self, addr: u16, val: u8) {
        if addr >= 0x8000 {
            self.prg_bank = (val as usize & 0x07) % self.prg_bank_count;
            self.mirroring = if val & 0x10 != 0 {
                Mirroring::SingleScreenUpper
            } else {
                Mirroring::SingleScreenLower
            };
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
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_axrom(banks: usize) -> AxRom {
        let mut prg_rom = vec![0u8; banks * 0x8000];
        for bank in 0..banks {
            for i in 0..0x8000 {
                prg_rom[bank * 0x8000 + i] = bank as u8;
            }
        }
        let rom = INesRom {
            prg_rom,
            chr_rom: Vec::new(),
            mapper: 7,
            mirroring: Mirroring::SingleScreenLower,
            has_battery: false,
        };
        AxRom::new(&rom)
    }

    #[test]
    fn initial_bank_zero() {
        let m = make_axrom(4);
        assert_eq!(m.read_prg(0x8000), 0);
    }

    #[test]
    fn bank_switch_32kb() {
        let mut m = make_axrom(4);
        m.write_prg(0x8000, 2);
        assert_eq!(m.read_prg(0x8000), 2);
        assert_eq!(m.read_prg(0xC000), 2); // same 32KB bank
    }

    #[test]
    fn mirroring_toggle() {
        let mut m = make_axrom(4);
        assert_eq!(m.mirroring(), Mirroring::SingleScreenLower);
        m.write_prg(0x8000, 0x10); // bit 4 set
        assert_eq!(m.mirroring(), Mirroring::SingleScreenUpper);
        m.write_prg(0x8000, 0x00); // bit 4 clear
        assert_eq!(m.mirroring(), Mirroring::SingleScreenLower);
    }

    #[test]
    fn chr_ram() {
        let mut m = make_axrom(2);
        m.write_chr(0x0100, 0x42);
        assert_eq!(m.read_chr(0x0100), 0x42);
    }
}
