use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use chrono::NaiveDate;
use futures::stream::{self, StreamExt};
use tracing::{info, warn};

use crate::client::{expected_dates, BybitHttpClient, DownloadCircuitBreaker};
use crate::error::{Error, Result};
use crate::export::{convert_archive, convert_trade_archive};
use crate::model::{manifest_key_for, Archive, Dataset, ManifestEntry, MirrorStatus};
pub use crate::model::{CoverageCell, CoverageReport};

/// End-of-run mirror counters plus a failure-reason histogram.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MirrorRunReport {
    pub status: MirrorStatus,
    /// Failure reason classes sorted by count descending.
    pub failure_histogram: Vec<(String, usize)>,
}

/// Classify a manifest `error` string into a stable histogram bucket.
pub fn classify_failure(error: &str) -> String {
    let trimmed = error.trim();
    if trimmed.is_empty() {
        return "unknown".into();
    }
    let lower = trimmed.to_ascii_lowercase();
    if lower.contains("circuit breaker open") {
        return "circuit breaker open".into();
    }
    if lower.contains("s3 upload") {
        return "S3 upload".into();
    }
    if lower.contains("portal listing failed") {
        return "portal listing".into();
    }
    if lower.contains("archive url is empty") {
        return "empty URL".into();
    }
    if lower.contains("portal did not list") {
        return "portal gap".into();
    }
    if let Some(code) = extract_http_status(&lower) {
        return match code {
            403 => "HTTP 403".into(),
            429 => "HTTP 429".into(),
            500..=599 => "HTTP 5xx".into(),
            other => format!("HTTP {other}"),
        };
    }
    if lower.contains("dns")
        || lower.contains("name or service not known")
        || lower.contains("failed to lookup address")
    {
        return "DNS".into();
    }
    if lower.contains("connection reset")
        || lower.contains("connection refused")
        || lower.contains("timed out")
        || lower.contains("timeout")
        || lower.contains("error sending request")
    {
        return "connection/timeout".into();
    }
    // Stable prefix: up to first " for " (URL suffix) or 96 chars.
    let prefix = trimmed
        .split_once(" for ")
        .map(|(head, _)| head)
        .unwrap_or(trimmed);
    let mut class = prefix.chars().take(96).collect::<String>();
    if prefix.chars().count() > 96 {
        class.push('…');
    }
    class
}

fn extract_http_status(lower: &str) -> Option<u16> {
    const MARKER: &str = "http ";
    let mut search = lower;
    while let Some(idx) = search.find(MARKER) {
        let after = &search[idx + MARKER.len()..];
        let digits: String = after.chars().take_while(|c| c.is_ascii_digit()).collect();
        if let Ok(code) = digits.parse::<u16>() {
            return Some(code);
        }
        search = &after[digits.len().max(1)..];
    }
    None
}

/// Build a descending histogram from failure error strings.
pub fn failure_histogram_from_errors<'a, I>(errors: I) -> Vec<(String, usize)>
where
    I: IntoIterator<Item = &'a str>,
{
    let mut counts: HashMap<String, usize> = HashMap::new();
    for error in errors {
        *counts.entry(classify_failure(error)).or_insert(0) += 1;
    }
    let mut rows: Vec<_> = counts.into_iter().collect();
    rows.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    rows
}

fn record_failure(histogram: &mut HashMap<String, usize>, error: Option<&str>) {
    if let Some(error) = error {
        *histogram.entry(classify_failure(error)).or_insert(0) += 1;
    }
}

fn finalize_histogram(counts: HashMap<String, usize>) -> Vec<(String, usize)> {
    let mut rows: Vec<_> = counts.into_iter().collect();
    rows.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    rows
}

#[derive(Debug, Clone)]
struct ManifestCell {
    status: String,
    error: Option<String>,
    complete: bool,
}

