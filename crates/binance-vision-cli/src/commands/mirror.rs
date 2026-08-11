//! Mirror subcommands.

use anyhow::{Context, Result};
use binance_vision::{
    mirror_status, mirror_status_s3, run_mirror, run_s3_mirror, BinanceRestClient, Config,
    DataType, Frequency, Market, MirrorOptions, S3Config, SymbolCache, VisionHttpClient,
};
use chrono::NaiveDate;
use clap::{Args, Subcommand, ValueEnum};

#[derive(Debug, Args)]
pub struct MirrorArgs {
    #[command(subcommand)]
    pub command: MirrorCommand,
}

#[derive(Debug, Subcommand)]
pub enum MirrorCommand {
    /// Show mirror progress from manifest.jsonl (local or S3).
    Status(StatusArgs),
    /// Mirror to local datasets directory.
    Run(RunArgs),
    /// Mirror directly to S3-compatible storage.
    S3(S3RunArgs),
}

#[derive(Debug, Args)]
pub struct StatusArgs {
    #[arg(long)]
    pub s3: bool,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum MarketArg {
    Spot,
    Um,
    Cm,
    Option,
}

impl From<MarketArg> for Market {
    fn from(value: MarketArg) -> Self {
        match value {
            MarketArg::Spot => Market::Spot,
            MarketArg::Um => Market::Um,
            MarketArg::Cm => Market::Cm,
            MarketArg::Option => Market::Option,
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum FrequencyArg {
    Daily,
    Monthly,
}

impl From<FrequencyArg> for Frequency {
    fn from(value: FrequencyArg) -> Self {
        match value {
            FrequencyArg::Daily => Frequency::Daily,
            FrequencyArg::Monthly => Frequency::Monthly,
        }
    }
}

#[derive(Debug, Args)]
pub struct RunArgs {
    #[arg(long, value_enum, default_value_t = MarketArg::Um)]
    pub market: MarketArg,
    #[arg(long, value_enum, default_value_t = FrequencyArg::Daily)]
    pub frequency: FrequencyArg,
    #[arg(long, default_value = "trades")]
    pub data_type: String,
    #[arg(long, value_delimiter = ',')]
    pub symbols: Vec<String>,
    #[arg(
        long,
        help = "Sync all TRADING symbols for the market (default when --symbols is omitted)"
    )]
    pub all_symbols: bool,
    #[arg(long)]
    pub from: String,
    #[arg(long)]
    pub to: String,
    #[arg(long)]
    pub interval: Option<String>,
    #[arg(
        long,
        num_args = 0..=1,
        default_value_t = true,
        default_missing_value = "true",
        action = clap::ArgAction::Set
    )]
    pub resume: bool,
    #[arg(long, default_value_t = 8)]
    pub parallel: usize,
    #[arg(long)]
    pub verify_checksum: bool,
}

#[derive(Debug, Args)]
pub struct S3RunArgs {
    #[command(flatten)]
    pub run: RunArgs,
}

pub async fn run(args: MirrorArgs, config: Config) -> Result<()> {
    match args.command {
        MirrorCommand::Status(status) => {
            if status.s3 {
                let s3 = S3Config::from_env().context("S3 config")?;
                let summary = mirror_status_s3(&s3).await.context("S3 status")?;
                print_status(&summary);
            } else {
                let summary = mirror_status(&config).context("local status")?;
                print_status(&summary);
            }
        }
        MirrorCommand::Run(run) => {
            let http = VisionHttpClient::new(config.clone())?;
            let options = resolve_options(&http, &config, run).await?;
            let summary = run_mirror(&http, &config, options).await?;
            print_status(&summary);
        }
        MirrorCommand::S3(s3_run) => {
            let http = VisionHttpClient::new(config.clone())?;
            let s3 = S3Config::from_env().context("S3 config")?;
            let options = resolve_options(&http, &config, s3_run.run).await?;
            let summary = run_s3_mirror(&http, &s3, options).await?;
            print_status(&summary);
        }
    }
    Ok(())
}

async fn resolve_options(
    http: &VisionHttpClient,
    config: &Config,
    args: RunArgs,
) -> Result<MirrorOptions> {
    let data_type = DataType::parse(&args.data_type).context("invalid data type")?;
    if data_type.requires_interval() && args.interval.is_none() {
        anyhow::bail!("--interval is required for {}", args.data_type);
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

fn print_status(summary: &binance_vision::MirrorStatus) {
    println!(
        "total={} done={} skipped={} failed={} pending={}",
        summary.total, summary.done, summary.skipped, summary.failed, summary.pending
    );
}
