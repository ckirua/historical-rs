//! ClickHouse subcommands.

use anyhow::{Context, Result};
use binance_vision::{
    run_fetch_ingest, run_ingest, run_s3_ingest, BinanceRestClient, ClickHouseConfig, Config,
    FetchIngestOptions, IngestOptions, MirrorOptions, S3Config, SymbolCache, VisionHttpClient,
};
use chrono::NaiveDate;
use clap::{Args, Subcommand};

use super::mirror::RunArgs;

#[derive(Debug, Args)]
pub struct ClickhouseArgs {
    #[command(subcommand)]
    pub command: ClickhouseCommand,
}

#[derive(Debug, Subcommand)]
pub enum ClickhouseCommand {
    /// Ingest mirrored parquet files (local manifest or S3 bucket).
    Ingest(IngestArgs),
    /// Download Vision archives and insert directly into ClickHouse (no S3).
    Fetch(FetchIngestArgs),
    /// Ping ClickHouse connectivity.
    Status,
}

#[derive(Debug, Args)]
pub struct IngestArgs {
    /// Read parquet from S3 bucket instead of local manifest paths.
    #[arg(long)]
    pub s3: bool,
    #[arg(long, default_value = "data/futures/um/daily/aggTrades/")]
    pub s3_prefix: String,
    #[arg(long)]
    pub resume: bool,
    #[arg(long, default_value_t = 4)]
    pub parallel: usize,
    #[arg(long)]
    pub limit: Option<usize>,
    /// Only ingest files on or after this date (YYYY-MM-DD).
    #[arg(long)]
    pub since: Option<String>,
}

#[derive(Debug, Args)]
pub struct FetchIngestArgs {
    #[command(flatten)]
    pub run: RunArgs,
    #[arg(long)]
    pub limit: Option<usize>,
}

pub async fn run(args: ClickhouseArgs, config: Config) -> Result<()> {
    let ch = ClickHouseConfig::from_env().context("ClickHouse config")?;
    match args.command {
        ClickhouseCommand::Ingest(ingest) => {
            let since = match ingest.since {
                Some(ref s) => Some(
                    NaiveDate::parse_from_str(s, "%Y-%m-%d")
                        .context("invalid --since (expected YYYY-MM-DD)")?,
                ),
                None => None,
            };
            let options = IngestOptions {
                resume: ingest.resume,
                parallel: ingest.parallel,
                limit: ingest.limit,
                s3_prefix: if ingest.s3 {
                    Some(ingest.s3_prefix)
                } else {
                    None
                },
                since,
            };
            let summary = if ingest.s3 {
                let s3 = S3Config::from_env().context("S3 config")?;
                run_s3_ingest(&s3, &ch, options).await?
            } else {
                run_ingest(&config, &ch, options).await?
            };
            println!(
                "total={} done={} skipped={} failed={}",
                summary.total, summary.done, summary.skipped, summary.failed
            );
        }
        ClickhouseCommand::Fetch(fetch) => {
            let http = VisionHttpClient::new(config.clone())?;
            let mirror = resolve_mirror_options(&http, &config, fetch.run).await?;
            let summary = run_fetch_ingest(
                &http,
                &ch,
                FetchIngestOptions {
                    mirror,
                    limit: fetch.limit,
                },
            )
            .await?;
            println!(
                "total={} done={} skipped={} failed={}",
                summary.total, summary.done, summary.skipped, summary.failed
            );
        }
        ClickhouseCommand::Status => {
            let client = binance_vision::ClickHouseClient::new(&ch)?;
            client.ping().await.context("ping failed")?;
            println!("ClickHouse OK: {}@{}", ch.user, ch.url());
        }
    }
    Ok(())
}

async fn resolve_mirror_options(
    http: &VisionHttpClient,
    config: &Config,
    args: RunArgs,
) -> Result<MirrorOptions> {
    use binance_vision::{DataType, Market};

    let data_type = DataType::parse(&args.data_type).context("invalid data type")?;
    if data_type.requires_interval() && args.interval.is_none() {
        anyhow::bail!("--interval is required for {}", args.data_type);
    }
    match data_type {
        DataType::Trades | DataType::Klines | DataType::BookDepth | DataType::BvolIndex => {}
        other => {
            anyhow::bail!(
                "clickhouse fetch supports trades/klines/bookDepth/BVOLIndex, not {}",
                other.path_segment()
            );
        }
    }

    let market: Market = args.market.into();
    let symbols = if args.all_symbols || args.symbols.is_empty() {
        let cache = SymbolCache::open(config, market)
            .await
            .context("symbol cache")?;
        let rest = BinanceRestClient::new(http.clone());
        let symbols = cache
            .trading_symbols(&rest, market)
            .await
            .context("resolve trading symbols")?;
        eprintln!(
            "resolved {} TRADING symbols for {}",
            symbols.len(),
            market.as_str()
        );
        symbols
    } else {
        args.symbols
    };

    Ok(MirrorOptions {
        market,
        frequency: args.frequency.into(),
        data_type,
        symbols,
        from: NaiveDate::parse_from_str(&args.from, "%Y-%m-%d").context("invalid --from")?,
        to: NaiveDate::parse_from_str(&args.to, "%Y-%m-%d").context("invalid --to")?,
        interval: args.interval,
        resume: args.resume,
        parallel: args.parallel,
        verify_checksum: args.verify_checksum,
    })
}
