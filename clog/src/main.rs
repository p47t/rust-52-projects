use std::collections::HashMap;

use clap::{App, Arg};
use colored::*;

mod fuchsia;

#[derive(PartialEq, Debug)]
pub enum Field {
    Plain(String, String),
    Space(String, String), // TODO: figure out a better name
}

fn main() {
    let matches = App::new("clog")
        .version("1.0")
        .author("Patrick Tsai")
        .about("Color your log")
        .arg(Arg::with_name("format")
            .short("f")
            .long("format")
            .default_value("auto")
            .help("Specify the log format"))
        .get_matches();

    match matches.value_of("format").unwrap() {
        "auto" => println!("detect format automatically"),
        f => println!("use format {}", f),
    }

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
                        print!("{}", text.color(style.get(&ft as &str).unwrap().as_str()))
                    }
                    Field::Space(ft, text) => {
                        print!(" {}", text.color(style.get(&ft as &str).unwrap().as_str()))
                    }
                }
            }
            print!("\n");
        }
        line.clear();
    }
}
