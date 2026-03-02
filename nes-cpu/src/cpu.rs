use crate::bus::Bus;
use crate::opcodes::{AddrMode, Instruction, OpKind, get_opcodes, instr_byte_count};

// ── Status flag bits ─────────────────────────────────────────────────────────
pub mod flags {
    pub const C: u8 = 0b0000_0001; // Carry
    pub const Z: u8 = 0b0000_0010; // Zero
    pub const I: u8 = 0b0000_0100; // IRQ Disable
    pub const D: u8 = 0b0000_1000; // Decimal (ignored on 2A03)
    pub const B: u8 = 0b0001_0000; // Break
    pub const U: u8 = 0b0010_0000; // Unused (always 1)
    pub const V: u8 = 0b0100_0000; // Overflow
    pub const N: u8 = 0b1000_0000; // Negative
}

pub struct Cpu {
    pub a: u8,
    pub x: u8,
    pub y: u8,
    pub pc: u16,
    pub sp: u8,
    pub p: u8,
    pub cycles: u64,
    pub bus: Bus,
}

impl Cpu {
    pub fn new(bus: Bus) -> Self {
        Self {
            a: 0,
            x: 0,
            y: 0,
            pc: 0xC000,
            sp: 0xFD,
            p: 0x24, // IRQ disabled | unused bit
            cycles: 7,
            bus,
        }
    }

    // ── Flag helpers ─────────────────────────────────────────────────────────

    pub fn flag_val(&self, flag: u8) -> bool {
        self.p & flag != 0
    }

    fn flag_set(&mut self, flag: u8) {
        self.p |= flag;
    }

    fn flag_clear(&mut self, flag: u8) {
        self.p &= !flag;
    }

    fn set_flag_if(&mut self, flag: u8, cond: bool) {
        if cond {
            self.flag_set(flag);
        } else {
            self.flag_clear(flag);
        }
    }

    fn update_nz(&mut self, val: u8) {
        self.set_flag_if(flags::Z, val == 0);
        self.set_flag_if(flags::N, val & 0x80 != 0);
    }

    // ── Stack ────────────────────────────────────────────────────────────────

    fn push8(&mut self, val: u8) {
        self.bus.write(0x0100 | self.sp as u16, val);
        self.sp = self.sp.wrapping_sub(1);
    }

    fn pull8(&mut self) -> u8 {
        self.sp = self.sp.wrapping_add(1);
        self.bus.read(0x0100 | self.sp as u16)
    }

    fn push16(&mut self, val: u16) {
        self.push8((val >> 8) as u8);
        self.push8(val as u8);
    }

    fn pull16(&mut self) -> u16 {
        let lo = self.pull8() as u16;
        let hi = self.pull8() as u16;
        (hi << 8) | lo
    }

    // ── Interrupt vectors ───────────────────────────────────────────────────

    pub fn nmi(&mut self) {
        self.push16(self.pc);
        // Push P with B clear, U set (hardware interrupt)
        self.push8((self.p & !flags::B) | flags::U);
        self.flag_set(flags::I);
        let lo = self.bus.read(0xFFFA) as u16;
        let hi = self.bus.read(0xFFFB) as u16;
        self.pc = (hi << 8) | lo;
        self.cycles += 7;
    }

    pub fn reset(&mut self) {
        let lo = self.bus.read(0xFFFC) as u16;
        let hi = self.bus.read(0xFFFD) as u16;
        self.pc = (hi << 8) | lo;
        self.sp = self.sp.wrapping_sub(3);
        self.flag_set(flags::I);
        self.cycles += 7;
    }

    // ── Execute one instruction, return the log line captured before ─────────

    pub fn step(&mut self) -> String {
        let log_line = self.log_state();

        let opcode_byte = self.bus.read(self.pc);
        self.pc = self.pc.wrapping_add(1);

        let instr = get_opcodes()[opcode_byte as usize];
        let (addr, page_crossed) = self.resolve_addr(instr.mode);

        let extra = if instr.page_cross_penalty && page_crossed { 1u64 } else { 0 };
        let cost = instr.cycles as u64 + extra;

        // Add cycle cost BEFORE execute so that PPU catch-up during
        // bus reads sees the correct CPU cycle count.
        self.cycles += cost;

        self.execute(instr, addr);

        log_line
    }

    // ── Addressing mode resolution ────────────────────────────────────────────

