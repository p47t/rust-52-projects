#![feature(try_trait)]

use std::collections::HashMap;
use std::option::NoneError;

use colored::*;

mod fuchsia;

#[derive(PartialEq, Debug)]
pub struct Field<'a> {
    prefix: &'static str,
    tag: &'static str,
    content: &'a str,
    postfix: &'static str
}

impl<'a> Field<'a> {
    fn new(prefix: &'static str, tag: &'static str, content: &'a str, postfix: &'static str) -> Self {
        Field{prefix, tag, content, postfix}
    }

    fn pos(tag: &'static str, content: &'a str, postfix: &'static str) -> Self {
        Field{prefix: "", tag, content, postfix}
    }

    fn pre(prefix: &'static str, tag: &'static str, content: &'a str) -> Self {
        Field{prefix, tag, content, postfix: ""}
    }

    fn format(&self, style: &HashMap<&str, &str>) -> Result<String, NoneError> {
        Ok(format!("{}{}{}",
                   self.prefix.color(*style.get("text")?),
                   self.content.color(*style.get(self.tag)?),
                   self.postfix.color(*style.get("text")?)))
    }
}

fn main() -> Result<(), NoneError> {
    let style: HashMap<&str, &str> = [
        ("text", "white"),
        ("time", "cyan"),
        ("source", "green"),
        ("thread", "cyan"),
        ("info", "white"),
        ("warning", "magenta"),
        ("error", "red"),
    ].iter().cloned().collect();

    let mut line = String::new();
    while let Ok(n) = std::io::stdin().read_line(&mut line) {
        if n == 0 {
            break;
        }
        if let Ok(fields) = crate::fuchsia::parse_line(&line) {
            for field in fields {
                print!("{}", field.format(&style)?);
            }
            print!("\n");
        }
        line.clear();
    }

    Ok(())
}
