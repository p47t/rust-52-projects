import init, {
    markdown_to_html,
    get_statistics,
    count_words,
    count_characters,
    reading_time
} from '../pkg/wasm_markdown_editor.js';

// Sample markdown for demonstration
const SAMPLE_MARKDOWN = `# Welcome to WASM Markdown Editor! 🎉

This is a **WebAssembly-powered** markdown editor built with *Rust*!

## Features

- ⚡ **Blazing fast** markdown parsing using \`pulldown-cmark\`
- 🔄 **Live preview** as you type
- 📊 **Real-time statistics** (word count, reading time)
- 💾 **Export to HTML**
- 🎨 **Clean, modern interface**

## Code Example

Here's some Rust code:

\`\`\`rust
#[wasm_bindgen]
pub fn markdown_to_html(markdown: &str) -> String {
    parser::parse_markdown(markdown)
}
\`\`\`

## Why Rust + WASM?

1. **Performance**: Near-native speed in the browser
2. **Safety**: Memory safety without garbage collection
3. **Portability**: Write once, run everywhere

## Markdown Features

This editor supports:

- **Bold** and *italic* text
- [Links](https://www.rust-lang.org/)
- \`inline code\`
- Code blocks with syntax
- Lists (ordered and unordered)
- Tables
- Blockquotes
- ~~Strikethrough~~
- And more!

> "Rust is the future of WebAssembly" - Someone, probably

## Try It Out!

Start editing this text or click "Clear" to start fresh. Your work is automatically saved in your browser's local storage!

---

**Happy writing!** 📝
`;

// State
let wasmModule = null;
let debounceTimer = null;
const DEBOUNCE_DELAY = 300; // ms

// DOM elements
const markdownInput = document.getElementById('markdownInput');
const preview = document.getElementById('preview');
const clearBtn = document.getElementById('clearBtn');
const exportBtn = document.getElementById('exportBtn');
const loadSampleBtn = document.getElementById('loadSampleBtn');
const wordCount = document.getElementById('wordCount');
const charCount = document.getElementById('charCount');
const readTime = document.getElementById('readTime');

// Local storage key
const STORAGE_KEY = 'wasm-markdown-editor-content';

/**
 * Initialize the WASM module and set up event listeners
 */
async function run() {
    try {
        // Initialize WASM module
        wasmModule = await init();
        console.log('WASM module loaded successfully!');

        // Load saved content from localStorage
        loadFromStorage();

        // Set up event listeners
        markdownInput.addEventListener('input', handleInput);
        clearBtn.addEventListener('click', handleClear);
        exportBtn.addEventListener('click', handleExport);
        loadSampleBtn.addEventListener('click', handleLoadSample);

        // Initial render
        updatePreview();
        updateStatistics();

    } catch (err) {
        console.error('Failed to initialize WASM module:', err);
        preview.innerHTML = `
            <div style="color: red; padding: 20px;">
                <h3>Error Loading WASM Module</h3>
                <p>${err.message}</p>
                <p>Make sure you've built the WASM module with <code>wasm-pack build --target web</code></p>
            </div>
        `;
    }
}

/**
 * Handle input changes with debouncing
 */
function handleInput() {
    // Clear existing timer
    if (debounceTimer) {
        clearTimeout(debounceTimer);
    }

    // Update statistics immediately (they're fast)
    updateStatistics();

    // Debounce the preview update
    debounceTimer = setTimeout(() => {
        updatePreview();
        saveToStorage();
    }, DEBOUNCE_DELAY);
}

/**
 * Update the preview pane with rendered HTML
 */
function updatePreview() {
    const markdown = markdownInput.value;

    if (!markdown.trim()) {
        preview.innerHTML = '<p class="placeholder">Preview will appear here...</p>';
        return;
    }

    try {
        const html = markdown_to_html(markdown);
        preview.innerHTML = html;
    } catch (err) {
        console.error('Error rendering markdown:', err);
        preview.innerHTML = `<p style="color: red;">Error rendering markdown: ${err.message}</p>`;
    }
}

/**
 * Update statistics display
 */
