//! Ingest local parquet files into ClickHouse.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use chrono::NaiveDate;
use futures::stream::{self, StreamExt};
use tracing::{info, warn};

use crate::config::Config;
use crate::error::Result;
use crate::mirror::ManifestEntry;

use super::client::ClickHouseClient;
use super::config::ClickHouseConfig;
use super::parquet::parse_agg_trades_parquet;

#[derive(Debug, Clone)]
pub struct IngestOptions {
    pub resume: bool,
    pub parallel: usize,
    pub limit: Option<usize>,
    pub s3_prefix: Option<String>,
    /// Only ingest files on or after this date (from S3 key or manifest period).
    pub since: Option<NaiveDate>,
}

#[derive(Debug, Clone, Default)]
pub struct IngestStatus {
    pub total: usize,
    pub done: usize,
    pub skipped: usize,
    pub failed: usize,
}

pub async fn run_ingest(
    config: &Config,
    ch: &ClickHouseConfig,
    options: IngestOptions,
) -> Result<IngestStatus> {
    let client = Arc::new(ClickHouseClient::new(ch)?);
    client.ping().await?;
    client.ensure_schema().await?;

    let targets = load_local_manifest_targets(config, options.since)?;
    let limited: Vec<PathBuf> = match options.limit {
        Some(n) => targets.into_iter().take(n).collect(),
        None => targets,
    };

    let total = limited.len();
    let client_ref = client.clone();
    let status = stream::iter(limited)
        .map(|path| {
            let client = client_ref.clone();
            async move { ingest_parquet_file(&client, &path).await }
        })
        .buffer_unordered(options.parallel.max(1))
        .fold(
            IngestStatus {
                total,
                ..Default::default()
            },
            |mut status, result| async move {
                match result {
                    Ok(true) => status.done += 1,
                    Ok(false) => status.skipped += 1,
                    Err(err) => {
                        status.failed += 1;
                        warn!(%err, "ingest failed");
                    }
                }
                status
            },
        )
        .await;

    info!(
        total = status.total,
        done = status.done,
        skipped = status.skipped,
        failed = status.failed,
        "ClickHouse ingest finished"
    );
    Ok(status)
}

fn load_local_manifest_targets(config: &Config, since: Option<NaiveDate>) -> Result<Vec<PathBuf>> {
    let manifest_path = config.manifest_path()?;
    if !manifest_path.exists() {
        return Ok(Vec::new());
    }
    let content = std::fs::read_to_string(manifest_path)?;
    let mut paths = Vec::new();
    for line in content.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let entry: ManifestEntry = serde_json::from_str(line)?;
        if entry.status == "ok" {
            if let Some(since) = since {
                if !period_on_or_after(&entry.period, since) {
                    continue;
                }
            }
            if let Some(path) = entry.path {
                paths.push(PathBuf::from(path));
            }
        }
    }
    Ok(paths)
}

fn period_on_or_after(period: &str, since: NaiveDate) -> bool {
    parse_period_date(period).is_some_and(|date| date >= since)
}

fn parse_period_date(period: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(period, "%Y-%m-%d")
        .ok()
        .or_else(|| chrono::NaiveDate::parse_from_str(&format!("{period}-01"), "%Y-%m-%d").ok())
}

async fn ingest_parquet_file(client: &ClickHouseClient, path: &Path) -> Result<bool> {
    if !path.exists() {
        return Ok(false);
    }
    let bytes = std::fs::read(path)?;
    let rows = parse_agg_trades_parquet(&bytes)?;
    if rows.is_empty() {
        return Ok(false);
    }
    client.insert_agg_trades(&rows).await?;
    Ok(true)
}
