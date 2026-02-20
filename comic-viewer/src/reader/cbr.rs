use std::path::Path;

use iced::widget::image;

/// Stub implementation for CBR (RAR) archives.
///
/// Full RAR support requires the unrar native library (unrar.dll / libunrar.so).
/// This implementation documents the pattern while returning a clear error at
/// open time so the user gets an actionable message.
#[derive(Debug)]
pub struct CbrReader {
    _private: (),
}

impl CbrReader {
    pub fn open(_path: &Path) -> Result<Self, String> {
        Err(
            "CBR (RAR) format is not yet supported. \
             Consider converting to CBZ using Calibre or 7-Zip."
                .to_string(),
        )
    }
}

impl super::ComicReader for CbrReader {
    fn title(&self) -> &str {
        ""
    }

    fn page_count(&self) -> usize {
        0
    }

    fn extract_page(&self, _index: usize) -> Result<image::Handle, String> {
        Err("CBR format is not supported".to_string())
    }
}
