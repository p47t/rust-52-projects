#![feature(try_trait)]

use std::collections::HashMap;
use std::option::NoneError;

use colored::*;

mod fuchsia;

struct StyleSheet<'a> {
    inner: HashMap<&'a str, &'a str>,
}

impl<'a> StyleSheet<'a> {
    fn new(init: Vec<(&'a str, &'a str)>) -> Self {
        StyleSheet { inner: init.iter().cloned().collect() }
    }

    fn get(&self, class: &str) -> Result<&str, NoneError> {
        Ok(*self.inner.get(class)?)
    }
}

#[derive(PartialEq, Debug)]
pub struct Field<'a> {
    prefix: &'static str,
    class: &'static str,
    content: &'a str,
    postfix: &'static str,
}

impl<'a> Field<'a> {
    fn new(prefix: &'static str, class: &'static str, content: &'a str, postfix: &'static str) -> Self {
        Field { prefix, class, content, postfix }
    }

    fn pos(class: &'static str, content: &'a str, postfix: &'static str) -> Self {
        Field { prefix: "", class, content, postfix }
    }

    fn pre(prefix: &'static str, class: &'static str, content: &'a str) -> Self {
        Field { prefix, class, content, postfix: "" }
    }

    fn format(&self, style_sheet: &StyleSheet) -> Result<String, NoneError> {
        Ok(format!("{}{}{}",
                   self.prefix.color(style_sheet.get(".text")?),
                   self.content.color(style_sheet.get(self.class)?),
                   self.postfix.color(style_sheet.get(".text")?)))
    }
}

fn main() -> Result<(), NoneError> {
    let style_sheet = StyleSheet::new(vec![
        (".text", "white"),
        (".time", "cyan"),
        (".source", "green"),
        (".thread", "cyan"),
        (".info", "white"),
        (".warning", "magenta"),
        (".error", "red"),
    ]);

    let mut line = String::new();
    while let Ok(n) = std::io::stdin().read_line(&mut line) {
        if n == 0 {
            break;
        }
        match crate::fuchsia::parse_line(&line) {
            Ok(fields) => {
                for field in fields {
                    print!("{}", field.format(&style_sheet)?);
                }
                print!("\n");
            }
            Err(..) => {
                print!("{}", line); // print as it is
            }
        }
        line.clear();
    }

    Ok(())
}
