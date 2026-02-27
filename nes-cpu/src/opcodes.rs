#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AddrMode {
    Implied,
    Accumulator,
    Immediate,
    ZeroPage,
    ZeroPageX,
    ZeroPageY,
    Absolute,
    AbsoluteX,
    AbsoluteY,
    Indirect,
    IndirectX,
    IndirectY,
    Relative,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum OpKind {
    // Load/Store
    Lda, Ldx, Ldy, Sta, Stx, Sty,
    // Register transfers
    Tax, Tay, Txa, Tya, Tsx, Txs,
    // Stack
    Pha, Php, Pla, Plp,
    // Arithmetic
    Adc, Sbc,
    // Increment/Decrement
    Inc, Inx, Iny, Dec, Dex, Dey,
    // Logical
    And, Ora, Eor,
    // Shift/Rotate
    Asl, Lsr, Rol, Ror,
    // Comparison
    Cmp, Cpx, Cpy,
    // Bit test
    Bit,
    // Branch
    Bcc, Bcs, Beq, Bmi, Bne, Bpl, Bvc, Bvs,
    // Jump/Call
    Jmp, Jsr, Rts, Rti,
    // Flags
    Clc, Cld, Cli, Clv, Sec, Sed, Sei,
    // No-op
    Nop,
    // Unofficial combined ops
    Lax, Sax, Dcp, Isc, Slo, Rla, Sre, Rra,
    // Unofficial NOP (reads operand, discards)
    UnopNop,
    // Unofficial ALR (ASR): AND + LSR
    Alr,
    // Unofficial ANC: AND + carry from bit 7
    Anc,
    // Unofficial ARR: AND + ROR
    Arr,
    // Unofficial XAA (unstable, treat as NOP for nestest)
    Xaa,
    // Unofficial AXS (SBX): (A AND X) - imm -> X
    Axs,
    // Unofficial SHY (SYA)
    Shy,
    // Unofficial SHX (SXA)
    Shx,
    // Unofficial TAS
    Tas,
    // Unofficial AHX (SHA)
    Ahx,
    // Unofficial LAS
    Las,
    // KIL / JAM
    Kil,
}

#[derive(Clone, Copy, Debug)]
pub struct Instruction {
    pub kind: OpKind,
    pub mnemonic: &'static str,
    pub mode: AddrMode,
    pub cycles: u8,
    pub page_cross_penalty: bool,
}

impl Instruction {
    const fn new(
        kind: OpKind,
        mnemonic: &'static str,
        mode: AddrMode,
        cycles: u8,
        page_cross_penalty: bool,
    ) -> Self {
        Self { kind, mnemonic, mode, cycles, page_cross_penalty }
    }
}

const KIL: Instruction = Instruction::new(OpKind::Kil, "KIL", AddrMode::Implied, 2, false);

pub fn get_opcodes() -> &'static [Instruction; 256] {
    use std::sync::OnceLock;
    static TABLE: OnceLock<[Instruction; 256]> = OnceLock::new();
    TABLE.get_or_init(build_table)
}

