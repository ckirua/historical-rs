//! SQLite symbol cache from Binance exchangeInfo.

use std::str::FromStr;
use std::time::Duration;

use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
use sqlx::{Row, SqlitePool};

use crate::client::{BinanceRestClient, ExchangeInfoSymbol};
use crate::config::Config;
use crate::error::{Error, Result};
use crate::model::Market;

pub struct SymbolCache {
    pool: SqlitePool,
}

impl SymbolCache {
    /// Open the per-market symbol cache (`symbols-{market}.db`) with WAL and a 60s busy timeout.
    pub async fn open(config: &Config, market: Market) -> Result<Self> {
        let path = config.symbols_db_path(market)?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let options = SqliteConnectOptions::from_str(&format!("sqlite:{}", path.display()))?
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal)
            .busy_timeout(Duration::from_secs(60));
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await?;
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS symbols (
                market TEXT NOT NULL,
                symbol TEXT NOT NULL,
                status TEXT NOT NULL,
                base_asset TEXT NOT NULL,
                quote_asset TEXT NOT NULL,
                updated_at TEXT NOT NULL DEFAULT (datetime('now')),
                PRIMARY KEY (market, symbol)
            )",
        )
        .execute(&pool)
        .await?;
        Ok(Self { pool })
    }

    pub async fn refresh_market(&self, rest: &BinanceRestClient, market: Market) -> Result<usize> {
        let symbols = rest.exchange_info(market).await?;
        let mut count = 0usize;
        for sym in symbols {
            sqlx::query(
                "INSERT INTO symbols (market, symbol, status, base_asset, quote_asset, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, datetime('now'))
                 ON CONFLICT(market, symbol) DO UPDATE SET
                   status=excluded.status,
                   base_asset=excluded.base_asset,
                   quote_asset=excluded.quote_asset,
                   updated_at=datetime('now')",
            )
            .bind(market.as_str())
            .bind(&sym.symbol)
            .bind(&sym.status)
            .bind(&sym.base_asset)
            .bind(&sym.quote_asset)
            .execute(&self.pool)
            .await?;
            count += 1;
        }
        Ok(count)
    }

    pub async fn list(
        &self,
        market: Option<Market>,
        trading_only: bool,
    ) -> Result<Vec<ExchangeInfoSymbol>> {
        let rows = if let Some(market) = market {
            if trading_only {
                sqlx::query(
                    "SELECT market, symbol, status, base_asset, quote_asset FROM symbols
                     WHERE market = ?1 AND status = 'TRADING' ORDER BY symbol",
                )
                .bind(market.as_str())
                .fetch_all(&self.pool)
                .await?
            } else {
                sqlx::query(
                    "SELECT market, symbol, status, base_asset, quote_asset FROM symbols
                     WHERE market = ?1 ORDER BY symbol",
                )
                .bind(market.as_str())
                .fetch_all(&self.pool)
                .await?
            }
        } else if trading_only {
            sqlx::query(
                "SELECT market, symbol, status, base_asset, quote_asset FROM symbols
                 WHERE status = 'TRADING' ORDER BY market, symbol",
            )
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query(
                "SELECT market, symbol, status, base_asset, quote_asset FROM symbols
                 ORDER BY market, symbol",
            )
            .fetch_all(&self.pool)
            .await?
        };

        rows.into_iter()
            .map(|row| {
                let market_str: String = row.try_get("market")?;
                let market = Market::parse(&market_str)
                    .ok_or_else(|| Error::Parse(format!("unknown market '{market_str}'")))?;
                Ok(ExchangeInfoSymbol {
                    market,
                    symbol: row.try_get("symbol")?,
                    status: row.try_get("status")?,
                    base_asset: row.try_get("base_asset")?,
                    quote_asset: row.try_get("quote_asset")?,
                })
            })
            .collect()
    }

    /// Refresh exchangeInfo and return TRADING symbols for a market.
    pub async fn trading_symbols(
        &self,
        rest: &BinanceRestClient,
        market: Market,
    ) -> Result<Vec<String>> {
        self.refresh_market(rest, market).await?;
        Ok(self
            .list(Some(market), true)
            .await?
            .into_iter()
            .map(|s| s.symbol)
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[tokio::test]
    async fn two_markets_open_concurrently() {
        let dir = tempfile::tempdir().unwrap();
        let mut config = Config::default();
        config.data_dir = Some(PathBuf::from(dir.path()));

        // Concurrent open (CREATE TABLE) on distinct DB files must not hit "database is locked".
        let (um, spot) = tokio::join!(
            SymbolCache::open(&config, Market::Um),
            SymbolCache::open(&config, Market::Spot),
        );
        um.expect("open um cache");
        spot.expect("open spot cache");

        assert!(dir.path().join("symbols-um.db").exists());
        assert!(dir.path().join("symbols-spot.db").exists());
        assert!(!dir.path().join("symbols.db").exists());
    }
}
