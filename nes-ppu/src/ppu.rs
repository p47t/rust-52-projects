use nes_cpu::ines::Mirroring;

pub const PPU_2000_CTRL: u16 = 0x2000;
pub const PPU_2001_MASK: u16 = 0x2001;
pub const PPU_2002_STATUS: u16 = 0x2002;
pub const PPU_2003_OAM_ADDR: u16 = 0x2003;
pub const PPU_2004_OAM_DATA: u16 = 0x2004;
pub const PPU_2005_SCROLL: u16 = 0x2005;
pub const PPU_2006_ADDR: u16 = 0x2006;
pub const PPU_2007_DATA: u16 = 0x2007;
pub const PPU_4014_OAM_DMA: u16 = 0x4014;

pub struct Ppu {
    // Control registers
    pub ctrl: u8,   // $2000: VPHB SINN
    pub mask: u8,   // $2001: BGRs bMmG
    pub status: u8, // $2002: VSO- ----

    // OAM
    pub oam_addr: u8,
    pub oam: [u8; 256],

    // Internal registers (Loopy)
    pub v: u16, // current VRAM address (15-bit)
    pub t: u16, // temporary VRAM address (15-bit)
    pub fine_x: u8,
    pub addr_latch: bool, // w toggle (false = first write)

    // Data buffer for $2007 reads
    pub read_buffer: u8,

    // Open bus / data bus latch
    pub data_bus: u8,

    // Memory
    pub vram: [u8; 2048], // 2 nametables
    pub palette: [u8; 32],
    pub chr_rom: Vec<u8>,

    // Timing
    pub scanline: i16, // -1 (pre-render) through 260
    pub dot: u16,      // 0 through 340
    pub odd_frame: bool,

    // NMI
    pub nmi_output: bool,   // PPUCTRL bit 7: NMI enable
    pub nmi_occurred: bool, // VBlank flag (status bit 7)
    pub nmi_pending: bool,  // Edge-detected NMI to deliver to CPU
    pub nmi_line: bool,     // Previous NMI line state for edge detection
    /// PPU ticks elapsed since nmi_pending was last set. Used to determine
    /// if a register-write-triggered falling edge can cancel the NMI: if the
    /// NMI was set within the same CPU cycle as the write (age < 2), it
    /// hasn't been polled by the CPU yet and can be cancelled.
    pub nmi_pending_age: u16,
    /// Countdown for register-write-triggered NMI ($2000 enables NMI while
    /// VBlank active). On real hardware, the write happens on the last CPU
    /// cycle, so NMI isn't detected until the next instruction's penultimate
    /// cycle — effectively a 1-instruction delay. While counting down, the
    /// pending NMI is protected from falling-edge cancellation.
    pub nmi_write_delay: u8,

    // VBlank suppression: set when $2002 is read on the exact PPU cycle
    // before VBlank would be set. Prevents VBlank from being set this frame.
    pub suppress_vbl: bool,

    // DMA
    pub dma_page: Option<u8>,

    // Mirroring
    pub mirroring: Mirroring,
}

impl Ppu {
    pub fn new(chr_rom: Vec<u8>, mirroring: Mirroring) -> Self {
        Self {
            ctrl: 0,
            mask: 0,
            status: 0,
            oam_addr: 0,
            oam: [0; 256],
            v: 0,
            t: 0,
            fine_x: 0,
            addr_latch: false,
            read_buffer: 0,
            data_bus: 0,
            vram: [0; 2048],
            palette: [0; 32],
            chr_rom,
            scanline: 0,
            dot: 0,
            odd_frame: false,
            nmi_output: false,
            nmi_occurred: false,
            nmi_pending: false,
            nmi_line: false,
            nmi_pending_age: 0,
            nmi_write_delay: 0,
            suppress_vbl: false,
            dma_page: None,
            mirroring,
        }
    }

    // ── Register reads ($2000-$2007) ────────────────────────────────────────

