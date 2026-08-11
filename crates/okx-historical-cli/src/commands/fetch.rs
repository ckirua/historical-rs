//! Fetch subcommand.

use anyhow::{Context, Result};
use chrono::NaiveDate;
use clap::Args;
use okx_historical::{Config, DataType, Frequency, Market, OkxFile, OkxHttpClient};

#[derive(Debug, Args)]
pub struct FetchArgs {
    pub market: String,
    pub data_type: String,
    pub symbol: String,
    /// YYYY-MM-DD (daily) or YYYY-MM (monthly borrow).
    pub date: String,
    #[arg(long)]
    pub output: Option<std::path::PathBuf>,
}

pub async fn run(args: FetchArgs, config: Config) -> Result<()> {
    let market = Market::parse(&args.market).context("invalid market")?;
    let data_type = DataType::parse(&args.data_type).context("invalid data type")?;
    let file = if args.date.len() == 7 {
        let year: i32 = args.date[0..4].parse().context("invalid year")?;
        let month: u32 = args.date[5..7].parse().context("invalid month")?;
        OkxFile {
            market,
            frequency: Frequency::Monthly,
            data_type,
            symbol: args.symbol,
            period: format!("{year:04}-{month:02}"),
        }
    } else {
        let date = NaiveDate::parse_from_str(&args.date, "%Y-%m-%d")
            .context("date must be YYYY-MM-DD or YYYY-MM")?;
        OkxFile::daily(market, data_type, args.symbol, date)
    };
    let http = OkxHttpClient::new(config)?;
    let url = http.url_builder().build(&file);
    println!("GET {url}");
    let bytes = http.get_bytes(&url).await?;
    println!("bytes={}", bytes.len());
    if let Some(output) = args.output {
        std::fs::write(&output, bytes).context("write output")?;
        println!("wrote {}", output.display());
    }
    Ok(())
}
