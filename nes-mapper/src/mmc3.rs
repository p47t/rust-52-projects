use nes_cpu::ines::{INesRom, Mirroring};
use nes_cpu::mapper::{Mapper, MapperIrq};

/// Mapper 4 (MMC3/TxROM): Fine-grained PRG/CHR banking with scanline counter IRQ.
///
/// Registers (selected by address bit 0, even/odd):
/// - $8000 (even): Bank select — bits 0-2 target, bit 6 PRG mode, bit 7 CHR inversion
/// - $8001 (odd):  Bank data — written to register selected by $8000
/// - $A000 (even): Mirroring — bit 0: 0=Vertical, 1=Horizontal
/// - $A001 (odd):  PRG-RAM protect (stored, not enforced)
/// - $C000 (even): IRQ latch value
/// - $C001 (odd):  IRQ counter reload
/// - $E000 (even): IRQ disable + acknowledge
/// - $E001 (odd):  IRQ enable
pub struct Mmc3 {
    prg_rom: Vec<u8>,
    prg_ram: [u8; 8192],
    chr: Vec<u8>,
    chr_ram: bool,

    bank_select: u8,
    bank_regs: [u8; 8],
    mirroring: Mirroring,

    prg_bank_count: usize, // in 8KB units
    chr_bank_count: usize, // in 1KB units

    // Scanline counter / IRQ
    irq_latch: u8,
    irq_counter: u8,
    irq_reload: bool,
    irq_enabled: bool,
    irq_pending: bool,
}

impl Mmc3 {
    pub fn new(rom: &INesRom) -> Self {
        let chr_ram = rom.chr_rom.is_empty();
        let chr = if chr_ram {
            vec![0u8; 8192]
        } else {
            rom.chr_rom.clone()
        };
        let prg_bank_count = rom.prg_rom.len() / 0x2000;
        let chr_bank_count = (chr.len() / 0x0400).max(1);

        Self {
            prg_rom: rom.prg_rom.clone(),
            prg_ram: [0; 8192],
            chr,
            chr_ram,
            bank_select: 0,
            bank_regs: [0; 8],
            mirroring: rom.mirroring,
            prg_bank_count,
            chr_bank_count,
            irq_latch: 0,
            irq_counter: 0,
            irq_reload: false,
            irq_enabled: false,
            irq_pending: false,
        }
    }

    fn prg_mode(&self) -> bool {
        self.bank_select & 0x40 != 0
    }

    fn chr_inversion(&self) -> bool {
        self.bank_select & 0x80 != 0
    }

    /// Map a PRG address ($8000-$FFFF) to a ROM offset.
    fn prg_offset(&self, addr: u16) -> usize {
        let second_last = self.prg_bank_count - 2;
        let last = self.prg_bank_count - 1;
        let r6 = (self.bank_regs[6] as usize) % self.prg_bank_count;
        let r7 = (self.bank_regs[7] as usize) % self.prg_bank_count;

        let bank = match addr {
            0x8000..=0x9FFF => {
                if self.prg_mode() {
                    second_last
                } else {
                    r6
                }
            }
            0xA000..=0xBFFF => r7,
            0xC000..=0xDFFF => {
                if self.prg_mode() {
                    r6
                } else {
                    second_last
                }
            }
            _ => last, // $E000-$FFFF
        };

        bank * 0x2000 + (addr & 0x1FFF) as usize
    }

