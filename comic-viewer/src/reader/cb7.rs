use std::io::Read;
use std::path::Path;

use iced::widget::image;
use sevenz_rust::{Password, SevenZReader};

/// Reads CB7 (7-Zip) comic archives.
///
/// All image pages are pre-loaded into memory at open time because 7-Zip uses
/// block compression (LZMA/LZMA2) which does not support cheap random access
/// like ZIP does.
#[derive(Debug)]
pub struct Cb7Reader {
    title: String,
    /// Raw image bytes for each page, sorted by entry name at open time.
    pages: Vec<Vec<u8>>,
}

impl Cb7Reader {
    pub fn open(path: &Path) -> Result<Self, String> {
        let mut reader =
            SevenZReader::open(path, Password::empty()).map_err(|e| e.to_string())?;

        let mut entries: Vec<(String, Vec<u8>)> = Vec::new();

        reader
            .for_each_entries(|entry: &sevenz_rust::SevenZArchiveEntry, reader: &mut dyn Read| {
                if entry.is_directory() || !super::is_image_file(entry.name()) {
                    return Ok(true); // skip, continue
                }
                let mut bytes = Vec::new();
                reader.read_to_end(&mut bytes).map_err(sevenz_rust::Error::from)?;
                entries.push((entry.name().to_owned(), bytes));
                Ok(true)
            })
            .map_err(|e| e.to_string())?;

        if entries.is_empty() {
            return Err("No image files found in archive".to_string());
        }

        entries.sort_by(|(a, _), (b, _)| a.cmp(b));

        let title = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Unknown")
            .to_string();

        Ok(Self {
            title,
            pages: entries.into_iter().map(|(_, b)| b).collect(),
        })
    }
}

impl super::ComicReader for Cb7Reader {
    fn title(&self) -> &str {
        &self.title
    }

    fn page_count(&self) -> usize {
        self.pages.len()
    }

    fn extract_page(&self, index: usize) -> Result<image::Handle, String> {
        let bytes = self
            .pages
            .get(index)
            .ok_or_else(|| format!("Page index {index} out of bounds"))?;
        Ok(image::Handle::from_bytes(bytes.clone()))
    }
}
