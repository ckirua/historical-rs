use std::collections::HashSet;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration as StdDuration;

use chrono::{Duration, NaiveDate};
use serde::Deserialize;
use serde_json::Value;
use tracing::warn;

use crate::config::{Config, INSTRUMENTS_INFO_URL};
use crate::error::{Error, Result};
use crate::model::{Archive, Dataset, Market};

/// Shared circuit breaker for sustained `public.bybit.com` HTTP 403 storms.
///
/// After [`Self::trip_threshold`] consecutive download attempts exhaust retries on
/// HTTP 403, the breaker opens and subsequent downloads fail fast without further
/// long backoff. A successful download resets the consecutive failure counter.
#[derive(Debug)]
pub struct DownloadCircuitBreaker {
    consecutive_403: AtomicU32,
    open: AtomicBool,
    trip_threshold: u32,
}

impl DownloadCircuitBreaker {
    pub fn new(trip_threshold: u32) -> Arc<Self> {
        Arc::new(Self {
            consecutive_403: AtomicU32::new(0),
            open: AtomicBool::new(false),
            trip_threshold: trip_threshold.max(1),
        })
    }

    pub fn trip_threshold(&self) -> u32 {
        self.trip_threshold
    }

    pub fn is_open(&self) -> bool {
        self.open.load(Ordering::Acquire)
    }

    pub fn record_success(&self) {
        self.consecutive_403.store(0, Ordering::Release);
    }

    pub fn record_403_exhausted(&self) {
        let prior = self.consecutive_403.fetch_add(1, Ordering::AcqRel);
        let count = prior.saturating_add(1);
        if count >= self.trip_threshold
            && self
                .open
                .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
        {
            warn!(
                threshold = self.trip_threshold,
                consecutive_403 = count,
                "Bybit download circuit breaker open after sustained HTTP 403"
            );
        }
    }
}

#[derive(Clone)]
pub struct BybitHttpClient {
    client: reqwest::Client,
    config: Config,
}

