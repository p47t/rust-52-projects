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

named!(command<Command>,
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

named!(cmdline<Box<dyn Cmdline>>, alt!(
    pipeline |
    command => {|c| Box::new(c) as Box<dyn Cmdline>}
));

named!(pipeline<Box<dyn Cmdline>>,
    do_parse!(
        left: command >>
        pipe >>
        right: cmdline >>
        (Box::new(Pipeline{left, right}) as Box<dyn Cmdline>)
    )
);

trait Cmdline {
    fn execute(&self) -> Result<ExitStatus, Error>;
}

#[derive(Debug)]
struct Command {
    program: String,
    args: Vec<String>,
}

struct Pipeline {
    left: Command,
    right: Box<dyn Cmdline>,
}

impl Cmdline for Pipeline {
    fn execute(&self) -> Result<ExitStatus, Error> {
        self.left.execute();
        self.right.execute()
    }
}

impl Cmdline for Command {
    fn execute(&self) -> Result<ExitStatus, Error> {
        let mut child = std::process::Command::new(&self.program).args(&self.args).spawn()?;
        Ok(child.wait()?)
    }
}

fn main() {
    let mut rl = Editor::<()>::new();
    loop {
        if let Ok(line) = rl.readline("> ") {
            match cmdline(line.as_bytes()) {
                Ok((rest, cl)) => {
                    cl.execute();
                }
                _ => {}
            }
        }
    }
}
