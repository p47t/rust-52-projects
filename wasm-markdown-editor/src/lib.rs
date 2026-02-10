mod parser;
mod utils;

use wasm_bindgen::prelude::*;

/// Initialize the WASM module
/// Sets up panic hooks for better error messages in the browser console
#[wasm_bindgen(start)]
pub fn init() {
    utils::set_panic_hook();
}

/// Convert markdown text to HTML
///
/// # Arguments
/// * `markdown` - A string slice containing markdown text
///
/// # Returns
/// A String containing the rendered HTML
#[wasm_bindgen]
pub fn markdown_to_html(markdown: &str) -> String {
    parser::parse_markdown(markdown)
}

/// Get statistics about the markdown text
///
/// # Arguments
/// * `text` - A string slice containing markdown text
///
/// # Returns
/// A JsValue containing statistics (characters, words, lines, reading time)
#[wasm_bindgen]
pub fn get_statistics(text: &str) -> JsValue {
    let stats = parser::calculate_stats(text);
    serde_wasm_bindgen::to_value(&stats).unwrap()
}

/// Count words in text
/// Exported as a simple utility function
#[wasm_bindgen]
pub fn count_words(text: &str) -> usize {
    parser::count_words(text)
}

/// Count characters in text (excluding whitespace)
#[wasm_bindgen]
pub fn count_characters(text: &str) -> usize {
    text.chars().filter(|c| !c.is_whitespace()).count()
}

/// Estimate reading time in minutes (assuming 200 words per minute)
#[wasm_bindgen]
pub fn reading_time(text: &str) -> f64 {
    let word_count = parser::count_words(text);
    (word_count as f64 / 200.0).ceil()
}
