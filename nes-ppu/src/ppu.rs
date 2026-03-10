use std::cell::RefCell;
use std::rc::Rc;

use nes_cpu::ines::Mirroring;
use nes_cpu::mapper::Mapper;

/// PPU memory-mapped registers
///
/// Name      Address Bits                Type  Notes
/// PPUCTRL   $2000   VPHB SINN           W     NMI enable (V), PPU master/slave (P), sprite height (H), background tile select (B), sprite tile select (S), increment mode (I), nametable select / X and Y scroll bit 8 (NN)
/// PPUMASK   $2001   BGRs bMmG           W     color emphasis (BGR), sprite enable (s), background enable (b), sprite left column enable (M), background left column enable (m), greyscale (G)
/// PPUSTATUS $2002   VSO- ----           R     vblank (V), sprite 0 hit (S), sprite overflow (O); read resets write pair for $2005/$2006
/// OAMADDR   $2003   AAAA AAAA           W     OAM read/write address
/// OAMDATA   $2004   DDDD DDDD           RW    OAM data read/write
/// PPUSCROLL $2005   XXXX XXXX YYYY YYYY Wx2   X and Y scroll bits 7-0 (two writes: X scroll, then Y scroll)
/// PPUADDR   $2006   ..AA AAAA AAAA AAAA Wx2   VRAM address (two writes: most significant byte, then least significant byte)
/// PPUDATA   $2007   DDDD DDDD           RW    VRAM data read/write
/// OAMDMA    $4014   AAAA AAAA           W     OAM DMA high address
pub mod registers {
    pub const CTRL: u16 = 0x2000;
    pub const MASK: u16 = 0x2001;
    pub const STATUS: u16 = 0x2002;
    pub const OAM_ADDR: u16 = 0x2003;
    pub const OAM_DATA: u16 = 0x2004;
    pub const SCROLL: u16 = 0x2005;
    pub const ADDR: u16 = 0x2006;
    pub const DATA: u16 = 0x2007;
    pub const OAM_DMA: u16 = 0x4014;
}

/// PPUCTRL ($2000) bit flags.
///
/// 7  bit  0
/// ---- ----
/// VPHB SINN
/// |||| ||||
/// |||| ||++- Base nametable address
/// |||| ||    (0 = $2000; 1 = $2400; 2 = $2800; 3 = $2C00)
/// |||| |+--- VRAM address increment per CPU read/write of PPUDATA
/// |||| |     (0: add 1, going across; 1: add 32, going down)
/// |||| +---- Sprite pattern table address for 8x8 sprites
/// ||||       (0: $0000; 1: $1000; ignored in 8x16 mode)
/// |||+------ Background pattern table address (0: $0000; 1: $1000)
/// ||+------- Sprite size (0: 8x8 pixels; 1: 8x16 pixels – see PPU OAM#Byte 1)
/// |+-------- PPU master/slave select
/// |          (0: read backdrop from EXT pins; 1: output color on EXT pins)
/// +--------- Vblank NMI enable (0: off, 1: on)
pub mod ctrl {
    pub const NAMETABLE_SELECT: u8 = 0b0000_0011;
    pub const VRAM_INCREMENT: u8 = 0b0000_0100;
    pub const SPRITE_PATTERN: u8 = 0b0000_1000;
    pub const BG_PATTERN: u8 = 0b0001_0000;
    pub const SPRITE_SIZE: u8 = 0b0010_0000;
    pub const NMI_ENABLE: u8 = 0b1000_0000;
}

