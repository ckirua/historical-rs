//! Fetch subcommand.

use anyhow::{Context, Result};
use binance_vision::{Config, DataType, Market, VisionFile, VisionHttpClient};
use chrono::NaiveDate;
use clap::Args;

#[derive(Debug, Args)]
pub struct FetchArgs {
    pub market: String,
    pub data_type: String,
    pub symbol: String,
    pub date: String,
    #[arg(long)]
    pub interval: Option<String>,
    #[arg(long)]
    pub verify_checksum: bool,
    #[arg(long)]
    pub output: Option<std::path::PathBuf>,
}

pub async fn run(args: FetchArgs, config: Config) -> Result<()> {
    let market = Market::parse(&args.market).context("invalid market")?;
    let data_type = DataType::parse(&args.data_type).context("invalid data type")?;
    let date =
        NaiveDate::parse_from_str(&args.date, "%Y-%m-%d").context("date must be YYYY-MM-DD")?;
    let file = VisionFile::daily(
        market,
        data_type,
        &args.symbol,
        date,
        args.interval.as_deref(),
    );
    let http = VisionHttpClient::new(config)?;
    let url = http.url_builder().build(&file);
    println!("GET {url}");
    let (bytes, verified) = if args.verify_checksum {
        http.download_with_checksum(&url).await?
    } else {
        (http.get_bytes(&url).await?, false)
    };
    println!("bytes={} checksum_verified={verified}", bytes.len());
    if let Some(output) = args.output {
        std::fs::write(&output, bytes).context("write output")?;
        println!("wrote {}", output.display());
    }
    Ok(())
}
