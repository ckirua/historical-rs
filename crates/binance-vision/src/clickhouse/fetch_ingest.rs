//! Fetch Vision archives over HTTP and insert into `binance_vision.*` (no S3).

use std::collections::HashSet;
use std::sync::Arc;

use chrono::{DateTime, TimeZone, Utc};
use futures::stream::{self, StreamExt};
use tracing::{info, warn};

use crate::client::VisionHttpClient;
use crate::error::{Error, Result};
use crate::export::dataset_object_key;
use crate::mirror::{
    is_not_found, iter_files, spawn_progress_reporter, MirrorOptions, MirrorProgress,
};
use crate::model::{DataType, VisionFile};
use crate::parse::{extract_csv_from_zip, parse_generic_csv, parse_klines_csv, parse_trades_csv};

use super::client::ClickHouseClient;
use super::config::ClickHouseConfig;
use super::ingest::IngestStatus;
use super::row::{
    VisionBookDepthInsertRow, VisionBvolInsertRow, VisionKlineInsertRow, VisionTradeInsertRow,
};

#[derive(Debug, Clone)]
pub struct FetchIngestOptions {
    pub mirror: MirrorOptions,
    pub limit: Option<usize>,
}

/// Ledger key in `ingest_log` for direct Vision→CH (typed by data product).
pub fn direct_source_key(file: &VisionFile) -> String {
    let dtype = file.data_type.path_segment();
    format!("direct-{dtype}/{}", dataset_object_key(file))
}

