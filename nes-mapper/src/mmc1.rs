use nes_cpu::ines::{INesRom, Mirroring};
use nes_cpu::mapper::Mapper;
use nes_cpu::state::*;

/// Mapper 1 (MMC1/SxROM): Serial shift register for PRG/CHR banking and mirroring.
///
/// Registers (selected by addr bits 14-13 on 5th shift write):
/// - $8000-$9FFF (reg 0): Control — mirroring, PRG mode, CHR mode
/// - $A000-$BFFF (reg 1): CHR bank 0
/// - $C000-$DFFF (reg 2): CHR bank 1
/// - $E000-$FFFF (reg 3): PRG bank + PRG-RAM enable
pub struct Mmc1 {
    prg_rom: Vec<u8>,
    prg_ram: [u8; 8192],
    chr: Vec<u8>,
    chr_ram: bool,

    // 5-bit serial shift register
    shift: u8,
    shift_count: u8,

    // Internal registers
    control: u8,
    chr_bank_0: u8,
    chr_bank_1: u8,
    prg_bank: u8,

    prg_bank_count: usize,
    chr_bank_count: usize, // in 4KB units
}

impl Mmc1 {
    pub fn new(rom: &INesRom) -> Self {
        let chr_ram = rom.chr_rom.is_empty();
        let chr = if chr_ram {
            vec![0u8; 8192]
        } else {
            rom.chr_rom.clone()
        };
        let prg_bank_count = rom.prg_rom.len() / 0x4000;
        let chr_bank_count = (chr.len() / 0x1000).max(1);

        Self {
            prg_rom: rom.prg_rom.clone(),
            prg_ram: [0; 8192],
            chr,
            chr_ram,
            shift: 0,
            shift_count: 0,
            // Default control: PRG mode 3 (fix last, switch $8000), CHR 8KB mode
            control: 0x0C,
            chr_bank_0: 0,
            chr_bank_1: 0,
            prg_bank: 0,
            prg_bank_count,
            chr_bank_count,
        }
    }

    fn prg_mode(&self) -> u8 {
        (self.control >> 2) & 0x03
    }

    fn chr_mode(&self) -> bool {
        self.control & 0x10 != 0 // true = 4KB mode, false = 8KB mode
    }
}

impl Mapper for Mmc1 {
    fn read_prg(&self, addr: u16) -> u8 {
        match addr {
            0x6000..=0x7FFF => self.prg_ram[(addr - 0x6000) as usize],
            0x8000..=0xBFFF => {
                let bank = match self.prg_mode() {
                    0 | 1 => (self.prg_bank as usize & 0xFE) % self.prg_bank_count, // 32KB: even bank
                    2 => 0,                                                         // fix first
                    _ => (self.prg_bank as usize) % self.prg_bank_count,            // switch
                };
                let offset = bank * 0x4000 + (addr - 0x8000) as usize;
                self.prg_rom[offset % self.prg_rom.len()]
            }
            0xC000..=0xFFFF => {
                let bank = match self.prg_mode() {
                    0 | 1 => {
                        // 32KB: odd bank (even + 1)
                        ((self.prg_bank as usize & 0xFE) + 1) % self.prg_bank_count
                    }
                    2 => (self.prg_bank as usize) % self.prg_bank_count, // switch
                    _ => self.prg_bank_count - 1,                        // fix last
                };
                let offset = bank * 0x4000 + (addr - 0xC000) as usize;
                self.prg_rom[offset % self.prg_rom.len()]
            }
            _ => 0,
        }
    }

