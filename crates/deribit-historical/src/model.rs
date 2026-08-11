use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

pub const DATA_TYPE_TRADE: &str = "trade";
pub const DATA_TYPE_VOLATILITY_INDEX: &str = "volatility_index";
pub const EXCHANGE: &str = "deribit";
pub const S3_MANIFEST_KEY: &str = "historical/manifest/deribit/trade/manifest.jsonl";
pub const S3_DATA_PREFIX: &str = "historical/data/trade/";
pub const S3_VOL_MANIFEST_KEY: &str =
    "historical/manifest/deribit/volatility_index/manifest.jsonl";
pub const S3_VOL_DATA_PREFIX: &str = "historical/data/volatility_index/";

/// One Deribit future (incl. perpetual) instrument used for trade mirroring.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Instrument {
    pub instrument_name: String,
    pub currency: String,
    pub kind: String,
    pub is_active: bool,
    #[serde(default)]
    pub settlement_period: Option<String>,
    #[serde(default)]
    pub creation_timestamp: Option<i64>,
    #[serde(default)]
    pub expiration_timestamp: Option<i64>,
}

/// One public trade row for parquet export.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TradeRow {
    pub instrument_name: String,
    pub currency: String,
    pub trade_id: String,
    pub trade_seq: i64,
    pub timestamp_ms: i64,
    pub direction: String,
    pub price: f64,
    pub amount: f64,
    #[serde(default)]
    pub contracts: Option<f64>,
    #[serde(default)]
    pub index_price: Option<f64>,
    #[serde(default)]
    pub mark_price: Option<f64>,
    #[serde(default)]
    pub tick_direction: Option<i32>,
}

/// One DVOL / volatility-index OHLC candle from `public/get_volatility_index_data`.
///
/// Deribit returns values as annualized volatility in **percent** (e.g. `35.08`),
/// not a 0–1 fraction. Index price names: `btcdvol_usdc`, `ethdvol_usdc`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VolatilityIndexCandle {
    pub currency: String,
    pub index_name: String,
    pub resolution: String,
    pub timestamp_ms: i64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
}

/// Planned UTC day cell for one instrument.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DayJob {
    pub currency: String,
    pub instrument_name: String,
    pub date: NaiveDate,
}

impl DayJob {
    pub fn object_key(&self) -> String {
        storage_key(&self.currency, &self.instrument_name, self.date)
    }
}

/// Planned UTC day cell for one volatility-index currency + resolution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VolDayJob {
    pub currency: String,
    pub resolution: String,
    pub date: NaiveDate,
}

impl VolDayJob {
    pub fn object_key(&self) -> String {
        volatility_storage_key(&self.currency, &self.resolution, self.date)
    }

    pub fn index_name(&self) -> String {
        volatility_index_name(&self.currency)
    }
}

/// Object key layout (v1):
/// `historical/data/trade/{currency}/{instrument}/{YYYY-MM-DD}.parquet.zstd`
pub fn storage_key(currency: &str, instrument: &str, date: NaiveDate) -> String {
    format!(
        "historical/data/trade/{}/{}/{}.parquet.zstd",
        currency.to_ascii_uppercase(),
        instrument,
        date.format("%Y-%m-%d")
    )
}

/// `historical/data/volatility_index/{currency}/{resolution}/{YYYY-MM-DD}.parquet.zstd`
pub fn volatility_storage_key(currency: &str, resolution: &str, date: NaiveDate) -> String {
    format!(
        "historical/data/volatility_index/{}/{}/{}.parquet.zstd",
        currency.to_ascii_uppercase(),
        resolution,
        date.format("%Y-%m-%d")
    )
}

/// Deribit index_name for the DVOL index (`btcdvol_usdc` / `ethdvol_usdc`).
pub fn volatility_index_name(currency: &str) -> String {
    format!("{}dvol_usdc", currency.trim().to_ascii_lowercase())
}

