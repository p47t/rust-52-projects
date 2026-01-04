//! Test suite for the Web and headless browsers.

#![cfg(target_arch = "wasm32")]

extern crate wasm_bindgen_test;
use wasm_bindgen_test::*;
use wasm_markdown_editor::*;

wasm_bindgen_test_configure!(run_in_browser);

#[wasm_bindgen_test]
fn test_markdown_to_html() {
    let markdown = "# Hello\n\nThis is **bold**.";
    let html = markdown_to_html(markdown);

    assert!(html.contains("<h1>"));
    assert!(html.contains("Hello"));
    assert!(html.contains("<strong>"));
    assert!(html.contains("bold"));
}

#[wasm_bindgen_test]
fn test_count_words() {
    assert_eq!(count_words("hello world"), 2);
    assert_eq!(count_words(""), 0);
    assert_eq!(count_words("   hello   world   "), 2);
}

#[wasm_bindgen_test]
fn test_count_characters() {
    assert_eq!(count_characters("hello"), 5);
    assert_eq!(count_characters("hello world"), 10); // no spaces
    assert_eq!(count_characters(""), 0);
}

#[wasm_bindgen_test]
fn test_reading_time() {
    // 200 words should be 1 minute
    let text = (0..200).map(|_| "word").collect::<Vec<_>>().join(" ");
    assert_eq!(reading_time(&text), 1.0);

    // 400 words should be 2 minutes
    let text = (0..400).map(|_| "word").collect::<Vec<_>>().join(" ");
    assert_eq!(reading_time(&text), 2.0);
}

#[wasm_bindgen_test]
fn test_get_statistics() {
    use wasm_bindgen::JsValue;

    let text = "Hello world.\n\nThis is a test.";
    let stats = get_statistics(text);

    // Just verify it returns a valid JsValue without panicking
    assert!(!stats.is_null());
    assert!(!stats.is_undefined());
}

#[wasm_bindgen_test]
fn test_markdown_features() {
    // Test various markdown features
    let markdown = r#"
# Heading

**bold** *italic* ~~strikethrough~~

- List item 1
- List item 2

[link](https://example.com)

`code`

```rust
fn main() {}
```

> Blockquote
"#;

    let html = markdown_to_html(markdown);

    assert!(html.contains("<h1>"));
    assert!(html.contains("<strong>"));
    assert!(html.contains("<em>"));
    assert!(html.contains("<del>"));
    assert!(html.contains("<ul>"));
    assert!(html.contains("<li>"));
    assert!(html.contains("<a href"));
    assert!(html.contains("<code>"));
    assert!(html.contains("<pre>"));
    assert!(html.contains("<blockquote>"));
}
