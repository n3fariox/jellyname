mod cli;
mod mkv;
mod models;
mod processor;
mod tmdb;
mod tui;
mod utils;

use clap::Parser;
use cli::Cli;
use color_eyre::eyre::Result;

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();
    tracing::info!(?cli, "jellyname-rs started");

    // TODO: wire up TUI
    println!("{cli:#?}");

    Ok(())
}
