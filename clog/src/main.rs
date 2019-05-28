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

fn main() -> Result<(), NoneError> {
    let style: HashMap<&str, String> = [
        ("text", "white".to_string()),
        ("time", "cyan".to_string()),
        ("source", "green".to_string()),
        ("thread", "cyan".to_string()),
        ("info", "yellow".to_string()),
        ("warning", "magenta".to_string()),
        ("error", "red".to_string()),
    ].iter().cloned().collect();

    let mut line = String::new();
    while let Ok(n) = std::io::stdin().read_line(&mut line) {
        if n == 0 {
            break;
        }
        if let Ok(fields) = crate::fuchsia::parse_line(&line) {
            for field in fields {
                match field {
                    Field::Plain(ft, text) => {
                        print!("{}", text.color(style.get(&ft as &str)?.as_str()))
                    }
                    Field::Space(ft, text) => {
                        print!(" {}", text.color(style.get(&ft as &str)?.as_str()))
                    }
                }
            }
            print!("\n");
        }
        line.clear();
    }

    Ok(())
}
