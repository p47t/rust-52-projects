use nes_cpu::bus::Bus;
use nes_cpu::cpu::Cpu;
use nes_cpu::ines::INesRom;

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let rom_path = args.get(1).map(String::as_str).unwrap_or("roms/nestest.nes");
    let log_path = args.get(2).map(String::as_str).unwrap_or("roms/nestest.log");

    // Load ROM
    let rom = INesRom::load(rom_path)?;
    let bus = Bus::from_rom(rom);
    let mut cpu = Cpu::new(bus);

    // Load reference log (optional — if absent we just run and check memory)
    let reference_log = std::fs::read_to_string(log_path).ok();
    let ref_lines: Vec<&str> = reference_log
        .as_deref()
        .map(|s| s.lines().collect())
        .unwrap_or_default();

    if reference_log.is_none() {
        eprintln!("Note: no reference log at '{}' — running without log comparison", log_path);
    }

    // Run nestest
    let mut our_lines: Vec<String> = Vec::new();

    loop {
        let log_line = cpu.step();
        our_lines.push(log_line);

        if cpu.pc == 0xC66E {
            break;
        }

        if our_lines.len() > 10_000 {
            eprintln!("ERROR: exceeded step limit without reaching 0xC66E (PC={:04X})", cpu.pc);
            std::process::exit(1);
        }
    }

    // Check pass condition: $02 = official result, $03 = unofficial result
    let official = cpu.bus.read(0x0002);
    let unofficial = cpu.bus.read(0x0003);

    println!("nestest completed after {} instructions", our_lines.len());
    println!("Result codes: official=${:02X} unofficial=${:02X}", official, unofficial);

    let mut all_match = true;
    let mut first_mismatch: Option<usize> = None;

    if !ref_lines.is_empty() {
        for (i, (ours, reference)) in our_lines.iter().zip(ref_lines.iter()).enumerate() {
            if !cpu_columns_match(ours, reference) {
                if first_mismatch.is_none() {
                    first_mismatch = Some(i);
                    eprintln!("FIRST MISMATCH at line {}:", i + 1);
                    eprintln!("  OURS: {}", ours);
                    eprintln!("  REF:  {}", reference);
                }
                all_match = false;
            }
        }
        if all_match {
            println!("Log comparison: all {} lines match", our_lines.len().min(ref_lines.len()));
        } else {
            eprintln!(
                "Log comparison: MISMATCH (first at line {})",
                first_mismatch.map(|i| i + 1).unwrap_or(0)
            );
        }
    }

    let cpu_passed = official == 0;
    let unofficial_passed = unofficial == 0;

    if cpu_passed && unofficial_passed && all_match {
        println!("PASS: all nestest checks passed");
    } else {
        if !cpu_passed {
            eprintln!("FAIL: official opcodes test returned error code ${:02X}", official);
        }
        if !unofficial_passed {
            eprintln!("FAIL: unofficial opcodes test returned error code ${:02X}", unofficial);
        }
        if !all_match {
            eprintln!("FAIL: log comparison mismatch");
        }
        std::process::exit(1);
    }

    Ok(())
}

/// Compare CPU state columns, ignoring PPU (implementation-dependent).
/// Strips everything from "PPU:" onward before comparing.
fn cpu_columns_match(ours: &str, reference: &str) -> bool {
    fn cpu_portion(line: &str) -> &str {
        match line.find("PPU:") {
            Some(pos) => line[..pos].trim_end(),
            None => line,
        }
    }
    cpu_portion(ours) == cpu_portion(reference)
}