/// Reconcile expected symbol/date cells with a Bybit manifest.
///
/// This helper is deliberately pure: it only examines the supplied dataset,
/// symbols, date range, and manifest entries. Entries for another dataset or
/// malformed dates are ignored. Duplicate entries for a cell are collapsed;
/// any successful (`ok` or `skipped`) entry makes that cell complete, matching
/// resume behavior.
pub fn reconcile_coverage(
    dataset: Dataset,
    symbols: &[String],
    from: NaiveDate,
    to: NaiveDate,
    manifest_entries: &[ManifestEntry],
) -> CoverageReport {
    let mut by_cell: HashMap<(String, NaiveDate), ManifestCell> = HashMap::new();
    for entry in manifest_entries {
        if !entry.exchange.eq_ignore_ascii_case("bybit")
            || !entry.data_type.eq_ignore_ascii_case(dataset.data_type.as_str())
            || !entry.market.eq_ignore_ascii_case(dataset.market.as_str())
            || !entry.interval.eq_ignore_ascii_case(dataset.interval.as_str())
        {
            continue;
        }
        let Ok(date) = NaiveDate::parse_from_str(&entry.date, "%Y-%m-%d") else {
            continue;
        };
        let status = entry.status.to_ascii_lowercase();
        let complete = matches!(status.as_str(), "ok" | "skipped") && entry.error.is_none();
        let key = (entry.symbol.to_ascii_uppercase(), date);
        match by_cell.get(&key) {
            Some(existing) if existing.complete && !complete => continue,
            _ => {
                by_cell.insert(
                    key,
                    ManifestCell {
                        status,
                        error: entry.error.clone(),
                        complete,
                    },
                );
            }
        }
    }

    let mut unique_symbols = Vec::with_capacity(symbols.len());
    let mut seen_symbols = HashSet::with_capacity(symbols.len());
    for symbol in symbols {
        let key = symbol.to_ascii_uppercase();
        if seen_symbols.insert(key) {
            unique_symbols.push(symbol.as_str());
        }
    }
    let dates: Vec<_> = if from <= to {
        expected_dates(from, to).collect()
    } else {
        Vec::new()
    };

    let mut report = CoverageReport {
        requested: unique_symbols.len() * dates.len(),
        ..CoverageReport::default()
    };
    for symbol in unique_symbols {
        let symbol_key = symbol.to_ascii_uppercase();
        for date in &dates {
            match by_cell.get(&(symbol_key.clone(), *date)) {
                Some(cell) if cell.complete && cell.status == "ok" => report.ok += 1,
                Some(cell) if cell.complete && cell.status == "skipped" => report.skipped += 1,
                Some(cell) => {
                    report.failed += 1;
                    report.failed_cells.push(CoverageCell {
                        symbol: symbol.to_string(),
                        date: *date,
                        status: Some(cell.status.clone()),
                        error: cell.error.clone(),
                    });
                }
                None => {
                    report.missing += 1;
                    report.missing_cells.push(CoverageCell {
                        symbol: symbol.to_string(),
                        date: *date,
                        status: None,
                        error: None,
                    });
                }
            }
        }
    }
    report
}

pub const S3_MANIFEST_KEY: &str = "historical/manifest/bybit/orderbook/manifest.jsonl";

#[derive(Clone)]
pub struct S3Config(historical_core::S3Config);

impl std::fmt::Debug for S3Config {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(formatter)
    }
}

impl S3Config {
    pub fn from_env() -> Result<Self> {
        Ok(Self(historical_core::S3Config::from_env_keys(
            &["S3_BYBIT_HISTORICAL_BUCKET", "S3_BYBIT_BUCKET", "S3_BUCKET"],
            &["S3_BYBIT_HISTORICAL_ENDPOINT", "S3_URL"],
            &["S3_BYBIT_HISTORICAL_REGION", "S3_REGION"],
            &[
                "S3_BYBIT_HISTORICAL_ACCESS_KEY",
                "S3_BYBIT_ACCESS_KEY",
                "S3_ACCESS_KEY",
            ],
            &[
                "S3_BYBIT_HISTORICAL_SECRET_KEY",
                "S3_BYBIT_SECRET_KEY",
                "S3_SECRET_KEY",
            ],
            "mercat-bybit",
            "fsn1",
        )?))
    }