function updateStatistics() {
    const text = markdownInput.value;

    try {
        // Option 1: Use the comprehensive stats function
        const stats = get_statistics(text);
        wordCount.textContent = `Words: ${stats.words}`;
        charCount.textContent = `Characters: ${stats.characters_no_spaces}`;
        readTime.textContent = `Reading time: ${stats.reading_time_minutes} min`;

        // Option 2: Use individual functions (commented out)
        // const words = count_words(text);
        // const chars = count_characters(text);
        // const time = reading_time(text);
        // wordCount.textContent = `Words: ${words}`;
        // charCount.textContent = `Characters: ${chars}`;
        // readTime.textContent = `Reading time: ${time} min`;

    } catch (err) {
        console.error('Error calculating statistics:', err);
    }
}

/**
 * Clear the editor
 */
function handleClear() {
    if (markdownInput.value && !confirm('Are you sure you want to clear the editor?')) {
        return;
    }

    markdownInput.value = '';
    updatePreview();
    updateStatistics();
    saveToStorage();
    markdownInput.focus();
}

/**
 * Export the rendered HTML
 */
function handleExport() {
    const markdown = markdownInput.value;

    if (!markdown.trim()) {
        alert('Nothing to export! Write some markdown first.');
        return;
    }

    try {
        const html = markdown_to_html(markdown);

        // Create a complete HTML document
        const fullHTML = `<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Exported Markdown</title>
    <style>
        body {
            max-width: 800px;
            margin: 40px auto;
            padding: 20px;
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Oxygen, Ubuntu, Cantarell, sans-serif;
            line-height: 1.6;
            color: #333;
        }
        code {
            background: #f4f4f4;
            padding: 2px 6px;
            border-radius: 3px;
            font-family: Monaco, 'Courier New', monospace;
        }
        pre {
            background: #f4f4f4;
            padding: 15px;
            border-radius: 5px;
            overflow-x: auto;
        }
        pre code {
            background: none;
            padding: 0;
        }
        blockquote {
            border-left: 4px solid #e67e22;
            padding-left: 1rem;
            margin-left: 0;
            color: #666;
        }
    </style>
</head>
<body>
${html}
</body>
</html>`;

        // Create a download link
        const blob = new Blob([fullHTML], { type: 'text/html' });
        const url = URL.createObjectURL(blob);
        const a = document.createElement('a');
        a.href = url;
        a.download = 'exported-markdown.html';
        document.body.appendChild(a);
        a.click();
        document.body.removeChild(a);
        URL.revokeObjectURL(url);

        console.log('HTML exported successfully!');
    } catch (err) {
        console.error('Error exporting HTML:', err);
        alert('Error exporting HTML. Check the console for details.');
    }
}

/**
 * Load sample markdown
 */
function handleLoadSample() {
    if (markdownInput.value && !confirm('This will replace your current content. Continue?')) {
        return;
    }

    markdownInput.value = SAMPLE_MARKDOWN;
    updatePreview();
    updateStatistics();
    saveToStorage();
}

/**
 * Save content to localStorage
 */
function saveToStorage() {
    try {
        localStorage.setItem(STORAGE_KEY, markdownInput.value);
    } catch (err) {
        console.error('Error saving to localStorage:', err);
    }
}

/**
 * Load content from localStorage
 */
function loadFromStorage() {
    try {
        const saved = localStorage.getItem(STORAGE_KEY);
        if (saved) {
            markdownInput.value = saved;
        } else {
            // Load sample on first visit
            markdownInput.value = SAMPLE_MARKDOWN;
        }
    } catch (err) {
        console.error('Error loading from localStorage:', err);
    }
}

// Keyboard shortcuts
document.addEventListener('keydown', (e) => {
    // Ctrl/Cmd + S to save (export)
    if ((e.ctrlKey || e.metaKey) && e.key === 's') {
        e.preventDefault();
        handleExport();
    }

    // Ctrl/Cmd + K to clear
    if ((e.ctrlKey || e.metaKey) && e.key === 'k') {
        e.preventDefault();
        handleClear();
    }
});

// Start the application
run();