    pub fn read_register(&mut self, addr: u16) -> u8 {
        match addr {
            PPU_2000_CTRL | PPU_2001_MASK | PPU_2003_OAM_ADDR | PPU_2005_SCROLL
            | PPU_2006_ADDR => self.data_bus,

            PPU_2002_STATUS => {
                let effective_status = self.status;
                // Status: bits 7-5 from status, bits 4-0 from data bus latch
                let val = (effective_status & 0xE0) | (self.data_bus & 0x1F);
                // Reading clears VBlank flag
                self.status &= !0x80;
                self.nmi_occurred = false;
                self.update_nmi(false);
                // NMI suppression window: reading $2002 within 1 PPU tick of
                // VBlank set (scanline 241, dots 1-2) pulls /NMI high too
                // quickly after it went low — the CPU never latches the edge.
                if self.scanline == 241 && self.dot >= 1 && self.dot <= 2 {
                    self.nmi_pending = false;
                }
                // Reset address latch
                self.addr_latch = false;
                self.data_bus = val;
                val
            }

            PPU_2004_OAM_DATA => {
                // OAM data read
                let val = self.oam[self.oam_addr as usize];
                self.data_bus = val;
                val
            }

            PPU_2007_DATA => {
                // VRAM data read (buffered, except palette)
                let addr = self.v & 0x3FFF;
                let val = if addr >= 0x3F00 {
                    // Palette read bypasses buffer, but still updates it
                    // with the nametable byte "underneath"
                    self.read_buffer = self.vram_read(addr & 0x2FFF);
                    let palette_val = self.palette_read(addr);
                    self.data_bus = palette_val;
                    palette_val
                } else {
                    let buffered = self.read_buffer;
                    self.read_buffer = self.vram_read(addr);
                    self.data_bus = buffered;
                    buffered
                };
                // Increment v by 1 or 32 based on PPUCTRL bit 2
                self.v = self.v.wrapping_add(self.vram_increment()) & 0x7FFF;
                val
            }

            _ => self.data_bus,
        }
    }

    // ── Register writes ($2000-$2007, $4014) ────────────────────────────────

    pub fn write_register(&mut self, addr: u16, val: u8) {
        self.data_bus = val;

        match addr {
            PPU_2000_CTRL => {
                // PPUCTRL
                self.ctrl = val;
                // t: ...GH.. ........ = val: ......GH (nametable select)
                self.t = (self.t & 0xF3FF) | ((val as u16 & 0x03) << 10);
                let was_output = self.nmi_output;
                self.nmi_output = val & 0x80 != 0;
                self.update_nmi(false);
                // Age-based NMI cancellation: if NMI was just disabled and
                // VBlank set the NMI within the last few PPU ticks, the CPU
                // hasn't polled it yet (penultimate cycle) so we can cancel.
                if was_output && !self.nmi_output
                    && self.nmi_pending
                    && self.nmi_pending_age < 3
                {
                    self.nmi_pending = false;
                }
                // Register-write-triggered NMI: delay by 1 instruction.
                // On real hardware, the write happens on the last CPU cycle,
                // so NMI isn't sampled until the next instruction's penultimate
                // cycle. The delay is protected from falling-edge cancellation
                // (e.g. VBlank clearing during the delay instruction).
                if !was_output && self.nmi_output && self.nmi_pending {
                    self.nmi_write_delay = 2;
                }
            }

            PPU_2001_MASK => {
                // PPUMASK
                self.mask = val;
            }

            PPU_2003_OAM_ADDR => {
                // OAMADDR
                self.oam_addr = val;
            }

            PPU_2004_OAM_DATA => {
                // OAMDATA write
                self.oam[self.oam_addr as usize] = val;
                self.oam_addr = self.oam_addr.wrapping_add(1);
            }

            PPU_2005_SCROLL => {
                // PPUSCROLL (two writes)
                if !self.addr_latch {
                    // First write: X scroll
                    self.t = (self.t & 0xFFE0) | ((val as u16) >> 3);
                    self.fine_x = val & 0x07;
                } else {
                    // Second write: Y scroll
                    self.t = (self.t & 0x8C1F)
                        | ((val as u16 & 0x07) << 12)
                        | ((val as u16 & 0xF8) << 2);
                }
                self.addr_latch = !self.addr_latch;
            }

            PPU_2006_ADDR => {
                // PPUADDR (two writes)
                if !self.addr_latch {
                    // First write: high byte
                    self.t = (self.t & 0x00FF) | ((val as u16 & 0x3F) << 8);
                } else {
                    // Second write: low byte, then t -> v
                    self.t = (self.t & 0xFF00) | val as u16;
                    self.v = self.t;
                }
                self.addr_latch = !self.addr_latch;
            }

            PPU_2007_DATA => {
                // VRAM data write
                let a = self.v & 0x3FFF;
                if a >= 0x3F00 {
                    self.palette_write(a, val);
                } else {
                    self.vram_write(a, val);
                }
                self.v = self.v.wrapping_add(self.vram_increment()) & 0x7FFF;
            }

            PPU_4014_OAM_DMA => {
                // OAM DMA — handled by System, just record the page
                self.dma_page = Some(val);
            }

            _ => {}
        }
    }

