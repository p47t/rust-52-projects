#![allow(unused_variables)]

#[macro_use]
extern crate nom;

use rustyline::Editor;
use failure::Error;
use std::process::ExitStatus;
use std::str::from_utf8;

named!(pipe, is_a!("|"));
named!(unquoted_arg, is_not!(" \t\r\n'|"));
named!(single_quoted_arg, delimited!(tag!("'"), take_until!("'"), tag!("'")));
named!(arg, delimited!(nom::space0, alt!(unquoted_arg | single_quoted_arg), nom::space0));

named!(command<&[u8], Command>,
    do_parse!(
        args: many1!(arg) >>
        ({
            let args: Vec<String> = args.iter().map(|s| from_utf8(s).unwrap().to_string()).collect();
            Command {
                program: args.first().unwrap().to_string(),
                args: args[1..].to_vec(),
            }
        })
    )
);

#[derive(Debug)]
struct Command {
    program: String,
    args: Vec<String>,
}

impl Command {
    fn execute(&self) -> Result<ExitStatus, Error> {
        let mut child = std::process::Command::new(&self.program).args(&self.args).spawn()?;
        Ok(child.wait()?)
    }
}

struct Parser;

impl Parser {
    fn new() -> Parser {
        Parser {}
    }

    fn parse(&self, line: String) -> Option<Command> {
        if let Ok((rest, command)) = command(line.as_bytes()) {
            Some(command)
        } else {
            None
        }
    }
}

fn main() {
    let mut rl = Editor::<()>::new();
    let parser = Parser::new();
    loop {
        if let Ok(cmdline) = rl.readline("> ") {
            if let Some(command) = parser.parse(cmdline) {
                match command.program.as_str() {
                    "exit" | "quit" => {
                        break;
                    }
                    _ => {
                        println!("{:?}", command);
                        if let Ok(exit_status) = command.execute() {
                            println!("exit_status = {:?}", exit_status.code());
                        }
                    }
                }
            }
        }
    }
}
