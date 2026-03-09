pub mod axrom;
pub mod cnrom;
pub mod mmc1;
pub mod nrom;
pub mod uxrom;

pub use nes_cpu::mapper::Mapper;

use nes_cpu::ines::INesRom;

/// Create the appropriate mapper for a ROM based on its iNES header.
pub fn from_rom(rom: &INesRom) -> anyhow::Result<Box<dyn Mapper>> {
    match rom.mapper {
        0 => Ok(Box::new(nrom::Nrom::new(rom))),
        1 => Ok(Box::new(mmc1::Mmc1::new(rom))),
        2 => Ok(Box::new(uxrom::UxRom::new(rom))),
        3 => Ok(Box::new(cnrom::Cnrom::new(rom))),
        7 => Ok(Box::new(axrom::AxRom::new(rom))),
        n => anyhow::bail!("Unsupported mapper: {n}"),
    }
}