/// PPUMASK ($2001) bit flags.
///
/// 7  bit  0
/// ---- ----
/// BGRs bMmG
/// |||| ||||
/// |||| |||+- Greyscale (0: normal color, 1: greyscale)
/// |||| ||+-- 1: Show background in leftmost 8 pixels of screen, 0: Hide
/// |||| |+--- 1: Show sprites in leftmost 8 pixels of screen, 0: Hide
/// |||| +---- 1: Enable background rendering
/// |||+------ 1: Enable sprite rendering
/// ||+------- Emphasize red (green on PAL/Dendy)
/// |+-------- Emphasize green (red on PAL/Dendy)
/// +--------- Emphasize blue
pub mod mask {
    pub const SHOW_BG_LEFT: u8 = 0b0000_0010;
    pub const SHOW_SPR_LEFT: u8 = 0b0000_0100;
    pub const SHOW_BG: u8 = 0b0000_1000;
    pub const SHOW_SPRITES: u8 = 0b0001_0000;
    pub const RENDERING: u8 = SHOW_BG | SHOW_SPRITES;
}

/// PPUSTATUS ($2002) bit flags.
///
/// 7  bit  0
/// ---- ----
/// VSOx xxxx
/// |||| ||||
/// |||+-++++- (PPU open bus or 2C05 PPU identifier)
/// ||+------- Sprite overflow flag
/// |+-------- Sprite 0 hit flag
/// +--------- Vblank flag, cleared on read. Unreliable; see below.
pub mod status {
    pub const SPRITE_OVERFLOW: u8 = 0b0010_0000;
    pub const SPRITE0_HIT: u8 = 0b0100_0000;
    pub const VBLANK: u8 = 0b1000_0000;
    pub const FLAGS: u8 = SPRITE_OVERFLOW | SPRITE0_HIT | VBLANK;
}

/// PPU timing constants.
pub mod timing {
    /// Scanline where VBlank begins (dot 1 sets the flag).
    pub const VBLANK_LINE: i16 = 241;
    /// Pre-render scanline (clears flags at dot 1, odd-frame skip at dot 339).
    pub const PRERENDER_LINE: i16 = 261;
    /// Last dot index on a scanline (0-indexed).
    pub const LAST_DOT: u16 = 340;
    /// Dot on the pre-render line where the odd-frame skip decision is made.
    pub const ODD_SKIP_DOT: u16 = 339;
    /// Dot where per-scanline events fire (flag set/clear).
    pub const EVENT_DOT: u16 = 1;
}

/// Canonical NES 2C02 system palette: 64 colors as 0x00RRGGBB.
#[rustfmt::skip]
pub const NES_PALETTE: [u32; 64] = [
    0x00666666, 0x00002A88, 0x001412A7, 0x003B00A4,
    0x005C007E, 0x006E0040, 0x006C0600, 0x00561D00,
    0x00333500, 0x000B4800, 0x00005200, 0x00004F08,
    0x0000404D, 0x00000000, 0x00000000, 0x00000000,
    0x00ADADAD, 0x00155FD9, 0x004240FF, 0x007527FE,
    0x00A01ACC, 0x00B71E7B, 0x00B53120, 0x00994E00,
    0x006B6D00, 0x00388700, 0x000C9300, 0x00008F32,
    0x00007C8D, 0x00000000, 0x00000000, 0x00000000,
    0x00FFFEFF, 0x0064B0FF, 0x009290FF, 0x00C676FF,
    0x00F36AFF, 0x00FE6ECC, 0x00FE8170, 0x00EA9E22,
    0x00BCBE00, 0x0088D800, 0x005CE430, 0x0045E082,
    0x0048CDDE, 0x004F4F4F, 0x00000000, 0x00000000,
    0x00FFFEFF, 0x00C0DFFF, 0x00D3D2FF, 0x00E8C8FF,
    0x00FBC2FF, 0x00FEC4EA, 0x00FECCC5, 0x00F7D8A5,
    0x00E4E594, 0x00CFEF96, 0x00BDF4AB, 0x00B3F3CC,
    0x00B5EBF2, 0x00B8B8B8, 0x00000000, 0x00000000,
];

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
    pub mapper: Rc<RefCell<Box<dyn Mapper>>>,

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

    // Mirroring (cached from mapper for nametable address calculation)

    // Rendering pipeline
    pub framebuffer: Box<[u32; 256 * 240]>,
    pub frame_ready: bool,

    // Background shift registers
    bg_pattern_lo: u16,
    bg_pattern_hi: u16,
    bg_attr_lo: u16,
    bg_attr_hi: u16,

    // Background tile fetch latches
    bg_next_tile_id: u8,
    bg_next_attr: u8,
    bg_next_pattern_lo: u8,
    bg_next_pattern_hi: u8,

    // Sprite evaluation state (for current scanline)
    sprite_scanline: [(u8, u8, u8, u8); 8], // (y, tile, attr, x)
    sprite_count: u8,
    sprite_pattern_lo: [u8; 8],
    sprite_pattern_hi: [u8; 8],
    sprite_zero_on_line: bool,
}

