//! Discover subcommand.

use anyhow::Result;
use clap::Args;
use kraken_historical::{discover, Config, DataType};

#[derive(Debug, Args)]
pub struct DiscoverArgs {
    #[arg(long, value_name = "trades|ohlcvt")]
    pub data_type: Option<String>,
}

pub async fn run(args: DiscoverArgs, config: Config) -> Result<()> {
    let data_type = match args.data_type.as_deref() {
        Some(s) => Some(DataType::parse(s).ok_or_else(|| anyhow::anyhow!("invalid data type"))?),
        None => None,
    };
    let artifacts = discover(&config, data_type).await?;
    for a in &artifacts {
        let size = a
            .size_hint
            .map(|s| format!("{s}"))
            .unwrap_or_else(|| "-".into());
        println!(
            "{}\t{}\t{}\t{}\t{size}",
            a.data_type.as_str(),
            a.source.label(),
            a.file_id,
            a.name
        );
    }
    eprintln!("total={}", artifacts.len());
    Ok(())
}
