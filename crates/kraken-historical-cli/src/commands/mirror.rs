//! Mirror subcommands.

use anyhow::{Context, Result};
use clap::{Args, Subcommand, ValueEnum};
use kraken_historical::{
    config::default_parallel, mirror_status, mirror_status_s3, parse_quarter, run_mirror,
    run_s3_mirror, Config, DataType, MirrorOptions, S3Config, SourceMode,
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
pub enum SourceArg {
    Complete,
    Quarterly,
    Both,
}

impl From<SourceArg> for SourceMode {
    fn from(value: SourceArg) -> Self {
        match value {
            SourceArg::Complete => SourceMode::Complete,
            SourceArg::Quarterly => SourceMode::Quarterly,
            SourceArg::Both => SourceMode::Both,
        }
    }
}

#[derive(Debug, Args)]
pub struct RunArgs {
    #[arg(long, default_value = "trades")]
    pub data_type: String,
    #[arg(long, value_enum, default_value_t = SourceArg::Quarterly)]
    pub source: SourceArg,
    #[arg(long)]
    pub from_quarter: Option<String>,
    #[arg(long)]
    pub to_quarter: Option<String>,
    #[arg(long, value_delimiter = ',')]
    pub pairs: Vec<String>,
    #[arg(
        long,
        num_args = 0..=1,
        default_value_t = true,
        default_missing_value = "true",
        action = clap::ArgAction::Set
    )]
    pub resume: bool,
    #[arg(long, default_value_t = default_parallel())]
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
            let options = resolve_options(run)?;
            let summary = run_mirror(&config, options).await?;
            print_status(&summary);
        }
        MirrorCommand::S3(s3_run) => {
            let s3 = S3Config::from_env().context("S3 config")?;
            let options = resolve_options(s3_run.run)?;
            let summary = run_s3_mirror(&config, &s3, options).await?;
            print_status(&summary);
        }
    }
    Ok(())
}

fn resolve_options(args: RunArgs) -> Result<MirrorOptions> {
    let data_type = DataType::parse(&args.data_type).context("invalid data type")?;
    Ok(MirrorOptions {
        data_type,
        source_mode: args.source.into(),
        from_quarter: normalize_opt_quarter(args.from_quarter)?,
        to_quarter: normalize_opt_quarter(args.to_quarter)?,
        pairs: if args.pairs.is_empty() {
            None
        } else {
            Some(args.pairs)
        },
        resume: args.resume,
        parallel: args.parallel,
    })
}

fn normalize_opt_quarter(value: Option<String>) -> Result<Option<String>> {
    match value {
        None => Ok(None),
        Some(q) => {
            let normalized = parse_quarter(&q)
                .or_else(|| parse_quarter(&q.to_ascii_uppercase()))
                .ok_or_else(|| anyhow::anyhow!("invalid quarter: {q} (expected YYYYQn)"))?;
            Ok(Some(normalized))
        }
    }
}

fn print_status(summary: &kraken_historical::MirrorStatus) {
    println!(
        "total={} done={} skipped={} failed={} pending={}",
        summary.total, summary.done, summary.skipped, summary.failed, summary.pending
    );
}
