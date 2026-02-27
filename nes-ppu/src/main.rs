use anyhow::{bail, Context};
use nes_cpu::ines::INesRom;
use nes_ppu::system::System;

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let rom_path = args
        .get(1)
        .context("Usage: nes-ppu <rom-path>\n  Place blargg ppu_vbl_nmi ROMs in roms/ directory")?;

    let rom = INesRom::load(rom_path)
        .with_context(|| format!("Failed to load ROM: {}", rom_path))?;

    println!("Loaded: {}", rom_path);
    println!(
        "PRG: {}KB, CHR: {}KB, Mapper: {}, Mirroring: {:?}",
        rom.prg_rom.len() / 1024,
        rom.chr_rom.len() / 1024,
        rom.mapper,
        rom.mirroring
    );

    let mut sys = System::from_rom(rom);

    // Blargg test protocol:
    //   $6000 = status: $80 = running, $00 = pass, $01+ = specific failure
    //   $6001-$6003 = signature: $DE $B0 $61 when result is valid
    //   $6004+ = null-terminated ASCII result message

    let timeout = 100_000_000u64; // ~100M CPU cycles
    let mut total_cycles = 0u64;
    let mut signature_seen = false;

    loop {
        total_cycles += sys.step();

        // Check for valid signature
        let sig0 = sys.cpu.bus.read(0x6001);
        let sig1 = sys.cpu.bus.read(0x6002);
        let sig2 = sys.cpu.bus.read(0x6003);

        if sig0 == 0xDE && sig1 == 0xB0 && sig2 == 0x61 {
            if !signature_seen {
                signature_seen = true;
            }

            let status = sys.cpu.bus.read(0x6000);
            if status != 0x80 {
                // Test has finished
                let message = read_result_message(&mut sys);
                if status == 0x00 {
                    println!("\nPASS (after {} CPU cycles)", total_cycles);
                    if !message.is_empty() {
                        println!("Message: {}", message);
                    }
                    return Ok(());
                } else {
                    println!("\nFAIL: status=${:02X} (after {} cycles)", status, total_cycles);
                    println!("Message: {}", message);
                    bail!("Test failed with status ${:02X}: {}", status, message);
                }
            }
        }

        if total_cycles > timeout {
            let status = sys.cpu.bus.read(0x6000);
            let message = if signature_seen {
                read_result_message(&mut sys)
            } else {
                String::from("(no signature detected)")
            };
            bail!(
                "Timeout after {} cycles. Status=$6000={:02X}, signature_seen={}, message: {}",
                total_cycles,
                status,
                signature_seen,
                message
            );
        }
    }
}

fn read_result_message(sys: &mut System) -> String {
    let mut msg = Vec::new();
    for i in 0..256u16 {
        let b = sys.cpu.bus.read(0x6004 + i);
        if b == 0 {
            break;
        }
        msg.push(b);
    }
    String::from_utf8_lossy(&msg).to_string()
}