    /// Map a CHR address ($0000-$1FFF) to a CHR data offset.
    fn chr_offset(&self, addr: u16) -> usize {
        let addr = addr & 0x1FFF;
        let inv = self.chr_inversion();

        // Determine which 1KB slot this address falls into (0-7)
        let slot = (addr >> 10) as usize; // 0-7

        // Map slot to bank register based on CHR inversion
        //
        // Normal (inv=false):
        //   Slots 0,1 → R0 (2KB), Slots 2,3 → R1 (2KB),
        //   Slot 4 → R2, Slot 5 → R3, Slot 6 → R4, Slot 7 → R5
        //
        // Inverted (inv=true):
        //   Slot 0 → R2, Slot 1 → R3, Slot 2 → R4, Slot 3 → R5,
        //   Slots 4,5 → R0 (2KB), Slots 6,7 → R1 (2KB)
        let bank = if !inv {
            match slot {
                0 => (self.bank_regs[0] & 0xFE) as usize, // R0 low
                1 => (self.bank_regs[0] | 0x01) as usize, // R0 high
                2 => (self.bank_regs[1] & 0xFE) as usize, // R1 low
                3 => (self.bank_regs[1] | 0x01) as usize, // R1 high
                4 => self.bank_regs[2] as usize,          // R2
                5 => self.bank_regs[3] as usize,          // R3
                6 => self.bank_regs[4] as usize,          // R4
                _ => self.bank_regs[5] as usize,          // R5
            }
        } else {
            match slot {
                0 => self.bank_regs[2] as usize,          // R2
                1 => self.bank_regs[3] as usize,          // R3
                2 => self.bank_regs[4] as usize,          // R4
                3 => self.bank_regs[5] as usize,          // R5
                4 => (self.bank_regs[0] & 0xFE) as usize, // R0 low
                5 => (self.bank_regs[0] | 0x01) as usize, // R0 high
                6 => (self.bank_regs[1] & 0xFE) as usize, // R1 low
                _ => (self.bank_regs[1] | 0x01) as usize, // R1 high
            }
        };

        let bank = bank % self.chr_bank_count;
        bank * 0x0400 + (addr & 0x03FF) as usize
    }
}

impl Mapper for Mmc3 {
    fn read_prg(&self, addr: u16) -> u8 {
        match addr {
            0x6000..=0x7FFF => self.prg_ram[(addr - 0x6000) as usize],
            0x8000..=0xFFFF => {
                let offset = self.prg_offset(addr);
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
            0x8000..=0x9FFF => {
                if addr & 1 == 0 {
                    // Bank select
                    self.bank_select = val;
                } else {
                    // Bank data
                    let target = (self.bank_select & 0x07) as usize;
                    self.bank_regs[target] = val;
                }
            }
            0xA000..=0xBFFF => {
                if addr & 1 == 0 {
                    // Mirroring
                    self.mirroring = if val & 1 != 0 {
                        Mirroring::Horizontal
                    } else {
                        Mirroring::Vertical
                    };
                }
                // Odd: PRG-RAM protect (ignored)
            }
            0xC000..=0xDFFF => {
                if addr & 1 == 0 {
                    // IRQ latch
                    self.irq_latch = val;
                } else {
                    // IRQ reload
                    self.irq_reload = true;
                }
            }
            0xE000..=0xFFFF => {
                if addr & 1 == 0 {
                    // IRQ disable + acknowledge
                    self.irq_enabled = false;
                    self.irq_pending = false;
                } else {
                    // IRQ enable
                    self.irq_enabled = true;
                }
            }
            _ => {}
        }
    }

    fn read_chr(&self, addr: u16) -> u8 {
        let offset = self.chr_offset(addr);
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
        let offset = self.chr_offset(addr);
        if offset < self.chr.len() {
            self.chr[offset] = val;
        }
    }

    fn mirroring(&self) -> Mirroring {
        self.mirroring
    }

    fn as_irq(&mut self) -> Option<&mut dyn MapperIrq> {
        Some(self)
    }
}

impl MapperIrq for Mmc3 {
    fn clock_scanline(&mut self) {
        if self.irq_counter == 0 || self.irq_reload {
            self.irq_counter = self.irq_latch;
            self.irq_reload = false;
        } else {
            self.irq_counter -= 1;
        }

        if self.irq_counter == 0 && self.irq_enabled {
            self.irq_pending = true;
        }
    }