    fn write_prg(&mut self, addr: u16, val: u8) {
        match addr {
            0x6000..=0x7FFF => {
                self.prg_ram[(addr - 0x6000) as usize] = val;
            }
            0x8000..=0xFFFF => {
                if val & 0x80 != 0 {
                    // Reset shift register and set PRG mode to 3
                    self.shift = 0;
                    self.shift_count = 0;
                    self.control |= 0x0C;
                    return;
                }

                self.shift |= (val & 1) << self.shift_count;
                self.shift_count += 1;

                if self.shift_count == 5 {
                    let value = self.shift;
                    match (addr >> 13) & 0x03 {
                        0 => self.control = value,    // $8000-$9FFF
                        1 => self.chr_bank_0 = value, // $A000-$BFFF
                        2 => self.chr_bank_1 = value, // $C000-$DFFF
                        _ => self.prg_bank = value,   // $E000-$FFFF
                    }
                    self.shift = 0;
                    self.shift_count = 0;
                }
            }
            _ => {}
        }
    }

    fn read_chr(&self, addr: u16) -> u8 {
        let offset = if self.chr_mode() {
            // 4KB mode
            match addr {
                0x0000..=0x0FFF => {
                    let bank = (self.chr_bank_0 as usize) % self.chr_bank_count;
                    bank * 0x1000 + (addr & 0x0FFF) as usize
                }
                _ => {
                    let bank = (self.chr_bank_1 as usize) % self.chr_bank_count;
                    bank * 0x1000 + (addr & 0x0FFF) as usize
                }
            }
        } else {
            // 8KB mode: chr_bank_0 selects 8KB (bit 0 ignored)
            let bank = (self.chr_bank_0 as usize & 0xFE) % self.chr_bank_count;
            bank * 0x1000 + (addr & 0x1FFF) as usize
        };
        if offset < self.chr.len() {
            self.chr[offset]
        } else {
            0
        }
    }

    fn write_chr(&mut self, addr: u16, val: u8) {
        if !self.chr_ram {
            return;
        }
        let offset = (addr & 0x1FFF) as usize;
        if offset < self.chr.len() {
            self.chr[offset] = val;
        }
    }

    fn mirroring(&self) -> Mirroring {
        match self.control & 0x03 {
            0 => Mirroring::SingleScreenLower,
            1 => Mirroring::SingleScreenUpper,
            2 => Mirroring::Vertical,
            _ => Mirroring::Horizontal,
        }
    }

