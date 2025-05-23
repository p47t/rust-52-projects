# Markdown Viewer

A simple command-line tool to display markdown files in the terminal.

## How to build

```bash
cargo build
```

## How to run

```bash
cargo run -- <path_to_markdown_file>
```

## Example usage

```bash
cargo run -- README.md
```
This will display the content of the `README.md` file itself, formatted in the terminal.

### Example Markdown File (`example.md`)

```markdown
# This is a heading

This is a paragraph with some **bold text** and some _italic text_.

    This is a code block.

Another paragraph.
`inline code`
---
* Item 1
* Item 2
```

### Expected Output for `example.md`

```
#This is a heading
This is a paragraph with some *bold text* and some _italic text_.
    This is a code block.
Another paragraph.
`inline code`
---
* Item 1
* Item 2
```