    fn take_irq(&mut self) -> bool {
        let pending = self.irq_pending;
        self.irq_pending = false;
        pending
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Create a test MMC3 with identifiable bank contents.
    /// Each 8KB PRG bank is filled with its bank index.
    /// Each 1KB CHR bank is filled with its bank index.
    fn make_mmc3(prg_8k_banks: usize, chr_1k_banks: usize) -> Mmc3 {
        let prg_size = prg_8k_banks * 0x2000;
        let mut prg_rom = vec![0u8; prg_size];
        for bank in 0..prg_8k_banks {
            for i in 0..0x2000 {
                prg_rom[bank * 0x2000 + i] = bank as u8;
            }
        }
        let chr_size = chr_1k_banks * 0x0400;
        let chr_rom = if chr_size > 0 {
            let mut chr = vec![0u8; chr_size];
            for bank in 0..chr_1k_banks {
                for i in 0..0x0400 {
                    chr[bank * 0x0400 + i] = bank as u8;
                }
            }
            chr
        } else {
            Vec::new()
        };
        let rom = INesRom {
            prg_rom,
            chr_rom,
            mapper: 4,
            mirroring: Mirroring::Vertical,
            has_battery: false,
        };
        Mmc3::new(&rom)
    }

    #[test]
    fn default_prg_last_bank_at_e000() {
        // 16 × 8KB banks = 128KB PRG
        let m = make_mmc3(16, 0);
        // $E000 is always the last bank
        assert_eq!(m.read_prg(0xE000), 15);
        // $C000 defaults to second-to-last (mode 0, R6=0 → bank 0 at $8000)
        assert_eq!(m.read_prg(0xC000), 14);
    }

    #[test]
    fn prg_mode_0_switch_8000() {
        let mut m = make_mmc3(16, 0);
        // Set R6 = 5 (bank select target 6, mode 0)
        m.write_prg(0x8000, 0x06); // target R6, mode 0
        m.write_prg(0x8001, 5); // R6 = 5
        assert_eq!(m.read_prg(0x8000), 5);
        assert_eq!(m.read_prg(0xC000), 14); // second-to-last fixed
        assert_eq!(m.read_prg(0xE000), 15); // last fixed
    }

    #[test]
    fn prg_mode_1_swap_8000_c000() {
        let mut m = make_mmc3(16, 0);
        // Set R6 = 5 with PRG mode 1 (bit 6 set)
        m.write_prg(0x8000, 0x46); // target R6, mode 1
        m.write_prg(0x8001, 5); // R6 = 5
                                // Mode 1: $8000 = second-to-last, $C000 = R6
        assert_eq!(m.read_prg(0x8000), 14); // second-to-last
        assert_eq!(m.read_prg(0xC000), 5); // R6
        assert_eq!(m.read_prg(0xE000), 15); // last fixed
    }

    #[test]
    fn prg_a000_always_r7() {
        let mut m = make_mmc3(16, 0);
        // Set R7 = 3
        m.write_prg(0x8000, 0x07); // target R7
        m.write_prg(0x8001, 3);
        assert_eq!(m.read_prg(0xA000), 3);

        // R7 stays at $A000 regardless of PRG mode
        m.write_prg(0x8000, 0x47); // target R7, mode 1
        m.write_prg(0x8001, 10);
        assert_eq!(m.read_prg(0xA000), 10);
    }

    #[test]
    fn chr_normal_mode() {
        // 32 × 1KB CHR banks
        let mut m = make_mmc3(4, 32);
        // R0 = 4 (2KB at $0000-$07FF)
        m.write_prg(0x8000, 0x00); // target R0, no inversion
        m.write_prg(0x8001, 4);
        // R0 selects 2KB: slots 0,1 → banks 4,5
        assert_eq!(m.read_chr(0x0000), 4);
        assert_eq!(m.read_chr(0x0400), 5);

        // R1 = 10 (2KB at $0800-$0FFF)
        m.write_prg(0x8000, 0x01); // target R1
        m.write_prg(0x8001, 10);
        assert_eq!(m.read_chr(0x0800), 10);
        assert_eq!(m.read_chr(0x0C00), 11);

        // R2 = 20 (1KB at $1000)
        m.write_prg(0x8000, 0x02);
        m.write_prg(0x8001, 20);
        assert_eq!(m.read_chr(0x1000), 20);

        // R3 = 21 (1KB at $1400)
        m.write_prg(0x8000, 0x03);
        m.write_prg(0x8001, 21);
        assert_eq!(m.read_chr(0x1400), 21);

        // R4 = 22 (1KB at $1800)
        m.write_prg(0x8000, 0x04);
        m.write_prg(0x8001, 22);
        assert_eq!(m.read_chr(0x1800), 22);

        // R5 = 23 (1KB at $1C00)
        m.write_prg(0x8000, 0x05);
        m.write_prg(0x8001, 23);
        assert_eq!(m.read_chr(0x1C00), 23);
    }

    #[test]
    fn chr_inverted_mode() {
        let mut m = make_mmc3(4, 32);
        // Set CHR inversion (bit 7)
        // R0 = 4, R2 = 20
        m.write_prg(0x8000, 0x80); // target R0, CHR inverted
        m.write_prg(0x8001, 4);
        m.write_prg(0x8000, 0x82); // target R2, CHR inverted
        m.write_prg(0x8001, 20);

        // Inverted: R2 at $0000, R0 (2KB) at $1000-$17FF
        assert_eq!(m.read_chr(0x0000), 20); // R2
        assert_eq!(m.read_chr(0x1000), 4); // R0 low
        assert_eq!(m.read_chr(0x1400), 5); // R0 high
    }

    #[test]
    fn mirroring_toggle() {
        let mut m = make_mmc3(4, 0);
        assert_eq!(m.mirroring(), Mirroring::Vertical); // default from ROM header
        m.write_prg(0xA000, 0x01); // horizontal
        assert_eq!(m.mirroring(), Mirroring::Horizontal);
        m.write_prg(0xA000, 0x00); // vertical
        assert_eq!(m.mirroring(), Mirroring::Vertical);
    }

    #[test]
    fn prg_ram() {
        let mut m = make_mmc3(4, 0);
        m.write_prg(0x6000, 0x42);
        assert_eq!(m.read_prg(0x6000), 0x42);
        m.write_prg(0x7FFF, 0xAB);
        assert_eq!(m.read_prg(0x7FFF), 0xAB);
    }

    #[test]
    fn chr_ram_when_no_chr_rom() {
        let mut m = make_mmc3(4, 0); // 0 CHR = CHR-RAM
        m.write_chr(0x0100, 0x55);
        assert_eq!(m.read_chr(0x0100), 0x55);
    }

    #[test]
    fn irq_counter_basic() {
        let mut m = make_mmc3(4, 0);
        // Set latch to 3
        m.write_prg(0xC000, 3); // IRQ latch = 3
        m.write_prg(0xC001, 0); // reload flag
        m.write_prg(0xE001, 0); // enable IRQ

        // First clock: reload (counter was 0), counter = 3
        m.clock_scanline();
        assert!(!m.take_irq());

        // Second clock: counter = 2
        m.clock_scanline();
        assert!(!m.take_irq());

        // Third clock: counter = 1
        m.clock_scanline();
        assert!(!m.take_irq());

        // Fourth clock: counter = 0, IRQ fires
        m.clock_scanline();
        assert!(m.take_irq());
        // take_irq clears pending
        assert!(!m.take_irq());
    }

    #[test]
    fn irq_disabled_no_fire() {
        let mut m = make_mmc3(4, 0);
        m.write_prg(0xC000, 1); // latch = 1
        m.write_prg(0xC001, 0); // reload
                                // IRQ NOT enabled

        m.clock_scanline(); // reload to 1
        m.clock_scanline(); // counter → 0, but IRQ disabled
        assert!(!m.take_irq());
    }

    #[test]
    fn irq_acknowledge_clears_pending() {
        let mut m = make_mmc3(4, 0);
        m.write_prg(0xC000, 1);
        m.write_prg(0xC001, 0);
        m.write_prg(0xE001, 0); // enable

        m.clock_scanline(); // reload to 1
        m.clock_scanline(); // counter → 0, IRQ pending

        // Acknowledge by writing to $E000 (disable + clear)
        m.write_prg(0xE000, 0);
        assert!(!m.take_irq()); // cleared
    }

    #[test]
    fn irq_reload_mid_count() {
        let mut m = make_mmc3(4, 0);
        m.write_prg(0xC000, 5); // latch = 5
        m.write_prg(0xC001, 0); // reload
        m.write_prg(0xE001, 0); // enable

        m.clock_scanline(); // reload to 5
        m.clock_scanline(); // 4
        m.clock_scanline(); // 3

        // Force reload
        m.write_prg(0xC000, 2); // change latch to 2
        m.write_prg(0xC001, 0); // set reload flag

        m.clock_scanline(); // reload to 2 (from new latch)
        assert!(!m.take_irq());
        m.clock_scanline(); // 1
        assert!(!m.take_irq());
        m.clock_scanline(); // 0 → IRQ
        assert!(m.take_irq());
    }
}
