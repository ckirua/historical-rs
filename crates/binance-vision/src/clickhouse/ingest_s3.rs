//! Ingest parquet datasets from S3 into ClickHouse.

use std::collections::HashSet;
use std::sync::Arc;

use futures::stream::{self, StreamExt};
use tracing::{info, warn};

use crate::error::{Error, Result};
use crate::mirror::S3Config;

use super::client::ClickHouseClient;
use super::config::ClickHouseConfig;
use super::ingest::{IngestOptions, IngestStatus};
use super::parquet::{
    market_symbol_from_s3_key, parse_agg_trades_parquet_with_context, period_from_s3_key,
};

const DEFAULT_S3_PREFIX: &str = "data/futures/um/daily/aggTrades/";

pub async fn run_s3_ingest(
    s3: &S3Config,
    ch: &ClickHouseConfig,
    options: IngestOptions,
) -> Result<IngestStatus> {
    let client = Arc::new(ClickHouseClient::new(ch)?);
    client.ping().await?;
    client.ensure_schema().await?;

    let prefix = options
        .s3_prefix
        .as_deref()
        .unwrap_or(DEFAULT_S3_PREFIX)
        .to_string();

    let ingested = if options.resume {
        client.load_ingested_s3_keys().await?
    } else {
        HashSet::new()
    };

    let bucket = s3.bucket()?;
    let keys = list_parquet_keys(&bucket, &prefix).await?;
    let mut targets: Vec<String> = keys
        .into_iter()
        .filter(|key| {
            // Skip if already ingested from S3 or via direct Vision→CH (`direct/{key}`).
            !ingested.contains(key) && !ingested.contains(&format!("direct/{key}"))
        })
        .filter(|key| {
            options
                .since
                .is_none_or(|since| period_from_s3_key(key).is_some_and(|d| d >= since))
        })
        .collect();
    if let Some(limit) = options.limit {
        targets.truncate(limit);
    }

    eprintln!(
        "S3 ingest: {} files under {prefix} (resume={}, since={})",
        targets.len(),
        options.resume,
        options
            .since
            .map(|d| d.format("%Y-%m-%d").to_string())
            .unwrap_or_else(|| "none".into())
    );

    let total = targets.len();
    let s3 = Arc::new(s3.clone());
    let client_ref = client.clone();
    let status = stream::iter(targets)
        .map(|key| {
            let s3 = s3.clone();
            let client = client_ref.clone();
            async move { ingest_s3_key(&s3, &client, &key).await }
        })
        .buffer_unordered(options.parallel.max(1))
        .fold(
            IngestStatus {
                total,
                ..Default::default()
            },
            |mut status, result| {
                let client = client.clone();
                async move {
                    match result {
                        Ok(IngestOutcome::Done) => status.done += 1,
                        Ok(IngestOutcome::Skipped) => status.skipped += 1,
                        Err((key, err)) => {
                            status.failed += 1;
                            warn!(%key, %err, "S3 ingest failed");
                            if let Err(log_err) = client
                                .insert_ingest_log(&key, 0, "failed", Some(&err.to_string()))
                                .await
                            {
                                warn!(%log_err, "failed to write ingest_log");
                            }
                        }
                    }
                    status
                }
            },
        )
        .await;

    info!(
        total = status.total,
        done = status.done,
        skipped = status.skipped,
        failed = status.failed,
        "S3 ClickHouse ingest finished"
    );
    Ok(status)
}

enum IngestOutcome {
    Done,
    Skipped,
}

async fn ingest_s3_key(
    s3: &S3Config,
    client: &ClickHouseClient,
    key: &str,
) -> std::result::Result<IngestOutcome, (String, Error)> {
    let bucket = s3.bucket().map_err(|e| (key.to_string(), e))?;
    let response = bucket
        .get_object(key)
        .await
        .map_err(|e| (key.to_string(), Error::S3(e.to_string())))?;
    if !(200..300).contains(&response.status_code()) {
        return Err((
            key.to_string(),
            Error::S3(format!(
                "GET {} returned HTTP {}",
                key,
                response.status_code()
            )),
        ));
    }
    let bytes = response.bytes();
    if bytes.is_empty() {
        client
            .insert_ingest_log(key, 0, "skipped", Some("empty object"))
            .await
            .map_err(|e| (key.to_string(), e))?;
        return Ok(IngestOutcome::Skipped);
    }
    let (market, symbol) = market_symbol_from_s3_key(key).ok_or_else(|| {
        (
            key.to_string(),
            Error::Parse(format!("cannot infer market/symbol from key {key}")),
        )
    })?;
    let rows = parse_agg_trades_parquet_with_context(bytes, Some(&market), Some(&symbol))
        .map_err(|e| (key.to_string(), e))?;
    if rows.is_empty() {
        client
            .insert_ingest_log(key, 0, "skipped", Some("no rows"))
            .await
            .map_err(|e| (key.to_string(), e))?;
        return Ok(IngestOutcome::Skipped);
    }
    let row_count = rows.len();
    client
        .insert_agg_trades(&rows)
        .await
        .map_err(|e| (key.to_string(), e))?;
    client
        .insert_ingest_log(key, row_count as u64, "ok", None)
        .await
        .map_err(|e| (key.to_string(), e))?;
    Ok(IngestOutcome::Done)
}

async fn list_parquet_keys(bucket: &s3::bucket::Bucket, prefix: &str) -> Result<Vec<String>> {
    let pages = bucket
        .list(prefix.to_string(), None)
        .await
        .map_err(|e| Error::S3(e.to_string()))?;
    let mut keys = Vec::new();
    for page in pages {
        for object in page.contents {
            if object.key.ends_with(".parquet.zstd") || object.key.ends_with(".parquet") {
                keys.push(object.key);
            }
        }
    }
    keys.sort();
    Ok(keys)
}
