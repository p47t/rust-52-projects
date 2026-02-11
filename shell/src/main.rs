use std::io::Error;
use std::process::{Child, Stdio};
use std::str::from_utf8;

use nom::{
    branch::alt,
    bytes::complete::{is_a, is_not, tag, take_until},
    character::complete::space0,
    combinator::map,
    multi::many1,
    sequence::{delimited, tuple},
    IResult,
};
use rustyline::{Editor, Result as RustylineResult};

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
        std::process::Command::new(&self.program)
            .stdin(cin)
            .stdout(cout)
            .args(&self.args)
            .spawn()
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
        let right = self
            .right
            .execute(Stdio::from(left.stdout.take().unwrap()), cout)?;
        let mut children = vec![left];
        children.extend(right);
        Ok(children)
    }
}

// Parser functions
fn pipe(input: &[u8]) -> IResult<&[u8], &[u8]> {
    is_a("|")(input)
}

fn unquoted_arg(input: &[u8]) -> IResult<&[u8], &[u8]> {
    is_not(" \t\r\n'|")(input)
}

fn single_quoted_arg(input: &[u8]) -> IResult<&[u8], &[u8]> {
    delimited(tag("'"), take_until("'"), tag("'"))(input)
}

fn arg(input: &[u8]) -> IResult<&[u8], &[u8]> {
    delimited(space0, alt((unquoted_arg, single_quoted_arg)), space0)(input)
}

fn command(input: &[u8]) -> IResult<&[u8], Command> {
    map(many1(arg), |args| {
        let args: Vec<&str> = args.iter().map(|bytes| from_utf8(bytes).unwrap()).collect();
        Command {
            program: args.first().unwrap().to_string(),
            args: args[1..].iter().map(|str| str.to_string()).collect(),
        }
    })(input)
}

fn pipeline(input: &[u8]) -> IResult<&[u8], Box<dyn Executable>> {
    map(tuple((command, pipe, cmdline)), |(left, _, right)| {
        Box::new(Pipeline { left, right }) as Box<dyn Executable>
    })(input)
}

fn cmdline(input: &[u8]) -> IResult<&[u8], Box<dyn Executable>> {
    alt((
        pipeline,
        map(command, |c| Box::new(c) as Box<dyn Executable>),
    ))(input)
}

fn parse_and_execute(line: &str) {
    if line.trim().is_empty() {
        return;
    }
    match cmdline(line.as_bytes()) {
        Ok((_, exe)) => match exe.execute(Stdio::inherit(), Stdio::inherit()) {
            Err(why) => eprintln!("Failed to execute: {}", why),
            Ok(children) => {
                for mut child in children {
                    let _ = child.wait();
                }
            }
        },
        Err(why) => eprintln!("Failed to parse: {}", why),
    }
}

fn main() -> RustylineResult<()> {
    let mut rl = Editor::<()>::new()?;
    loop {
        let readline = rl.readline("> ");
        if let Ok(line) = readline {
            parse_and_execute(&line);
        }
    }
}
