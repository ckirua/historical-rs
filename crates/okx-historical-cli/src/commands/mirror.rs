//! Mirror subcommands.

use anyhow::{Context, Result};
use chrono::NaiveDate;
use clap::{Args, Subcommand, ValueEnum};
use okx_historical::{
    mirror_status, mirror_status_s3, run_mirror, run_s3_mirror, Config, DataType, Frequency,
    Market, MirrorOptions, OkxHttpClient, OkxRestClient, S3Config, SymbolCache,
};

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
    Swap,
}

impl From<MarketArg> for Market {
    fn from(value: MarketArg) -> Self {
        match value {
            MarketArg::Spot => Market::Spot,
            MarketArg::Swap => Market::Swap,
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
    #[arg(long, value_enum, default_value_t = MarketArg::Spot)]
    pub market: MarketArg,
    #[arg(long, value_enum)]
    pub frequency: Option<FrequencyArg>,
    #[arg(long, default_value = "trades")]
    pub data_type: String,
    #[arg(long, value_delimiter = ',')]
    pub symbols: Vec<String>,
    #[arg(
        long,
        help = "Sync all live instruments (or currencies for borrowrates)"
    )]
    pub all_symbols: bool,
    #[arg(long)]
    pub from: String,
    #[arg(long)]
    pub to: String,
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
            let http = OkxHttpClient::new(config.clone())?;
            let options = resolve_options(&http, &config, run).await?;
            let summary = run_mirror(&http, &config, options).await?;
            print_status(&summary);
        }
        MirrorCommand::S3(s3_run) => {
            let http = OkxHttpClient::new(config.clone())?;
            let s3 = S3Config::from_env().context("S3 config")?;
            let options = resolve_options(&http, &config, s3_run.run).await?;
            let summary = run_s3_mirror(&http, &s3, options).await?;
            print_status(&summary);
        }
    }
    Ok(())
}

async fn resolve_options(
    http: &OkxHttpClient,
    config: &Config,
    args: RunArgs,
) -> Result<MirrorOptions> {
    let data_type = DataType::parse(&args.data_type).context("invalid data type")?;
    let market: Market = args.market.into();
    let frequency = args
        .frequency
        .map(Frequency::from)
        .unwrap_or_else(|| data_type.default_frequency());

    if data_type == DataType::Funding && market != Market::Swap {
        anyhow::bail!("funding rates are SWAP-only");
    }
    if data_type == DataType::BorrowRates && frequency != Frequency::Monthly {
        anyhow::bail!("borrow rates are monthly-only on the CDN");
    }

    let symbols = if args.all_symbols || args.symbols.is_empty() {
        let cache = SymbolCache::open(config).await.context("symbol cache")?;
        let rest = OkxRestClient::new(http.clone());
        let symbols = match data_type {
            DataType::Funding => {
                eprintln!("using ALL swap funding bundle (allswap-fundingrates)");
                vec!["ALL".to_string()]
            }
            DataType::BorrowRates => {
                let ccys = cache.live_currencies(&rest).await.context("currencies")?;
                eprintln!("resolved {} currencies for borrow rates", ccys.len());
                ccys
            }
            _ => {
                let symbols = cache
                    .live_instruments(&rest, market)
                    .await
                    .context("resolve live instruments")?;
                eprintln!(
                    "resolved {} live instruments for {}",
                    symbols.len(),
                    market.as_str()
                );
                symbols
            }
        };
        symbols
    } else {
        args.symbols
    };

    Ok(MirrorOptions {
        market,
        frequency,
        data_type,
        symbols,
        from: NaiveDate::parse_from_str(&args.from, "%Y-%m-%d").context("invalid --from")?,
        to: NaiveDate::parse_from_str(&args.to, "%Y-%m-%d").context("invalid --to")?,
        resume: args.resume,
        parallel: args.parallel,
    })
}

fn print_status(summary: &okx_historical::MirrorStatus) {
    println!(
        "total={} done={} skipped={} failed={} pending={}",
        summary.total, summary.done, summary.skipped, summary.failed, summary.pending
    );
}
