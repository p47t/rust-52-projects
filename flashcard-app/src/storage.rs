use std::path::{Path, PathBuf};

use crate::card::Deck;
use crate::error::AppError;
use crate::sample;

pub fn data_dir() -> PathBuf {
    #[cfg(target_os = "android")]
    {
        PathBuf::from("/data/data/com.example.flashcard_app/files")
    }
    #[cfg(not(target_os = "android"))]
    {
        let mut path = dirs::data_dir().unwrap_or_else(|| PathBuf::from("."));
        path.push("flashcard-app");
        path
    }
}

pub fn data_path() -> PathBuf {
    data_dir().join("decks.json")
}

pub fn load(path: &Path) -> Result<Vec<Deck>, AppError> {
    let data = std::fs::read_to_string(path)?;
    let decks = serde_json::from_str(&data)?;
    Ok(decks)
}

pub fn save(decks: &[Deck], path: &Path) -> Result<(), AppError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let data = serde_json::to_string_pretty(decks)?;
    std::fs::write(path, data)?;
    Ok(())
}

pub fn load_or_default(path: &Path) -> Vec<Deck> {
    match load(path) {
        Ok(decks) if !decks.is_empty() => decks,
        _ => sample::sample_decks(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_save_load() {
        let dir = std::env::temp_dir().join("flashcard-app-test");
        let path = dir.join("test_decks.json");
        let decks = sample::sample_decks();

        save(&decks, &path).unwrap();
        let loaded = load(&path).unwrap();

        assert_eq!(loaded.len(), decks.len());
        assert_eq!(loaded[0].name, decks[0].name);
        assert_eq!(loaded[0].cards.len(), decks[0].cards.len());

        // Cleanup
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn load_missing_file_returns_default() {
        let path = Path::new("/tmp/nonexistent_flashcard_test.json");
        let decks = load_or_default(path);
        assert_eq!(decks.len(), 2);
    }
}