impl BybitHttpClient {
    pub fn new(config: Config) -> Result<Self> {
        let client = reqwest::Client::builder()
            .user_agent(&config.user_agent)
            .connect_timeout(StdDuration::from_secs(30))
            .timeout(config.timeout())
            .pool_max_idle_per_host(0)
            .build()?;
        Ok(Self { client, config })
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    /// Fetch the symbols currently available for Spot trading.
    pub async fn spot_trading_symbols(&self) -> Result<Vec<String>> {
        self.trading_symbols("spot").await
    }

    /// Fetch Trading symbols for a V5 instruments category (`spot`, `linear`, or `inverse`).
    pub async fn trading_symbols(&self, category: &str) -> Result<Vec<String>> {
        let category = category.trim().to_ascii_lowercase();
        if !matches!(category.as_str(), "spot" | "linear" | "inverse") {
            return Err(Error::Config(format!(
                "unsupported Bybit instruments category `{category}`; expected spot, linear, or inverse"
            )));
        }
        let policy = historical_core::http::RetryPolicy {
            max_retries: self.config.max_retries,
            retry_backoff_ms: self.config.retry_backoff_ms,
            retry_on_429: true,
            ..Default::default()
        };
        let mut cursor: Option<String> = None;
        let mut symbols = Vec::new();
        loop {
            let mut endpoint = format!(
                "{}?category={category}&limit=1000",
                INSTRUMENTS_INFO_URL.trim_end_matches('/')
            );
            if let Some(cursor) = cursor.as_ref() {
                endpoint.push_str("&cursor=");
                endpoint.push_str(cursor);
            }
            let bytes =
                historical_core::http::get_bytes_with_retry(&self.client, &endpoint, policy).await?;
            let page = parse_trading_symbols_page(&bytes, &category)?;
            symbols.extend(page.symbols);
            match page.next_page_cursor {
                Some(next) if !next.is_empty() => cursor = Some(next),
                _ => break,
            }
        }
        symbols.sort();
        symbols.dedup();
        Ok(symbols)
    }

    /// Union of linear + inverse Trading symbols for portal `bizType=contract`.
    pub async fn contract_trading_symbols(&self) -> Result<Vec<String>> {
        let mut symbols = self.trading_symbols("linear").await?;
        symbols.extend(self.trading_symbols("inverse").await?);
        symbols.sort();
        symbols.dedup();
        Ok(symbols)
    }

    /// Discover Trading symbols appropriate for a verified market.
    pub async fn trading_symbols_for_market(&self, market: Market) -> Result<Vec<String>> {
        match market {
            Market::Spot => self.spot_trading_symbols().await,
            Market::Contract => self.contract_trading_symbols().await,
        }
    }

    /// Legacy listing entry point, retaining Spot/daily orderbook behavior.
    pub async fn list_files(
        &self,
        symbol: &str,
        start: NaiveDate,
        end: NaiveDate,
    ) -> Result<Vec<Archive>> {
        self.list_files_for(Dataset::spot_orderbook(), symbol, start, end)
            .await
    }

    /// List a verified Spot or Contract orderbook/trade dataset.
    pub async fn list_files_for(
        &self,
        dataset: Dataset,
        symbol: &str,
        start: NaiveDate,
        end: NaiveDate,
    ) -> Result<Vec<Archive>> {
        if start > end {
            return Err(Error::Config("listing start date must not be after end date".into()));
        }
        // Portal accepts multi-day windows but fails (~ret_code 10016) above ~8 days.
        const LIST_CHUNK_DAYS: i64 = 7;
        let endpoint = format!(
            "{}/list-files",
            self.config.portal_url.trim_end_matches('/')
        );
        let policy = historical_core::http::RetryPolicy {
            max_retries: self.config.max_retries,
            retry_backoff_ms: self.config.retry_backoff_ms,
            retry_on_429: true,
            ..Default::default()
        };
        let mut archives = Vec::new();
        let mut chunk_start = start;
        while chunk_start <= end {
            let chunk_end = std::cmp::min(
                chunk_start + Duration::days(LIST_CHUNK_DAYS - 1),
                end,
            );
            let response = self
                .client
                .get(&endpoint)
                .query(&[
                    ("bizType", dataset.market.to_string()),
                    ("productId", dataset.data_type.to_string()),
                    ("symbols", symbol.to_string()),
                    ("interval", dataset.interval.to_string()),
                    ("startDay", chunk_start.to_string()),
                    ("endDay", chunk_end.to_string()),
                ])
                .send()
                .await?;
            let bytes = if response.status().is_success() {
                response.bytes().await?.to_vec()
            } else if response.status().is_server_error() || response.status().as_u16() == 429 {
                let url = response.url().to_string();
                drop(response);
                historical_core::http::get_bytes_with_retry(&self.client, &url, policy).await?
            } else {
                return Err(Error::HttpStatus {
                    status: response.status().as_u16(),
                    url: response.url().to_string(),
                });
            };
            archives.extend(parse_archives(
                &bytes,
                dataset,
                symbol,
                chunk_start,
                chunk_end,
                None,
            )?);
            chunk_start = chunk_end + Duration::days(1);
        }
        Ok(archives)
    }

    pub async fn download_to_file(&self, url: &str, path: &Path) -> Result<u64> {
        self.download_to_file_with_circuit(url, path, None).await
    }

    /// Download with optional shared circuit breaker for sustained HTTP 403.
    pub async fn download_to_file_with_circuit(
        &self,
        url: &str,
        path: &Path,
        circuit: Option<&DownloadCircuitBreaker>,
    ) -> Result<u64> {
        if let Some(circuit) = circuit {
            if circuit.is_open() {
                return Err(Error::Config(format!(
                    "download circuit breaker open after sustained HTTP 403 (threshold={})",
                    circuit.trip_threshold()
                )));
            }
        }
        let policy = historical_core::http::RetryPolicy {
            max_retries: self.config.max_retries,
            retry_backoff_ms: self.config.download_retry_backoff_ms,
            retry_on_429: true,
            retry_on_403: true,
            max_backoff_ms: self.config.download_max_backoff_ms,
        };
        match historical_core::http::get_to_file_with_retry(&self.client, url, policy, path).await
        {
            Ok(bytes) => {
                if let Some(circuit) = circuit {
                    circuit.record_success();
                }
                Ok(bytes)
            }
            Err(error) => {
                if let Some(circuit) = circuit {
                    if matches!(
                        &error,
                        historical_core::Error::HttpStatus { status: 403, .. }
                    ) {
                        circuit.record_403_exhausted();
                    }
                }
                Err(error.into())
            }
        }
    }
}

#[derive(Debug, Deserialize)]
struct InstrumentsResponse {
    #[serde(rename = "retCode")]
    ret_code: i64,
    #[serde(rename = "retMsg", default)]
    ret_msg: String,
    result: InstrumentsResult,
}

#[derive(Debug, Deserialize)]
struct InstrumentsResult {
    list: Vec<Instrument>,
    #[serde(rename = "nextPageCursor", default)]
    next_page_cursor: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Instrument {
    symbol: String,
    status: String,
}

struct InstrumentsPage {
    symbols: Vec<String>,
    next_page_cursor: Option<String>,
}

fn parse_trading_symbols_page(bytes: &[u8], category: &str) -> Result<InstrumentsPage> {
    let response: InstrumentsResponse = serde_json::from_slice(bytes)?;
    if response.ret_code != 0 {
        return Err(Error::Config(format!(
            "Bybit {category} instruments-info retCode={}: {}",
            response.ret_code, response.ret_msg
        )));
    }
    Ok(InstrumentsPage {
        symbols: response
            .result
            .list
            .into_iter()
            .filter(|instrument| instrument.status == "Trading")
            .map(|instrument| instrument.symbol)
            .collect(),
        next_page_cursor: response.result.next_page_cursor,
    })
}

fn parse_spot_symbols(bytes: &[u8]) -> Result<Vec<String>> {
    Ok(parse_trading_symbols_page(bytes, "spot")?.symbols)
}

fn parse_archives(
    bytes: &[u8],
    dataset: Dataset,
    symbol: &str,
    start: NaiveDate,
    end: NaiveDate,
    fallback_date: Option<NaiveDate>,
) -> Result<Vec<Archive>> {
    let value: Value = serde_json::from_slice(bytes)?;
    if let Some(code) = value.get("ret_code").and_then(|v| v.as_i64()) {
        if code != 0 {
            let msg = value
                .get("ret_msg")
                .and_then(|v| v.as_str())
                .unwrap_or("portal error");
            return Err(Error::Config(format!(
                "Bybit portal list-files ret_code={code}: {msg} ({start}..{end} {symbol})"
            )));
        }
    }
    let mut found = Vec::new();
    collect_urls(
        &value,
        fallback_date,
        dataset,
        symbol,
        start,
        end,
        &mut found,
    );
    let mut seen = HashSet::new();
    found.retain(|archive| seen.insert((archive.date, archive.url.clone())));
    Ok(found)
}

fn collect_urls(
    value: &Value,
    inherited_date: Option<NaiveDate>,
    dataset: Dataset,
    symbol: &str,
    start: NaiveDate,
    end: NaiveDate,
    output: &mut Vec<Archive>,
) {
    match value {
        Value::Object(map) => {
            let object_date = inherited_date.or_else(|| {
                map.iter().find_map(|(key, value)| {
                    if key.eq_ignore_ascii_case("date")
                        || key.eq_ignore_ascii_case("day")
                        || key.eq_ignore_ascii_case("fileDate")
                        || key.eq_ignore_ascii_case("file_date")
                    {
                        value.as_str().and_then(parse_date)
                    } else if key.eq_ignore_ascii_case("fileName")
                        || key.eq_ignore_ascii_case("file_name")
                    {
                        value.as_str().and_then(date_from_text)
                    } else {
                        None
                    }
                })
            });
            for (key, child) in map {
                if let Some(url) = child.as_str().filter(|url| looks_like_archive_url(url)) {
                    if let Some(date) = object_date.or_else(|| date_from_text(url)) {
                        if (start..=end).contains(&date) {
                            output.push(Archive {
                                market: dataset.market,
                                data_type: dataset.data_type,
                                interval: dataset.interval,
                                symbol: symbol.to_string(),
                                date,
                                url: url.to_string(),
                                filename: map_filename(map, url),
                                size: map_size(map),
                            });
                        }
                    }
                } else if key != "message" {
                    collect_urls(child, object_date, dataset, symbol, start, end, output);
                }
            }
        }
        Value::Array(values) => {
            for child in values {
                collect_urls(child, inherited_date, dataset, symbol, start, end, output);
            }
        }
        _ => {}
    }
}

fn map_filename(map: &serde_json::Map<String, Value>, url: &str) -> String {
    for key in ["fileName", "file_name", "filename", "name"] {
        if let Some(name) = map.get(key).and_then(Value::as_str) {
            return name.to_string();
        }
    }
    url.rsplit('/').next().unwrap_or(url).to_string()
}

fn map_size(map: &serde_json::Map<String, Value>) -> Option<u64> {
    for key in ["size", "fileSize", "file_size"] {
        if let Some(value) = map.get(key) {
            if let Some(size) = value.as_u64() {
                return Some(size);
            }
            if let Some(size) = value.as_str().and_then(|value| value.parse().ok()) {
                return Some(size);
            }
        }
    }
    None
}

fn looks_like_archive_url(value: &str) -> bool {
    value.starts_with("http://") || value.starts_with("https://")
}

fn parse_date(value: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(value, "%Y-%m-%d")
        .or_else(|_| NaiveDate::parse_from_str(value, "%Y%m%d"))
        .ok()
}

fn date_from_text(value: &str) -> Option<NaiveDate> {
    let bytes = value.as_bytes();
    for window in bytes.windows(10) {
        if window[4] == b'-'
            && window[7] == b'-'
            && window[..4].iter().all(u8::is_ascii_digit)
            && window[5..7].iter().all(u8::is_ascii_digit)
            && window[8..].iter().all(u8::is_ascii_digit)
        {
            if let Ok(text) = std::str::from_utf8(window) {
                if let Some(date) = parse_date(text) {
                    return Some(date);
                }
            }
        }
    }
    for window in bytes.windows(8) {
        if window.iter().all(u8::is_ascii_digit) {
            if let Ok(text) = std::str::from_utf8(window) {
                if let Some(date) = parse_date(text) {
                    return Some(date);
                }
            }
        }
    }
    None
}

pub fn expected_dates(start: NaiveDate, end: NaiveDate) -> impl Iterator<Item = NaiveDate> {
    std::iter::successors(Some(start), move |date| {
        let next = *date + Duration::days(1);
        (next <= end).then_some(next)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_only_trading_spot_symbols() {
        let body = serde_json::json!({
            "retCode": 0,
            "retMsg": "OK",
            "result": {
                "category": "spot",
                "list": [
                    {"symbol": "BTCUSDT", "status": "Trading"},
                    {"symbol": "ETHUSDT", "status": "PreLaunch"},
                    {"symbol": "SOLUSDT", "status": "Trading"}
                ]
            }
        });

        assert_eq!(
            parse_spot_symbols(body.to_string().as_bytes()).unwrap(),
            ["BTCUSDT", "SOLUSDT"]
        );
    }

    #[test]
    fn parses_linear_trading_symbols_with_cursor() {
        let body = serde_json::json!({
            "retCode": 0,
            "retMsg": "OK",
            "result": {
                "category": "linear",
                "list": [
                    {"symbol": "BTCUSDT", "status": "Trading"},
                    {"symbol": "ETHUSDT", "status": "Closed"}
                ],
                "nextPageCursor": "page-2"
            }
        });
        let page = parse_trading_symbols_page(body.to_string().as_bytes(), "linear").unwrap();
        assert_eq!(page.symbols, ["BTCUSDT"]);
        assert_eq!(page.next_page_cursor.as_deref(), Some("page-2"));
    }

    #[test]
    fn rejects_nonzero_instruments_response() {
        let body = serde_json::json!({
            "retCode": 10001,
            "retMsg": "bad request",
            "result": {"list": []}
        });

        let error = parse_spot_symbols(body.to_string().as_bytes()).unwrap_err();
        assert!(matches!(error, Error::Config(message) if message.contains("retCode=10001")));
    }

    #[test]
    fn circuit_breaker_trips_after_threshold_and_resets_on_success() {
        let circuit = DownloadCircuitBreaker::new(3);
        assert!(!circuit.is_open());
        circuit.record_403_exhausted();
        circuit.record_403_exhausted();
        assert!(!circuit.is_open());
        circuit.record_403_exhausted();
        assert!(circuit.is_open());
        // Success clears the streak but does not re-close an already-open breaker.
        circuit.record_success();
        assert!(circuit.is_open());
    }
}