    fn save_state(&self) -> Vec<u8> {
        let mut out = Vec::new();
        write_bytes(&mut out, &self.prg_ram);
        if self.chr_ram {
            write_bytes(&mut out, &self.chr);
        }
        write_u8(&mut out, self.shift);
        write_u8(&mut out, self.shift_count);
        write_u8(&mut out, self.control);
        write_u8(&mut out, self.chr_bank_0);
        write_u8(&mut out, self.chr_bank_1);
        write_u8(&mut out, self.prg_bank);
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
        self.shift = read_u8(&mut cursor);
        self.shift_count = read_u8(&mut cursor);
        self.control = read_u8(&mut cursor);
        self.chr_bank_0 = read_u8(&mut cursor);
        self.chr_bank_1 = read_u8(&mut cursor);
        self.prg_bank = read_u8(&mut cursor);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_mmc1(prg_banks: usize, chr_4k_banks: usize) -> Mmc1 {
        let prg_size = prg_banks * 0x4000;
        let mut prg_rom = vec![0u8; prg_size];
        for bank in 0..prg_banks {
            for i in 0..0x4000 {
                prg_rom[bank * 0x4000 + i] = bank as u8;
            }
        }
        let chr_size = chr_4k_banks * 0x1000;
        let chr_rom = if chr_size > 0 {
            let mut chr = vec![0u8; chr_size];
            for bank in 0..chr_4k_banks {
                for i in 0..0x1000 {
                    chr[bank * 0x1000 + i] = bank as u8;
                }
            }
            chr
        } else {
            Vec::new()
        };
        let rom = INesRom {
            prg_rom,
            chr_rom,
            mapper: 1,
            mirroring: Mirroring::Horizontal,
            has_battery: false,
        };
        Mmc1::new(&rom)
    }

    /// Helper to write a 5-bit value to a mapper register via the shift register.
    fn shift_write(m: &mut Mmc1, addr: u16, value: u8) {
        for bit in 0..5 {
            m.write_prg(addr, (value >> bit) & 1);
        }
    }

    #[test]
    fn default_prg_mode_3_fix_last() {
        let m = make_mmc1(8, 0);
        // Mode 3: fix last bank at $C000
        assert_eq!(m.read_prg(0xC000), 7); // last bank
    }

    #[test]
    fn shift_register_5_writes() {
        let mut m = make_mmc1(8, 0);
        // Switch PRG bank to 3 via register at $E000
        shift_write(&mut m, 0xE000, 3);
        assert_eq!(m.read_prg(0x8000), 3);
    }

    #[test]
    fn shift_register_reset() {
        let mut m = make_mmc1(8, 0);
        // Partial writes then reset
        m.write_prg(0xE000, 1); // bit 0
        m.write_prg(0xE000, 0); // bit 1
        m.write_prg(0xE000, 0x80); // reset!
                                   // Shift should be cleared, need fresh 5 writes
        shift_write(&mut m, 0xE000, 5);
        assert_eq!(m.read_prg(0x8000), 5);
    }

    #[test]
    fn prg_mode_2_fix_first_switch_last() {
        let mut m = make_mmc1(8, 0);
        // Set control to mode 2 (fix first at $8000, switch $C000)
        shift_write(&mut m, 0x8000, 0x08); // bits 3-2 = 10 = mode 2
        shift_write(&mut m, 0xE000, 5);
        assert_eq!(m.read_prg(0x8000), 0); // first bank fixed
        assert_eq!(m.read_prg(0xC000), 5); // switched
    }

    #[test]
    fn prg_mode_0_32kb() {
        let mut m = make_mmc1(8, 0);
        // Set control to mode 0 (32KB switching)
        shift_write(&mut m, 0x8000, 0x00); // mode 0
        shift_write(&mut m, 0xE000, 4); // bank 4
                                        // 32KB mode: $8000 = bank 4 (even), $C000 = bank 5 (odd)
        assert_eq!(m.read_prg(0x8000), 4);
        assert_eq!(m.read_prg(0xC000), 5);
    }

    #[test]
    fn chr_4kb_mode() {
        let mut m = make_mmc1(2, 8); // 8 × 4KB CHR banks
                                     // Enable 4KB CHR mode
        shift_write(&mut m, 0x8000, 0x10); // bit 4 = 1
                                           // Set CHR bank 0 to bank 3
        shift_write(&mut m, 0xA000, 3);
        assert_eq!(m.read_chr(0x0000), 3);
        // Set CHR bank 1 to bank 5
        shift_write(&mut m, 0xC000, 5);
        assert_eq!(m.read_chr(0x1000), 5);
    }

    #[test]
    fn chr_8kb_mode() {
        let mut m = make_mmc1(2, 8);
        // 8KB mode (default, bit 4 = 0)
        shift_write(&mut m, 0x8000, 0x0C); // mode 3, chr 8kb
        shift_write(&mut m, 0xA000, 4); // selects 8KB bank (bit 0 ignored → bank 4)
        assert_eq!(m.read_chr(0x0000), 4);
        assert_eq!(m.read_chr(0x1000), 5); // next 4KB
    }

    #[test]
    fn mirroring_changes() {
        let mut m = make_mmc1(2, 0);
        shift_write(&mut m, 0x8000, 0x0C | 0x02); // vertical
        assert_eq!(m.mirroring(), Mirroring::Vertical);
        shift_write(&mut m, 0x8000, 0x0C | 0x03); // horizontal
        assert_eq!(m.mirroring(), Mirroring::Horizontal);
        shift_write(&mut m, 0x8000, 0x0C); // single lower (mirror bits = 00)
        assert_eq!(m.mirroring(), Mirroring::SingleScreenLower);
        shift_write(&mut m, 0x8000, 0x0C | 0x01); // single upper
        assert_eq!(m.mirroring(), Mirroring::SingleScreenUpper);
    }

    #[test]
    fn prg_ram() {
        let mut m = make_mmc1(2, 0);
        m.write_prg(0x6000, 0x42);
        assert_eq!(m.read_prg(0x6000), 0x42);
    }
}