    pub(crate) fn core(&self) -> &historical_core::S3Config {
        &self.0
    }

    pub(crate) fn bucket(&self) -> Result<Box<s3::bucket::Bucket>> {
        Ok(self.0.bucket()?)
    }
}

#[derive(Debug, Clone)]
pub struct MirrorOptions {
    pub symbols: Vec<String>,
    pub from: NaiveDate,
    pub to: NaiveDate,
    pub resume: bool,
    pub parallel: usize,
}

/// Object-key prefix for resume listing. Must stay dataset-scoped — listing all of
/// `historical/data/` pulls every product (e.g. orderbook) into memory and can hang
/// or starve a first trade/contract run.
fn resume_data_prefix(dataset: Dataset) -> String {
    if dataset == Dataset::spot_orderbook() {
        "historical/data/orderbook/".into()
    } else {
        format!(
            "historical/data/{}/{}/{}/",
            dataset.data_type, dataset.market, dataset.interval
        )
    }
}

/// Build the resume skip set from objects already under the dataset prefix.
///
/// Listing existing parquet keys is the source of truth for S3 resume. Merging the
/// full JSONL manifest is intentionally skipped: a prior failed run can leave a
/// multi-hundred-thousand-line failure-only manifest that stalls startup for hours
/// while adding no successful keys.
async fn load_resume_keys(
    s3: &S3Config,
    dataset: Dataset,
    manifest_key: String,
) -> Option<Arc<HashSet<String>>> {
    let data_prefix = resume_data_prefix(dataset);
    info!(%data_prefix, %manifest_key, "loading S3 resume keys from data prefix");
    match historical_core::s3_manifest::list_s3_object_keys(s3.core(), &data_prefix).await {
        Ok(keys) => {
            info!(resume_keys = keys.len(), "S3 resume key set ready");
            Some(Arc::new(keys.into_iter().collect()))
        }
        Err(error) => {
            warn!(%error, "unable to list Bybit S3 data keys; using HEAD resume checks");
            None
        }
    }
}

/// Legacy mirror entry point, retaining Spot/daily orderbook behavior.
pub async fn run_s3_mirror(
    http: &BybitHttpClient,
    s3: &S3Config,
    options: MirrorOptions,
) -> Result<MirrorRunReport> {
    run_s3_mirror_for(http, s3, options, Dataset::spot_orderbook()).await
}