#[allow(clippy::too_many_lines)]
fn build_table() -> [Instruction; 256] {
    use AddrMode::*;
    use OpKind::*;

    let mut t = [KIL; 256];

    macro_rules! op {
        ($byte:expr, $kind:expr, $mne:expr, $mode:expr, $cyc:expr) => {
            t[$byte] = Instruction::new($kind, $mne, $mode, $cyc, false);
        };
        ($byte:expr, $kind:expr, $mne:expr, $mode:expr, $cyc:expr, page) => {
            t[$byte] = Instruction::new($kind, $mne, $mode, $cyc, true);
        };
    }

    // ── LDA ──────────────────────────────────────────────────────────────────
    op!(0xA9, Lda, "LDA", Immediate,  2);
    op!(0xA5, Lda, "LDA", ZeroPage,   3);
    op!(0xB5, Lda, "LDA", ZeroPageX,  4);
    op!(0xAD, Lda, "LDA", Absolute,   4);
    op!(0xBD, Lda, "LDA", AbsoluteX,  4, page);
    op!(0xB9, Lda, "LDA", AbsoluteY,  4, page);
    op!(0xA1, Lda, "LDA", IndirectX,  6);
    op!(0xB1, Lda, "LDA", IndirectY,  5, page);

    // ── LDX ──────────────────────────────────────────────────────────────────
    op!(0xA2, Ldx, "LDX", Immediate,  2);
    op!(0xA6, Ldx, "LDX", ZeroPage,   3);
    op!(0xB6, Ldx, "LDX", ZeroPageY,  4);
    op!(0xAE, Ldx, "LDX", Absolute,   4);
    op!(0xBE, Ldx, "LDX", AbsoluteY,  4, page);

    // ── LDY ──────────────────────────────────────────────────────────────────
    op!(0xA0, Ldy, "LDY", Immediate,  2);
    op!(0xA4, Ldy, "LDY", ZeroPage,   3);
    op!(0xB4, Ldy, "LDY", ZeroPageX,  4);
    op!(0xAC, Ldy, "LDY", Absolute,   4);
    op!(0xBC, Ldy, "LDY", AbsoluteX,  4, page);

    // ── STA ──────────────────────────────────────────────────────────────────
    op!(0x85, Sta, "STA", ZeroPage,   3);
    op!(0x95, Sta, "STA", ZeroPageX,  4);
    op!(0x8D, Sta, "STA", Absolute,   4);
    op!(0x9D, Sta, "STA", AbsoluteX,  5);
    op!(0x99, Sta, "STA", AbsoluteY,  5);
    op!(0x81, Sta, "STA", IndirectX,  6);
    op!(0x91, Sta, "STA", IndirectY,  6);

    // ── STX ──────────────────────────────────────────────────────────────────
    op!(0x86, Stx, "STX", ZeroPage,   3);
    op!(0x96, Stx, "STX", ZeroPageY,  4);
    op!(0x8E, Stx, "STX", Absolute,   4);

    // ── STY ──────────────────────────────────────────────────────────────────
    op!(0x84, Sty, "STY", ZeroPage,   3);
    op!(0x94, Sty, "STY", ZeroPageX,  4);
    op!(0x8C, Sty, "STY", Absolute,   4);

    // ── Register transfers ───────────────────────────────────────────────────
    op!(0xAA, Tax, "TAX", Implied, 2);
    op!(0xA8, Tay, "TAY", Implied, 2);
    op!(0x8A, Txa, "TXA", Implied, 2);
    op!(0x98, Tya, "TYA", Implied, 2);
    op!(0xBA, Tsx, "TSX", Implied, 2);
    op!(0x9A, Txs, "TXS", Implied, 2);

    // ── Stack ────────────────────────────────────────────────────────────────
    op!(0x48, Pha, "PHA", Implied, 3);
    op!(0x08, Php, "PHP", Implied, 3);
    op!(0x68, Pla, "PLA", Implied, 4);
    op!(0x28, Plp, "PLP", Implied, 4);

    // ── ADC ──────────────────────────────────────────────────────────────────
    op!(0x69, Adc, "ADC", Immediate, 2);
    op!(0x65, Adc, "ADC", ZeroPage,  3);
    op!(0x75, Adc, "ADC", ZeroPageX, 4);
    op!(0x6D, Adc, "ADC", Absolute,  4);
    op!(0x7D, Adc, "ADC", AbsoluteX, 4, page);
    op!(0x79, Adc, "ADC", AbsoluteY, 4, page);
    op!(0x61, Adc, "ADC", IndirectX, 6);
    op!(0x71, Adc, "ADC", IndirectY, 5, page);

    // ── SBC ──────────────────────────────────────────────────────────────────
    op!(0xE9, Sbc, "SBC", Immediate, 2);
    op!(0xE5, Sbc, "SBC", ZeroPage,  3);
    op!(0xF5, Sbc, "SBC", ZeroPageX, 4);
    op!(0xED, Sbc, "SBC", Absolute,  4);
    op!(0xFD, Sbc, "SBC", AbsoluteX, 4, page);
    op!(0xF9, Sbc, "SBC", AbsoluteY, 4, page);
    op!(0xE1, Sbc, "SBC", IndirectX, 6);
    op!(0xF1, Sbc, "SBC", IndirectY, 5, page);

    // ── INC ──────────────────────────────────────────────────────────────────
    op!(0xE6, Inc, "INC", ZeroPage,  5);
    op!(0xF6, Inc, "INC", ZeroPageX, 6);
    op!(0xEE, Inc, "INC", Absolute,  6);
    op!(0xFE, Inc, "INC", AbsoluteX, 7); // RMW: always 7, no page penalty

    // ── DEC ──────────────────────────────────────────────────────────────────
    op!(0xC6, Dec, "DEC", ZeroPage,  5);
    op!(0xD6, Dec, "DEC", ZeroPageX, 6);
    op!(0xCE, Dec, "DEC", Absolute,  6);
    op!(0xDE, Dec, "DEC", AbsoluteX, 7); // RMW

    // ── INX / INY / DEX / DEY ────────────────────────────────────────────────
    op!(0xE8, Inx, "INX", Implied, 2);
    op!(0xC8, Iny, "INY", Implied, 2);
    op!(0xCA, Dex, "DEX", Implied, 2);
    op!(0x88, Dey, "DEY", Implied, 2);

    // ── AND ──────────────────────────────────────────────────────────────────
    op!(0x29, And, "AND", Immediate, 2);
    op!(0x25, And, "AND", ZeroPage,  3);
    op!(0x35, And, "AND", ZeroPageX, 4);
    op!(0x2D, And, "AND", Absolute,  4);
    op!(0x3D, And, "AND", AbsoluteX, 4, page);
    op!(0x39, And, "AND", AbsoluteY, 4, page);
    op!(0x21, And, "AND", IndirectX, 6);
    op!(0x31, And, "AND", IndirectY, 5, page);

    // ── ORA ──────────────────────────────────────────────────────────────────
    op!(0x09, Ora, "ORA", Immediate, 2);
    op!(0x05, Ora, "ORA", ZeroPage,  3);
    op!(0x15, Ora, "ORA", ZeroPageX, 4);
    op!(0x0D, Ora, "ORA", Absolute,  4);
    op!(0x1D, Ora, "ORA", AbsoluteX, 4, page);
    op!(0x19, Ora, "ORA", AbsoluteY, 4, page);
    op!(0x01, Ora, "ORA", IndirectX, 6);
    op!(0x11, Ora, "ORA", IndirectY, 5, page);

    // ── EOR ──────────────────────────────────────────────────────────────────
    op!(0x49, Eor, "EOR", Immediate, 2);
    op!(0x45, Eor, "EOR", ZeroPage,  3);
    op!(0x55, Eor, "EOR", ZeroPageX, 4);
    op!(0x4D, Eor, "EOR", Absolute,  4);
    op!(0x5D, Eor, "EOR", AbsoluteX, 4, page);
    op!(0x59, Eor, "EOR", AbsoluteY, 4, page);
    op!(0x41, Eor, "EOR", IndirectX, 6);
    op!(0x51, Eor, "EOR", IndirectY, 5, page);

    // ── ASL ──────────────────────────────────────────────────────────────────
    op!(0x0A, Asl, "ASL", Accumulator, 2);
    op!(0x06, Asl, "ASL", ZeroPage,    5);
    op!(0x16, Asl, "ASL", ZeroPageX,   6);
    op!(0x0E, Asl, "ASL", Absolute,    6);
    op!(0x1E, Asl, "ASL", AbsoluteX,   7); // RMW

    // ── LSR ──────────────────────────────────────────────────────────────────
    op!(0x4A, Lsr, "LSR", Accumulator, 2);
    op!(0x46, Lsr, "LSR", ZeroPage,    5);
    op!(0x56, Lsr, "LSR", ZeroPageX,   6);
    op!(0x4E, Lsr, "LSR", Absolute,    6);
    op!(0x5E, Lsr, "LSR", AbsoluteX,   7); // RMW

    // ── ROL ──────────────────────────────────────────────────────────────────
    op!(0x2A, Rol, "ROL", Accumulator, 2);
    op!(0x26, Rol, "ROL", ZeroPage,    5);
    op!(0x36, Rol, "ROL", ZeroPageX,   6);
    op!(0x2E, Rol, "ROL", Absolute,    6);
    op!(0x3E, Rol, "ROL", AbsoluteX,   7); // RMW

    // ── ROR ──────────────────────────────────────────────────────────────────
    op!(0x6A, Ror, "ROR", Accumulator, 2);
    op!(0x66, Ror, "ROR", ZeroPage,    5);
    op!(0x76, Ror, "ROR", ZeroPageX,   6);
    op!(0x6E, Ror, "ROR", Absolute,    6);
    op!(0x7E, Ror, "ROR", AbsoluteX,   7); // RMW

    // ── CMP ──────────────────────────────────────────────────────────────────
    op!(0xC9, Cmp, "CMP", Immediate, 2);
    op!(0xC5, Cmp, "CMP", ZeroPage,  3);
    op!(0xD5, Cmp, "CMP", ZeroPageX, 4);
    op!(0xCD, Cmp, "CMP", Absolute,  4);
    op!(0xDD, Cmp, "CMP", AbsoluteX, 4, page);
    op!(0xD9, Cmp, "CMP", AbsoluteY, 4, page);
    op!(0xC1, Cmp, "CMP", IndirectX, 6);
    op!(0xD1, Cmp, "CMP", IndirectY, 5, page);

    // ── CPX ──────────────────────────────────────────────────────────────────
    op!(0xE0, Cpx, "CPX", Immediate, 2);
    op!(0xE4, Cpx, "CPX", ZeroPage,  3);
    op!(0xEC, Cpx, "CPX", Absolute,  4);

    // ── CPY ──────────────────────────────────────────────────────────────────
    op!(0xC0, Cpy, "CPY", Immediate, 2);
    op!(0xC4, Cpy, "CPY", ZeroPage,  3);
    op!(0xCC, Cpy, "CPY", Absolute,  4);

    // ── BIT ──────────────────────────────────────────────────────────────────
    op!(0x24, Bit, "BIT", ZeroPage, 3);
    op!(0x2C, Bit, "BIT", Absolute, 4);

    // ── Branches ─────────────────────────────────────────────────────────────
    op!(0x90, Bcc, "BCC", Relative, 2);
    op!(0xB0, Bcs, "BCS", Relative, 2);
    op!(0xF0, Beq, "BEQ", Relative, 2);
    op!(0x30, Bmi, "BMI", Relative, 2);
    op!(0xD0, Bne, "BNE", Relative, 2);
    op!(0x10, Bpl, "BPL", Relative, 2);
    op!(0x50, Bvc, "BVC", Relative, 2);
    op!(0x70, Bvs, "BVS", Relative, 2);

    // ── JMP / JSR / RTS / RTI ────────────────────────────────────────────────
    op!(0x4C, Jmp, "JMP", Absolute, 3);
    op!(0x6C, Jmp, "JMP", Indirect, 5);
    op!(0x20, Jsr, "JSR", Absolute, 6);
    op!(0x60, Rts, "RTS", Implied,  6);
    op!(0x40, Rti, "RTI", Implied,  6);

    // ── Flag ops ─────────────────────────────────────────────────────────────
    op!(0x18, Clc, "CLC", Implied, 2);
    op!(0xD8, Cld, "CLD", Implied, 2);
    op!(0x58, Cli, "CLI", Implied, 2);
    op!(0xB8, Clv, "CLV", Implied, 2);
    op!(0x38, Sec, "SEC", Implied, 2);
    op!(0xF8, Sed, "SED", Implied, 2);
    op!(0x78, Sei, "SEI", Implied, 2);

    // ── NOP (legal) ──────────────────────────────────────────────────────────
    op!(0xEA, Nop, "NOP", Implied, 2);

    // ── BRK ──────────────────────────────────────────────────────────────────
    // BRK is not tested by nestest directly but needs to exist
    op!(0x00, Kil, "BRK", Implied, 7);

    // ═══════════════════════════════════════════════════════════════════════════
    // UNOFFICIAL OPCODES
    // ═══════════════════════════════════════════════════════════════════════════

    // ── Unofficial NOPs (implied, 1 byte) ────────────────────────────────────
    op!(0x1A, UnopNop, "*NOP", Implied, 2);
    op!(0x3A, UnopNop, "*NOP", Implied, 2);
    op!(0x5A, UnopNop, "*NOP", Implied, 2);
    op!(0x7A, UnopNop, "*NOP", Implied, 2);
    op!(0xDA, UnopNop, "*NOP", Implied, 2);
    op!(0xFA, UnopNop, "*NOP", Implied, 2);

    // ── Unofficial NOPs (immediate, 2 bytes) ─────────────────────────────────
    op!(0x80, UnopNop, "*NOP", Immediate, 2);
    op!(0x82, UnopNop, "*NOP", Immediate, 2);
    op!(0x89, UnopNop, "*NOP", Immediate, 2);
    op!(0xC2, UnopNop, "*NOP", Immediate, 2);
    op!(0xE2, UnopNop, "*NOP", Immediate, 2);

    // ── Unofficial NOPs (zero page, 2 bytes) ─────────────────────────────────
    op!(0x04, UnopNop, "*NOP", ZeroPage, 3);
    op!(0x44, UnopNop, "*NOP", ZeroPage, 3);
    op!(0x64, UnopNop, "*NOP", ZeroPage, 3);

    // ── Unofficial NOPs (zero page X, 2 bytes) ───────────────────────────────
    op!(0x14, UnopNop, "*NOP", ZeroPageX, 4);
    op!(0x34, UnopNop, "*NOP", ZeroPageX, 4);
    op!(0x54, UnopNop, "*NOP", ZeroPageX, 4);
    op!(0x74, UnopNop, "*NOP", ZeroPageX, 4);
    op!(0xD4, UnopNop, "*NOP", ZeroPageX, 4);
    op!(0xF4, UnopNop, "*NOP", ZeroPageX, 4);

    // ── Unofficial NOPs (absolute, 3 bytes) ──────────────────────────────────
    op!(0x0C, UnopNop, "*NOP", Absolute, 4);

    // ── Unofficial NOPs (absolute X, 3 bytes, page penalty) ─────────────────
    op!(0x1C, UnopNop, "*NOP", AbsoluteX, 4, page);
    op!(0x3C, UnopNop, "*NOP", AbsoluteX, 4, page);
    op!(0x5C, UnopNop, "*NOP", AbsoluteX, 4, page);
    op!(0x7C, UnopNop, "*NOP", AbsoluteX, 4, page);
    op!(0xDC, UnopNop, "*NOP", AbsoluteX, 4, page);
    op!(0xFC, UnopNop, "*NOP", AbsoluteX, 4, page);

    // ── LAX (LDA + LDX) ──────────────────────────────────────────────────────
    op!(0xA7, Lax, "*LAX", ZeroPage,  3);
    op!(0xB7, Lax, "*LAX", ZeroPageY, 4);
    op!(0xAF, Lax, "*LAX", Absolute,  4);
    op!(0xBF, Lax, "*LAX", AbsoluteY, 4, page);
    op!(0xA3, Lax, "*LAX", IndirectX, 6);
    op!(0xB3, Lax, "*LAX", IndirectY, 5, page);

    // ── SAX (Store A AND X) ───────────────────────────────────────────────────
    op!(0x87, Sax, "*SAX", ZeroPage,  3);
    op!(0x97, Sax, "*SAX", ZeroPageY, 4);
    op!(0x8F, Sax, "*SAX", Absolute,  4);
    op!(0x83, Sax, "*SAX", IndirectX, 6);

    // ── DCP (DEC + CMP) ───────────────────────────────────────────────────────
    op!(0xC7, Dcp, "*DCP", ZeroPage,  5);
    op!(0xD7, Dcp, "*DCP", ZeroPageX, 6);
    op!(0xCF, Dcp, "*DCP", Absolute,  6);
    op!(0xDF, Dcp, "*DCP", AbsoluteX, 7);
    op!(0xDB, Dcp, "*DCP", AbsoluteY, 7);
    op!(0xC3, Dcp, "*DCP", IndirectX, 8);
    op!(0xD3, Dcp, "*DCP", IndirectY, 8);

    // ── ISC / ISB (INC + SBC) ─────────────────────────────────────────────────
    op!(0xE7, Isc, "*ISB", ZeroPage,  5);
    op!(0xF7, Isc, "*ISB", ZeroPageX, 6);
    op!(0xEF, Isc, "*ISB", Absolute,  6);
    op!(0xFF, Isc, "*ISB", AbsoluteX, 7);
    op!(0xFB, Isc, "*ISB", AbsoluteY, 7);
    op!(0xE3, Isc, "*ISB", IndirectX, 8);
    op!(0xF3, Isc, "*ISB", IndirectY, 8);

    // ── SLO (ASL + ORA) ───────────────────────────────────────────────────────
    op!(0x07, Slo, "*SLO", ZeroPage,  5);
    op!(0x17, Slo, "*SLO", ZeroPageX, 6);
    op!(0x0F, Slo, "*SLO", Absolute,  6);
    op!(0x1F, Slo, "*SLO", AbsoluteX, 7);
    op!(0x1B, Slo, "*SLO", AbsoluteY, 7);
    op!(0x03, Slo, "*SLO", IndirectX, 8);
    op!(0x13, Slo, "*SLO", IndirectY, 8);

    // ── RLA (ROL + AND) ───────────────────────────────────────────────────────
    op!(0x27, Rla, "*RLA", ZeroPage,  5);
    op!(0x37, Rla, "*RLA", ZeroPageX, 6);
    op!(0x2F, Rla, "*RLA", Absolute,  6);
    op!(0x3F, Rla, "*RLA", AbsoluteX, 7);
    op!(0x3B, Rla, "*RLA", AbsoluteY, 7);
    op!(0x23, Rla, "*RLA", IndirectX, 8);
    op!(0x33, Rla, "*RLA", IndirectY, 8);

    // ── SRE (LSR + EOR) ───────────────────────────────────────────────────────
    op!(0x47, Sre, "*SRE", ZeroPage,  5);
    op!(0x57, Sre, "*SRE", ZeroPageX, 6);
    op!(0x4F, Sre, "*SRE", Absolute,  6);
    op!(0x5F, Sre, "*SRE", AbsoluteX, 7);
    op!(0x5B, Sre, "*SRE", AbsoluteY, 7);
    op!(0x43, Sre, "*SRE", IndirectX, 8);
    op!(0x53, Sre, "*SRE", IndirectY, 8);

    // ── RRA (ROR + ADC) ───────────────────────────────────────────────────────
    op!(0x67, Rra, "*RRA", ZeroPage,  5);
    op!(0x77, Rra, "*RRA", ZeroPageX, 6);
    op!(0x6F, Rra, "*RRA", Absolute,  6);
    op!(0x7F, Rra, "*RRA", AbsoluteX, 7);
    op!(0x7B, Rra, "*RRA", AbsoluteY, 7);
    op!(0x63, Rra, "*RRA", IndirectX, 8);
    op!(0x73, Rra, "*RRA", IndirectY, 8);

    // ── ALR (AND + LSR) ───────────────────────────────────────────────────────
    op!(0x4B, Alr, "*ALR", Immediate, 2);

    // ── ANC (AND, C = bit7 of result) ────────────────────────────────────────
    op!(0x0B, Anc, "*ANC", Immediate, 2);
    op!(0x2B, Anc, "*ANC", Immediate, 2);

    // ── ARR (AND + ROR) ───────────────────────────────────────────────────────
    op!(0x6B, Arr, "*ARR", Immediate, 2);

    // ── AXS / SBX (A AND X - imm -> X) ───────────────────────────────────────
    op!(0xCB, Axs, "*AXS", Immediate, 2);

    // ── XAA (unstable, approximate) ──────────────────────────────────────────
    op!(0x8B, Xaa, "*XAA", Immediate, 2);

    // ── LAS ───────────────────────────────────────────────────────────────────
    op!(0xBB, Las, "*LAS", AbsoluteY, 4, page);

    // ── Unofficial SBC (same as official 0xE9) ────────────────────────────────
    op!(0xEB, Sbc, "*SBC", Immediate, 2);

    // ── SHY / SHX / TAS / AHX (unstable, rarely tested) ─────────────────────
    op!(0x9C, Shy, "*SHY", AbsoluteX, 5);
    op!(0x9E, Shx, "*SHX", AbsoluteY, 5);
    op!(0x9B, Tas, "*TAS", AbsoluteY, 5);
    op!(0x93, Ahx, "*AHX", IndirectY, 6);
    op!(0x9F, Ahx, "*AHX", AbsoluteY, 5);

    t
}

/// Returns the number of bytes an instruction occupies (opcode + operands).
pub fn instr_byte_count(mode: AddrMode) -> u8 {
    match mode {
        AddrMode::Implied | AddrMode::Accumulator => 1,
        AddrMode::Immediate
        | AddrMode::ZeroPage
        | AddrMode::ZeroPageX
        | AddrMode::ZeroPageY
        | AddrMode::IndirectX
        | AddrMode::IndirectY
        | AddrMode::Relative => 2,
        AddrMode::Absolute
        | AddrMode::AbsoluteX
        | AddrMode::AbsoluteY
        | AddrMode::Indirect => 3,
    }
}
