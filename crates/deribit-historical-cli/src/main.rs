#![forbid(unsafe_code)]

use anyhow::{Context, Result};
use chrono::NaiveDate;
use clap::{Args, Parser, Subcommand};

use deribit_historical::{
    default_currencies, mirror_status_s3, mirror_volatility_status_s3, parse_currency_list,
    parse_volatility_currency_list, parse_volatility_resolution, run_dvol_trades_s3_mirror,
    run_s3_mirror, run_volatility_index_s3_mirror, Config, DeribitHttpClient, InstrumentsCache,
    MirrorOptions, S3Config, VolatilityMirrorOptions,
};
use historical_core::init_tracing;

#[derive(Debug, Parser)]
#[command(
    about = "Mirror Deribit historical futures public trades and DVOL volatility index to S3"
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
    /// Mirror DVOL volatility-index OHLC candles (not trades).
    VolatilityIndex(VolatilityIndexArgs),
    /// Mirror public trades for instruments whose name contains DVOL (USDC futures).
    DvolTrades(DvolTradesArgs),
    Status(StatusArgs),
    /// Refresh and list futures instruments for the selected currencies.
    Instruments(InstrumentsArgs),
}

#[derive(Debug, Args)]
struct MirrorS3Args {
    #[arg(
        long,
        default_value = "BTC,ETH",
        help = "Comma-separated currencies (v1: BTC,ETH,USDC)"
    )]
    currency: String,
    #[arg(long, default_value = "future", help = "Instrument kind (v1: future only)")]
    kind: String,
    #[arg(
        long,
        value_delimiter = ',',
        help = "Optional explicit instrument names (default: all futures for --currency)"
    )]
    instruments: Option<Vec<String>>,
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
    #[arg(long, default_value_t = 2)]
    parallel: usize,
    #[arg(
        long,
        default_value_t = false,
        help = "Also fetch expired dated futures via get_instruments expired=true"
    )]
    include_expired: bool,
}

#[derive(Debug, Args)]
struct VolatilityIndexArgs {
    #[arg(
        long,
        default_value = "BTC,ETH",
        help = "DVOL index currencies for get_volatility_index_data (BTC/ETH, not BTCUSD)"
    )]
    currency: String,
    #[arg(
        long,
        default_value = "3600",
        help = "Candle resolution: 1, 60, 3600, 43200, or 1D"
    )]
    resolution: String,
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
    #[arg(long, default_value_t = 2)]
    parallel: usize,
}

#[derive(Debug, Args)]
struct DvolTradesArgs {
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
    #[arg(long, default_value_t = 2)]
    parallel: usize,
    #[arg(long, default_value_t = false)]
    include_expired: bool,
}

#[derive(Debug, Args)]
struct StatusArgs {
    #[arg(long)]
    s3: bool,
    #[arg(
        long,
        default_value = "trade",
        help = "Manifest to inspect: trade | volatility-index"
    )]
    data_type: String,
}

#[derive(Debug, Args)]
struct InstrumentsArgs {
    #[arg(long, default_value = "BTC,ETH")]
    currency: String,
    #[arg(long, default_value = "future")]
    kind: String,
    #[arg(long, default_value_t = false)]
    include_expired: bool,
    #[arg(
        long,
        default_value_t = false,
        help = "Only print instruments whose name contains DVOL"
    )]
    dvol_only: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    init_tracing(cli.verbose)?;
    match cli.command {
        Command::Mirror(mirror) => match mirror.command {
            MirrorCommand::S3(args) => run_s3(args).await,
            MirrorCommand::VolatilityIndex(args) => run_volatility_index(args).await,
            MirrorCommand::DvolTrades(args) => run_dvol_trades(args).await,
            MirrorCommand::Status(args) => run_status(args).await,
            MirrorCommand::Instruments(args) => run_instruments(args).await,
        },
    }
}

fn build_http_client() -> Result<DeribitHttpClient> {
    let mut config = Config::default();
    if !config.has_credentials() {
        eprintln!(
            "warning: DERIBIT_CLIENT_ID/DERIBIT_CLIENT_SECRET unset; using unauthenticated \
             per-IP limits (ok for tiny smokes only — production sync requires credentials)"
        );
        config = config.for_unauthenticated_smoke();
    }
    DeribitHttpClient::new(config).context("HTTP client")
}