    fn resolve_addr(&mut self, mode: AddrMode) -> (u16, bool) {
        match mode {
            AddrMode::Implied | AddrMode::Accumulator => (0, false),

            AddrMode::Immediate => {
                let addr = self.pc;
                self.pc = self.pc.wrapping_add(1);
                (addr, false)
            }

            AddrMode::ZeroPage => {
                let addr = self.bus.read(self.pc) as u16;
                self.pc = self.pc.wrapping_add(1);
                (addr, false)
            }

            AddrMode::ZeroPageX => {
                let base = self.bus.read(self.pc);
                self.pc = self.pc.wrapping_add(1);
                (base.wrapping_add(self.x) as u16, false)
            }

            AddrMode::ZeroPageY => {
                let base = self.bus.read(self.pc);
                self.pc = self.pc.wrapping_add(1);
                (base.wrapping_add(self.y) as u16, false)
            }

            AddrMode::Absolute => {
                let lo = self.bus.read(self.pc) as u16;
                let hi = self.bus.read(self.pc.wrapping_add(1)) as u16;
                self.pc = self.pc.wrapping_add(2);
                ((hi << 8) | lo, false)
            }

            AddrMode::AbsoluteX => {
                let lo = self.bus.read(self.pc) as u16;
                let hi = self.bus.read(self.pc.wrapping_add(1)) as u16;
                self.pc = self.pc.wrapping_add(2);
                let base = (hi << 8) | lo;
                let addr = base.wrapping_add(self.x as u16);
                let page_crossed = (base & 0xFF00) != (addr & 0xFF00);
                (addr, page_crossed)
            }

            AddrMode::AbsoluteY => {
                let lo = self.bus.read(self.pc) as u16;
                let hi = self.bus.read(self.pc.wrapping_add(1)) as u16;
                self.pc = self.pc.wrapping_add(2);
                let base = (hi << 8) | lo;
                let addr = base.wrapping_add(self.y as u16);
                let page_crossed = (base & 0xFF00) != (addr & 0xFF00);
                (addr, page_crossed)
            }

            AddrMode::Indirect => {
                // 6502 page-wrap bug: if low byte of pointer is 0xFF,
                // high byte is read from the SAME page (wraps around).
                let lo = self.bus.read(self.pc) as u16;
                let hi = self.bus.read(self.pc.wrapping_add(1)) as u16;
                self.pc = self.pc.wrapping_add(2);
                let ptr = (hi << 8) | lo;
                let lo2 = self.bus.read(ptr) as u16;
                let hi2 = self.bus.read((ptr & 0xFF00) | ((ptr + 1) & 0x00FF)) as u16;
                ((hi2 << 8) | lo2, false)
            }

            AddrMode::IndirectX => {
                let base = self.bus.read(self.pc);
                self.pc = self.pc.wrapping_add(1);
                let ptr = base.wrapping_add(self.x) as u16;
                let lo = self.bus.read(ptr & 0x00FF) as u16;
                let hi = self.bus.read((ptr.wrapping_add(1)) & 0x00FF) as u16;
                ((hi << 8) | lo, false)
            }

            AddrMode::IndirectY => {
                let ptr = self.bus.read(self.pc) as u16;
                self.pc = self.pc.wrapping_add(1);
                let lo = self.bus.read(ptr & 0x00FF) as u16;
                let hi = self.bus.read((ptr.wrapping_add(1)) & 0x00FF) as u16;
                let base = (hi << 8) | lo;
                let addr = base.wrapping_add(self.y as u16);
                let page_crossed = (base & 0xFF00) != (addr & 0xFF00);
                (addr, page_crossed)
            }

            AddrMode::Relative => {
                let offset = self.bus.read(self.pc);
                self.pc = self.pc.wrapping_add(1);
                (offset as u16, false)
            }
        }
    }

    // ── Operand read helper ───────────────────────────────────────────────────

    fn operand_read(&mut self, mode: AddrMode, addr: u16) -> u8 {
        match mode {
            AddrMode::Accumulator => self.a,
            _ => self.bus.read(addr),
        }
    }

    // ── ADC / SBC core ────────────────────────────────────────────────────────

    fn exec_adc_val(&mut self, val: u8) {
        let carry = self.flag_val(flags::C) as u16;
        let sum = self.a as u16 + val as u16 + carry;
        let result = sum as u8;
        let overflow = (!(self.a ^ val) & (self.a ^ result)) & 0x80 != 0;
        self.set_flag_if(flags::C, sum > 0xFF);
        self.set_flag_if(flags::V, overflow);
        self.a = result;
        self.update_nz(self.a);
    }

