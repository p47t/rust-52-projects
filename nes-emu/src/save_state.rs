use nes_cpu::state::*;

use crate::emulation::NesSystem;

/// Magic bytes identifying a save state file.
const MAGIC: &[u8; 4] = b"NESS";
/// Save state format version.
const VERSION: u8 = 1;

/// Capture the entire NES system state into a byte blob.
pub fn save(nes: &NesSystem) -> Vec<u8> {
    let mut out = Vec::with_capacity(64 * 1024);

    // Header
    out.extend_from_slice(MAGIC);
    write_u8(&mut out, VERSION);

    // CPU
    let cpu = &nes.sys.cpu;
    write_u8(&mut out, cpu.a);
    write_u8(&mut out, cpu.x);
    write_u8(&mut out, cpu.y);
    write_u16(&mut out, cpu.pc);
    write_u8(&mut out, cpu.sp);
    write_u8(&mut out, cpu.p);
    write_u64(&mut out, cpu.cycles);

    // Bus RAM
    write_bytes(&mut out, cpu.bus.ram());

    // Mapper
    if let Some(mapper) = &cpu.bus.mapper {
        let mapper_state = mapper.borrow().save_state();
        write_bytes(&mut out, &mapper_state);
    }

    // PPU
    let ppu = nes.sys.ppu.borrow();
    let ppu_state = ppu.save_state();
    write_bytes(&mut out, &ppu_state);
    drop(ppu);

    // APU
    let apu = nes.sys.apu.borrow();
    let apu_state = apu.save_state();
    write_bytes(&mut out, &apu_state);
    drop(apu);

    // Joypads
    let jp1 = nes.sys.joypad1.borrow();
    let jp1_state = jp1.save_state();
    write_bytes(&mut out, &jp1_state);
    drop(jp1);

    let jp2 = nes.sys.joypad2.borrow();
    let jp2_state = jp2.save_state();
    write_bytes(&mut out, &jp2_state);
    drop(jp2);

    // System cycle counters
    write_u64(&mut out, nes.sys.cpu_cycles());
    write_u64(&mut out, nes.sys.ppu_cycles());

    out
}

/// Restore the entire NES system state from a byte blob.
/// Returns an error message on failure.
pub fn load(nes: &mut NesSystem, data: &[u8]) -> Result<(), String> {
    let mut cursor: &[u8] = data;

    // Validate header
    if cursor.len() < 5 {
        return Err("Save state too small".to_string());
    }
    if &cursor[..4] != MAGIC {
        return Err("Invalid save state magic".to_string());
    }
    cursor = &cursor[4..];
    let version = read_u8(&mut cursor);
    if version != VERSION {
        return Err(format!("Unsupported save state version: {version}"));
    }

    // CPU
    nes.sys.cpu.a = read_u8(&mut cursor);
    nes.sys.cpu.x = read_u8(&mut cursor);
    nes.sys.cpu.y = read_u8(&mut cursor);
    nes.sys.cpu.pc = read_u16(&mut cursor);
    nes.sys.cpu.sp = read_u8(&mut cursor);
    nes.sys.cpu.p = read_u8(&mut cursor);
    nes.sys.cpu.cycles = read_u64(&mut cursor);

    // Bus RAM
    let ram = read_bytes(&mut cursor);
    if ram.len() != 2048 {
        return Err(format!("Invalid RAM size: {}", ram.len()));
    }
    let ram_arr: &[u8; 2048] = ram.as_slice().try_into().unwrap();
    nes.sys.cpu.bus.set_ram(ram_arr);

    // Mapper
    let mapper_state = read_bytes(&mut cursor);
    if let Some(mapper) = &nes.sys.cpu.bus.mapper {
        mapper.borrow_mut().load_state(&mapper_state);
    }

    // PPU
    let ppu_state = read_bytes(&mut cursor);
    nes.sys.ppu.borrow_mut().load_state(&ppu_state);

    // APU
    let apu_state = read_bytes(&mut cursor);
    nes.sys.apu.borrow_mut().load_state(&apu_state);

    // Joypads
    let jp1_state = read_bytes(&mut cursor);
    nes.sys.joypad1.borrow_mut().load_state(&jp1_state);

    let jp2_state = read_bytes(&mut cursor);
    nes.sys.joypad2.borrow_mut().load_state(&jp2_state);

    // System cycle counters
    let cpu_cycles = read_u64(&mut cursor);
    let ppu_cycles = read_u64(&mut cursor);
    nes.sys.set_cpu_cycles(cpu_cycles);
    nes.sys.set_ppu_cycles(ppu_cycles);

    Ok(())
}
