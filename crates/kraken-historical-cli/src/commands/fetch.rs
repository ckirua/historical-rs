//! Fetch subcommand — download + convert one complete/quarter window locally.

use anyhow::{Context, Result};
use clap::Args;
use kraken_historical::{
    parse_quarter, run_mirror, Config, DataType, MirrorOptions, Source, SourceMode,
};

#[derive(Debug, Args)]
pub struct FetchArgs {
    /// `trades` or `ohlcvt`
    pub data_type: String,
    /// `complete` or `YYYYQn` (e.g. 2024Q3)
    pub source: String,
    #[arg(long, value_delimiter = ',')]
    pub pairs: Vec<String>,
    #[arg(long, default_value_t = kraken_historical::config::default_parallel())]
    pub parallel: usize,
}

pub async fn run(args: FetchArgs, config: Config) -> Result<()> {
    let data_type = DataType::parse(&args.data_type).context("invalid data type")?;
    let source = Source::parse(&args.source)
        .or_else(|| parse_quarter(&args.source).map(|q| Source::Quarterly { quarter: q }))
        .context("source must be complete or YYYYQn")?;

    let (source_mode, from_quarter, to_quarter) = match source {
        Source::Complete => (SourceMode::Complete, None, None),
        Source::Quarterly { quarter } => {
            (SourceMode::Quarterly, Some(quarter.clone()), Some(quarter))
        }
    };

    let options = MirrorOptions {
        data_type,
        source_mode,
        from_quarter,
        to_quarter,
        pairs: if args.pairs.is_empty() {
            None
        } else {
            Some(args.pairs)
        },
        resume: true,
        parallel: args.parallel,
    };

    let summary = run_mirror(&config, options).await?;
    println!(
        "total={} done={} skipped={} failed={} pending={}",
        summary.total, summary.done, summary.skipped, summary.failed, summary.pending
    );
    Ok(())
}
