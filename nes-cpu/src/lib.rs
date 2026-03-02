pub mod bus;
pub mod cpu;
pub mod ines;
pub mod opcodes;

#[cfg(test)]
mod tests {
    use crate::bus::Bus;
    use crate::cpu::Cpu;
    use crate::ines::INesRom;

    /// Compare CPU state columns, ignoring PPU (implementation-dependent).
    fn cpu_columns_match(ours: &str, reference: &str) -> bool {
        fn cpu_portion(line: &str) -> &str {
            match line.find("PPU:") {
                Some(pos) => line[..pos].trim_end(),
                None => line,
            }
        }
        cpu_portion(ours) == cpu_portion(reference)
    }

    #[test]
    fn nestest() {
        let rom = INesRom::load("roms/nestest.nes").expect("failed to load nestest ROM");
        let bus = Bus::from_rom(rom);
        let mut cpu = Cpu::new(bus);

        let reference_log =
            std::fs::read_to_string("roms/nestest.log").expect("failed to load nestest log");
        let ref_lines: Vec<&str> = reference_log.lines().collect();

        let mut our_lines: Vec<String> = Vec::new();

        loop {
            let log_line = cpu.step();
            our_lines.push(log_line);

            if cpu.pc == 0xC66E {
                break;
            }

            assert!(
                our_lines.len() <= 10_000,
                "exceeded step limit without reaching 0xC66E (PC={:04X})",
                cpu.pc
            );
        }

        // Check pass condition: $02 = official result, $03 = unofficial result
        let official = cpu.bus.read(0x0002);
        let unofficial = cpu.bus.read(0x0003);

        assert_eq!(official, 0, "official opcodes test returned error code ${official:02X}");
        assert_eq!(
            unofficial, 0,
            "unofficial opcodes test returned error code ${unofficial:02X}"
        );

        // Log comparison
        for (i, (ours, reference)) in our_lines.iter().zip(ref_lines.iter()).enumerate() {
            assert!(
                cpu_columns_match(ours, reference),
                "log mismatch at line {}:\n  OURS: {}\n  REF:  {}",
                i + 1,
                ours,
                reference
            );
        }
    }
}
