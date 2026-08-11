//! Symbols subcommands.

use anyhow::Result;
use clap::{Args, Subcommand};
use okx_historical::{Config, Market, OkxHttpClient, OkxRestClient, SymbolCache};

use super::mirror::MarketArg;

#[derive(Debug, Args)]
pub struct SymbolsArgs {
    #[command(subcommand)]
    pub command: SymbolsCommand,
}

#[derive(Debug, Subcommand)]
pub enum SymbolsCommand {
    /// Fetch instruments and refresh local cache.
    Refresh(RefreshArgs),
    /// List cached instruments.
    List(ListArgs),
}

#[derive(Debug, Args)]
pub struct RefreshArgs {
    #[arg(long, value_enum)]
    pub market: Option<MarketArg>,
}

#[derive(Debug, Args)]
pub struct ListArgs {
    #[arg(long, value_enum)]
    pub market: Option<MarketArg>,
    #[arg(long, default_value_t = true)]
    pub live_only: bool,
}

pub async fn run(args: SymbolsArgs, config: Config) -> Result<()> {
    let http = OkxHttpClient::new(config.clone())?;
    let rest = OkxRestClient::new(http);
    let cache = SymbolCache::open(&config).await?;
    match args.command {
        SymbolsCommand::Refresh(refresh) => {
            let count = if let Some(market) = refresh.market {
                cache.refresh_market(&rest, market.into()).await?
            } else {
                cache.refresh_all(&rest).await?
            };
            println!("refreshed {count} instruments");
        }
        SymbolsCommand::List(list) => {
            let market = list.market.map(Market::from);
            let symbols = cache.list(market, list.live_only).await?;
            let count = symbols.len();
            for sym in symbols {
                println!(
                    "{} {} {} {}/{}",
                    sym.market.as_str(),
                    sym.inst_id,
                    sym.state,
                    sym.base_ccy,
                    sym.quote_ccy
                );
            }
            println!("count={count}");
        }
    }
    Ok(())
}
