//! CLI command definitions.

mod fetch;
mod mirror;
mod symbols;

use anyhow::Result;
use clap::{Parser, Subcommand};
use okx_historical::Config;

#[derive(Debug, Parser)]
#[command(name = "okx-historical", version, about, long_about = None)]
pub struct Cli {
    #[arg(short, long, global = true)]
    pub verbose: bool,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Download a single CDN archive (debug).
    Fetch(fetch::FetchArgs),
    /// Mirror CDN archives to local parquet.zstd.
    Mirror(mirror::MirrorArgs),
    /// Refresh and list instruments.
    Symbols(symbols::SymbolsArgs),
}

pub async fn run(command: Commands, config: Config) -> Result<()> {
    match command {
        Commands::Fetch(args) => fetch::run(args, config).await,
        Commands::Mirror(args) => mirror::run(args, config).await,
        Commands::Symbols(args) => symbols::run(args, config).await,
    }
}
