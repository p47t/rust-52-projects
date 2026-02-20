pub mod cb7;
pub mod cbr;
pub mod cbz;

use std::path::Path;

/// Returns true if `name` is an image filename we want to display as a comic page.
pub(crate) fn is_image_file(name: &str) -> bool {
    if name.ends_with('/') {
        return false;
    }
    matches!(
        Path::new(&name.to_lowercase())
            .extension()
            .and_then(|e| e.to_str()),
        Some("jpg" | "jpeg" | "png" | "webp" | "gif")
    )
}

/// Abstraction over comic book archive formats.
///
/// `extract_page` takes `&self` so implementations can be held behind `Arc<dyn ComicReader>`:
/// - CBZ: re-opens a `ZipArchive` over an in-memory `Cursor` on each call (fast for random access)
/// - CB7: pre-loads all page bytes at open time, returning a clone per call
/// - CBR: stub that always errors
pub trait ComicReader: Send + Sync + std::fmt::Debug {
    fn title(&self) -> &str;
    fn page_count(&self) -> usize;
    fn extract_page(&self, index: usize) -> Result<iced::widget::image::Handle, String>;
}

/// Open a comic archive at `path`, dispatching by file extension.
///
/// Returns `Box<dyn ComicReader>` so callers can wrap it in `Arc::from(box)`.
pub fn open(path: &Path) -> Result<Box<dyn ComicReader>, String> {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_lowercase)
        .as_deref()
    {
        Some("cbz") => cbz::CbzReader::open(path).map(|r| Box::new(r) as Box<dyn ComicReader>),
        Some("cbr") => cbr::CbrReader::open(path).map(|r| Box::new(r) as Box<dyn ComicReader>),
        Some("cb7") => cb7::Cb7Reader::open(path).map(|r| Box::new(r) as Box<dyn ComicReader>),
        ext => Err(format!(
            "Unsupported format: {}",
            ext.unwrap_or("none")
        )),
    }
}