/// Infer S3 folder currency from an instrument name.
///
/// Inverse / coin-margined: `BTC-PERPETUAL` → `BTC`.
/// Linear / USDC-settled: `BTC_USDC-PERPETUAL` → `USDC` (settlement), not `BTC_USDC`.
pub fn currency_from_instrument_name(name: &str) -> String {
    let head = name.split('-').next().unwrap_or(name);
    if let Some((_, settle)) = head.rsplit_once('_') {
        if !settle.is_empty() {
            return settle.to_ascii_uppercase();
        }
    }
    head.to_ascii_uppercase()
}

pub fn manifest_key() -> String {
    S3_MANIFEST_KEY.to_string()
}

pub fn volatility_manifest_key() -> String {
    S3_VOL_MANIFEST_KEY.to_string()
}

pub type MirrorStatus = historical_core::MirrorStatus;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestEntry {
    pub exchange: String,
    pub data_type: String,
    pub currency: String,
    pub symbol: String,
    pub date: String,
    pub source_url: String,
    pub path: Option<String>,
    pub status: String,
    pub rows: Option<usize>,
    pub error: Option<String>,
}

impl ManifestEntry {
    pub fn pending(job: &DayJob, source_url: String) -> Self {
        Self {
            exchange: EXCHANGE.into(),
            data_type: DATA_TYPE_TRADE.into(),
            currency: job.currency.clone(),
            symbol: job.instrument_name.clone(),
            date: job.date.to_string(),
            source_url,
            path: Some(job.object_key()),
            status: "pending".into(),
            rows: None,
            error: None,
        }
    }

    pub fn pending_volatility(job: &VolDayJob, source_url: String) -> Self {
        Self {
            exchange: EXCHANGE.into(),
            data_type: DATA_TYPE_VOLATILITY_INDEX.into(),
            currency: job.currency.clone(),
            symbol: format!("{}@{}", job.index_name(), job.resolution),
            date: job.date.to_string(),
            source_url,
            path: Some(job.object_key()),
            status: "pending".into(),
            rows: None,
            error: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn storage_key_matches_plan_layout() {
        let date = NaiveDate::from_ymd_opt(2024, 1, 2).unwrap();
        assert_eq!(
            storage_key("btc", "BTC-PERPETUAL", date),
            "historical/data/trade/BTC/BTC-PERPETUAL/2024-01-02.parquet.zstd"
        );
        assert_eq!(manifest_key(), S3_MANIFEST_KEY);
    }

    #[test]
    fn storage_key_usdc_perpetual_under_usdc() {
        let date = NaiveDate::from_ymd_opt(2026, 5, 6).unwrap();
        assert_eq!(
            storage_key("USDC", "BTC_USDC-PERPETUAL", date),
            "historical/data/trade/USDC/BTC_USDC-PERPETUAL/2026-05-06.parquet.zstd"
        );
    }

    #[test]
    fn currency_from_instrument_name_settlement() {
        assert_eq!(currency_from_instrument_name("BTC-PERPETUAL"), "BTC");
        assert_eq!(currency_from_instrument_name("ETH-26JUN26"), "ETH");
        assert_eq!(currency_from_instrument_name("BTC_USDC-PERPETUAL"), "USDC");
        assert_eq!(currency_from_instrument_name("ETH_USDC-PERPETUAL"), "USDC");
        assert_eq!(currency_from_instrument_name("ADA_USDC-PERPETUAL"), "USDC");
        assert_eq!(currency_from_instrument_name("BTCDVOL_USDC-26AUG26"), "USDC");
    }

    #[test]
    fn volatility_storage_key_layout() {
        let date = NaiveDate::from_ymd_opt(2026, 8, 5).unwrap();
        assert_eq!(
            volatility_storage_key("btc", "3600", date),
            "historical/data/volatility_index/BTC/3600/2026-08-05.parquet.zstd"
        );
        assert_eq!(volatility_index_name("BTC"), "btcdvol_usdc");
        assert_eq!(volatility_index_name("ETH"), "ethdvol_usdc");
        assert_eq!(volatility_manifest_key(), S3_VOL_MANIFEST_KEY);
    }
}
