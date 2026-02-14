use std::env;
use std::process;

use tilesplit::{EXIT_USAGE, SplitParams, default_output_paths, run};

fn debug_enabled_from_env() -> bool {
    match env::var("TILESPLIT_DEBUG") {
        Ok(value) => {
            let normalized = value.trim().to_ascii_lowercase();
            !normalized.is_empty()
                && normalized != "0"
                && normalized != "false"
                && normalized != "no"
                && normalized != "off"
        }
        Err(_) => false,
    }
}

fn parse_args() -> Result<SplitParams, &'static str> {
    let mut input = None;
    let mut left_output = None;
    let mut right_output = None;
    let mut debug = false;

    let mut iter = env::args().skip(1);
    while let Some(flag) = iter.next() {
        match flag.as_str() {
            "--input" => input = iter.next(),
            "--left-output" => left_output = iter.next(),
            "--right-output" => right_output = iter.next(),
            "--debug" => debug = true,
            "--help" | "-h" => return Err("help"),
            _ => return Err("unknown"),
        }
    }

    if !debug {
        debug = debug_enabled_from_env();
    }

    match input {
        Some(input) => {
            let (default_left_output, default_right_output) = default_output_paths(&input);
            Ok(SplitParams {
                input,
                left_output: left_output.unwrap_or(default_left_output),
                right_output: right_output.unwrap_or(default_right_output),
                debug,
            })
        }
        None => Err("missing"),
    }
}

fn print_usage() {
    eprintln!("Usage:");
    eprintln!(
        "  tilesplit --input <path> [--left-output <path>] [--right-output <path>] [--debug]"
    );
    eprintln!("Defaults:");
    eprintln!("  --left-output  <input-stem>-left.jpg");
    eprintln!("  --right-output <input-stem>-right.jpg");
    eprintln!("Debug:");
    eprintln!("  --debug or TILESPLIT_DEBUG=1");
}

fn main() {
    let args = match parse_args() {
        Ok(args) => args,
        Err(_) => {
            print_usage();
            process::exit(EXIT_USAGE);
        }
    };

    match run(args) {
        Ok(()) => process::exit(0),
        Err(code) => process::exit(code),
    }
}
