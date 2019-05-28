#![feature(try_trait)]

use std::collections::HashMap;
use std::option::NoneError;

use colored::*;

mod fuchsia;

#[derive(PartialEq, Debug)]
pub enum Field<'a> {
    Plain(&'static str, &'a str),
    Space(&'static str, &'a str), // TODO: figure out a better name
}

impl<'a> Field<'a> {
    fn format(&self, style: &HashMap<&str, &str>) -> Result<String, NoneError> {
        match *self {
            Field::Plain(ft, text) => Ok(format!("{}", text.color(*style.get(ft)?))),
            Field::Space(ft, text) => Ok(format!(" {}", text.color(*style.get(ft)?))),
        }
    }
}

fn main() -> Result<(), NoneError> {
    let style: HashMap<&str, &str> = [
        ("text", "white"),
        ("time", "cyan"),
        ("source", "green"),
        ("thread", "cyan"),
        ("info", "yellow"),
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