impl Ppu {
    pub fn new(mapper: Rc<RefCell<Box<dyn Mapper>>>) -> Self {
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
            mapper,
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
            framebuffer: Box::new([0u32; 256 * 240]),
            frame_ready: false,
            bg_pattern_lo: 0,
            bg_pattern_hi: 0,
            bg_attr_lo: 0,
            bg_attr_hi: 0,
            bg_next_tile_id: 0,
            bg_next_attr: 0,
            bg_next_pattern_lo: 0,
            bg_next_pattern_hi: 0,
            sprite_scanline: [(0, 0, 0, 0); 8],
            sprite_count: 0,
            sprite_pattern_lo: [0; 8],
            sprite_pattern_hi: [0; 8],
            sprite_zero_on_line: false,
        }
    }

    // ── Register reads ($2000-$2007) ────────────────────────────────────────

    pub fn read_register(&mut self, addr: u16) -> u8 {
        match addr {
            registers::CTRL
            | registers::MASK
            | registers::OAM_ADDR
            | registers::SCROLL
            | registers::ADDR => self.data_bus,

            registers::STATUS => {
                let effective_status = self.status;
                // Status: bits 7-5 from status, bits 4-0 from data bus latch
                let val = (effective_status & status::FLAGS) | (self.data_bus & !status::FLAGS);
                // Reading clears VBlank flag
                self.status &= !status::VBLANK;
                self.nmi_occurred = false;
                self.update_nmi(false);
                // NMI suppression window: reading $2002 within 1 PPU tick of
                // VBlank set (scanline 241, dots 1-2) pulls /NMI high too
                // quickly after it went low — the CPU never latches the edge.
                if self.scanline == timing::VBLANK_LINE
                    && self.dot >= timing::EVENT_DOT
                    && self.dot <= timing::EVENT_DOT + 1
                {
                    self.nmi_pending = false;
                }
                // Reset address latch
                self.addr_latch = false;
                self.data_bus = val;
                val
            }

            registers::OAM_DATA => {
                // OAM data read
                let val = self.oam[self.oam_addr as usize];
                self.data_bus = val;
                val
            }

            registers::DATA => {
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
            registers::CTRL => {
                // PPUCTRL
                self.ctrl = val;
                // t: ...GH.. ........ = val: ......GH (nametable select)
                self.t = (self.t & 0xF3FF) | ((val as u16 & ctrl::NAMETABLE_SELECT as u16) << 10);
                let was_output = self.nmi_output;
                self.nmi_output = val & ctrl::NMI_ENABLE != 0;
                self.update_nmi(false);
                // Age-based NMI cancellation: if NMI was just disabled and
                // VBlank set the NMI within the last few PPU ticks, the CPU
                // hasn't polled it yet (penultimate cycle) so we can cancel.
                if was_output && !self.nmi_output && self.nmi_pending && self.nmi_pending_age < 3 {
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

            registers::MASK => {
                // PPUMASK
                self.mask = val;
            }

            registers::OAM_ADDR => {
                // OAMADDR
                self.oam_addr = val;
            }

            registers::OAM_DATA => {
                // OAMDATA write
                self.oam[self.oam_addr as usize] = val;
                self.oam_addr = self.oam_addr.wrapping_add(1);
            }

            registers::SCROLL => {
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

            registers::ADDR => {
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

            registers::DATA => {
                // VRAM data write
                let a = self.v & 0x3FFF;
                if a >= 0x3F00 {
                    self.palette_write(a, val);
                } else {
                    self.vram_write(a, val);
                }
                self.v = self.v.wrapping_add(self.vram_increment()) & 0x7FFF;
            }

            registers::OAM_DMA => {
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
        if self.scanline == timing::PRERENDER_LINE
            && self.dot == timing::ODD_SKIP_DOT
            && self.odd_frame
            && self.rendering_enabled()
        {
            self.dot = timing::LAST_DOT;
            // Fall through — next tick increments to 341, triggering wrap
        }

        if self.dot > timing::LAST_DOT {
            self.dot = 0;
            self.scanline += 1;
            if self.scanline > timing::PRERENDER_LINE {
                self.scanline = 0;
                self.odd_frame = !self.odd_frame;
            }
        }

        // ── Rendering ────────────────────────────────────────────────────
        let visible = self.scanline >= 0 && self.scanline <= 239;
        let prerender = self.scanline == timing::PRERENDER_LINE;

        if (visible || prerender) && self.rendering_enabled() {
            // Pixel output (dots 1-256 on visible scanlines only)
            if visible && self.dot >= 1 && self.dot <= 256 {
                self.render_pixel();
            }

            // Background tile fetch (dots 1-256 and 321-336)
            if (self.dot >= 1 && self.dot <= 256) || (self.dot >= 321 && self.dot <= 336) {
                self.update_bg_shifters();
                match self.dot % 8 {
                    1 => self.fetch_nametable_byte(),
                    3 => self.fetch_attribute_byte(),
                    5 => self.fetch_pattern_lo(),
                    7 => self.fetch_pattern_hi(),
                    0 => self.load_bg_shifters(),
                    _ => {}
                }
            }

            // Scroll register updates
            if self.dot == 256 {
                self.increment_scroll_y();
            }
            if self.dot == 257 {
                self.copy_horizontal_bits();
            }
            if prerender && self.dot >= 280 && self.dot <= 304 {
                self.copy_vertical_bits();
            }

            // Sprite evaluation at dot 257 of visible scanlines
            if self.dot == 257 && visible {
                self.evaluate_sprites();
            }

            // Clock mapper scanline counter at dot 260 (after BG fetches,
            // during sprite fetches — the point where A12 transitions on
            // real hardware for MMC3 and similar mappers).
            if self.dot == 260 {
                if let Some(irq) = self.mapper.borrow_mut().as_irq() {
                    irq.clock_scanline();
                }
            }
        }

        // ── Timing events ───────────────────────────────────────────────

        // Pre-render scanline (261): clear flags at dot 1
        if self.scanline == timing::PRERENDER_LINE && self.dot == timing::EVENT_DOT {
            self.status &= !status::FLAGS; // clear VBlank, sprite 0, overflow
            self.nmi_occurred = false;
            self.update_nmi(true);
        }

        // Scanline 241 (start of VBlank): set flag at dot 1
        if self.scanline == timing::VBLANK_LINE && self.dot == timing::EVENT_DOT {
            self.frame_ready = true;
            if self.suppress_vbl {
                // VBlank suppressed: $2002 was read on the exact cycle before
                // VBlank would be set. On real hardware this race condition
                // prevents the flag from ever being set this frame.
                self.suppress_vbl = false;
            } else {
                self.status |= status::VBLANK;
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

    // ── Rendering pipeline ─────────────────────────────────────────────────

    fn render_pixel(&mut self) {
        let x = (self.dot - 1) as usize;
        let y = self.scanline as usize;

        // Background pixel
        let (bg_pixel, bg_palette) =
            if self.mask & mask::SHOW_BG != 0 && (x >= 8 || self.mask & mask::SHOW_BG_LEFT != 0) {
                let bit_select = 15 - self.fine_x as u16;
                let lo = (self.bg_pattern_lo >> bit_select) & 1;
                let hi = (self.bg_pattern_hi >> bit_select) & 1;
                let pixel = ((hi << 1) | lo) as u8;
                let attr_lo = (self.bg_attr_lo >> bit_select) & 1;
                let attr_hi = (self.bg_attr_hi >> bit_select) & 1;
                let palette = ((attr_hi << 1) | attr_lo) as u8;
                (pixel, palette)
            } else {
                (0, 0)
            };

        // Sprite pixel
        let mut spr_pixel = 0u8;
        let mut spr_palette = 0u8;
        let mut spr_priority = false;
        let mut spr_zero = false;
        if self.mask & mask::SHOW_SPRITES != 0 && (x >= 8 || self.mask & mask::SHOW_SPR_LEFT != 0) {
            for i in 0..self.sprite_count as usize {
                let sx = self.sprite_scanline[i].3 as usize;
                if x < sx || x >= sx + 8 {
                    continue;
                }
                let col = (x - sx) as u8;
                let lo = (self.sprite_pattern_lo[i] >> (7 - col)) & 1;
                let hi = (self.sprite_pattern_hi[i] >> (7 - col)) & 1;
                let pixel = (hi << 1) | lo;
                if pixel == 0 {
                    continue;
                }
                spr_pixel = pixel;
                spr_palette = (self.sprite_scanline[i].2 & 0x03) + 4;
                spr_priority = self.sprite_scanline[i].2 & 0x20 != 0;
                spr_zero = i == 0 && self.sprite_zero_on_line;
                break;
            }
        }

        // Priority multiplexer
        let (final_pixel, final_palette) = match (bg_pixel, spr_pixel) {
            (0, 0) => (0u8, 0u8),
            (0, _) => (spr_pixel, spr_palette),
            (_, 0) => (bg_pixel, bg_palette),
            (_, _) => {
                if spr_zero && x != 255 {
                    self.status |= status::SPRITE0_HIT;
                }
                if spr_priority {
                    (bg_pixel, bg_palette)
                } else {
                    (spr_pixel, spr_palette)
                }
            }
        };

        let color_idx = if final_pixel == 0 {
            self.palette[0] as usize & 0x3F
        } else {
            self.palette[(final_palette as usize * 4 + final_pixel as usize) & 0x1F] as usize & 0x3F
        };

        self.framebuffer[y * 256 + x] = NES_PALETTE[color_idx];
    }

    // ── Background tile fetch ────────────────────────────────────────────

    fn update_bg_shifters(&mut self) {
        if self.mask & mask::SHOW_BG != 0 {
            self.bg_pattern_lo <<= 1;
            self.bg_pattern_hi <<= 1;
            self.bg_attr_lo <<= 1;
            self.bg_attr_hi <<= 1;
        }
    }

    fn load_bg_shifters(&mut self) {
        self.bg_pattern_lo = (self.bg_pattern_lo & 0xFF00) | self.bg_next_pattern_lo as u16;
        self.bg_pattern_hi = (self.bg_pattern_hi & 0xFF00) | self.bg_next_pattern_hi as u16;
        let attr_lo_fill: u16 = if self.bg_next_attr & 0x01 != 0 {
            0xFF
        } else {
            0x00
        };
        let attr_hi_fill: u16 = if self.bg_next_attr & 0x02 != 0 {
            0xFF
        } else {
            0x00
        };
        self.bg_attr_lo = (self.bg_attr_lo & 0xFF00) | attr_lo_fill;
        self.bg_attr_hi = (self.bg_attr_hi & 0xFF00) | attr_hi_fill;
        self.increment_coarse_x();
    }

    fn fetch_nametable_byte(&mut self) {
        let addr = 0x2000 | (self.v & 0x0FFF);
        self.bg_next_tile_id = self.vram_read(addr);
    }

    fn fetch_attribute_byte(&mut self) {
        let addr = 0x23C0 | (self.v & 0x0C00) | ((self.v >> 4) & 0x38) | ((self.v >> 2) & 0x07);
        let attr = self.vram_read(addr);
        let shift = (((self.v >> 4) & 0x04) | (self.v & 0x02)) as u8;
        self.bg_next_attr = (attr >> shift) & 0x03;
    }

    fn fetch_pattern_lo(&mut self) {
        let table: u16 = if self.ctrl & ctrl::BG_PATTERN != 0 {
            0x1000
        } else {
            0
        };
        let fine_y = (self.v >> 12) & 0x07;
        let addr = table + (self.bg_next_tile_id as u16) * 16 + fine_y;
        self.bg_next_pattern_lo = self.vram_read(addr);
    }

    fn fetch_pattern_hi(&mut self) {
        let table: u16 = if self.ctrl & ctrl::BG_PATTERN != 0 {
            0x1000
        } else {
            0
        };
        let fine_y = (self.v >> 12) & 0x07;
        let addr = table + (self.bg_next_tile_id as u16) * 16 + fine_y + 8;
        self.bg_next_pattern_hi = self.vram_read(addr);
    }

    // ── Scroll register helpers (loopy) ──────────────────────────────────

    fn increment_coarse_x(&mut self) {
        if self.v & 0x001F == 31 {
            self.v &= !0x001F;
            self.v ^= 0x0400;
        } else {
            self.v += 1;
        }
    }

    fn increment_scroll_y(&mut self) {
        if self.v & 0x7000 != 0x7000 {
            self.v += 0x1000;
        } else {
            self.v &= !0x7000;
            let mut y = (self.v & 0x03E0) >> 5;
            if y == 29 {
                y = 0;
                self.v ^= 0x0800;
            } else if y == 31 {
                y = 0;
            } else {
                y += 1;
            }
            self.v = (self.v & !0x03E0) | (y << 5);
        }
    }

    fn copy_horizontal_bits(&mut self) {
        self.v = (self.v & !0x041F) | (self.t & 0x041F);
    }

    fn copy_vertical_bits(&mut self) {
        self.v = (self.v & !0x7BE0) | (self.t & 0x7BE0);
    }

    // ── Sprite evaluation ────────────────────────────────────────────────

    fn evaluate_sprites(&mut self) {
        let sprite_height: i16 = if self.ctrl & ctrl::SPRITE_SIZE != 0 {
            16
        } else {
            8
        };
        let scanline = self.scanline;

        self.sprite_count = 0;
        self.sprite_zero_on_line = false;

        for i in 0..64 {
            let y = self.oam[i * 4] as i16;
            let diff = scanline - y;
            if diff < 0 || diff >= sprite_height {
                continue;
            }
            if self.sprite_count >= 8 {
                self.status |= status::SPRITE_OVERFLOW;
                break;
            }

            let tile_idx = self.oam[i * 4 + 1];
            let attr = self.oam[i * 4 + 2];
            let x = self.oam[i * 4 + 3];

            if i == 0 {
                self.sprite_zero_on_line = true;
            }

            let flip_v = attr & 0x80 != 0;
            let mut row = diff as u16;

            let pattern_addr = if sprite_height == 16 {
                let table = (tile_idx as u16 & 1) * 0x1000;
                let tile = tile_idx as u16 & 0xFE;
                if flip_v {
                    row = 15 - row;
                }
                if row >= 8 {
                    table + (tile + 1) * 16 + (row - 8)
                } else {
                    table + tile * 16 + row
                }
            } else {
                let table: u16 = if self.ctrl & ctrl::SPRITE_PATTERN != 0 {
                    0x1000
                } else {
                    0
                };
                if flip_v {
                    row = 7 - row;
                }
                table + (tile_idx as u16) * 16 + row
            };

            let mut lo = self.vram_read(pattern_addr);
            let mut hi = self.vram_read(pattern_addr + 8);

            if attr & 0x40 != 0 {
                lo = lo.reverse_bits();
                hi = hi.reverse_bits();
            }

            let idx = self.sprite_count as usize;
            self.sprite_scanline[idx] = (y as u8, tile_idx, attr, x);
            self.sprite_pattern_lo[idx] = lo;
            self.sprite_pattern_hi[idx] = hi;
            self.sprite_count += 1;
        }
    }

    // ── Helpers ─────────────────────────────────────────────────────────────

    fn vram_increment(&self) -> u16 {
        if self.ctrl & ctrl::VRAM_INCREMENT != 0 {
            32
        } else {
            1
        }
    }

    fn rendering_enabled(&self) -> bool {
        self.mask & mask::RENDERING != 0 // show background or sprites
    }

    // ── VRAM address mapping ────────────────────────────────────────────────

    fn mirror_nametable_addr(&self, addr: u16) -> usize {
        let addr = (addr - 0x2000) & 0x0FFF; // strip to 0x000-0xFFF
        let table = addr / 0x400; // 0-3
        let offset = addr % 0x400;

        let mirroring = self.mapper.borrow().mirroring();
        let mapped = match mirroring {
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
            Mirroring::SingleScreenLower => offset as usize,
            Mirroring::SingleScreenUpper => 0x400 + offset as usize,
        };
        mapped & 0x7FF // clamp to 2KB VRAM
    }

    fn vram_read(&self, addr: u16) -> u8 {
        let addr = addr & 0x3FFF;
        match addr {
            0x0000..=0x1FFF => self.mapper.borrow().read_chr(addr),
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
                self.mapper.borrow_mut().write_chr(addr, val);
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

    fn make_mapper(mirroring: Mirroring) -> Rc<RefCell<Box<dyn Mapper>>> {
        use nes_cpu::ines::INesRom;
        let rom = INesRom {
            prg_rom: vec![0u8; 0x4000],
            chr_rom: vec![0u8; 0x2000],
            mapper: 0,
            mirroring,
            has_battery: false,
        };
        Rc::new(RefCell::new(nes_mapper::from_rom(&rom).unwrap()))
    }

    fn make_ppu() -> Ppu {
        Ppu::new(make_mapper(Mirroring::Horizontal))
    }

    #[test]
    fn test_status_read_clears_vblank() {
        let mut ppu = make_ppu();
        ppu.status = status::VBLANK;
        ppu.nmi_occurred = true;
        let val = ppu.read_register(registers::STATUS);
        assert_eq!(val & status::VBLANK, status::VBLANK);
        assert_eq!(ppu.status & status::VBLANK, 0);
        assert!(!ppu.nmi_occurred);
    }

    #[test]
    fn test_status_read_resets_latch() {
        let mut ppu = make_ppu();
        ppu.addr_latch = true;
        ppu.read_register(registers::STATUS);
        assert!(!ppu.addr_latch);
    }

    #[test]
    fn test_ppuaddr_two_writes() {
        let mut ppu = make_ppu();
        ppu.write_register(registers::ADDR, 0x21); // high byte
        ppu.write_register(registers::ADDR, 0x08); // low byte → v = $2108
        assert_eq!(ppu.v, 0x2108);
    }

    #[test]
    fn test_vram_write_and_read() {
        let mut ppu = make_ppu();
        ppu.write_register(registers::ADDR, 0x20);
        ppu.write_register(registers::ADDR, 0x00);
        ppu.write_register(registers::DATA, 0x42); // write to $2000

        // Read back: first read returns buffer, second returns value
        ppu.write_register(registers::ADDR, 0x20);
        ppu.write_register(registers::ADDR, 0x00);
        ppu.read_register(registers::DATA); // primes buffer
        let val = ppu.read_register(registers::DATA);
        assert_eq!(val, 0x42);
    }

    #[test]
    fn test_palette_mirror() {
        let mut ppu = make_ppu();
        // Write to $3F10 should mirror to $3F00
        ppu.write_register(registers::ADDR, 0x3F);
        ppu.write_register(registers::ADDR, 0x10);
        ppu.write_register(registers::DATA, 0x2A);
        assert_eq!(ppu.palette[0x00], 0x2A);
    }

    #[test]
    fn test_vblank_timing() {
        let mut ppu = make_ppu();
        ppu.scanline = timing::VBLANK_LINE;
        ppu.dot = 0;
        ppu.nmi_output = true;
        ppu.tick(); // dot 0 → dot 1: VBlank sets
        assert!(ppu.nmi_occurred);
        assert!(ppu.status & status::VBLANK != 0);
        assert!(ppu.nmi_pending);
    }

    #[test]
    fn test_prerender_clears_flags() {
        let mut ppu = make_ppu();
        ppu.status = status::FLAGS; // VBlank + sprite0 + overflow
        ppu.nmi_occurred = true;
        ppu.scanline = timing::PRERENDER_LINE;
        ppu.dot = 0;
        ppu.tick(); // dot 1: clear flags
        assert_eq!(ppu.status & status::FLAGS, 0);
        assert!(!ppu.nmi_occurred);
    }

    #[test]
    fn test_nmi_enable_during_vblank() {
        let mut ppu = make_ppu();
        ppu.nmi_occurred = true;
        ppu.status |= status::VBLANK;
        ppu.nmi_output = false;
        ppu.nmi_pending = false;
        // Enable NMI while VBlank flag is set → should trigger
        ppu.write_register(registers::CTRL, ctrl::NMI_ENABLE);
        assert!(ppu.nmi_pending);
    }

    #[test]
    fn test_ctrl_nametable_bits_to_t() {
        let mut ppu = make_ppu();
        ppu.write_register(registers::CTRL, 0x03); // nametable = 3
        assert_eq!(ppu.t & 0x0C00, 0x0C00);
    }

    #[test]
    fn test_vram_increment_32() {
        let mut ppu = make_ppu();
        ppu.write_register(registers::CTRL, ctrl::VRAM_INCREMENT); // increment by 32
        ppu.write_register(registers::ADDR, 0x20);
        ppu.write_register(registers::ADDR, 0x00);
        ppu.write_register(registers::DATA, 0x11);
        assert_eq!(ppu.v, 0x2020); // $2000 + 32
    }

    #[test]
    fn test_horizontal_mirroring() {
        let mut ppu = make_ppu();
        // Write to nametable 0 ($2000)
        ppu.write_register(registers::ADDR, 0x20);
        ppu.write_register(registers::ADDR, 0x05);
        ppu.write_register(registers::DATA, 0xAB);
        // Nametable 1 ($2400) should mirror nametable 0 in horizontal
        let idx0 = ppu.mirror_nametable_addr(0x2005);
        let idx1 = ppu.mirror_nametable_addr(0x2405);
        assert_eq!(idx0, idx1);
    }

    #[test]
    fn test_vertical_mirroring() {
        let ppu = Ppu::new(make_mapper(Mirroring::Vertical));
        // Nametable 0 ($2000) and nametable 2 ($2800) should mirror
        let idx0 = ppu.mirror_nametable_addr(0x2005);
        let idx2 = ppu.mirror_nametable_addr(0x2805);
        assert_eq!(idx0, idx2);
        // Nametable 0 and 1 should be different
        let idx1 = ppu.mirror_nametable_addr(0x2405);
        assert_ne!(idx0, idx1);
    }
}
