use pulldown_cmark::{html, Options, Parser};
use serde::Serialize;

/// Statistics about the markdown document
#[derive(Serialize)]
pub struct Statistics {
    pub characters: usize,
    pub characters_no_spaces: usize,
    pub words: usize,
    pub lines: usize,
    pub paragraphs: usize,
    pub reading_time_minutes: f64,
}

/// Parse markdown text and convert it to HTML
pub fn parse_markdown(markdown: &str) -> String {
    // Set up options for parsing
    let mut options = Options::empty();
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_FOOTNOTES);
    options.insert(Options::ENABLE_TASKLISTS);
    options.insert(Options::ENABLE_HEADING_ATTRIBUTES);

    // Parse the markdown
    let parser = Parser::new_ext(markdown, options);

    // Render to HTML
    let mut html_output = String::new();
    html::push_html(&mut html_output, parser);

    html_output
}

/// Count words in text
pub fn count_words(text: &str) -> usize {
    text.split_whitespace().count()
}

/// Calculate comprehensive statistics about the text
pub fn calculate_stats(text: &str) -> Statistics {
    let characters = text.chars().count();
    let characters_no_spaces = text.chars().filter(|c| !c.is_whitespace()).count();
    let words = count_words(text);
    let lines = text.lines().count();

    // Count paragraphs (groups of non-empty lines)
    let paragraphs = text.split("\n\n").filter(|s| !s.trim().is_empty()).count();

    // Reading time: average reading speed is ~200 words per minute
    let reading_time_minutes = if words > 0 {
        (words as f64 / 200.0).ceil()
    } else {
        0.0
    };

    Statistics {
        characters,
        characters_no_spaces,
        words,
        lines,
        paragraphs,
        reading_time_minutes,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_markdown_parsing() {
        let markdown = "# Hello\n\nThis is **bold** text.";
        let html = parse_markdown(markdown);
        assert!(html.contains("<h1>Hello</h1>"));
        assert!(html.contains("<strong>bold</strong>"));
    }

    #[test]
    fn test_word_count() {
        assert_eq!(count_words("hello world"), 2);
        assert_eq!(count_words("  hello   world  "), 2);
        assert_eq!(count_words(""), 0);
    }

    #[test]
    fn test_statistics() {
        let text = "Hello world.\n\nThis is a test.";
        let stats = calculate_stats(text);
        assert_eq!(stats.words, 6);
        assert_eq!(stats.lines, 3);
        assert_eq!(stats.paragraphs, 2);
    }
}
