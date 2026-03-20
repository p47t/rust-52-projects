use crate::ines::Mirroring;

/// Trait for mappers that generate IRQs (e.g., MMC3 scanline counter).
pub trait MapperIrq {
    /// Clock the mapper's scanline counter. Called once per visible scanline.
    fn clock_scanline(&mut self);

    /// Check and clear the pending IRQ flag.
    fn take_irq(&mut self) -> bool;
}

/// Trait for NES cartridge mappers. Handles PRG-ROM/RAM banking (CPU side)
/// and CHR-ROM/RAM banking (PPU side), plus dynamic nametable mirroring.
pub trait Mapper {
    /// Read from PRG address space ($6000-$FFFF).
    fn read_prg(&self, addr: u16) -> u8;

    /// Write to PRG address space ($6000-$FFFF).
    /// $6000-$7FFF = PRG-RAM, $8000-$FFFF = bank switch registers.
    fn write_prg(&mut self, addr: u16, val: u8);

    /// Read from CHR address space ($0000-$1FFF).
    fn read_chr(&self, addr: u16) -> u8;

    /// Write to CHR address space ($0000-$1FFF). Only effective for CHR-RAM.
    fn write_chr(&mut self, addr: u16, val: u8);

    /// Current nametable mirroring mode (may change at runtime).
    fn mirroring(&self) -> Mirroring;

    /// Return a mutable reference to the IRQ interface, if this mapper supports it.
    fn as_irq(&mut self) -> Option<&mut dyn MapperIrq> {
        None
    }

    /// Serialize all mapper-specific state into a byte blob for save states.
    fn save_state(&self) -> Vec<u8>;

    /// Restore mapper-specific state from a byte blob produced by `save_state`.
    fn load_state(&mut self, data: &[u8]);
}
