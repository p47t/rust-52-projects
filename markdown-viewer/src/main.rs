use std::env;
use std::fs;
use pulldown_cmark::{Parser, Event, Tag};

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        eprintln!("Usage: markdown-viewer <markdown_file>");
        return;
    }

    let filepath = &args[1];
    let markdown_input = match fs::read_to_string(filepath) {
        Ok(content) => content,
        Err(e) => {
            eprintln!("Error reading file {}: {}", filepath, e);
            return;
        }
    };

    let parser = Parser::new(&markdown_input);
    let mut in_bold = false;
    let mut in_italic = false;

    for event in parser {
        match event {
            Event::Start(tag) => {
                match tag {
                    Tag::Heading(level, _, _) => {
                        print!("{}", "#".repeat(level as usize));
                    }
                    Tag::Strong => {
                        print!("*");
                        in_bold = true;
                    }
                    Tag::Emphasis => {
                        print!("_");
                        in_italic = true;
                    }
                    Tag::CodeBlock(_) => {
                        print!("    ");
                    }
                    Tag::Paragraph => {
                        // Handled by Text event for now, might need adjustment
                    }
                    _ => {} // Ignore other tags for now
                }
            }
            Event::End(tag) => {
                match tag {
                    Tag::Heading(_, _, _) => {
                        println!(); // Add a newline after headings
                    }
                    Tag::Strong => {
                        if in_bold {
                            print!("*");
                            in_bold = false;
                        }
                    }
                    Tag::Emphasis => {
                        if in_italic {
                            print!("_");
                            in_italic = false;
                        }
                    }
                    Tag::CodeBlock(_) => {
                        println!(); // Add a newline after code blocks
                    }
                    Tag::Paragraph => {
                         println!(); // Add a newline after paragraphs
                    }
                    _ => {} // Ignore other tags for now
                }
            }
            Event::Text(text) => {
                print!("{}", text);
            }
            Event::Code(text) => { // For inline code
                print!("`{}`", text);
            }
            Event::Html(_) => {} // Ignore HTML for now
            Event::FootnoteReference(_) => {} // Ignore FootnoteReference for now
            Event::SoftBreak => {
                print!(" "); // Replace soft breaks with a space
            }
            Event::HardBreak => {
                println!(); // Replace hard breaks with a newline
            }
            Event::Rule => {
                println!("---"); // Print a rule
            }
            Event::TaskListMarker(_) => {} // Ignore TaskListMarker for now
        }
    }
    println!(); // Ensure a final newline
}
