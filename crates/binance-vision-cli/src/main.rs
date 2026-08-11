//! Binance Vision CLI entry point.

mod commands;
mod logging;

use anyhow::Result;
use binance_vision::Config;
use clap::Parser;

use crate::commands::Cli;

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    logging::init(cli.verbose)?;
    let config = Config::load()?;
    commands::run(cli.command, config).await
}
