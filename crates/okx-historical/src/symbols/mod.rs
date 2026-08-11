//! SQLite instrument cache from OKX public instruments.

use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Row, SqlitePool};
use std::collections::BTreeSet;
use std::str::FromStr;

use crate::client::{InstrumentInfo, OkxRestClient};
use crate::config::Config;
use crate::error::{Error, Result};
use crate::model::Market;

pub struct SymbolCache {
    pool: SqlitePool,
}

impl SymbolCache {
    pub async fn open(config: &Config) -> Result<Self> {
        if let Some(parent) = config.symbols_db_path()?.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let options = SqliteConnectOptions::from_str(&format!(
            "sqlite:{}",
            config.symbols_db_path()?.display()
        ))?
        .create_if_missing(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await?;
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS symbols (
                market TEXT NOT NULL,
                inst_id TEXT NOT NULL,
                state TEXT NOT NULL,
                base_ccy TEXT NOT NULL,
                quote_ccy TEXT NOT NULL,
                settle_ccy TEXT NOT NULL,
                updated_at TEXT NOT NULL DEFAULT (datetime('now')),
                PRIMARY KEY (market, inst_id)
            )",
        )
        .execute(&pool)
        .await?;
        Ok(Self { pool })
    }

    pub async fn refresh_market(&self, rest: &OkxRestClient, market: Market) -> Result<usize> {
        let symbols = rest.instruments(market).await?;
        let mut count = 0usize;
        for sym in symbols {
            sqlx::query(
                "INSERT INTO symbols (market, inst_id, state, base_ccy, quote_ccy, settle_ccy, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, datetime('now'))
                 ON CONFLICT(market, inst_id) DO UPDATE SET
                   state=excluded.state,
                   base_ccy=excluded.base_ccy,
                   quote_ccy=excluded.quote_ccy,
                   settle_ccy=excluded.settle_ccy,
                   updated_at=datetime('now')",
            )
            .bind(market.as_str())
            .bind(&sym.inst_id)
            .bind(&sym.state)
            .bind(&sym.base_ccy)
            .bind(&sym.quote_ccy)
            .bind(&sym.settle_ccy)
            .execute(&self.pool)
            .await?;
            count += 1;
        }
        Ok(count)
    }

    pub async fn refresh_all(&self, rest: &OkxRestClient) -> Result<usize> {
        let mut total = 0usize;
        for market in [Market::Spot, Market::Swap] {
            total += self.refresh_market(rest, market).await?;
        }
        Ok(total)
    }

    pub async fn list(
        &self,
        market: Option<Market>,
        live_only: bool,
    ) -> Result<Vec<InstrumentInfo>> {
        let rows = if let Some(market) = market {
            if live_only {
                sqlx::query(
                    "SELECT market, inst_id, state, base_ccy, quote_ccy, settle_ccy FROM symbols
                     WHERE market = ?1 AND state = 'live' ORDER BY inst_id",
                )
                .bind(market.as_str())
                .fetch_all(&self.pool)
                .await?
            } else {
                sqlx::query(
                    "SELECT market, inst_id, state, base_ccy, quote_ccy, settle_ccy FROM symbols
                     WHERE market = ?1 ORDER BY inst_id",
                )
                .bind(market.as_str())
                .fetch_all(&self.pool)
                .await?
            }
        } else if live_only {
            sqlx::query(
                "SELECT market, inst_id, state, base_ccy, quote_ccy, settle_ccy FROM symbols
                 WHERE state = 'live' ORDER BY market, inst_id",
            )
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query(
                "SELECT market, inst_id, state, base_ccy, quote_ccy, settle_ccy FROM symbols
                 ORDER BY market, inst_id",
            )
            .fetch_all(&self.pool)
            .await?
        };

        rows.into_iter()
            .map(|row| {
                let market_str: String = row.try_get("market")?;
                let market = Market::parse(&market_str)
                    .ok_or_else(|| Error::Parse(format!("unknown market '{market_str}'")))?;
                Ok(InstrumentInfo {
                    market,
                    inst_id: row.try_get("inst_id")?,
                    state: row.try_get("state")?,
                    base_ccy: row.try_get("base_ccy")?,
                    quote_ccy: row.try_get("quote_ccy")?,
                    settle_ccy: row.try_get("settle_ccy")?,
                })
            })
            .collect()
    }

    pub async fn live_instruments(
        &self,
        rest: &OkxRestClient,
        market: Market,
    ) -> Result<Vec<String>> {
        self.refresh_market(rest, market).await?;
        Ok(self
            .list(Some(market), true)
            .await?
            .into_iter()
            .map(|s| s.inst_id)
            .collect())
    }

    /// Distinct base/quote/settle currencies from live instruments (for borrow rates).
    pub async fn live_currencies(&self, rest: &OkxRestClient) -> Result<Vec<String>> {
        self.refresh_all(rest).await?;
        let symbols = self.list(None, true).await?;
        let mut set = BTreeSet::new();
        for s in symbols {
            for ccy in [s.base_ccy, s.quote_ccy, s.settle_ccy] {
                if !ccy.is_empty() {
                    set.insert(ccy);
                }
            }
        }
        Ok(set.into_iter().collect())
    }
}