    // ── PPU tick (one dot) ──────────────────────────────────────────────────

    pub fn tick(&mut self) {
        // Advance dot/scanline first, then process events at the new position.
        // This matches hardware behavior where events at dot N happen when
        // the PPU clock reaches that dot.
        self.dot += 1;

        // Track NMI pending age BEFORE events, so the VBlank-setting tick
        // ends with age=0 (set in update_nmi). This gives age = K-1 after
        // K ticks post-VBlank, matching the CPU's penultimate-cycle poll.
        if self.nmi_pending {
            self.nmi_pending_age = self.nmi_pending_age.saturating_add(1);
        }

        // Odd-frame skip: at dot 339 of the pre-render scanline, skip one
        // cycle by advancing dot to 340. The next tick increments to 341,
        // triggering the normal scanline wrap. On real hardware, the BG
        // fetch circuit evaluates rendering state at cycle 339 (not 340).
        // Reference: Mesen PPU.cpp checks IsRenderingEnabled() at _cycle==339.
        if self.scanline == 261
            && self.dot == 339
            && self.odd_frame
            && self.rendering_enabled()
        {
            self.dot = 340;
            // Fall through — next tick increments to 341, triggering wrap
        }

        if self.dot > 340 {
            self.dot = 0;
            self.scanline += 1;
            if self.scanline > 261 {
                self.scanline = 0;
                self.odd_frame = !self.odd_frame;
            }
        }

        // Pre-render scanline (261): clear flags at dot 1
        if self.scanline == 261 && self.dot == 1 {
            self.status &= !0xE0; // clear VBlank, sprite 0, overflow
            self.nmi_occurred = false;
            self.update_nmi(true);
        }

        // Scanline 241 (start of VBlank): set flag at dot 1
        if self.scanline == 241 && self.dot == 1 {
            if self.suppress_vbl {
                // VBlank suppressed: $2002 was read on the exact cycle before
                // VBlank would be set. On real hardware this race condition
                // prevents the flag from ever being set this frame.
                self.suppress_vbl = false;
            } else {
                self.status |= 0x80;
                self.nmi_occurred = true;
                self.update_nmi(true);
            }
        }
    }

    // ── NMI edge detection ──────────────────────────────────────────────────

    /// NMI edge detection. `from_tick` distinguishes PPU-tick-generated
    /// edges (which can cancel pending NMI at pre-render clear) from
    /// register-write-generated edges (which must NOT cancel an already-
    /// committed NMI — the CPU has already latched the /NMI falling edge).
    fn update_nmi(&mut self, from_tick: bool) {
        let active = self.nmi_output && self.nmi_occurred;
        if active && !self.nmi_line {
            // Rising edge: NMI line just went active
            self.nmi_pending = true;
            self.nmi_pending_age = 0;
        } else if !active && self.nmi_line {
            // Falling edge: only tick-generated edges can cancel pending NMI.
            // Register-triggered edges ($2002 read, $2000 write) don't cancel
            // here — $2002 uses dot-based suppression, $2000 uses age-based
            // cancellation directly in write_register.
            if from_tick && self.nmi_write_delay == 0 {
                self.nmi_pending = false;
            }
        }
        self.nmi_line = active;
    }

    pub fn take_nmi(&mut self) -> bool {
        if self.nmi_write_delay > 0 {
            self.nmi_write_delay -= 1;
            if self.nmi_write_delay == 0 && self.nmi_pending {
                self.nmi_pending = false;
                return true;
            }
            return false;
        }
        // Only deliver NMI if it was set long enough ago that the CPU's
        // penultimate-cycle poll would have seen it. If set within the
        // last 3 PPU ticks (= final CPU cycle), defer to next instruction.
        if self.nmi_pending && self.nmi_pending_age >= 3 {
            self.nmi_pending = false;
            return true;
        }
        false
    }