    fn exec_sbc_val(&mut self, val: u8) {
        self.exec_adc_val(!val);
    }

    // ── Branch ───────────────────────────────────────────────────────────────

    fn branch(&mut self, condition: bool, offset: u16) {
        if condition {
            let offset_signed = offset as i8 as i32;
            let new_pc = (self.pc as i32 + offset_signed) as u16;
            self.cycles += if (self.pc & 0xFF00) != (new_pc & 0xFF00) { 2 } else { 1 };
            self.pc = new_pc;
        }
    }

    // ── Compare helper ────────────────────────────────────────────────────────

    fn compare(&mut self, reg: u8, val: u8) {
        let diff = reg.wrapping_sub(val);
        self.set_flag_if(flags::C, reg >= val);
        self.update_nz(diff);
    }

    // ── Execute dispatch ──────────────────────────────────────────────────────

    #[allow(clippy::too_many_lines)]
    fn execute(&mut self, instr: Instruction, addr: u16) {
        use OpKind::*;

        match instr.kind {
            // ── Load/Store ───────────────────────────────────────────────────
            Lda => {
                self.a = self.operand_read(instr.mode, addr);
                self.update_nz(self.a);
            }
            Ldx => {
                self.x = self.operand_read(instr.mode, addr);
                self.update_nz(self.x);
            }
            Ldy => {
                self.y = self.operand_read(instr.mode, addr);
                self.update_nz(self.y);
            }
            Sta => self.bus.write(addr, self.a),
            Stx => self.bus.write(addr, self.x),
            Sty => self.bus.write(addr, self.y),

            // ── Register transfers ───────────────────────────────────────────
            Tax => {
                self.x = self.a;
                self.update_nz(self.x);
            }
            Tay => {
                self.y = self.a;
                self.update_nz(self.y);
            }
            Txa => {
                self.a = self.x;
                self.update_nz(self.a);
            }
            Tya => {
                self.a = self.y;
                self.update_nz(self.a);
            }
            Tsx => {
                self.x = self.sp;
                self.update_nz(self.x);
            }
            Txs => self.sp = self.x,

            // ── Stack ────────────────────────────────────────────────────────
            Pha => self.push8(self.a),
            Php => self.push8(self.p | flags::B | flags::U),
            Pla => {
                self.a = self.pull8();
                self.update_nz(self.a);
            }
            Plp => {
                self.p = (self.pull8() & !flags::B) | flags::U;
            }

            // ── Arithmetic ───────────────────────────────────────────────────
            Adc => {
                let val = self.operand_read(instr.mode, addr);
                self.exec_adc_val(val);
            }
            Sbc => {
                let val = self.operand_read(instr.mode, addr);
                self.exec_sbc_val(val);
            }

            // ── Increment / Decrement ─────────────────────────────────────────
            Inc => {
                let val = self.bus.read(addr).wrapping_add(1);
                self.bus.write(addr, val);
                self.update_nz(val);
            }
            Inx => {
                self.x = self.x.wrapping_add(1);
                self.update_nz(self.x);
            }
            Iny => {
                self.y = self.y.wrapping_add(1);
                self.update_nz(self.y);
            }
            Dec => {
                let val = self.bus.read(addr).wrapping_sub(1);
                self.bus.write(addr, val);
                self.update_nz(val);
            }
            Dex => {
                self.x = self.x.wrapping_sub(1);
                self.update_nz(self.x);
            }
            Dey => {
                self.y = self.y.wrapping_sub(1);
                self.update_nz(self.y);
            }

            // ── Logical ──────────────────────────────────────────────────────
            And => {
                self.a &= self.operand_read(instr.mode, addr);
                self.update_nz(self.a);
            }
            Ora => {
                self.a |= self.operand_read(instr.mode, addr);
                self.update_nz(self.a);
            }
            Eor => {
                self.a ^= self.operand_read(instr.mode, addr);
                self.update_nz(self.a);
            }

            // ── Shifts / Rotates ─────────────────────────────────────────────
            Asl => {
                let val = self.operand_read(instr.mode, addr);
                self.set_flag_if(flags::C, val & 0x80 != 0);
                let result = val << 1;
                self.update_nz(result);
                if instr.mode == AddrMode::Accumulator {
                    self.a = result;
                } else {
                    self.bus.write(addr, result);
                }
            }
            Lsr => {
                let val = self.operand_read(instr.mode, addr);
                self.set_flag_if(flags::C, val & 0x01 != 0);
                let result = val >> 1;
                self.update_nz(result);
                if instr.mode == AddrMode::Accumulator {
                    self.a = result;
                } else {
                    self.bus.write(addr, result);
                }
            }
            Rol => {
                let val = self.operand_read(instr.mode, addr);
                let carry_in = self.flag_val(flags::C) as u8;
                self.set_flag_if(flags::C, val & 0x80 != 0);
                let result = (val << 1) | carry_in;
                self.update_nz(result);
                if instr.mode == AddrMode::Accumulator {
                    self.a = result;
                } else {
                    self.bus.write(addr, result);
                }
            }
            Ror => {
                let val = self.operand_read(instr.mode, addr);
                let carry_in = if self.flag_val(flags::C) { 0x80 } else { 0 };
                self.set_flag_if(flags::C, val & 0x01 != 0);
                let result = (val >> 1) | carry_in;
                self.update_nz(result);
                if instr.mode == AddrMode::Accumulator {
                    self.a = result;
                } else {
                    self.bus.write(addr, result);
                }
            }

            // ── Compare ──────────────────────────────────────────────────────
            Cmp => {
                let val = self.operand_read(instr.mode, addr);
                self.compare(self.a, val);
            }
            Cpx => {
                let val = self.operand_read(instr.mode, addr);
                self.compare(self.x, val);
            }
            Cpy => {
                let val = self.operand_read(instr.mode, addr);
                self.compare(self.y, val);
            }

            // ── Bit test ─────────────────────────────────────────────────────
            Bit => {
                let val = self.bus.read(addr);
                self.set_flag_if(flags::Z, self.a & val == 0);
                self.set_flag_if(flags::V, val & 0x40 != 0);
                self.set_flag_if(flags::N, val & 0x80 != 0);
            }

            // ── Branches ─────────────────────────────────────────────────────
            Bcc => self.branch(!self.flag_val(flags::C), addr),
            Bcs => self.branch(self.flag_val(flags::C), addr),
            Beq => self.branch(self.flag_val(flags::Z), addr),
            Bmi => self.branch(self.flag_val(flags::N), addr),
            Bne => self.branch(!self.flag_val(flags::Z), addr),
            Bpl => self.branch(!self.flag_val(flags::N), addr),
            Bvc => self.branch(!self.flag_val(flags::V), addr),
            Bvs => self.branch(self.flag_val(flags::V), addr),

            // ── Jump / Call ──────────────────────────────────────────────────
            Jmp => self.pc = addr,
            Jsr => {
                // Push return address - 1 (the byte before the next instruction)
                self.push16(self.pc.wrapping_sub(1));
                self.pc = addr;
            }
            Rts => {
                self.pc = self.pull16().wrapping_add(1);
            }
            Rti => {
                self.p = (self.pull8() & !flags::B) | flags::U;
                self.pc = self.pull16();
            }

            // ── Flags ────────────────────────────────────────────────────────
            Clc => self.flag_clear(flags::C),
            Cld => self.flag_clear(flags::D),
            Cli => self.flag_clear(flags::I),
            Clv => self.flag_clear(flags::V),
            Sec => self.flag_set(flags::C),
            Sed => self.flag_set(flags::D),
            Sei => self.flag_set(flags::I),

            // ── NOP ──────────────────────────────────────────────────────────
            Nop | UnopNop => {
                // Unofficial NOPs may read memory (for cycle accuracy) but discard
                if instr.mode != AddrMode::Implied && instr.mode != AddrMode::Accumulator {
                    let _ = self.bus.read(addr);
                }
            }

            // ── Unofficial: LAX (LDA + LDX) ──────────────────────────────────
            Lax => {
                let val = self.bus.read(addr);
                self.a = val;
                self.x = val;
                self.update_nz(val);
            }

            // ── Unofficial: SAX (Store A AND X) ──────────────────────────────
            Sax => {
                self.bus.write(addr, self.a & self.x);
            }

            // ── Unofficial: DCP (DEC + CMP) ───────────────────────────────────
            Dcp => {
                let val = self.bus.read(addr).wrapping_sub(1);
                self.bus.write(addr, val);
                self.compare(self.a, val);
            }

            // ── Unofficial: ISC / ISB (INC + SBC) ────────────────────────────
            Isc => {
                let val = self.bus.read(addr).wrapping_add(1);
                self.bus.write(addr, val);
                self.exec_sbc_val(val);
            }

            // ── Unofficial: SLO (ASL + ORA) ──────────────────────────────────
            Slo => {
                let val = self.bus.read(addr);
                self.set_flag_if(flags::C, val & 0x80 != 0);
                let shifted = val << 1;
                self.bus.write(addr, shifted);
                self.a |= shifted;
                self.update_nz(self.a);
            }

            // ── Unofficial: RLA (ROL + AND) ───────────────────────────────────
            Rla => {
                let val = self.bus.read(addr);
                let carry_in = self.flag_val(flags::C) as u8;
                self.set_flag_if(flags::C, val & 0x80 != 0);
                let rotated = (val << 1) | carry_in;
                self.bus.write(addr, rotated);
                self.a &= rotated;
                self.update_nz(self.a);
            }

            // ── Unofficial: SRE (LSR + EOR) ───────────────────────────────────
            Sre => {
                let val = self.bus.read(addr);
                self.set_flag_if(flags::C, val & 0x01 != 0);
                let shifted = val >> 1;
                self.bus.write(addr, shifted);
                self.a ^= shifted;
                self.update_nz(self.a);
            }

            // ── Unofficial: RRA (ROR + ADC) ───────────────────────────────────
            Rra => {
                let val = self.bus.read(addr);
                let carry_in = if self.flag_val(flags::C) { 0x80u8 } else { 0 };
                self.set_flag_if(flags::C, val & 0x01 != 0);
                let rotated = (val >> 1) | carry_in;
                self.bus.write(addr, rotated);
                self.exec_adc_val(rotated);
            }

            // ── Unofficial: ALR (AND + LSR immediate) ────────────────────────
            Alr => {
                self.a &= self.bus.read(addr);
                self.set_flag_if(flags::C, self.a & 0x01 != 0);
                self.a >>= 1;
                self.update_nz(self.a);
            }

            // ── Unofficial: ANC (AND, C = N) ─────────────────────────────────
            Anc => {
                self.a &= self.bus.read(addr);
                self.update_nz(self.a);
                self.set_flag_if(flags::C, self.flag_val(flags::N));
            }

            // ── Unofficial: ARR (AND + ROR) ───────────────────────────────────
            Arr => {
                self.a &= self.bus.read(addr);
                let carry_in = if self.flag_val(flags::C) { 0x80u8 } else { 0 };
                self.set_flag_if(flags::C, self.a & 0x01 != 0);
                self.a = (self.a >> 1) | carry_in;
                self.update_nz(self.a);
                // ARR: overflow = bit 6 XOR bit 5
                let v = ((self.a >> 6) ^ (self.a >> 5)) & 0x01 != 0;
                self.set_flag_if(flags::V, v);
                self.set_flag_if(flags::C, self.a & 0x40 != 0);
            }

            // ── Unofficial: AXS / SBX ((A AND X) - imm -> X) ─────────────────
            Axs => {
                let val = self.bus.read(addr);
                let ax = self.a & self.x;
                self.set_flag_if(flags::C, ax >= val);
                self.x = ax.wrapping_sub(val);
                self.update_nz(self.x);
            }

            // ── Unstable unofficials ──────────────────────────────────────────
            Xaa => {
                // Highly unstable; nestest approximates as: A = X AND imm
                self.a = self.x & self.bus.read(addr);
                self.update_nz(self.a);
            }
            Las => {
                let val = self.bus.read(addr) & self.sp;
                self.a = val;
                self.x = val;
                self.sp = val;
                self.update_nz(val);
            }
            Shy => {
                let hi = (addr >> 8) as u8;
                let result = self.y & hi.wrapping_add(1);
                self.bus.write(addr, result);
            }
            Shx => {
                let hi = (addr >> 8) as u8;
                let result = self.x & hi.wrapping_add(1);
                self.bus.write(addr, result);
            }
            Tas => {
                self.sp = self.a & self.x;
                let hi = (addr >> 8) as u8;
                self.bus.write(addr, self.sp & hi.wrapping_add(1));
            }
            Ahx => {
                let hi = (addr >> 8) as u8;
                let result = self.a & self.x & hi.wrapping_add(1);
                self.bus.write(addr, result);
            }

            Brk => {
                // BRK: push PC+1 (skip padding byte), push P with B set, set I, jump to IRQ vector
                self.pc = self.pc.wrapping_add(1); // skip padding byte
                self.push16(self.pc);
                self.push8(self.p | flags::B | flags::U);
                self.flag_set(flags::I);
                let lo = self.bus.read(0xFFFE) as u16;
                let hi = self.bus.read(0xFFFF) as u16;
                self.pc = (hi << 8) | lo;
            }

            Kil => {
                // KIL/JAM — halt the CPU
                self.pc = self.pc.wrapping_sub(1);
            }
        }
    }

