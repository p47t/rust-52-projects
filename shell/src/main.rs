#[macro_use]
extern crate nom;

use rustyline::{Editor};
use std::process::{Stdio, Child};
use std::str::from_utf8;
use std::io::Error;

trait Cmdline {
    fn execute(&self, stdin: Stdio, stdout: Stdio) -> Result<Vec<Child>, Error>;
}

#[derive(Debug)]
struct Command {
    program: String,
    args: Vec<String>,
}

impl Command {
    fn execute(&self, stdin: Stdio, stdout: Stdio) -> Result<Child, Error> {
        Ok(std::process::Command::new(&self.program).stdin(stdin).stdout(stdout).args(&self.args).spawn()?)
    }
}

impl Cmdline for Command {
    fn execute(&self, stdin: Stdio, stdout: Stdio) -> Result<Vec<Child>, Error> {
        Ok(vec![self.execute(stdin, stdout)?])
    }
}

struct Pipeline {
    left: Command,
    right: Box<dyn Cmdline>,
}

impl Cmdline for Pipeline {
    fn execute(&self, stdin: Stdio, stdout: Stdio) -> Result<Vec<Child>, Error> {
        let mut left = self.left.execute(stdin, Stdio::piped())?;
        let right = self.right.execute(Stdio::from(left.stdout.take().unwrap()), stdout)?;
        let mut children = vec![left];
        children.extend(right);
        Ok(children)
    }
}

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

fn parse_and_execute(line: &[u8]) {
    match cmdline(line) {
        Ok((_, cl)) => {
            match cl.execute(Stdio::inherit(), Stdio::inherit()) {
                Err(why) => eprintln!("Failed to execute: {}", why),
                Ok(mut children) => {
                    children.iter_mut().for_each(|child| {
                        let _ = child.wait();
                    });
                },
            }
        }
        Err(why) => eprintln!("Failed to parse: {}", why)
    }
}

fn main() {
    let mut rl = Editor::<()>::new();
    loop {
        if let Ok(line) = rl.readline("> ") {
            parse_and_execute(line.as_bytes());
        }
    }
}