    // ── Helpers ─────────────────────────────────────────────────────────────

    fn vram_increment(&self) -> u16 {
        if self.ctrl & 0x04 != 0 {
            32
        } else {
            1
        }
    }

    fn rendering_enabled(&self) -> bool {
        self.mask & 0x18 != 0 // show background or sprites
    }

    // ── VRAM address mapping ────────────────────────────────────────────────

    fn mirror_nametable_addr(&self, addr: u16) -> usize {
        let addr = (addr - 0x2000) & 0x0FFF; // strip to 0x000-0xFFF
        let table = addr / 0x400; // 0-3
        let offset = addr % 0x400;

        let mapped = match self.mirroring {
            Mirroring::Horizontal => {
                // Tables 0,1 → physical 0; tables 2,3 → physical 1
                let physical = if table < 2 { 0 } else { 1 };
                physical * 0x400 + offset as usize
            }
            Mirroring::Vertical => {
                // Tables 0,2 → physical 0; tables 1,3 → physical 1
                let physical = table & 1;
                (physical as usize) * 0x400 + offset as usize
            }
            Mirroring::FourScreen => addr as usize,
        };
        mapped & 0x7FF // clamp to 2KB VRAM
    }

    fn vram_read(&self, addr: u16) -> u8 {
        let addr = addr & 0x3FFF;
        match addr {
            0x0000..=0x1FFF => {
                if (addr as usize) < self.chr_rom.len() {
                    self.chr_rom[addr as usize]
                } else {
                    0
                }
            }
            0x2000..=0x3EFF => {
                let idx = self.mirror_nametable_addr(addr);
                self.vram[idx]
            }
            0x3F00..=0x3FFF => self.palette_read(addr),
            _ => 0,
        }
    }

    fn vram_write(&mut self, addr: u16, val: u8) {
        let addr = addr & 0x3FFF;
        match addr {
            0x0000..=0x1FFF => {
                // CHR-RAM case (if no CHR-ROM)
                if (addr as usize) < self.chr_rom.len() {
                    self.chr_rom[addr as usize] = val;
                }
            }
            0x2000..=0x3EFF => {
                let idx = self.mirror_nametable_addr(addr);
                self.vram[idx] = val;
            }
            0x3F00..=0x3FFF => self.palette_write(addr, val),
            _ => {}
        }
    }

    fn palette_read(&self, addr: u16) -> u8 {
        self.palette[Self::palette_index(addr)]
    }

    fn palette_write(&mut self, addr: u16, val: u8) {
        self.palette[Self::palette_index(addr)] = val;
    }

