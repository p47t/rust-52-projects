mod api;
mod chat_template;
mod cli;
mod engine_service;
mod hf;
mod inference;
mod registry;


use anyhow::Result;
use clap::Parser;

#[tokio::main]
async fn main() -> Result<()> {
    cli::Cli::parse().run().await
}

