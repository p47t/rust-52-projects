#![allow(unused_variables)]

use rustyline::Editor;
use failure::Error;
use std::process::ExitStatus;

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
        let args: Vec<String> = line.split_whitespace().map(|s| s.to_string()).collect();
        if let Some(program) = args.first() {
            Some(Command {
                program: program.to_string(),
                args: args[1..].to_vec(),
            })
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