    // ── Log state (captured BEFORE stepping) ─────────────────────────────────

    pub fn log_state(&mut self) -> String {
        let opcode_byte = self.bus.peek(self.pc);
        let instr = get_opcodes()[opcode_byte as usize];
        let byte_count = instr_byte_count(instr.mode);

        let b1 = if byte_count > 1 { self.bus.peek(self.pc.wrapping_add(1)) } else { 0 };
        let b2 = if byte_count > 2 { self.bus.peek(self.pc.wrapping_add(2)) } else { 0 };

        // Raw bytes column (padded to 9 chars)
        let raw = match byte_count {
            1 => format!("{:02X}       ", opcode_byte),
            2 => format!("{:02X} {:02X}    ", opcode_byte, b1),
            3 => format!("{:02X} {:02X} {:02X} ", opcode_byte, b1, b2),
            _ => unreachable!(),
        };

        // Operand/mnemonic string — unofficial ops start with '*',
        // official ops get a leading space so the 3-letter mnemonic
        // always lands at column 16.
        let operand = self.format_operand(instr, b1, b2);
        let operand_str = if instr.mnemonic.starts_with('*') {
            operand
        } else {
            format!(" {}", operand)
        };

        // PPU scanline/dot approximation
        let ppu_cycles = self.cycles * 3;
        let ppu_dot = ppu_cycles % 341;
        let ppu_line = (ppu_cycles / 341) % 262;

        format!(
            "{:04X}  {}{:<33}A:{:02X} X:{:02X} Y:{:02X} P:{:02X} SP:{:02X} PPU:{:>3},{:>3} CYC:{}",
            self.pc, raw, operand_str,
            self.a, self.x, self.y, self.p, self.sp,
            ppu_line, ppu_dot,
            self.cycles
        )
    }

