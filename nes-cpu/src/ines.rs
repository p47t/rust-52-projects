#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Mirroring {
    Horizontal,
    Vertical,
    FourScreen,
}

#[derive(Debug)]
#[allow(dead_code)] // chr_rom/has_battery stored for future PPU/save support
pub struct INesRom {
    pub prg_rom: Vec<u8>,
    pub chr_rom: Vec<u8>,
    pub mapper: u8,
    pub mirroring: Mirroring,
    pub has_battery: bool,
}

impl INesRom {
    pub fn load(path: &str) -> anyhow::Result<Self> {
        let bytes = std::fs::read(path)?;
        Self::parse(&bytes)
    }

    pub fn parse(bytes: &[u8]) -> anyhow::Result<Self> {
        anyhow::ensure!(
            bytes.len() >= 16 && &bytes[0..4] == b"NES\x1a",
            "Not a valid iNES file"
        );

        let prg_banks = bytes[4] as usize;
        let chr_banks = bytes[5] as usize;
        let flags6 = bytes[6];
        let flags7 = bytes[7];

        let has_trainer = flags6 & 0x04 != 0;
        let has_battery = flags6 & 0x02 != 0;

        let mirroring = if flags6 & 0x08 != 0 {
            Mirroring::FourScreen
        } else if flags6 & 0x01 != 0 {
            Mirroring::Vertical
        } else {
            Mirroring::Horizontal
        };

        let mapper = (flags6 >> 4) | (flags7 & 0xF0);

        let prg_start = 16 + if has_trainer { 512 } else { 0 };
        let prg_len = prg_banks * 16384;
        let chr_start = prg_start + prg_len;
        let chr_len = chr_banks * 8192;

        anyhow::ensure!(
            bytes.len() >= chr_start + chr_len,
            "iNES file too short: expected {} bytes, got {}",
            chr_start + chr_len,
            bytes.len()
        );

        let prg_rom = bytes[prg_start..prg_start + prg_len].to_vec();
        let chr_rom = bytes[chr_start..chr_start + chr_len].to_vec();

        Ok(Self {
            prg_rom,
            chr_rom,
            mapper,
            mirroring,
            has_battery,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_ines(prg_banks: u8, chr_banks: u8, flags6: u8, flags7: u8) -> Vec<u8> {
        let mut header = vec![0x4E, 0x45, 0x53, 0x1A]; // "NES\x1A"
        header.push(prg_banks);
        header.push(chr_banks);
        header.push(flags6);
        header.push(flags7);
        header.resize(16, 0); // pad rest of header

        let prg_len = prg_banks as usize * 16384;
        let chr_len = chr_banks as usize * 8192;
        header.resize(16 + prg_len + chr_len, 0xAA);
        header
    }

    #[test]
    fn test_parse_basic() {
        let data = make_ines(1, 0, 0x00, 0x00);
        let rom = INesRom::parse(&data).unwrap();
        assert_eq!(rom.prg_rom.len(), 16384);
        assert!(rom.chr_rom.is_empty());
        assert_eq!(rom.mapper, 0);
        assert_eq!(rom.mirroring, Mirroring::Horizontal);
        assert!(!rom.has_battery);
    }

    #[test]
    fn test_parse_vertical_mirroring() {
        let data = make_ines(1, 1, 0x01, 0x00);
        let rom = INesRom::parse(&data).unwrap();
        assert_eq!(rom.mirroring, Mirroring::Vertical);
        assert_eq!(rom.chr_rom.len(), 8192);
    }

    #[test]
    fn test_parse_four_screen() {
        let data = make_ines(1, 0, 0x08, 0x00);
        let rom = INesRom::parse(&data).unwrap();
        assert_eq!(rom.mirroring, Mirroring::FourScreen);
    }

    #[test]
    fn test_parse_mapper() {
        // Mapper 4 (MMC3): flags6 low nibble = 4, flags7 high nibble = 0
        let data = make_ines(1, 0, 0x40, 0x00);
        let rom = INesRom::parse(&data).unwrap();
        assert_eq!(rom.mapper, 4);

        // Mapper 66: flags6 high nibble = 2, flags7 high nibble = 4 → (0x20 >> 4) | 0x40 = 0x42 = 66
        let data = make_ines(1, 0, 0x20, 0x40);
        let rom = INesRom::parse(&data).unwrap();
        assert_eq!(rom.mapper, 66);
    }

    #[test]
    fn test_parse_battery() {
        let data = make_ines(1, 0, 0x02, 0x00);
        let rom = INesRom::parse(&data).unwrap();
        assert!(rom.has_battery);
    }

    #[test]
    fn test_parse_with_trainer() {
        let mut data = make_ines(1, 0, 0x04, 0x00); // trainer flag
        // Insert 512-byte trainer between header and PRG
        data.splice(16..16, vec![0xBB; 512]);
        let rom = INesRom::parse(&data).unwrap();
        assert_eq!(rom.prg_rom.len(), 16384);
    }

    #[test]
    fn test_parse_invalid_magic() {
        let data = vec![0x00; 32];
        assert!(INesRom::parse(&data).is_err());
    }

    #[test]
    fn test_parse_truncated() {
        let data = make_ines(2, 0, 0x00, 0x00);
        // Truncate so it's shorter than expected
        let truncated = &data[..data.len() - 100];
        assert!(INesRom::parse(truncated).is_err());
    }
}
