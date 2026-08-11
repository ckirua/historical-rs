use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

pub const DATA_TYPE_ORDERBOOK: &str = "orderbook";
pub const DATA_TYPE_TRADE: &str = "trade";

/// A Bybit market segment that has been verified against the download portal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Market {
    Spot,
    /// Portal `bizType=contract` covering linear and inverse futures symbols.
    Contract,
}

impl Market {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Spot => "spot",
            Self::Contract => "contract",
        }
    }
}

impl fmt::Display for Market {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for Market {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.to_ascii_lowercase().as_str() {
            "spot" => Ok(Self::Spot),
            "contract" => Ok(Self::Contract),
            other => Err(format!(
                "unsupported Bybit market `{other}`; verified values are `spot` and `contract` (not `linear`/`inverse`)"
            )),
        }
    }
}

/// A Bybit product whose download portal contract is verified.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DataType {
    Orderbook,
    Trade,
}

impl DataType {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Orderbook => DATA_TYPE_ORDERBOOK,
            Self::Trade => DATA_TYPE_TRADE,
        }
    }
}

impl fmt::Display for DataType {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for DataType {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.to_ascii_lowercase().as_str() {
            DATA_TYPE_ORDERBOOK => Ok(Self::Orderbook),
            DATA_TYPE_TRADE => Ok(Self::Trade),
            other => Err(format!(
                "unsupported Bybit data type `{other}`; verified values are `orderbook` and `trade`"
            )),
        }
    }
}

/// Archive cadence accepted by the verified Bybit portal contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Interval {
    Daily,
}

impl Interval {
    pub const fn as_str(self) -> &'static str {
        "daily"
    }
}

impl fmt::Display for Interval {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for Interval {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.to_ascii_lowercase().as_str() {
            "daily" => Ok(Self::Daily),
            other => Err(format!("unsupported Bybit interval `{other}`; only `daily` is verified")),
        }
    }
}

/// One verified Bybit portal dataset/product combination.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Dataset {
    pub market: Market,
    pub data_type: DataType,
    pub interval: Interval,
}

impl Dataset {
    pub const fn new(market: Market, data_type: DataType, interval: Interval) -> Self {
        Self {
            market,
            data_type,
            interval,
        }
    }

    pub const fn spot_orderbook() -> Self {
        Self::new(Market::Spot, DataType::Orderbook, Interval::Daily)
    }

    pub const fn spot_trade() -> Self {
        Self::new(Market::Spot, DataType::Trade, Interval::Daily)
    }

    pub const fn contract_orderbook() -> Self {
        Self::new(Market::Contract, DataType::Orderbook, Interval::Daily)
    }

    pub const fn contract_trade() -> Self {
        Self::new(Market::Contract, DataType::Trade, Interval::Daily)
    }

    pub fn from_strings(market: &str, data_type: &str, interval: &str) -> Result<Self, String> {
        Ok(Self::new(
            market.parse()?,
            data_type.parse()?,
            interval.parse()?,
        ))
    }

    pub const fn is_orderbook(self) -> bool {
        matches!(self.data_type, DataType::Orderbook)
    }

    pub const fn is_trade(self) -> bool {
        matches!(self.data_type, DataType::Trade)
    }
}

impl Default for Dataset {
    fn default() -> Self {
        Self::spot_orderbook()
    }
}

impl fmt::Display for Dataset {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}/{}/{}", self.market, self.data_type, self.interval)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Archive {
    pub market: Market,
    pub data_type: DataType,
    pub interval: Interval,
    pub symbol: String,
    pub date: NaiveDate,
    pub url: String,
    pub filename: String,
    pub size: Option<u64>,
}

impl Archive {
    pub fn object_key(&self) -> String {
        storage_key_for(
            Dataset::new(self.market, self.data_type, self.interval),
            &self.symbol,
            self.date,
        )
    }