    fn format_operand(&self, instr: Instruction, b1: u8, b2: u8) -> String {
        use AddrMode::*;
        use OpKind::*;

        let mne = instr.mnemonic;
        let lo = b1 as u16;
        let hi = b2 as u16;
        let abs_addr = (hi << 8) | lo;

        match instr.mode {
            Implied => mne.to_string(),

            Accumulator => format!("{} A", mne),

            Immediate => format!("{} #${:02X}", mne, b1),

            ZeroPage => {
                let val = self.bus.peek(b1 as u16);
                format!("{} ${:02X} = {:02X}", mne, b1, val)
            }

            ZeroPageX => {
                let eff = b1.wrapping_add(self.x);
                let val = self.bus.peek(eff as u16);
                format!("{} ${:02X},X @ {:02X} = {:02X}", mne, b1, eff, val)
            }

            ZeroPageY => {
                let eff = b1.wrapping_add(self.y);
                let val = self.bus.peek(eff as u16);
                format!("{} ${:02X},Y @ {:02X} = {:02X}", mne, b1, eff, val)
            }

            Absolute => {
                match instr.kind {
                    Jmp | Jsr => format!("{} ${:04X}", mne, abs_addr),
                    _ => {
                        let val = self.bus.peek(abs_addr);
                        format!("{} ${:04X} = {:02X}", mne, abs_addr, val)
                    }
                }
            }

            AbsoluteX => {
                let eff = abs_addr.wrapping_add(self.x as u16);
                let val = self.bus.peek(eff);
                format!("{} ${:04X},X @ {:04X} = {:02X}", mne, abs_addr, eff, val)
            }

            AbsoluteY => {
                let eff = abs_addr.wrapping_add(self.y as u16);
                let val = self.bus.peek(eff);
                format!("{} ${:04X},Y @ {:04X} = {:02X}", mne, abs_addr, eff, val)
            }

            Indirect => {
                let ptr = abs_addr;
                let lo2 = self.bus.peek(ptr) as u16;
                let hi2 = self.bus.peek((ptr & 0xFF00) | ((ptr + 1) & 0x00FF)) as u16;
                let dest = (hi2 << 8) | lo2;
                format!("{} (${:04X}) = {:04X}", mne, ptr, dest)
            }

            IndirectX => {
                let ptr = b1.wrapping_add(self.x) as u16;
                let lo2 = self.bus.peek(ptr & 0x00FF) as u16;
                let hi2 = self.bus.peek((ptr.wrapping_add(1)) & 0x00FF) as u16;
                let eff = (hi2 << 8) | lo2;
                let val = self.bus.peek(eff);
                format!("{} (${:02X},X) @ {:02X} = {:04X} = {:02X}", mne, b1, ptr as u8, eff, val)
            }

            IndirectY => {
                let ptr = b1 as u16;
                let lo2 = self.bus.peek(ptr & 0x00FF) as u16;
                let hi2 = self.bus.peek((ptr.wrapping_add(1)) & 0x00FF) as u16;
                let base = (hi2 << 8) | lo2;
                let eff = base.wrapping_add(self.y as u16);
                let val = self.bus.peek(eff);
                format!("{} (${:02X}),Y = {:04X} @ {:04X} = {:02X}", mne, b1, base, eff, val)
            }

            Relative => {
                let offset = b1 as i8 as i32;
                let target = (self.pc as i32 + 2 + offset) as u16;
                format!("{} ${:04X}", mne, target)
            }
        }
    }

