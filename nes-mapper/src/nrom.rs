use nes_cpu::ines::{INesRom, Mirroring};
use nes_cpu::mapper::Mapper;
use nes_cpu::state::*;

/// Mapper 0 (NROM): No bank switching.
/// - PRG: 16KB (mirrored) or 32KB
/// - CHR: 8KB ROM or 8KB RAM
pub struct Nrom {
    prg_rom: Vec<u8>,
    prg_ram: [u8; 8192],
    chr: Vec<u8>,
    chr_ram: bool,
    mirroring: Mirroring,
}

impl Nrom {
    pub fn new(rom: &INesRom) -> Self {
        let chr_ram = rom.chr_rom.is_empty();
        Self {
            prg_rom: rom.prg_rom.clone(),
            prg_ram: [0; 8192],
            chr: if chr_ram {
                vec![0u8; 8192]
            } else {
                rom.chr_rom.clone()
            },
            chr_ram,
            mirroring: rom.mirroring,
        }
    }
}

impl Mapper for Nrom {
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

    fn read_chr(&self, addr: u16) -> u8 {
        self.chr[(addr & 0x1FFF) as usize]
    }

    fn write_chr(&mut self, addr: u16, val: u8) {
        if self.chr_ram {
            self.chr[(addr & 0x1FFF) as usize] = val;
        }
    }

    fn mirroring(&self) -> Mirroring {
        self.mirroring
    }

    fn save_state(&self) -> Vec<u8> {
        let mut out = Vec::new();
        write_bytes(&mut out, &self.prg_ram);
        if self.chr_ram {
            write_bytes(&mut out, &self.chr);
        }
        out
    }

    fn load_state(&mut self, data: &[u8]) {
        let mut cursor = data;
        let ram = read_bytes(&mut cursor);
        self.prg_ram.copy_from_slice(&ram);
        if self.chr_ram {
            let chr = read_bytes(&mut cursor);
            self.chr.copy_from_slice(&chr);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_nrom(prg_size: usize, chr_size: usize) -> Nrom {
        let mut prg_rom = vec![0u8; prg_size];
        // Fill with pattern that varies across banks (hash of offset)
        for (i, byte) in prg_rom.iter_mut().enumerate() {
            *byte = (i.wrapping_mul(7) ^ (i >> 8)) as u8;
        }
        let chr_rom = if chr_size > 0 {
            let mut chr = vec![0u8; chr_size];
            for (i, byte) in chr.iter_mut().enumerate() {
                *byte = ((i + 0x80) & 0xFF) as u8;
            }
            chr
        } else {
            Vec::new()
        };
        let rom = INesRom {
            prg_rom,
            chr_rom,
            mapper: 0,
            mirroring: Mirroring::Horizontal,
            has_battery: false,
        };
        Nrom::new(&rom)
    }

    #[test]
    fn prg_16kb_mirrored() {
        let m = make_nrom(0x4000, 0x2000); // 16KB PRG
        assert_eq!(m.read_prg(0x8000), m.read_prg(0xC000));
        assert_eq!(m.read_prg(0x8001), m.read_prg(0xC001));
    }

    #[test]
    fn prg_32kb_no_mirror() {
        let m = make_nrom(0x8000, 0x2000); // 32KB PRG
                                           // With 32KB, $8000 and $C000 map to different offsets (0 vs 0x4000)
        assert_ne!(m.read_prg(0x9000), m.read_prg(0xD000));
    }

    #[test]
    fn prg_ram_readwrite() {
        let mut m = make_nrom(0x4000, 0x2000);
        m.write_prg(0x6000, 0x42);
        assert_eq!(m.read_prg(0x6000), 0x42);
        m.write_prg(0x7FFF, 0xAB);
        assert_eq!(m.read_prg(0x7FFF), 0xAB);
    }

    #[test]
    fn chr_rom_read() {
        let m = make_nrom(0x4000, 0x2000);
        assert_eq!(m.read_chr(0x0000), 0x80); // (0 + 0x80) & 0xFF
        assert_eq!(m.read_chr(0x0001), 0x81);
    }

    #[test]
    fn chr_rom_write_ignored() {
        let mut m = make_nrom(0x4000, 0x2000);
        let original = m.read_chr(0x0000);
        m.write_chr(0x0000, 0xFF);
        assert_eq!(m.read_chr(0x0000), original); // unchanged
    }

    #[test]
    fn chr_ram_readwrite() {
        let mut m = make_nrom(0x4000, 0); // no CHR-ROM → CHR-RAM
        assert!(m.chr_ram);
        m.write_chr(0x0000, 0x42);
        assert_eq!(m.read_chr(0x0000), 0x42);
    }
}
