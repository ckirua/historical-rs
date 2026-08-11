use anyhow::{Context, Result};
use chrono::NaiveDate;
use clap::{Args, Parser, Subcommand};

use bybit_historical::{
    mirror_status_s3_for, run_s3_mirror_for, BybitHttpClient, Config, Dataset, Market,
    MirrorOptions, S3Config,
};
use historical_core::init_tracing;

#[derive(Debug, Parser)]
#[command(
    about = "Mirror Bybit historical Spot and Contract order books and trades"
)]
struct Cli {
    #[arg(long, short = 'v', global = true)]
    verbose: bool,
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Mirror(MirrorArgs),
}

#[derive(Debug, Args)]
struct MirrorArgs {
    #[command(subcommand)]
    command: MirrorCommand,
}

#[derive(Debug, Subcommand)]
enum MirrorCommand {
    S3(MirrorS3Args),
    Status(StatusArgs),
    /// Refresh and list current trading symbols for a market.
    Symbols(SymbolsArgs),
}
#[derive(Debug, Args)]
struct MirrorS3Args {
    #[arg(
        long,
        default_value = "spot",
        help = "Verified market (spot or contract)"
    )]
    market: String,
    #[arg(
        long,
        default_value = "orderbook",
        help = "Verified data type (orderbook or trade)"
    )]
    data_type: String,
    #[arg(long, default_value = "daily", help = "Verified interval (daily only)")]
    interval: String,
    #[arg(long, value_delimiter = ',', conflicts_with = "all_symbols")]
    symbols: Option<Vec<String>>,
    #[arg(
        long,
        conflicts_with = "symbols",
        help = "Mirror all current trading symbols for the selected market"
    )]
    all_symbols: bool,
    #[arg(long)]
    from: String,
    #[arg(long)]
    to: String,
    #[arg(
        long,
        num_args = 0..=1,
        default_value_t = true,
        default_missing_value = "true",
        action = clap::ArgAction::Set
    )]
    resume: bool,
    #[arg(long, default_value_t = 8)]
    parallel: usize,
}

#[derive(Debug, Args)]
struct StatusArgs {
    #[arg(long)]
    s3: bool,
    #[arg(
        long,
        default_value = "spot",
        help = "Verified market (spot or contract)"
    )]
    market: String,
    #[arg(
        long,
        default_value = "orderbook",
        help = "Verified data type (orderbook or trade)"
    )]
    data_type: String,
    #[arg(long, default_value = "daily", help = "Verified interval (daily only)")]
    interval: String,
}

#[derive(Debug, Args)]
struct SymbolsArgs {
    #[arg(
        long,
        default_value = "spot",
        help = "Verified market (spot or contract)"
    )]
    market: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    init_tracing(cli.verbose)?;
    match cli.command {
        Command::Mirror(mirror) => match mirror.command {
            MirrorCommand::S3(args) => run_s3(args).await,
            MirrorCommand::Status(args) => run_status(args).await,
            MirrorCommand::Symbols(args) => run_symbols(args).await,
        },
    }
}

async fn run_s3(args: MirrorS3Args) -> Result<()> {
    let dataset = Dataset::from_strings(&args.market, &args.data_type, &args.interval)
        .map_err(anyhow::Error::msg)
        .context("invalid Bybit dataset")?;
    let from = NaiveDate::parse_from_str(&args.from, "%Y-%m-%d").context("invalid --from")?;
    let to = NaiveDate::parse_from_str(&args.to, "%Y-%m-%d").context("invalid --to")?;
    if args.all_symbols && args.symbols.is_some() {
        anyhow::bail!("--symbols and --all-symbols are mutually exclusive");
    }
    if !args.all_symbols && args.symbols.is_none() {
        anyhow::bail!("one of --symbols or --all-symbols is required");
    }
    let http = BybitHttpClient::new(Config::default()).context("HTTP client")?;
    let symbols = if args.all_symbols {
        let symbols = http
            .trading_symbols_for_market(dataset.market)
            .await
            .with_context(|| {
                format!(
                    "fetch current {} trading symbols",
                    dataset.market.as_str()
                )
            })?;
        println!(
            "resolved {} current {} trading symbols",
            symbols.len(),
            dataset.market.as_str()
        );
        symbols
    } else {
        let symbols = args.symbols.expect("validated --symbols presence");
        let symbols = symbols
            .into_iter()
            .map(|symbol| symbol.trim().to_ascii_uppercase())
            .filter(|symbol| !symbol.is_empty())
            .collect::<Vec<_>>();
        if symbols.is_empty() {
            anyhow::bail!("at least one non-empty symbol is required");
        }
        symbols
    };
    let s3 = S3Config::from_env().context("S3 config")?;
    let report = run_s3_mirror_for(
        &http,
        &s3,
        MirrorOptions {
            symbols,
            from,
            to,
            resume: args.resume,
            parallel: args.parallel,
        },
        dataset,
    )
    .await
    .context("S3 mirror")?;
    print_status(&report.status);
    print_failure_histogram(&report.failure_histogram, report.status.failed);
    if report.status.failed > 0 {
        anyhow::bail!(
            "Bybit mirror completed with {} failed archives",
            report.status.failed
        );
    }
    Ok(())
}

async fn run_symbols(args: SymbolsArgs) -> Result<()> {
    let market: Market = args
        .market
        .parse()
        .map_err(anyhow::Error::msg)
        .context("invalid --market")?;
    let http = BybitHttpClient::new(Config::default()).context("HTTP client")?;
    let symbols = http
        .trading_symbols_for_market(market)
        .await
        .with_context(|| format!("fetch current {} trading symbols", market.as_str()))?;
    for symbol in &symbols {
        println!("{symbol}");
    }
    println!("count={}", symbols.len());
    Ok(())
}

async fn run_status(args: StatusArgs) -> Result<()> {
    if !args.s3 {
        anyhow::bail!("Bybit status currently requires --s3");
    }
    let dataset = Dataset::from_strings(&args.market, &args.data_type, &args.interval)
        .map_err(anyhow::Error::msg)
        .context("invalid Bybit dataset")?;
    let s3 = S3Config::from_env().context("S3 config")?;
    let status = mirror_status_s3_for(&s3, dataset)
        .await
        .context("S3 status")?;
    print_status(&status);
    Ok(())
}

fn print_status(status: &bybit_historical::MirrorStatus) {
    println!(
        "total={} done={} skipped={} failed={} pending={}",
        status.total, status.done, status.skipped, status.failed, status.pending
    );
    let _ = std::io::Write::flush(&mut std::io::stdout());
}

fn print_failure_histogram(histogram: &[(String, usize)], failed: usize) {
    // Always emit the header when the run reported failures so prod log scrapers
    // see `failure_histogram:` even if classification produced an empty map.
    if failed == 0 && histogram.is_empty() {
        return;
    }
    println!("failure_histogram:");
    for (reason, count) in histogram {
        println!("  {count}\t{reason}");
    }
    let _ = std::io::Write::flush(&mut std::io::stdout());
}