async fn run_s3(args: MirrorS3Args) -> Result<()> {
    if !args.kind.eq_ignore_ascii_case("future") {
        anyhow::bail!("v1 only supports --kind future (includes perpetuals); got {}", args.kind);
    }
    let currencies = parse_currency_list(&args.currency).map_err(anyhow::Error::msg)?;
    let from = NaiveDate::parse_from_str(&args.from, "%Y-%m-%d").context("invalid --from")?;
    let to = NaiveDate::parse_from_str(&args.to, "%Y-%m-%d").context("invalid --to")?;
    let http = build_http_client()?;
    let cache = InstrumentsCache::default();
    let s3 = S3Config::from_env().context("S3 config")?;
    let report = run_s3_mirror(
        &http,
        &s3,
        &cache,
        MirrorOptions {
            currencies,
            instruments: args.instruments,
            from,
            to,
            resume: args.resume,
            parallel: args.parallel,
            include_expired: args.include_expired,
        },
    )
    .await
    .context("S3 mirror")?;
    print_status(&report.status);
    if report.status.failed > 0 {
        anyhow::bail!(
            "Deribit mirror completed with {} failed day objects",
            report.status.failed
        );
    }
    Ok(())
}

async fn run_volatility_index(args: VolatilityIndexArgs) -> Result<()> {
    let currencies =
        parse_volatility_currency_list(&args.currency).map_err(anyhow::Error::msg)?;
    let resolution = parse_volatility_resolution(&args.resolution).map_err(anyhow::Error::msg)?;
    let from = NaiveDate::parse_from_str(&args.from, "%Y-%m-%d").context("invalid --from")?;
    let to = NaiveDate::parse_from_str(&args.to, "%Y-%m-%d").context("invalid --to")?;
    let http = build_http_client()?;
    let s3 = S3Config::from_env().context("S3 config")?;
    let report = run_volatility_index_s3_mirror(
        &http,
        &s3,
        VolatilityMirrorOptions {
            currencies,
            resolution,
            from,
            to,
            resume: args.resume,
            parallel: args.parallel,
        },
    )
    .await
    .context("volatility-index S3 mirror")?;
    print_status(&report.status);
    if report.status.failed > 0 {
        anyhow::bail!(
            "Deribit volatility-index mirror completed with {} failed day objects",
            report.status.failed
        );
    }
    Ok(())
}

async fn run_dvol_trades(args: DvolTradesArgs) -> Result<()> {
    let from = NaiveDate::parse_from_str(&args.from, "%Y-%m-%d").context("invalid --from")?;
    let to = NaiveDate::parse_from_str(&args.to, "%Y-%m-%d").context("invalid --to")?;
    let http = build_http_client()?;
    let cache = InstrumentsCache::default();
    let s3 = S3Config::from_env().context("S3 config")?;
    let report = run_dvol_trades_s3_mirror(
        &http,
        &s3,
        &cache,
        from,
        to,
        args.resume,
        args.parallel,
        args.include_expired,
    )
    .await
    .context("DVOL trades S3 mirror")?;
    print_status(&report.status);
    if report.status.failed > 0 {
        anyhow::bail!(
            "Deribit DVOL trades mirror completed with {} failed day objects",
            report.status.failed
        );
    }
    Ok(())
}

async fn run_instruments(args: InstrumentsArgs) -> Result<()> {
    if !args.kind.eq_ignore_ascii_case("future") {
        anyhow::bail!("v1 only supports --kind future; got {}", args.kind);
    }
    let currencies = if args.currency.trim().is_empty() {
        default_currencies()
    } else {
        parse_currency_list(&args.currency).map_err(anyhow::Error::msg)?
    };
    let http = build_http_client()?;
    let cache = InstrumentsCache::default();
    let mut instruments = deribit_historical::load_future_instruments(
        &http,
        &cache,
        &currencies,
        args.include_expired,
    )
    .await
    .context("fetch instruments")?;
    if args.dvol_only {
        instruments = deribit_historical::filter_dvol_instruments(instruments);
    }
    for instrument in &instruments {
        println!(
            "{}\t{}\tactive={}",
            instrument.instrument_name, instrument.currency, instrument.is_active
        );
    }
    println!("count={}", instruments.len());
    Ok(())
}

async fn run_status(args: StatusArgs) -> Result<()> {
    if !args.s3 {
        anyhow::bail!("Deribit status currently requires --s3");
    }
    let s3 = S3Config::from_env().context("S3 config")?;
    let status = match args.data_type.to_ascii_lowercase().as_str() {
        "trade" | "trades" => mirror_status_s3(&s3).await.context("S3 status")?,
        "volatility-index" | "volatility_index" | "dvol" | "vol" => {
            mirror_volatility_status_s3(&s3)
                .await
                .context("volatility-index S3 status")?
        }
        other => anyhow::bail!(
            "unknown --data-type `{other}`; expected trade or volatility-index"
        ),
    };
    print_status(&status);
    Ok(())
}

fn print_status(status: &deribit_historical::MirrorStatus) {
    println!(
        "total={} done={} skipped={} failed={} pending={}",
        status.total, status.done, status.skipped, status.failed, status.pending
    );
    let _ = std::io::Write::flush(&mut std::io::stdout());
}
