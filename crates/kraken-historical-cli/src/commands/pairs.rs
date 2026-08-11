//! Pairs subcommand — list Kraken AssetPairs.

use anyhow::Result;
use clap::Args;
use kraken_historical::{fetch_asset_pairs, Config, HttpClient};

#[derive(Debug, Args)]
pub struct PairsArgs {
    /// Only print altname (or wsname) values, one per line.
    #[arg(long)]
    pub names_only: bool,
    /// Filter by quote currency (e.g. USD, EUR).
    #[arg(long)]
    pub quote: Option<String>,
}

pub async fn run(args: PairsArgs, config: Config) -> Result<()> {
    let http = HttpClient::new(config)?;
    let mut pairs = fetch_asset_pairs(&http).await?;
    if let Some(quote) = args.quote.as_deref() {
        let q = quote.to_ascii_uppercase();
        pairs.retain(|p| p.quote.to_ascii_uppercase().contains(&q) || p.altname.ends_with(&q));
    }
    if args.names_only {
        for p in pairs {
            println!("{}", p.altname);
        }
    } else {
        for p in pairs {
            let ws = p.wsname.as_deref().unwrap_or("-");
            let status = p.status.as_deref().unwrap_or("-");
            println!(
                "{}\t{}\t{}\t{}/{}\t{status}",
                p.name, p.altname, ws, p.base, p.quote
            );
        }
    }
    Ok(())
}