/// Mirror a verified dataset.
pub async fn run_s3_mirror_for(
    http: &BybitHttpClient,
    s3: &S3Config,
    options: MirrorOptions,
    dataset: Dataset,
) -> Result<MirrorRunReport> {
    if options.from > options.to {
        return Err(Error::Config("--from must not be after --to".into()));
    }
    let parallel = options.parallel.max(1);
    let symbol_count = options.symbols.len();
    info!(
        symbols = symbol_count,
        from = %options.from,
        to = %options.to,
        parallel,
        dataset = %dataset,
        "planning Bybit portal listings (7-day chunks per symbol)"
    );
    let portal_url = http.config().portal_url.clone();
    let from = options.from;
    let to = options.to;
    let listed = stream::iter(options.symbols.clone())
        .enumerate()
        .map(|(idx, symbol)| {
            let http = http.clone();
            async move {
                let result = http.list_files_for(dataset, &symbol, from, to).await;
                info!(
                    progress = format!("{}/{}", idx + 1, symbol_count),
                    %symbol,
                    "portal listing progress"
                );
                (symbol, result)
            }
        })
        .buffer_unordered(parallel)
        .collect::<Vec<_>>()
        .await;

    let mut planned = Vec::new();
    let mut planning_gaps = Vec::new();
    let mut planning_failures = Vec::new();
    for (symbol, result) in listed {
        match result {
            Ok(files) => {
                let mut by_date: HashMap<NaiveDate, Archive> = files
                    .into_iter()
                    .filter(|file| file.symbol.eq_ignore_ascii_case(&symbol))
                    .map(|file| (file.date, file))
                    .collect();
                for date in expected_dates(from, to) {
                    if let Some(file) = by_date.remove(&date) {
                        planned.push(file);
                    } else {
                        let mut entry =
                            ManifestEntry::missing(dataset, &symbol, date, portal_url.clone());
                        entry.error =
                            Some("portal did not list an archive for this date".into());
                        planning_gaps.push(entry);
                    }
                }
            }
            Err(error) => {
                for date in expected_dates(from, to) {
                    let mut entry =
                        ManifestEntry::missing(dataset, &symbol, date, portal_url.clone());
                    // Real listing errors remain hard failures for the CLI exit code.
                    entry.status = "failed".into();
                    entry.error = Some(format!("portal listing failed: {error}"));
                    planning_failures.push(entry);
                }
            }
        }
    }
    info!(
        planned = planned.len(),
        planning_gaps = planning_gaps.len(),
        planning_failures = planning_failures.len(),
        "portal planning complete; starting downloads/uploads"
    );

    let resume_keys = if options.resume {
        load_resume_keys(s3, dataset, manifest_key_for(dataset)).await
    } else {
        None
    };
    let manifest_path = manifest_key_for(dataset);
    let s3 = Arc::new(s3.clone());
    let manifest = Arc::new(historical_core::S3ManifestWriter::<ManifestEntry>::new(
        Arc::new(s3.core().clone()),
        &manifest_path,
    ));
    let mut status = MirrorStatus::default();
    let mut failure_counts: HashMap<String, usize> = HashMap::new();
    // Count portal date gaps in-process only. Persisting hundreds of thousands of
    // expected absences bloated the trade JSONL (~119MB) and stalled
    // write_compatibility_snapshot for hours. Resume already uses data-prefix listing.
    // Gaps stay non-failed (`source-not-found` → pending); do not histogram them.
    for entry in planning_gaps {
        status.record(&entry.status);
    }
    for entry in planning_failures {
        status.record(&entry.status);
        if entry.status == "failed" {
            record_failure(&mut failure_counts, entry.error.as_deref());
        }
        manifest.record(entry).await;
    }

    let circuit = DownloadCircuitBreaker::new(http.config().download_403_circuit_threshold);
    let http = http.clone();
    let resume = options.resume;
    let (worker_status, worker_failures) = stream::iter(planned)
        .map(|file| {
            let http = http.clone();
            let s3 = s3.clone();
            let manifest = manifest.clone();
            let resume_keys = resume_keys.clone();
            let circuit = circuit.clone();
            async move {
                let entry = upload_archive(
                    &http,
                    &s3,
                    &file,
                    resume,
                    resume_keys.as_deref(),
                    Some(circuit.as_ref()),
                )
                .await;
                let result = (entry.status.clone(), entry.error.clone());
                manifest.record(entry).await;
                result
            }
        })
        .buffer_unordered(parallel)
        .fold(
            (MirrorStatus::default(), HashMap::new()),
            |(mut status, mut failures), (item_status, error)| async move {
                status.record(&item_status);
                if item_status == "failed" {
                    record_failure(&mut failures, error.as_deref());
                }
                (status, failures)
            },
        )
        .await;
    status.merge(worker_status);
    for (class, count) in worker_failures {
        *failure_counts.entry(class).or_insert(0) += count;
    }
    manifest.flush().await?;
    if let Err(error) = manifest.write_compatibility_snapshot().await {
        warn!(
            %error,
            "Bybit compatibility snapshot failed; segment manifests remain authoritative"
        );
    }
    Ok(MirrorRunReport {
        status,
        failure_histogram: finalize_histogram(failure_counts),
    })
}

