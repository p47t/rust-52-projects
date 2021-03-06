#[macro_use]
extern crate nom;

use std::io::Error;
use std::process::{Child, Stdio};
use std::str::from_utf8;

use rustyline::Editor;

trait Executable {
    fn execute(&self, cin: Stdio, cout: Stdio) -> Result<Vec<Child>, Error>;
}

#[derive(Debug)]
struct Command {
    program: String,
    args: Vec<String>,
}

impl Command {
    fn execute(&self, cin: Stdio, cout: Stdio) -> Result<Child, Error> {
        Ok(std::process::Command::new(&self.program)
            .stdin(cin).stdout(cout).args(&self.args).spawn()?)
    }
}

impl Executable for Command {
    fn execute(&self, cin: Stdio, cout: Stdio) -> Result<Vec<Child>, Error> {
        Ok(vec![self.execute(cin, cout)?])
    }
}

struct Pipeline {
    left: Command,
    right: Box<dyn Executable>,
}

impl Executable for Pipeline {
    fn execute(&self, cin: Stdio, cout: Stdio) -> Result<Vec<Child>, Error> {
        let mut left = self.left.execute(cin, Stdio::piped())?;
        let right = self.right.execute(Stdio::from(left.stdout.take().unwrap()), cout)?;
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
            let args: Vec<&str> = args.iter().map(|bytes| from_utf8(bytes).unwrap()).collect();
            Command {
                program: args.first().unwrap().to_string(),
                args: args[1..].iter().map(|str| str.to_string()).collect(),
            }
        })
    )
);

named!(cmdline<Box<dyn Executable>>, alt!(
    pipeline |
    command => {|c| Box::new(c) as Box<dyn Executable>}
));

named!(pipeline<Box<dyn Executable>>,
    do_parse!(
        left: command >>
        pipe >>
        right: cmdline >>
        (Box::new(Pipeline{left, right}) as Box<dyn Executable>)
    )
);

fn parse_and_execute(line: &str) {
    if line.trim().is_empty() {
        return;
    }
    match cmdline(line.as_bytes()) {
        Ok((_, exe)) => {
            match exe.execute(Stdio::inherit(), Stdio::inherit()) {
                Err(why) => eprintln!("Failed to execute: {}", why),
                Ok(children) => {
                    for mut child in children {
                        let _ = child.wait();
                    }
                }
            }
        }
        Err(why) => eprintln!("Failed to parse: {}", why)
    }
}

fn main() {
    let mut rl = Editor::<()>::new();
    loop {
        if let Ok(line) = rl.readline("> ") {
            parse_and_execute(&line);
        }
    }
}
