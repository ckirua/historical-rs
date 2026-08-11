//! CLI command definitions.

mod discover;
mod fetch;
mod mirror;
mod pairs;

use anyhow::Result;
use clap::{Parser, Subcommand};
use kraken_historical::Config;

#[derive(Debug, Parser)]
#[command(name = "kraken-historical", version, about, long_about = None)]
pub struct Cli {
    #[arg(short, long, global = true)]
    pub verbose: bool,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    /// List Drive artifacts (complete + quarterly via public folder scrape).
    Discover(discover::DiscoverArgs),
    /// Download + convert one artifact window locally.
    Fetch(fetch::FetchArgs),
    /// Mirror Drive dumps to local parquet.zstd or S3.
    Mirror(mirror::MirrorArgs),
    /// List Kraken AssetPairs from the public REST API.
    Pairs(pairs::PairsArgs),
}

pub async fn run(command: Commands, config: Config) -> Result<()> {
    match command {
        Commands::Discover(args) => discover::run(args, config).await,
        Commands::Fetch(args) => fetch::run(args, config).await,
        Commands::Mirror(args) => mirror::run(args, config).await,
        Commands::Pairs(args) => pairs::run(args, config).await,
    }
}
