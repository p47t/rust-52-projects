use std::io::Read;
use std::path::Path;

use iced::widget::image;

#[derive(Debug)]
pub struct CbzReader {
    title: String,
    pages: Vec<String>,       // sorted image filenames within the archive
    archive_bytes: Vec<u8>,   // entire .cbz file bytes, held for random-access extraction
}

impl CbzReader {
    pub fn open(path: &Path) -> Result<Self, String> {
        let archive_bytes = std::fs::read(path).map_err(|e| e.to_string())?;
        let cursor = std::io::Cursor::new(&archive_bytes);
        let mut archive = zip::ZipArchive::new(cursor).map_err(|e| e.to_string())?;

        let mut pages: Vec<String> = (0..archive.len())
            .filter_map(|i| {
                let file = archive.by_index(i).ok()?;
                let name = file.name().to_string();
                super::is_image_file(&name).then_some(name)
            })
            .collect();

        if pages.is_empty() {
            return Err("No image files found in archive".to_string());
        }

        pages.sort();

        let title = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Unknown")
            .to_string();

        Ok(Self {
            title,
            pages,
            archive_bytes,
        })
    }
}

impl super::ComicReader for CbzReader {
    fn title(&self) -> &str {
        &self.title
    }

    fn page_count(&self) -> usize {
        self.pages.len()
    }

    fn extract_page(&self, index: usize) -> Result<image::Handle, String> {
        let filename = self
            .pages
            .get(index)
            .ok_or_else(|| format!("Page index {index} out of bounds"))?;
        let cursor = std::io::Cursor::new(&self.archive_bytes);
        let mut archive = zip::ZipArchive::new(cursor).map_err(|e| e.to_string())?;
        let mut file = archive.by_name(filename).map_err(|e| e.to_string())?;
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes).map_err(|e| e.to_string())?;
        Ok(image::Handle::from_bytes(bytes))
    }
}
