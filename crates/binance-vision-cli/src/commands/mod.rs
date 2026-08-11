//! CLI command definitions.

mod clickhouse;
mod fetch;
mod mirror;
mod symbols;

use anyhow::Result;
use binance_vision::Config;
use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "binance-vision", version, about, long_about = None)]
pub struct Cli {
    #[arg(short, long, global = true)]
    pub verbose: bool,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Download a single Vision archive (debug).
    Fetch(fetch::FetchArgs),
    /// Mirror Vision archives to local parquet.zstd.
    Mirror(mirror::MirrorArgs),
    /// Refresh and list trading symbols.
    Symbols(symbols::SymbolsArgs),
    /// Load mirrored parquet into ClickHouse.
    Clickhouse(clickhouse::ClickhouseArgs),
}

pub async fn run(command: Commands, config: Config) -> Result<()> {
    match command {
        Commands::Fetch(args) => fetch::run(args, config).await,
        Commands::Mirror(args) => mirror::run(args, config).await,
        Commands::Symbols(args) => symbols::run(args, config).await,
        Commands::Clickhouse(args) => clickhouse::run(args, config).await,
    }
}
