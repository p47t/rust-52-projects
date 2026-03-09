use nes_cpu::ines::{INesRom, Mirroring};
use nes_cpu::mapper::Mapper;

/// Mapper 3 (CNROM): Switchable 8KB CHR-ROM bank, fixed PRG-ROM.
pub struct Cnrom {
    prg_rom: Vec<u8>,
    chr_rom: Vec<u8>,
    chr_bank: usize,
    chr_bank_count: usize,
    mirroring: Mirroring,
}

impl Cnrom {
    pub fn new(rom: &INesRom) -> Self {
        let chr_bank_count = (rom.chr_rom.len() / 0x2000).max(1);
        Self {
            prg_rom: rom.prg_rom.clone(),
            chr_rom: rom.chr_rom.clone(),
            chr_bank: 0,
            chr_bank_count,
            mirroring: rom.mirroring,
        }
    }
}

impl Mapper for Cnrom {
    fn read_prg(&self, addr: u16) -> u8 {
        match addr {
            0x8000..=0xFFFF => {
                let offset = (addr - 0x8000) as usize % self.prg_rom.len();
                self.prg_rom[offset]
            }
            _ => 0,
        }
    }

    fn write_prg(&mut self, addr: u16, val: u8) {
        if addr >= 0x8000 {
            self.chr_bank = (val as usize) % self.chr_bank_count;
        }
    }

    fn read_chr(&self, addr: u16) -> u8 {
        let offset = self.chr_bank * 0x2000 + (addr & 0x1FFF) as usize;
        if offset < self.chr_rom.len() {
            self.chr_rom[offset]
        } else {
            0
        }
    }

    fn write_chr(&mut self, _addr: u16, _val: u8) {
        // CHR-ROM only, writes ignored
    }

    fn mirroring(&self) -> Mirroring {
        self.mirroring
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_cnrom(chr_banks: usize) -> Cnrom {
        let mut chr_rom = vec![0u8; chr_banks * 0x2000];
        // Fill each bank with its bank number
        for bank in 0..chr_banks {
            for i in 0..0x2000 {
                chr_rom[bank * 0x2000 + i] = bank as u8;
            }
        }
        let rom = INesRom {
            prg_rom: vec![0u8; 0x8000],
            chr_rom,
            mapper: 3,
            mirroring: Mirroring::Horizontal,
            has_battery: false,
        };
        Cnrom::new(&rom)
    }

    #[test]
    fn initial_bank_zero() {
        let m = make_cnrom(4);
        assert_eq!(m.read_chr(0x0000), 0);
    }

    #[test]
    fn chr_bank_switch() {
        let mut m = make_cnrom(4);
        m.write_prg(0x8000, 2);
        assert_eq!(m.read_chr(0x0000), 2);
    }

    #[test]
    fn chr_bank_wraps() {
        let mut m = make_cnrom(4);
        m.write_prg(0x8000, 5); // 5 % 4 = 1
        assert_eq!(m.read_chr(0x0000), 1);
    }

    #[test]
    fn prg_passthrough() {
        let m = make_cnrom(2);
        // PRG is 32KB of zeros
        assert_eq!(m.read_prg(0x8000), 0);
    }
}
