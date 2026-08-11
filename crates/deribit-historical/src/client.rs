use std::sync::Arc;
use std::time::Duration;

use serde::Deserialize;
use serde_json::Value;
use tracing::{debug, warn};

use crate::auth::AuthToken;
use crate::config::Config;
use crate::error::{Error, Result};
use crate::model::{Instrument, TradeRow, VolatilityIndexCandle};
use crate::rate_limit::TokenBucket;

#[derive(Clone)]
pub struct DeribitHttpClient {
    client: reqwest::Client,
    config: Config,
    limiter: Arc<TokenBucket>,
    instruments_limiter: Arc<TokenBucket>,
    auth: AuthToken,
}

impl DeribitHttpClient {
    pub fn new(config: Config) -> Result<Self> {
        let client = reqwest::Client::builder()
            .user_agent(&config.user_agent)
            .connect_timeout(Duration::from_secs(30))
            .timeout(config.timeout())
            .pool_max_idle_per_host(2)
            .build()?;
        let limiter = TokenBucket::new(config.requests_per_sec);
        let instruments_limiter = TokenBucket::new(config.instruments_requests_per_sec);
        let auth = AuthToken::new(client.clone(), config.clone(), limiter.clone());
        Ok(Self {
            client,
            config,
            limiter,
            instruments_limiter,
            auth,
        })
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    pub fn auth_enabled(&self) -> bool {
        self.auth.enabled()
    }

    pub async fn get_instruments(
        &self,
        currency: &str,
        kind: &str,
        include_expired: bool,
    ) -> Result<Vec<Instrument>> {
        self.instruments_limiter.acquire().await;
        // Live catalog on www — history.deribit.com returns a reduced set and
        // omits DVOL futures (e.g. BTCDVOL_USDC-*). Trades still use history_url.
        let url = format!(
            "{}/public/get_instruments",
            self.config.auth_url.trim_end_matches('/')
        );
        let bytes = self
            .get_json_bytes(
                &url,
                &[
                    ("currency", currency.to_string()),
                    ("kind", kind.to_string()),
                    ("expired", include_expired.to_string()),
                ],
            )
            .await?;
        parse_instruments(&bytes)
    }

    /// First page of trades for a closed UTC day via `get_last_trades_by_instrument_and_time`.
    pub async fn get_trades_page_by_time(
        &self,
        instrument_name: &str,
        start_timestamp_ms: i64,
        end_timestamp_ms: i64,
        count: u32,
    ) -> Result<TradesPage> {
        self.limiter.acquire().await;
        let url = format!(
            "{}/public/get_last_trades_by_instrument_and_time",
            self.config.history_url.trim_end_matches('/')
        );
        let bytes = self
            .get_json_bytes(
                &url,
                &[
                    ("instrument_name", instrument_name.to_string()),
                    ("start_timestamp", start_timestamp_ms.to_string()),
                    ("end_timestamp", end_timestamp_ms.to_string()),
                    ("count", count.min(1000).to_string()),
                    ("sorting", "asc".to_string()),
                ],
            )
            .await?;
        parse_trades_page(&bytes, instrument_name, "")
    }

    /// Continue pagination with `start_seq` via `get_last_trades_by_instrument`.
    pub async fn get_trades_page_by_seq(
        &self,
        instrument_name: &str,
        start_seq: i64,
        count: u32,
    ) -> Result<TradesPage> {
        self.limiter.acquire().await;
        let url = format!(
            "{}/public/get_last_trades_by_instrument",
            self.config.history_url.trim_end_matches('/')
        );
        let bytes = self
            .get_json_bytes(
                &url,
                &[
                    ("instrument_name", instrument_name.to_string()),
                    ("start_seq", start_seq.to_string()),
                    ("count", count.min(1000).to_string()),
                    ("sorting", "asc".to_string()),
                ],
            )
            .await?;
        parse_trades_page(&bytes, instrument_name, "")
    }

    /// Volatility index OHLC candles via `public/get_volatility_index_data` on **www**.
    ///
    /// The history host returns HTTP 400 for this method. Paginate with
    /// `continuation` as the next `end_timestamp` until `continuation` is null.
    pub async fn get_volatility_index_page(
        &self,
        currency: &str,
        start_timestamp_ms: i64,
        end_timestamp_ms: i64,
        resolution: &str,
    ) -> Result<VolatilityIndexPage> {
        self.limiter.acquire().await;
        let url = format!(
            "{}/public/get_volatility_index_data",
            self.config.auth_url.trim_end_matches('/')
        );
        let bytes = self
            .get_json_bytes(
                &url,
                &[
                    ("currency", currency.to_ascii_uppercase()),
                    ("start_timestamp", start_timestamp_ms.to_string()),
                    ("end_timestamp", end_timestamp_ms.to_string()),
                    ("resolution", resolution.to_string()),
                ],
            )
            .await?;
        parse_volatility_index_page(&bytes, currency, resolution)
    }

    async fn get_json_bytes(&self, url: &str, query: &[(&str, String)]) -> Result<Vec<u8>> {
        let mut attempt = 0u32;
        loop {
            let mut request = self.client.get(url).query(query);
            if let Some(token) = self.auth.bearer().await? {
                request = request.bearer_auth(token);
            }
            let response = match request.send().await {
                Ok(response) => response,
                Err(error) if is_retryable_transport(&error) && attempt < self.config.max_retries => {
                    attempt += 1;
                    let delay = self.backoff(attempt);
                    warn!(%url, attempt, ?delay, error = %error, "retrying Deribit transport error");
                    tokio::time::sleep(delay).await;
                    continue;
                }
                Err(error) => return Err(error.into()),
            };
            let status = response.status();
            let bytes = response.bytes().await?.to_vec();
            if status.as_u16() == 429 || is_rate_limit_body(&bytes) {
                attempt += 1;
                if attempt > self.config.max_retries {
                    return Err(Error::RateLimited(format!(
                        "exhausted retries for {url}"
                    )));
                }
                let delay = self.backoff(attempt);
                warn!(%url, attempt, ?delay, "Deribit 10028/429 backoff");
                self.auth.invalidate().await;
                tokio::time::sleep(delay).await;
                continue;
            }
            if status.is_server_error() && attempt < self.config.max_retries {
                attempt += 1;
                let delay = self.backoff(attempt);
                warn!(%url, attempt, ?delay, status = status.as_u16(), "retrying Deribit 5xx");
                tokio::time::sleep(delay).await;
                continue;
            }
            if !status.is_success() {
                return Err(Error::HttpStatus {
                    status: status.as_u16(),
                    url: url.to_string(),
                });
            }
            if let Some(error) = api_error_from_body(&bytes) {
                if error.code == 10028 {
                    attempt += 1;
                    if attempt > self.config.max_retries {
                        return Err(Error::RateLimited(error.message));
                    }
                    let delay = self.backoff(attempt);
                    warn!(%url, attempt, ?delay, "Deribit API 10028 backoff");
                    self.auth.invalidate().await;
                    tokio::time::sleep(delay).await;
                    continue;
                }
                return Err(Error::Api {
                    code: error.code,
                    message: error.message,
                });
            }
            debug!(%url, bytes = bytes.len(), "Deribit response ok");
            return Ok(bytes);
        }
    }

    fn backoff(&self, attempt: u32) -> Duration {
        let shift = attempt.saturating_sub(1).min(8);
        let millis = self
            .config
            .retry_backoff_ms
            .saturating_mul(1u64 << shift)
            .min(self.config.max_backoff_ms);
        Duration::from_millis(millis.max(self.config.retry_backoff_ms))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TradesPage {
    pub trades: Vec<TradeRow>,
    pub has_more: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct VolatilityIndexPage {
    pub candles: Vec<VolatilityIndexCandle>,
    /// Next `end_timestamp` when more history remains; `None` when complete.
    pub continuation: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct ApiErrorBody {
    code: i64,
    message: String,
}

fn is_retryable_transport(error: &reqwest::Error) -> bool {
    error.is_timeout() || error.is_connect()
}

fn is_rate_limit_body(bytes: &[u8]) -> bool {
    api_error_from_body(bytes).is_some_and(|error| error.code == 10028)
}

fn api_error_from_body(bytes: &[u8]) -> Option<ApiErrorBody> {
    let value: Value = serde_json::from_slice(bytes).ok()?;
    let error = value.get("error")?;
    Some(ApiErrorBody {
        code: error.get("code")?.as_i64()?,
        message: error
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("too_many_requests")
            .to_string(),
    })
}

fn parse_instruments(bytes: &[u8]) -> Result<Vec<Instrument>> {
    let value: Value = serde_json::from_slice(bytes)?;
    if let Some(error) = value.get("error") {
        return Err(Error::Api {
            code: error.get("code").and_then(Value::as_i64).unwrap_or(-1),
            message: error
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("instruments error")
                .to_string(),
        });
    }
    let result = value
        .get("result")
        .ok_or_else(|| Error::Config("instruments response missing result".into()))?;
    let list = result
        .as_array()
        .ok_or_else(|| Error::Config("instruments result is not an array".into()))?;
    let mut instruments = Vec::with_capacity(list.len());
    for item in list {
        let instrument_name = item
            .get("instrument_name")
            .and_then(Value::as_str)
            .ok_or_else(|| Error::InvalidRow("instrument missing instrument_name".into()))?
            .to_string();
        // S3 folder currency is settlement (USDC for linear perps), not base.
        let currency = item
            .get("settlement_currency")
            .or_else(|| item.get("base_currency"))
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_ascii_uppercase();
        let kind = item
            .get("kind")
            .and_then(Value::as_str)
            .unwrap_or("future")
            .to_string();
        instruments.push(Instrument {
            instrument_name,
            currency,
            kind,
            is_active: item
                .get("is_active")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            settlement_period: item
                .get("settlement_period")
                .and_then(Value::as_str)
                .map(str::to_string),
            creation_timestamp: item.get("creation_timestamp").and_then(Value::as_i64),
            expiration_timestamp: item.get("expiration_timestamp").and_then(Value::as_i64),
        });
    }
    Ok(instruments)
}

pub fn parse_trades_page(bytes: &[u8], instrument_name: &str, currency: &str) -> Result<TradesPage> {
    let value: Value = serde_json::from_slice(bytes)?;
    if let Some(error) = value.get("error") {
        return Err(Error::Api {
            code: error.get("code").and_then(Value::as_i64).unwrap_or(-1),
            message: error
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("trades error")
                .to_string(),
        });
    }
    let result = value
        .get("result")
        .ok_or_else(|| Error::Config("trades response missing result".into()))?;
    let has_more = result
        .get("has_more")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let trades_value = result
        .get("trades")
        .and_then(Value::as_array)
        .ok_or_else(|| Error::Config("trades result missing trades array".into()))?;
    let mut trades = Vec::with_capacity(trades_value.len());
    for item in trades_value {
        trades.push(parse_trade_row(item, instrument_name, currency)?);
    }
    Ok(TradesPage { trades, has_more })
}

pub fn parse_volatility_index_page(
    bytes: &[u8],
    currency: &str,
    resolution: &str,
) -> Result<VolatilityIndexPage> {
    let value: Value = serde_json::from_slice(bytes)?;
    if let Some(error) = value.get("error") {
        return Err(Error::Api {
            code: error.get("code").and_then(Value::as_i64).unwrap_or(-1),
            message: error
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("volatility index error")
                .to_string(),
        });
    }
    let result = value
        .get("result")
        .ok_or_else(|| Error::Config("volatility index response missing result".into()))?;
    let continuation = result
        .get("continuation")
        .and_then(|value| match value {
            Value::Null => None,
            Value::Number(number) => number.as_i64(),
            _ => None,
        });
    let data = result
        .get("data")
        .and_then(Value::as_array)
        .ok_or_else(|| Error::Config("volatility index result missing data array".into()))?;
    let currency = currency.to_ascii_uppercase();
    let index_name = crate::model::volatility_index_name(&currency);
    let mut candles = Vec::with_capacity(data.len());
    for item in data {
        let row = item
            .as_array()
            .ok_or_else(|| Error::InvalidRow("volatility candle must be an array".into()))?;
        if row.len() < 5 {
            return Err(Error::InvalidRow(format!(
                "volatility candle expected 5 values, got {}",
                row.len()
            )));
        }
        let timestamp_ms = row[0]
            .as_i64()
            .or_else(|| row[0].as_f64().map(|v| v as i64))
            .ok_or_else(|| Error::InvalidRow("volatility candle missing timestamp".into()))?;
        let open = row[1]
            .as_f64()
            .ok_or_else(|| Error::InvalidRow("volatility candle missing open".into()))?;
        let high = row[2]
            .as_f64()
            .ok_or_else(|| Error::InvalidRow("volatility candle missing high".into()))?;
        let low = row[3]
            .as_f64()
            .ok_or_else(|| Error::InvalidRow("volatility candle missing low".into()))?;
        let close = row[4]
            .as_f64()
            .ok_or_else(|| Error::InvalidRow("volatility candle missing close".into()))?;
        candles.push(VolatilityIndexCandle {
            currency: currency.clone(),
            index_name: index_name.clone(),
            resolution: resolution.to_string(),
            timestamp_ms,
            open,
            high,
            low,
            close,
        });
    }
    Ok(VolatilityIndexPage {
        candles,
        continuation,
    })
}

fn parse_trade_row(item: &Value, fallback_instrument: &str, currency: &str) -> Result<TradeRow> {
    let instrument_name = item
        .get("instrument_name")
        .and_then(Value::as_str)
        .unwrap_or(fallback_instrument)
        .to_string();
    let trade_id = item
        .get("trade_id")
        .map(|value| match value {
            Value::String(text) => Ok(text.clone()),
            Value::Number(number) => Ok(number.to_string()),
            _ => Err(Error::InvalidRow("trade_id must be string or number".into())),
        })
        .transpose()?
        .ok_or_else(|| Error::InvalidRow("trade missing trade_id".into()))?;
    let trade_seq = item
        .get("trade_seq")
        .and_then(Value::as_i64)
        .ok_or_else(|| Error::InvalidRow("trade missing trade_seq".into()))?;
    let timestamp_ms = item
        .get("timestamp")
        .and_then(Value::as_i64)
        .ok_or_else(|| Error::InvalidRow("trade missing timestamp".into()))?;
    let direction = item
        .get("direction")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let price = item
        .get("price")
        .and_then(Value::as_f64)
        .ok_or_else(|| Error::InvalidRow("trade missing price".into()))?;
    let amount = item
        .get("amount")
        .and_then(Value::as_f64)
        .ok_or_else(|| Error::InvalidRow("trade missing amount".into()))?;
    Ok(TradeRow {
        instrument_name,
        currency: currency.to_ascii_uppercase(),
        trade_id,
        trade_seq,
        timestamp_ms,
        direction,
        price,
        amount,
        contracts: item.get("contracts").and_then(Value::as_f64),
        index_price: item.get("index_price").and_then(Value::as_f64),
        mark_price: item.get("mark_price").and_then(Value::as_f64),
        tick_direction: item
            .get("tick_direction")
            .and_then(Value::as_i64)
            .map(|value| value as i32),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_instruments_fixture_shape() {
        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "result": [{
                "instrument_name": "BTC-PERPETUAL",
                "base_currency": "BTC",
                "kind": "future",
                "is_active": true,
                "settlement_period": "perpetual"
            }]
        });
        let instruments = parse_instruments(body.to_string().as_bytes()).unwrap();
        assert_eq!(instruments.len(), 1);
        assert_eq!(instruments[0].instrument_name, "BTC-PERPETUAL");
        assert_eq!(instruments[0].currency, "BTC");
    }

    #[test]
    fn parses_usdc_perp_uses_settlement_currency() {
        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "result": [{
                "instrument_name": "BTC_USDC-PERPETUAL",
                "base_currency": "BTC",
                "settlement_currency": "USDC",
                "quote_currency": "USDC",
                "kind": "future",
                "is_active": true,
                "settlement_period": "perpetual"
            }]
        });
        let instruments = parse_instruments(body.to_string().as_bytes()).unwrap();
        assert_eq!(instruments[0].currency, "USDC");
    }

    #[test]
    fn parses_trades_page_fixture() {
        let body = include_bytes!("../tests/fixtures/trades_page.json");
        let page = parse_trades_page(body, "BTC-PERPETUAL", "BTC").unwrap();
        assert!(page.has_more);
        assert_eq!(page.trades.len(), 2);
        assert_eq!(page.trades[0].trade_seq, 1);
        assert_eq!(page.trades[0].currency, "BTC");
    }

    #[test]
    fn parses_volatility_index_fixture() {
        let body = include_bytes!("../tests/fixtures/volatility_index_page.json");
        let page = parse_volatility_index_page(body, "BTC", "3600").unwrap();
        assert_eq!(page.candles.len(), 2);
        assert_eq!(page.candles[0].index_name, "btcdvol_usdc");
        assert_eq!(page.candles[0].open, 35.08);
        assert_eq!(page.continuation, Some(1_788_000_000_000));
    }
}