pub async fn run_fetch_ingest(
    http: &VisionHttpClient,
    ch: &ClickHouseConfig,
    options: FetchIngestOptions,
) -> Result<IngestStatus> {
    match options.mirror.data_type {
        DataType::Trades | DataType::Klines | DataType::BookDepth | DataType::BvolIndex => {}
        other => {
            return Err(Error::Config(format!(
                "clickhouse fetch supports trades/klines/bookDepth/BVOLIndex, not {}",
                other.path_segment()
            )));
        }
    }

    let client = Arc::new(ClickHouseClient::new(ch)?);
    client.ping().await?;
    client.ensure_schema().await?;

    let ingested = if options.mirror.resume {
        client.load_ingested_s3_keys().await?
    } else {
        HashSet::new()
    };

    let limit = options.limit.unwrap_or(usize::MAX);
    let make_files = || {
        iter_files(&options.mirror)
            .filter(|file| !ingested.contains(&direct_source_key(file)))
            .take(limit)
    };
    let total = make_files().count();
    let files = make_files();
    let parallel = options.mirror.parallel.max(1);
    let label = format!("fetch→{}→CH", options.mirror.data_type.path_segment());
    eprintln!(
        "{label}: {total} files (parallel={}, from={}, to={}, db={})",
        options.mirror.parallel, options.mirror.from, options.mirror.to, ch.database
    );
    let progress = MirrorProgress::new(total);
    let _reporter = spawn_progress_reporter(progress.clone(), &label);

    let http = http.clone();
    let verify = options.mirror.verify_checksum;
    let status = stream::iter(files)
        .map(|file| {
            let http = http.clone();
            let client = client.clone();
            let progress = progress.clone();
            async move {
                let outcome = fetch_one(&http, &client, &file, verify).await;
                match &outcome {
                    Ok(FetchOutcome::Done) => progress.record("ok"),
                    Ok(FetchOutcome::Skipped) => progress.record("skipped"),
                    Err(_) => progress.record("failed"),
                }
                outcome
            }
        })
        .buffer_unordered(parallel)
        .fold(
            IngestStatus {
                total,
                ..Default::default()
            },
            |mut status, result| {
                let client = client.clone();
                async move {
                    match result {
                        Ok(FetchOutcome::Done) => status.done += 1,
                        Ok(FetchOutcome::Skipped) => status.skipped += 1,
                        Err((key, err)) => {
                            status.failed += 1;
                            warn!(%key, %err, "direct fetch ingest failed");
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
        "direct fetch→ClickHouse finished"
    );
    Ok(status)
}

enum FetchOutcome {
    Done,
    Skipped,
}

async fn fetch_one(
    http: &VisionHttpClient,
    client: &ClickHouseClient,
    file: &VisionFile,
    verify_checksum: bool,
) -> std::result::Result<FetchOutcome, (String, Error)> {
    let key = direct_source_key(file);
    let url = http.url_builder().build(file);

    let zip_bytes = match if verify_checksum {
        http.download_with_checksum(&url).await.map(|(b, _)| b)
    } else {
        http.get_bytes(&url).await
    } {
        Ok(bytes) => bytes,
        Err(err) if is_not_found(&err) => {
            client
                .insert_ingest_log(&key, 0, "skipped", Some("not found on Vision"))
                .await
                .map_err(|e| (key.clone(), e))?;
            return Ok(FetchOutcome::Skipped);
        }
        Err(err) => return Err((key, err)),
    };

    let csv = extract_csv_from_zip(&zip_bytes).map_err(|e| (key.clone(), e))?;
    let row_count = match file.data_type {
        DataType::Trades => {
            let rows = parse_trades_csv(&csv, file.market).map_err(|e| (key.clone(), e))?;
            if rows.is_empty() {
                client
                    .insert_ingest_log(&key, 0, "skipped", Some("no rows"))
                    .await
                    .map_err(|e| (key.clone(), e))?;
                return Ok(FetchOutcome::Skipped);
            }
            let ch_rows: Vec<VisionTradeInsertRow> = rows
                .iter()
                .map(|row| VisionTradeInsertRow {
                    kind: file.market.kind_label().to_string(),
                    symbol: file.symbol.clone(),
                    id: row.trade_id,
                    price: row.price,
                    qty: row.qty,
                    quote_qty: row.quote_qty.unwrap_or(row.price * row.qty),
                    time: ms_to_dt(row.timestamp_ms),
                    is_buyer_maker: row.is_buyer_maker,
                })
                .collect();
            let n = ch_rows.len();
            client
                .insert_trades(&ch_rows)
                .await
                .map_err(|e| (key.clone(), e))?;
            n
        }
        DataType::Klines => {
            let interval = file
                .interval
                .clone()
                .ok_or_else(|| (key.clone(), Error::Config("klines require interval".into())))?;
            let rows = parse_klines_csv(&csv, file.market).map_err(|e| (key.clone(), e))?;
            if rows.is_empty() {
                client
                    .insert_ingest_log(&key, 0, "skipped", Some("no rows"))
                    .await
                    .map_err(|e| (key.clone(), e))?;
                return Ok(FetchOutcome::Skipped);
            }
            let ch_rows: Vec<VisionKlineInsertRow> = rows
                .iter()
                .map(|row| VisionKlineInsertRow {
                    kind: file.market.kind_label().to_string(),
                    symbol: file.symbol.clone(),
                    interval: interval.clone(),
                    open_time: ms_to_dt(row.open_time),
                    close_time: ms_to_dt(row.close_time),
                    open: row.open,
                    high: row.high,
                    low: row.low,
                    close: row.close,
                    volume: row.volume,
                    quote_volume: row.quote_volume.or(row.base_volume).unwrap_or(0.0),
                    count: row.num_trades as u32,
                    taker_buy_volume: row
                        .taker_buy_volume
                        .or(row.taker_buy_base_volume)
                        .unwrap_or(0.0),
                    taker_buy_quote_volume: row.taker_buy_quote_volume.unwrap_or(0.0),
                })
                .collect();
            let n = ch_rows.len();
            client
                .insert_klines(&ch_rows)
                .await
                .map_err(|e| (key.clone(), e))?;
            n
        }
        DataType::BookDepth => {
            let table = parse_generic_csv(&csv).map_err(|e| (key.clone(), e))?;
            let ch_rows = parse_book_depth_rows(&table.rows, file).map_err(|e| (key.clone(), e))?;
            if ch_rows.is_empty() {
                client
                    .insert_ingest_log(&key, 0, "skipped", Some("no rows"))
                    .await
                    .map_err(|e| (key.clone(), e))?;
                return Ok(FetchOutcome::Skipped);
            }
            let n = ch_rows.len();
            client
                .insert_book_depth(&ch_rows)
                .await
                .map_err(|e| (key.clone(), e))?;
            n
        }
        DataType::BvolIndex => {
            let table = parse_generic_csv(&csv).map_err(|e| (key.clone(), e))?;
            let ch_rows = parse_bvol_rows(&table.rows, file).map_err(|e| (key.clone(), e))?;
            if ch_rows.is_empty() {
                client
                    .insert_ingest_log(&key, 0, "skipped", Some("no rows"))
                    .await
                    .map_err(|e| (key.clone(), e))?;
                return Ok(FetchOutcome::Skipped);
            }
            let n = ch_rows.len();
            client
                .insert_bvol(&ch_rows)
                .await
                .map_err(|e| (key.clone(), e))?;
            n
        }
        _ => {
            return Err((
                key,
                Error::Config("unsupported data type for binance_vision fetch".into()),
            ));
        }
    };

    client
        .insert_ingest_log(&key, row_count as u64, "ok", None)
        .await
        .map_err(|e| (key.clone(), e))?;
    Ok(FetchOutcome::Done)
}

fn ms_to_dt(ms: i64) -> DateTime<Utc> {
    Utc.timestamp_millis_opt(ms)
        .single()
        .unwrap_or(DateTime::UNIX_EPOCH)
}

fn parse_book_depth_rows(
    rows: &[Vec<String>],
    file: &VisionFile,
) -> Result<Vec<VisionBookDepthInsertRow>> {
    let mut out = Vec::new();
    for cols in rows {
        if cols.len() < 4 {
            continue;
        }
        // Skip header-ish rows.
        if cols[0].eq_ignore_ascii_case("timestamp") || cols[0].eq_ignore_ascii_case("percentage") {
            continue;
        }
        // Vision bookDepth: timestamp, percentage, depth, notional
        // timestamp is usually "YYYY-MM-DD HH:MM:SS" (UTC); occasionally epoch ms/s.
        // percentage is float in CSV (e.g. -0.20); table column is Int8 — round to nearest.
        let ts = parse_book_depth_timestamp(&cols[0])
            .ok_or_else(|| Error::Parse(format!("bookDepth timestamp '{}'", cols[0])))?;
        let pct_f: f64 = cols[1]
            .parse()
            .map_err(|e: std::num::ParseFloatError| Error::Parse(e.to_string()))?;
        let pct = pct_f.round() as i8;
        let depth: f64 = cols[2]
            .parse()
            .map_err(|e: std::num::ParseFloatError| Error::Parse(e.to_string()))?;
        let notional: f64 = cols[3]
            .parse()
            .map_err(|e: std::num::ParseFloatError| Error::Parse(e.to_string()))?;
        out.push(VisionBookDepthInsertRow {
            kind: file.market.kind_label().to_string(),
            symbol: file.symbol.clone(),
            timestamp: ts,
            percentage: pct,
            depth,
            notional,
        });
    }
    Ok(out)
}

fn parse_book_depth_timestamp(raw: &str) -> Option<DateTime<Utc>> {
    let s = raw.trim();
    if let Ok(ts) = s.parse::<i64>() {
        let secs = if ts > 10_000_000_000 { ts / 1000 } else { ts };
        return Utc.timestamp_opt(secs, 0).single();
    }
    // "2026-07-10 00:00:03" (Vision) or ISO-ish with T
    let normalized = s.replace('T', " ");
    let naive = chrono::NaiveDateTime::parse_from_str(&normalized, "%Y-%m-%d %H:%M:%S")
        .or_else(|_| chrono::NaiveDateTime::parse_from_str(&normalized, "%Y-%m-%d %H:%M:%S%.f"))
        .ok()?;
    Some(DateTime::<Utc>::from_naive_utc_and_offset(naive, Utc))
}

fn parse_bvol_rows(rows: &[Vec<String>], file: &VisionFile) -> Result<Vec<VisionBvolInsertRow>> {
    let mut out = Vec::new();
    for cols in rows {
        if cols.len() < 2 {
            continue;
        }
        if cols[0].eq_ignore_ascii_case("calc_time")
            || cols[0].eq_ignore_ascii_case("time")
            || cols[0]
                .chars()
                .any(|c| c.is_alphabetic() && cols[0].len() > 4)
        {
            // heuristic skip header if first cell isn't numeric-ish
            if cols[0].parse::<i64>().is_err() {
                continue;
            }
        }
        // Common: calc_time, symbol, base, quote, index OR calc_time, index
        let calc_ms: i64 = cols[0]
            .parse()
            .or_else(|_| cols.last().and_then(|s| s.parse().ok()).ok_or(()))
            .map_err(|_| Error::Parse("bvol calc_time".into()))?;
        let (base, quote, index) = if cols.len() >= 5 {
            (
                cols[2].clone(),
                cols[3].clone(),
                cols[4].parse::<f32>().unwrap_or(0.0),
            )
        } else if cols.len() >= 2 {
            let idx = cols[1]
                .parse::<f32>()
                .or_else(|_| cols.last().map_or(Ok(0.0), |value| value.parse()))
                .unwrap_or(0.0);
            let base = file
                .symbol
                .trim_end_matches("USDT")
                .trim_end_matches("USD")
                .to_string();
            (base, "USDT".into(), idx)
        } else {
            continue;
        };
        out.push(VisionBvolInsertRow {
            symbol: file.symbol.clone(),
            base_asset: base,
            quote_asset: quote,
            calc_time: ms_to_dt(if calc_ms < 10_000_000_000 {
                calc_ms * 1000
            } else {
                calc_ms
            }),
            index_value: index,
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mirror::plan_files;
    use crate::model::{DataType, Frequency, Market};
    use chrono::NaiveDate;

    #[test]
    fn direct_source_key_is_typed() {
        let file = VisionFile {
            market: Market::Um,
            frequency: Frequency::Daily,
            data_type: DataType::Trades,
            symbol: "BTCUSDT".into(),
            period: "2024-01-15".into(),
            interval: None,
        };
        assert!(direct_source_key(&file).starts_with("direct-trades/"));
    }

    #[test]
    fn plans_trade_files() {
        let files = plan_files(&MirrorOptions {
            market: Market::Um,
            frequency: Frequency::Daily,
            data_type: DataType::Trades,
            symbols: vec!["BTCUSDT".into()],
            from: NaiveDate::from_ymd_opt(2024, 6, 1).unwrap(),
            to: NaiveDate::from_ymd_opt(2024, 6, 2).unwrap(),
            interval: None,
            resume: false,
            parallel: 1,
            verify_checksum: false,
        });
        assert_eq!(files.len(), 2);
    }
}