    fn palette_index(addr: u16) -> usize {
        let idx = (addr & 0x1F) as usize;
        // Mirrors: $3F10/$3F14/$3F18/$3F1C → $3F00/$3F04/$3F08/$3F0C
        match idx {
            0x10 | 0x14 | 0x18 | 0x1C => idx - 0x10,
            _ => idx,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_ppu() -> Ppu {
        Ppu::new(vec![0; 8192], Mirroring::Horizontal)
    }

    #[test]
    fn test_status_read_clears_vblank() {
        let mut ppu = make_ppu();
        ppu.status = 0x80;
        ppu.nmi_occurred = true;
        let val = ppu.read_register(PPU_2002_STATUS);
        assert_eq!(val & 0x80, 0x80);
        assert_eq!(ppu.status & 0x80, 0);
        assert!(!ppu.nmi_occurred);
    }

    #[test]
    fn test_status_read_resets_latch() {
        let mut ppu = make_ppu();
        ppu.addr_latch = true;
        ppu.read_register(PPU_2002_STATUS);
        assert!(!ppu.addr_latch);
    }

    #[test]
    fn test_ppuaddr_two_writes() {
        let mut ppu = make_ppu();
        ppu.write_register(PPU_2006_ADDR, 0x21); // high byte
        ppu.write_register(PPU_2006_ADDR, 0x08); // low byte → v = $2108
        assert_eq!(ppu.v, 0x2108);
    }

    #[test]
    fn test_vram_write_and_read() {
        let mut ppu = make_ppu();
        ppu.write_register(PPU_2006_ADDR, 0x20);
        ppu.write_register(PPU_2006_ADDR, 0x00);
        ppu.write_register(PPU_2007_DATA, 0x42); // write to $2000

        // Read back: first read returns buffer, second returns value
        ppu.write_register(PPU_2006_ADDR, 0x20);
        ppu.write_register(PPU_2006_ADDR, 0x00);
        ppu.read_register(PPU_2007_DATA); // primes buffer
        let val = ppu.read_register(PPU_2007_DATA);
        assert_eq!(val, 0x42);
    }

    #[test]
    fn test_palette_mirror() {
        let mut ppu = make_ppu();
        // Write to $3F10 should mirror to $3F00
        ppu.write_register(PPU_2006_ADDR, 0x3F);
        ppu.write_register(PPU_2006_ADDR, 0x10);
        ppu.write_register(PPU_2007_DATA, 0x2A);
        assert_eq!(ppu.palette[0x00], 0x2A);
    }

    #[test]
    fn test_vblank_timing() {
        let mut ppu = make_ppu();
        ppu.scanline = 241;
        ppu.dot = 0;
        ppu.nmi_output = true;
        ppu.tick(); // dot 0 → dot 1: VBlank sets
        assert!(ppu.nmi_occurred);
        assert!(ppu.status & 0x80 != 0);
        assert!(ppu.nmi_pending);
    }

    #[test]
    fn test_prerender_clears_flags() {
        let mut ppu = make_ppu();
        ppu.status = 0xE0; // VBlank + sprite0 + overflow
        ppu.nmi_occurred = true;
        ppu.scanline = 261;
        ppu.dot = 0;
        ppu.tick(); // dot 1: clear flags
        assert_eq!(ppu.status & 0xE0, 0);
        assert!(!ppu.nmi_occurred);
    }

    #[test]
    fn test_nmi_enable_during_vblank() {
        let mut ppu = make_ppu();
        ppu.nmi_occurred = true;
        ppu.status |= 0x80;
        ppu.nmi_output = false;
        ppu.nmi_pending = false;
        // Enable NMI while VBlank flag is set → should trigger
        ppu.write_register(PPU_2000_CTRL, 0x80);
        assert!(ppu.nmi_pending);
    }

    #[test]
    fn test_ctrl_nametable_bits_to_t() {
        let mut ppu = make_ppu();
        ppu.write_register(PPU_2000_CTRL, 0x03); // nametable = 3
        assert_eq!(ppu.t & 0x0C00, 0x0C00);
    }

    #[test]
    fn test_vram_increment_32() {
        let mut ppu = make_ppu();
        ppu.write_register(PPU_2000_CTRL, 0x04); // increment by 32
        ppu.write_register(PPU_2006_ADDR, 0x20);
        ppu.write_register(PPU_2006_ADDR, 0x00);
        ppu.write_register(PPU_2007_DATA, 0x11);
        assert_eq!(ppu.v, 0x2020); // $2000 + 32
    }

    #[test]
    fn test_horizontal_mirroring() {
        let mut ppu = make_ppu();
        // Write to nametable 0 ($2000)
        ppu.write_register(PPU_2006_ADDR, 0x20);
        ppu.write_register(PPU_2006_ADDR, 0x05);
        ppu.write_register(PPU_2007_DATA, 0xAB);
        // Nametable 1 ($2400) should mirror nametable 0 in horizontal
        let idx0 = ppu.mirror_nametable_addr(0x2005);
        let idx1 = ppu.mirror_nametable_addr(0x2405);
        assert_eq!(idx0, idx1);
    }

    #[test]
    fn test_vertical_mirroring() {
        let ppu = Ppu::new(vec![0; 8192], Mirroring::Vertical);
        // Nametable 0 ($2000) and nametable 2 ($2800) should mirror
        let idx0 = ppu.mirror_nametable_addr(0x2005);
        let idx2 = ppu.mirror_nametable_addr(0x2805);
        assert_eq!(idx0, idx2);
        // Nametable 0 and 1 should be different
        let idx1 = ppu.mirror_nametable_addr(0x2405);
        assert_ne!(idx0, idx1);
    }
}