async fn upload_archive(
    http: &BybitHttpClient,
    s3: &S3Config,
    file: &Archive,
    resume: bool,
    resume_keys: Option<&HashSet<String>>,
    circuit: Option<&DownloadCircuitBreaker>,
) -> ManifestEntry {
    let mut entry = ManifestEntry::new(file);
    let key = file.object_key();
    if resume {
        if let Some(keys) = resume_keys {
            if keys.contains(&key) {
                entry.status = "skipped".into();
                return entry;
            }
        }
    }
    let bucket = match s3.bucket() {
        Ok(bucket) => bucket,
        Err(error) => {
            entry.status = "failed".into();
            entry.error = Some(error.to_string());
            return entry;
        }
    };
    if resume && resume_keys.is_none() {
        match bucket.head_object(&key).await {
            Ok((_, code)) if (200..300).contains(&code) => {
                entry.status = "skipped".into();
                return entry;
            }
            Ok((_, 404)) => {}
            Ok((_, code)) => warn!(%key, code, "unexpected Bybit resume HEAD status"),
            Err(error) => warn!(%key, %error, "Bybit resume HEAD failed"),
        }
    }
    if file.url.is_empty() {
        entry.status = "failed".into();
        entry.error = Some("archive URL is empty".into());
        return entry;
    }
    let archive = match tempfile::NamedTempFile::new() {
        Ok(file) => file,
        Err(error) => return failed(entry, error.to_string()),
    };
    if let Err(error) = http
        .download_to_file_with_circuit(&file.url, archive.path(), circuit)
        .await
    {
        return failed(entry, error.to_string());
    }
    let output = match tempfile::NamedTempFile::new() {
        Ok(file) => file,
        Err(error) => return failed(entry, error.to_string()),
    };
    let rows = match if file.dataset().is_trade() {
        convert_trade_archive(archive.path(), &file.symbol, output.path())
    } else {
        convert_archive(archive.path(), &file.symbol, output.path())
    } {
        Ok(rows) => rows,
        Err(error) => return failed(entry, error.to_string()),
    };
    let mut parquet = match tokio::fs::File::open(output.path()).await {
        Ok(file) => file,
        Err(error) => return failed(entry, error.to_string()),
    };
    match bucket
        .put_object_stream_with_content_type(&mut parquet, &key, "application/vnd.apache.parquet")
        .await
    {
        Ok(response) if (200..300).contains(&response.status_code()) => {
            entry.status = "ok".into();
            entry.rows = Some(rows);
        }
        Ok(response) => {
            return failed(
                entry,
                format!("S3 upload failed with HTTP {}", response.status_code()),
            )
        }
        Err(error) => return failed(entry, error.to_string()),
    }
    entry
}

fn failed(mut entry: ManifestEntry, error: String) -> ManifestEntry {
    entry.status = "failed".into();
    entry.error = Some(error);
    entry
}

pub async fn mirror_status_s3(s3: &S3Config) -> Result<MirrorStatus> {
    mirror_status_s3_for(s3, Dataset::spot_orderbook()).await
}