    pub const fn dataset(&self) -> Dataset {
        Dataset::new(self.market, self.data_type, self.interval)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderBookRow {
    pub market: String,
    pub symbol: String,
    pub topic: String,
    pub message_type: String,
    pub ts_ms: i64,
    pub cts_ms: Option<i64>,
    pub update_id: Option<u64>,
    pub cross_seq: Option<u64>,
    pub side: String,
    pub price: f64,
    pub size: f64,
}

/// One executed trade from a Bybit daily archive (Spot or Contract).
///
/// `trade_id` is a string so Spot numeric ids and Contract UUID `trdMatchID`
/// values share one Parquet schema. The parser does not derive quantities or
/// infer order semantics.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TradeRow {
    pub symbol: String,
    pub timestamp_ms: i64,
    pub side: String,
    pub price: f64,
    pub size: f64,
    pub trade_id: String,
    pub quote_qty: Option<f64>,
    pub is_buyer_maker: Option<bool>,
}

/// Build the object key for a verified dataset.
///
/// The Spot/daily orderbook path intentionally retains the original layout so
/// existing objects and resume checks remain valid. New datasets include their
/// interval dimension explicitly.
pub fn storage_key_for(dataset: Dataset, symbol: &str, date: NaiveDate) -> String {
    if dataset == Dataset::spot_orderbook() {
        return storage_key(symbol, date);
    }
    format!(
        "historical/data/{}/{}/{}/{}/{}.parquet.zstd",
        dataset.data_type,
        dataset.market,
        dataset.interval,
        symbol,
        date.format("%Y-%m-%d")
    )
}

/// Legacy Spot orderbook object key.
pub fn storage_key(symbol: &str, date: NaiveDate) -> String {
    format!(
        "historical/data/orderbook/spot/{}/{}.parquet.zstd",
        symbol,
        date.format("%Y-%m-%d")
    )
}

pub type MirrorStatus = historical_core::MirrorStatus;

fn default_interval() -> String {
    "daily".into()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestEntry {
    pub exchange: String,
    pub data_type: String,
    pub market: String,
    #[serde(default = "default_interval")]
    pub interval: String,
    pub symbol: String,
    pub date: String,
    pub source_url: String,
    #[serde(default)]
    pub filename: Option<String>,
    #[serde(default)]
    pub size: Option<u64>,
    pub path: Option<String>,
    pub status: String,
    pub rows: Option<usize>,
    pub error: Option<String>,
}

impl ManifestEntry {
    pub fn new(file: &Archive) -> Self {
        Self {
            exchange: "bybit".into(),
            data_type: file.data_type.to_string(),
            market: file.market.to_string(),
            interval: file.interval.to_string(),
            symbol: file.symbol.clone(),
            date: file.date.to_string(),
            source_url: file.url.clone(),
            filename: Some(file.filename.clone()),
            size: file.size,
            path: Some(file.object_key()),
            status: "pending".into(),
            rows: None,
            error: None,
        }
    }

    pub fn missing(dataset: Dataset, symbol: &str, date: NaiveDate, source_url: String) -> Self {
        Self {
            exchange: "bybit".into(),
            data_type: dataset.data_type.to_string(),
            market: dataset.market.to_string(),
            interval: dataset.interval.to_string(),
            symbol: symbol.to_string(),
            date: date.to_string(),
            source_url,
            filename: None,
            size: None,
            path: Some(storage_key_for(dataset, symbol, date)),
            // Portal catalogs are sparse per symbol; absence is expected, not a hard failure.
            status: "source-not-found".into(),
            rows: None,
            error: None,
        }
    }
}
/// One requested symbol/date cell that is absent or incomplete in a manifest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoverageCell {
    pub symbol: String,
    pub date: NaiveDate,
    /// The manifest status for an incomplete cell. `None` denotes a missing
    /// manifest entry.
    pub status: Option<String>,
    pub error: Option<String>,
}

/// Reconciliation of expected symbol/date cells against manifest entries.
///
/// The counts describe unique requested cells rather than raw manifest lines.
/// A cell is complete when it has an `ok` or `skipped` entry without an error.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoverageReport {
    pub requested: usize,
    pub ok: usize,
    pub skipped: usize,
    pub failed: usize,
    pub missing: usize,
    pub missing_cells: Vec<CoverageCell>,
    pub failed_cells: Vec<CoverageCell>,
}

impl CoverageReport {
    pub const fn empty() -> Self {
        Self {
            requested: 0,
            ok: 0,
            skipped: 0,
            failed: 0,
            missing: 0,
            missing_cells: Vec::new(),
            failed_cells: Vec::new(),
        }
    }

    /// Return the number of complete cells represented by this report.
    pub const fn complete(&self) -> usize {
        self.ok + self.skipped
    }
}

impl Default for CoverageReport {
    fn default() -> Self {
        Self::empty()
    }
}



pub fn manifest_key_for(dataset: Dataset) -> String {
    if dataset == Dataset::spot_orderbook() {
        return "historical/manifest/bybit/orderbook/manifest.jsonl".into();
    }
    format!(
        "historical/manifest/bybit/{}/{}/{}/manifest.jsonl",
        dataset.data_type, dataset.market, dataset.interval
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn orderbook_storage_key_is_stable() {
        let date = NaiveDate::from_ymd_opt(2024, 1, 2).unwrap();
        assert_eq!(
            storage_key("BTCUSDT", date),
            "historical/data/orderbook/spot/BTCUSDT/2024-01-02.parquet.zstd"
        );
        assert_eq!(
            storage_key_for(Dataset::spot_orderbook(), "BTCUSDT", date),
            "historical/data/orderbook/spot/BTCUSDT/2024-01-02.parquet.zstd"
        );
    }

    #[test]
    fn unsupported_dimensions_are_rejected() {
        assert!(Dataset::from_strings("linear", "orderbook", "daily").is_err());
        assert!(Dataset::from_strings("inverse", "trade", "daily").is_err());
        assert!(Dataset::from_strings("spot", "kline", "daily").is_err());
        assert!(Dataset::from_strings("spot", "trade", "hourly").is_err());
    }

    #[test]
    fn contract_datasets_and_keys_are_accepted() {
        let date = NaiveDate::from_ymd_opt(2024, 1, 2).unwrap();
        let orderbook = Dataset::from_strings("contract", "orderbook", "daily").unwrap();
        let trade = Dataset::from_strings("contract", "trade", "daily").unwrap();
        assert_eq!(orderbook, Dataset::contract_orderbook());
        assert_eq!(trade, Dataset::contract_trade());
        assert_eq!(
            storage_key_for(orderbook, "BTCUSDT", date),
            "historical/data/orderbook/contract/daily/BTCUSDT/2024-01-02.parquet.zstd"
        );
        assert_eq!(
            storage_key_for(trade, "BTCUSD", date),
            "historical/data/trade/contract/daily/BTCUSD/2024-01-02.parquet.zstd"
        );
        assert_eq!(
            manifest_key_for(trade),
            "historical/manifest/bybit/trade/contract/daily/manifest.jsonl"
        );
        assert_eq!(
            manifest_key_for(orderbook),
            "historical/manifest/bybit/orderbook/contract/daily/manifest.jsonl"
        );
    }
}
