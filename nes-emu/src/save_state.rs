use planus::{ReadAsRoot, WriteAsOffset};

use crate::emulation::NesSystem;
use crate::generated::{CpuState, NesState, NesStateRef};

/// Capture the entire NES system state into a FlatBuffer byte blob.
pub fn save(nes: &NesSystem) -> Vec<u8> {
    // Collect subsystem blobs using existing save_state() methods
    let ram = nes.sys.cpu.bus.ram().to_vec();

    let mapper_blob = nes
        .sys
        .cpu
        .bus
        .mapper
        .as_ref()
        .map(|m| m.borrow().save_state())
        .unwrap_or_default();

    let ppu_blob = nes.sys.ppu.borrow().save_state();
    let apu_blob = nes.sys.apu.borrow().save_state();
    let jp1_blob = nes.sys.joypad1.borrow().save_state();
    let jp2_blob = nes.sys.joypad2.borrow().save_state();

    let cpu = &nes.sys.cpu;
    let state = NesState {
        version: 2,
        cpu: Some(Box::new(CpuState {
            a: cpu.a,
            x: cpu.x,
            y: cpu.y,
            pc: cpu.pc,
            sp: cpu.sp,
            p: cpu.p,
            cycles: cpu.cycles,
        })),
        ram: Some(ram),
        mapper: Some(mapper_blob),
        ppu: Some(ppu_blob),
        apu: Some(apu_blob),
        joypad1: Some(jp1_blob),
        joypad2: Some(jp2_blob),
        cpu_cycles: nes.sys.cpu_cycles(),
        ppu_cycles: nes.sys.ppu_cycles(),
    };

    let mut builder = planus::Builder::new();
    let offset = state.prepare(&mut builder);
    builder.finish(offset, None);
    builder.as_slice().to_vec()
}

/// Restore the entire NES system state from a FlatBuffer byte blob.
pub fn load(nes: &mut NesSystem, data: &[u8]) -> Result<(), String> {
    let state = NesStateRef::read_as_root(data).map_err(|e| format!("Invalid save state: {e}"))?;

    let cpu_ref = state
        .cpu()
        .map_err(|e| format!("Missing CPU state: {e}"))?
        .ok_or("No CPU state in save")?;

    nes.sys.cpu.a = cpu_ref.a().map_err(|e| format!("{e}"))?;
    nes.sys.cpu.x = cpu_ref.x().map_err(|e| format!("{e}"))?;
    nes.sys.cpu.y = cpu_ref.y().map_err(|e| format!("{e}"))?;
    nes.sys.cpu.pc = cpu_ref.pc().map_err(|e| format!("{e}"))?;
    nes.sys.cpu.sp = cpu_ref.sp().map_err(|e| format!("{e}"))?;
    nes.sys.cpu.p = cpu_ref.p().map_err(|e| format!("{e}"))?;
    nes.sys.cpu.cycles = cpu_ref.cycles().map_err(|e| format!("{e}"))?;

    // RAM
    let ram = state
        .ram()
        .map_err(|e| format!("Missing RAM: {e}"))?
        .ok_or("No RAM in save")?;
    if ram.len() != 2048 {
        return Err(format!("Invalid RAM size: {}", ram.len()));
    }
    let ram_arr: &[u8; 2048] = ram.try_into().unwrap();
    nes.sys.cpu.bus.set_ram(ram_arr);

    // Mapper
    if let Some(mapper_data) = state.mapper().map_err(|e| format!("{e}"))? {
        if let Some(mapper) = &nes.sys.cpu.bus.mapper {
            mapper.borrow_mut().load_state(mapper_data);
        }
    }

    // PPU
    if let Some(ppu_data) = state.ppu().map_err(|e| format!("{e}"))? {
        nes.sys.ppu.borrow_mut().load_state(ppu_data);
    }

    // APU
    if let Some(apu_data) = state.apu().map_err(|e| format!("{e}"))? {
        nes.sys.apu.borrow_mut().load_state(apu_data);
    }

    // Joypads
    if let Some(jp1_data) = state.joypad1().map_err(|e| format!("{e}"))? {
        nes.sys.joypad1.borrow_mut().load_state(jp1_data);
    }
    if let Some(jp2_data) = state.joypad2().map_err(|e| format!("{e}"))? {
        nes.sys.joypad2.borrow_mut().load_state(jp2_data);
    }

    // System cycle counters
    let cpu_cycles = state.cpu_cycles().map_err(|e| format!("{e}"))?;
    let ppu_cycles = state.ppu_cycles().map_err(|e| format!("{e}"))?;
    nes.sys.set_cpu_cycles(cpu_cycles);
    nes.sys.set_ppu_cycles(ppu_cycles);

    Ok(())
}