pub async fn mirror_status_s3_for(s3: &S3Config, dataset: Dataset) -> Result<MirrorStatus> {
    Ok(historical_core::s3_manifest::mirror_status_s3(
        s3.core(),
        &manifest_key_for(dataset),
    )
    .await?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(dataset: Dataset, symbol: &str, date: &str, status: &str) -> ManifestEntry {
        ManifestEntry {
            exchange: "bybit".into(),
            data_type: dataset.data_type.to_string(),
            market: dataset.market.to_string(),
            interval: dataset.interval.to_string(),
            symbol: symbol.into(),
            date: date.into(),
            source_url: "https://example.test/archive".into(),
            filename: None,
            size: None,
            path: None,
            status: status.into(),
            rows: None,
            error: None,
        }
    }

    #[test]
    fn reconciles_counts_and_cells_in_expected_order() {
        let dataset = Dataset::spot_orderbook();
        let symbols = vec!["BTCUSDT".into(), "ETHUSDT".into(), "btcusdt".into()];
        let mut failed_entry = entry(dataset, "BTCUSDT", "2024-01-02", "failed");
        failed_entry.error = Some("portal unavailable".into());
        let entries = vec![
            entry(dataset, "BTCUSDT", "2024-01-01", "ok"),
            failed_entry,
            entry(dataset, "ETHUSDT", "2024-01-01", "skipped"),
            entry(Dataset::spot_trade(), "ETHUSDT", "2024-01-02", "ok"),
        ];

        let report = reconcile_coverage(
            dataset,
            &symbols,
            NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
            NaiveDate::from_ymd_opt(2024, 1, 2).unwrap(),
            &entries,
        );

        assert_eq!(report.requested, 4);
        assert_eq!(report.ok, 1);
        assert_eq!(report.skipped, 1);
        assert_eq!(report.failed, 1);
        assert_eq!(report.missing, 1);
        assert_eq!(
            report.failed_cells,
            vec![CoverageCell {
                symbol: "BTCUSDT".into(),
                date: NaiveDate::from_ymd_opt(2024, 1, 2).unwrap(),
                status: Some("failed".into()),
                error: Some("portal unavailable".into()),
            }]
        );
        assert_eq!(
            report.missing_cells,
            vec![CoverageCell {
                symbol: "ETHUSDT".into(),
                date: NaiveDate::from_ymd_opt(2024, 1, 2).unwrap(),
                status: None,
                error: None,
            }]
        );
        assert_eq!(report.complete(), 2);
    }

    #[test]
    fn successful_duplicate_resolves_an_earlier_failure() {
        let dataset = Dataset::spot_orderbook();
        let mut failed_entry = entry(dataset, "BTCUSDT", "2024-01-01", "failed");
        failed_entry.error = Some("temporary".into());
        let entries = vec![failed_entry, entry(dataset, "BTCUSDT", "2024-01-01", "ok")];

        let report = reconcile_coverage(
            dataset,
            &["BTCUSDT".into()],
            NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
            NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
            &entries,
        );

        assert_eq!((report.ok, report.skipped, report.failed, report.missing), (1, 0, 0, 0));
        assert!(report.failed_cells.is_empty());
        assert!(report.missing_cells.is_empty());
    }

    #[test]
    fn reversed_date_range_has_no_requested_cells() {
        let report = reconcile_coverage(
            Dataset::spot_orderbook(),
            &["BTCUSDT".into()],
            NaiveDate::from_ymd_opt(2024, 1, 2).unwrap(),
            NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
            &[],
        );
        assert_eq!(report, CoverageReport::default());
    }

    #[test]
    fn classifies_dominant_403_and_circuit_breaker_errors() {
        assert_eq!(
            classify_failure(
                "shared infrastructure error: HTTP 403 for https://public.bybit.com/spot/ADAUSDT/ADAUSDT_2025-08-26.csv.gz"
            ),
            "HTTP 403"
        );
        assert_eq!(
            classify_failure(
                "download circuit breaker open after sustained HTTP 403 (threshold=24)"
            ),
            "circuit breaker open"
        );
        assert_eq!(
            classify_failure("portal listing failed: HTTP status 504 for https://example"),
            "portal listing"
        );
        assert_eq!(classify_failure("S3 upload failed with HTTP 503"), "S3 upload");
        assert_eq!(
            classify_failure("error sending request for url (https://x): dns error"),
            "DNS"
        );
    }

    #[test]
    fn failure_histogram_sorts_by_count_descending() {
        let rows = failure_histogram_from_errors([
            "shared infrastructure error: HTTP 403 for https://a",
            "shared infrastructure error: HTTP 403 for https://b",
            "archive URL is empty",
            "shared infrastructure error: HTTP 403 for https://c",
        ]);
        assert_eq!(
            rows,
            vec![("HTTP 403".into(), 3), ("empty URL".into(), 1)]
        );
    }
}