    /// Load raw program bytes at 0x0600 and set PC to the entry point.
    /// Used for simple test ROMs that run from RAM (e.g. the snake game).
    pub fn load_program(&mut self, program: &[u8]) {
        for (i, &byte) in program.iter().enumerate() {
            self.bus.write(0x0600 + i as u16, byte);
        }
        self.pc = 0x0600;
    }

    /// Run the CPU in a loop, calling `callback` before each instruction.
    /// The callback receives `&mut Cpu` and returns `true` to continue or `false` to stop.
    pub fn run_with_callback<F>(&mut self, mut callback: F)
    where
        F: FnMut(&mut Self) -> bool,
    {
        loop {
            if !callback(self) {
                break;
            }
            self.step();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bus::Bus;
    use crate::ines::{INesRom, Mirroring};

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

    fn make_cpu(program: &[u8]) -> Cpu {
        let mut rom = vec![0u8; 0x4000]; // 16 KiB
        rom[..program.len()].copy_from_slice(program);
        // Reset vector at 0x3FFC/3FFD → 0x8000
        rom[0x3FFC] = 0x00;
        rom[0x3FFD] = 0x80;
        let bus = make_bus(rom);
        let mut cpu = Cpu::new(bus);
        cpu.pc = 0x8000;
        cpu.cycles = 0;
        cpu
    }

    #[test]
    fn test_initial_state() {
        let bus = make_bus(vec![0u8; 0x4000]);
        let cpu = Cpu::new(bus);
        assert_eq!(cpu.pc, 0xC000);
        assert_eq!(cpu.sp, 0xFD);
        assert_eq!(cpu.p, 0x24);
        assert_eq!(cpu.cycles, 7);
    }

    #[test]
    fn test_lda_immediate() {
        let mut cpu = make_cpu(&[0xA9, 0x42]); // LDA #$42
        cpu.step();
        assert_eq!(cpu.a, 0x42);
        assert!(!cpu.flag_val(flags::Z));
        assert!(!cpu.flag_val(flags::N));
        assert_eq!(cpu.cycles, 2);
    }

    #[test]
    fn test_lda_zero_sets_z() {
        let mut cpu = make_cpu(&[0xA9, 0x00]); // LDA #$00
        cpu.step();
        assert_eq!(cpu.a, 0x00);
        assert!(cpu.flag_val(flags::Z));
        assert!(!cpu.flag_val(flags::N));
    }

    #[test]
    fn test_lda_negative_sets_n() {
        let mut cpu = make_cpu(&[0xA9, 0x80]); // LDA #$80
        cpu.step();
        assert!(!cpu.flag_val(flags::Z));
        assert!(cpu.flag_val(flags::N));
    }

    #[test]
    fn test_adc_no_carry_no_overflow() {
        let mut cpu = make_cpu(&[0xA9, 0x10, 0x69, 0x20]); // LDA #$10; ADC #$20
        cpu.step();
        cpu.step();
        assert_eq!(cpu.a, 0x30);
        assert!(!cpu.flag_val(flags::C));
        assert!(!cpu.flag_val(flags::V));
    }

    #[test]
    fn test_adc_carry_out() {
        let mut cpu = make_cpu(&[0xA9, 0xFF, 0x69, 0x01]); // LDA #$FF; ADC #$01
        cpu.step();
        cpu.step();
        assert_eq!(cpu.a, 0x00);
        assert!(cpu.flag_val(flags::C));
        assert!(cpu.flag_val(flags::Z));
    }

    #[test]
    fn test_adc_overflow() {
        // $50 + $50 = $A0: positive + positive = negative → overflow
        let mut cpu = make_cpu(&[0xA9, 0x50, 0x69, 0x50]);
        cpu.step();
        cpu.step();
        assert_eq!(cpu.a, 0xA0);
        assert!(cpu.flag_val(flags::V));
        assert!(cpu.flag_val(flags::N));
        assert!(!cpu.flag_val(flags::C));
    }

    #[test]
    fn test_sbc_basic() {
        // SEC; LDA #$50; SBC #$30 → $20
        let mut cpu = make_cpu(&[0x38, 0xA9, 0x50, 0xE9, 0x30]);
        cpu.step(); // SEC
        cpu.step(); // LDA
        cpu.step(); // SBC
        assert_eq!(cpu.a, 0x20);
        assert!(cpu.flag_val(flags::C)); // no borrow
        assert!(!cpu.flag_val(flags::V));
    }

    #[test]
    fn test_stack_push_pull() {
        let mut cpu = make_cpu(&[]);
        cpu.sp = 0xFF;
        cpu.push8(0xAB);
        assert_eq!(cpu.sp, 0xFE);
        let val = cpu.pull8();
        assert_eq!(val, 0xAB);
        assert_eq!(cpu.sp, 0xFF);
    }

    #[test]
    fn test_indirect_jmp_page_wrap_bug() {
        // JMP ($01FF): lo from $01FF, hi from $0100 (not $0200) — 6502 page-wrap bug
        // Instruction in ROM, target address bytes in RAM
        let mut cpu = make_cpu(&[0x6C, 0xFF, 0x01]); // JMP ($01FF)
        cpu.bus.write(0x01FF, 0x34); // lo byte of destination
        cpu.bus.write(0x0100, 0x12); // hi byte wraps to same page, not $0200
        cpu.step();
        assert_eq!(cpu.pc, 0x1234);
    }

    #[test]
    fn test_zero_page_x_wrap() {
        // LDA $FF,X with X=1 should read from $00 (wraps in zero page)
        let mut cpu = make_cpu(&[0xB5, 0xFF]); // LDA $FF,X
        cpu.x = 1;
        cpu.bus.write(0x0000, 0x42);
        cpu.step();
        assert_eq!(cpu.a, 0x42);
    }

    #[test]
    fn test_branch_taken_same_page() {
        // BEQ +2 when Z=1
        let mut cpu = make_cpu(&[0xF0, 0x02]); // BEQ $+2
        cpu.p |= flags::Z;
        cpu.cycles = 0;
        cpu.step();
        assert_eq!(cpu.pc, 0x8004); // 0x8002 + 2
        assert_eq!(cpu.cycles, 3);   // 2 base + 1 taken
    }

    #[test]
    fn test_branch_not_taken() {
        let mut cpu = make_cpu(&[0xF0, 0x02]); // BEQ $+2, Z=0
        cpu.p &= !flags::Z;
        cpu.cycles = 0;
        cpu.step();
        assert_eq!(cpu.pc, 0x8002);
        assert_eq!(cpu.cycles, 2);
    }

    #[test]
    fn test_php_plp_b_flag() {
        let mut cpu = make_cpu(&[0x08, 0x68]); // PHP; PLA (to check pushed value)
        cpu.p = 0x24;
        cpu.step(); // PHP
        cpu.pc = 0x8001;
        cpu.step(); // PLA — pull the byte PHP pushed into A
        // PHP should have pushed p | B | U = 0x24 | 0x10 | 0x20 = 0x34
        assert_eq!(cpu.a, 0x34);
    }

    #[test]
    fn test_inx_wraps() {
        let mut cpu = make_cpu(&[0xE8]); // INX
        cpu.x = 0xFF;
        cpu.step();
        assert_eq!(cpu.x, 0x00);
        assert!(cpu.flag_val(flags::Z));
    }

    #[test]
    fn test_jsr_rts() {
        // JSR $8003; NOP; RTS (with NOP at $8003 and RTS at $8004)
        let mut cpu = make_cpu(&[0x20, 0x03, 0x80, 0xEA, 0x60]);
        cpu.step(); // JSR $8003 → pushes 0x8002, PC = $8003
        assert_eq!(cpu.pc, 0x8003);
        cpu.step(); // NOP
        cpu.step(); // RTS → pulls 0x8002, PC = 0x8003
        assert_eq!(cpu.pc, 0x8003);
    }
}
