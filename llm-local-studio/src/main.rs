mod cli;
mod hf;
mod inference;
mod registry;

use anyhow::Result;
use clap::Parser;

fn main() -> Result<()> {
    cli::Cli::parse().run()
}
