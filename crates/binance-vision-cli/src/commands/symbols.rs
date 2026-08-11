//! Symbols subcommands.

use anyhow::Result;
use binance_vision::{BinanceRestClient, Config, Market, SymbolCache, VisionHttpClient};
use clap::{Args, Subcommand};

use super::mirror::MarketArg;

#[derive(Debug, Args)]
pub struct SymbolsArgs {
    #[command(subcommand)]
    pub command: SymbolsCommand,
}

#[derive(Debug, Subcommand)]
pub enum SymbolsCommand {
    /// Fetch exchangeInfo and refresh local cache.
    Refresh(RefreshArgs),
    /// List cached symbols.
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
    pub trading_only: bool,
}

const CACHE_MARKETS: [Market; 3] = [Market::Spot, Market::Um, Market::Cm];

pub async fn run(args: SymbolsArgs, config: Config) -> Result<()> {
    let http = VisionHttpClient::new(config.clone())?;
    let rest = BinanceRestClient::new(http);
    match args.command {
        SymbolsCommand::Refresh(refresh) => {
            let count = if let Some(market) = refresh.market {
                let market = Market::from(market);
                let cache = SymbolCache::open(&config, market).await?;
                cache.refresh_market(&rest, market).await?
            } else {
                let mut total = 0usize;
                for market in CACHE_MARKETS {
                    let cache = SymbolCache::open(&config, market).await?;
                    total += cache.refresh_market(&rest, market).await?;
                }
                total
            };
            println!("refreshed {count} symbols");
        }
        SymbolsCommand::List(list) => {
            let symbols = if let Some(market) = list.market {
                let market = Market::from(market);
                let cache = SymbolCache::open(&config, market).await?;
                cache.list(Some(market), list.trading_only).await?
            } else {
                let mut all = Vec::new();
                for market in CACHE_MARKETS {
                    let cache = SymbolCache::open(&config, market).await?;
                    all.extend(cache.list(Some(market), list.trading_only).await?);
                }
                all
            };
            let count = symbols.len();
            for sym in symbols {
                println!(
                    "{} {} {} {}/{}",
                    sym.market.as_str(),
                    sym.symbol,
                    sym.status,
                    sym.base_asset,
                    sym.quote_asset
                );
            }
            println!("count={count}");
        }
    }
    Ok(())
}
