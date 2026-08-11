//! OKX historical CLI entry point.

mod commands;
mod logging;

use anyhow::Result;
use clap::Parser;
use okx_historical::Config;

use crate::commands::Cli;

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    logging::init(cli.verbose)?;
    let config = Config::load()?;
    commands::run(cli.command, config).await
}
