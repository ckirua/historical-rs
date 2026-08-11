//! ClickHouse client and batch insert helpers for `binance_vision.*`.

use clickhouse::Client;
use clickhouse::Row;
use serde::Serialize;
use std::collections::HashSet;

use super::config::ClickHouseConfig;
use super::row::{
    AggTradeInsertRow, IngestLogInsertRow, VisionBookDepthInsertRow, VisionBvolInsertRow,
    VisionKlineInsertRow, VisionTradeInsertRow,
};
use crate::error::{Error, Result};

#[derive(Debug, Row, serde::Deserialize)]
struct IngestLogKeyRow {
    s3_key: String,
}

pub struct ClickHouseClient {
    client: Client,
    database: String,
}

impl ClickHouseClient {
    pub fn new(config: &ClickHouseConfig) -> Result<Self> {
        let client = Client::default()
            .with_url(config.url())
            .with_user(&config.user)
            .with_password(&config.password)
            .with_database(&config.database);
        Ok(Self {
            client,
            database: config.database.clone(),
        })
    }

    pub fn inner(&self) -> &Client {
        &self.client
    }

    pub fn database(&self) -> &str {
        &self.database
    }

    pub async fn ping(&self) -> Result<()> {
        let _: u8 = self
            .client
            .query("SELECT 1")
            .fetch_one()
            .await
            .map_err(|e| Error::ClickHouse(e.to_string()))?;
        Ok(())
    }

    /// Ensure database + ingest_log (+ optional *_mt stubs) exist.
    pub async fn ensure_schema(&self) -> Result<()> {
        let ddl = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../sql/clickhouse/010_binance_vision_merge_tree.sql"
        ));
        apply_statements(&self.client, ddl).await
    }

    pub async fn load_ingested_s3_keys(&self) -> Result<HashSet<String>> {
        let query = format!(
            "SELECT s3_key FROM (
                SELECT s3_key, argMax(status, ingested_at) AS st
                FROM {db}.ingest_log
                GROUP BY s3_key
            )
            WHERE st = 'ok'",
            db = self.database
        );
        let rows = self
            .client
            .query(&query)
            .fetch_all::<IngestLogKeyRow>()
            .await
            .map_err(|e| Error::ClickHouse(e.to_string()))?;
        Ok(rows.into_iter().map(|row| row.s3_key).collect())
    }

    pub async fn insert_ingest_log(
        &self,
        s3_key: &str,
        rows: u64,
        status: &str,
        error: Option<&str>,
    ) -> Result<()> {
        let row = IngestLogInsertRow {
            s3_key: s3_key.to_string(),
            rows,
            status: status.to_string(),
            error: error.map(str::to_string),
        };
        insert_rows(&self.client, "ingest_log", std::slice::from_ref(&row)).await
    }

    pub async fn insert_trades(&self, rows: &[VisionTradeInsertRow]) -> Result<()> {
        insert_rows(&self.client, "trades", rows).await
    }

    pub async fn insert_klines(&self, rows: &[VisionKlineInsertRow]) -> Result<()> {
        insert_rows(&self.client, "klines", rows).await
    }

    pub async fn insert_book_depth(&self, rows: &[VisionBookDepthInsertRow]) -> Result<()> {
        insert_rows(&self.client, "book_depth", rows).await
    }

    pub async fn insert_bvol(&self, rows: &[VisionBvolInsertRow]) -> Result<()> {
        insert_rows(&self.client, "bvol", rows).await
    }

    /// Legacy — forbidden for new work (see docs/GOAL.md).
    pub async fn insert_agg_trades(&self, rows: &[AggTradeInsertRow]) -> Result<()> {
        insert_rows(&self.client, "agg_trades", rows).await
    }
}

async fn insert_rows<T: Row + Serialize>(client: &Client, table: &str, rows: &[T]) -> Result<()> {
    if rows.is_empty() {
        return Ok(());
    }
    const BATCH: usize = 100_000;
    for chunk in rows.chunks(BATCH) {
        let mut insert = client
            .insert(table)
            .map_err(|e| Error::ClickHouse(e.to_string()))?;
        for row in chunk {
            insert
                .write(row)
                .await
                .map_err(|e| Error::ClickHouse(e.to_string()))?;
        }
        insert
            .end()
            .await
            .map_err(|e| Error::ClickHouse(e.to_string()))?;
    }
    Ok(())
}

async fn apply_statements(client: &Client, ddl: &str) -> Result<()> {
    let ddl = strip_line_comments(ddl);
    for statement in ddl.split(';') {
        let statement = statement.trim();
        if statement.is_empty() {
            continue;
        }
        if let Err(err) = client.query(statement).execute().await {
            let message = err.to_string();
            if message.contains("already exists")
                || message.contains("Cannot add projection")
                || message.contains("There is an unfinished mutation")
            {
                tracing::debug!(%message, "schema statement skipped");
                continue;
            }
            return Err(Error::ClickHouse(format!("schema apply failed: {message}")));
        }
    }
    Ok(())
}

fn strip_line_comments(sql: &str) -> String {
    sql.lines()
        .map(|line| line.split_once("--").map_or(line, |(prefix, _)| prefix))
        .collect::<Vec<_>>()
        .join("\n")
}
