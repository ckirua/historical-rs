//! Shared domain types.

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

/// OKX market segment for CDN archives.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Market {
    Spot,
    Swap,
}

impl Market {
    pub fn path_segment(self) -> &'static str {
        match self {
            Market::Spot => "spot",
            Market::Swap => "swap",
        }
    }

    pub fn as_str(self) -> &'static str {
        self.path_segment()
    }

    pub fn inst_type(self) -> &'static str {
        match self {
            Market::Spot => "SPOT",
            Market::Swap => "SWAP",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "spot" => Some(Market::Spot),
            "swap" | "perp" | "perpetual" => Some(Market::Swap),
            _ => None,
        }
    }
}

/// File cadence on OKX CDN.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Frequency {
    Daily,
    Monthly,
}

impl Frequency {
    pub fn as_str(self) -> &'static str {
        match self {
            Frequency::Daily => "daily",
            Frequency::Monthly => "monthly",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "daily" | "day" => Some(Frequency::Daily),
            "monthly" | "month" => Some(Frequency::Monthly),
            _ => None,
        }
    }
}

/// OKX historical CDN data product.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DataType {
    Trades,
    Candlesticks,
    Funding,
    BorrowRates,
}

impl DataType {
    pub fn path_segment(self) -> &'static str {
        match self {
            DataType::Trades => "trades",
            DataType::Candlesticks => "candlesticks",
            DataType::Funding => "funding",
            DataType::BorrowRates => "borrowrates",
        }
    }

    /// CDN directory under `traderecords/`.
    pub fn cdn_dir(self) -> &'static str {
        match self {
            DataType::Trades => "trades",
            DataType::Candlesticks => "candlesticks",
            DataType::Funding => "swaprates",
            DataType::BorrowRates => "borrowrates",
        }
    }

    pub fn module(self) -> &'static str {
        match self {
            DataType::Trades => "1",
            DataType::Candlesticks => "2",
            DataType::Funding => "3",
            DataType::BorrowRates => "11",
        }
    }

    pub fn default_frequency(self) -> Frequency {
        match self {
            DataType::BorrowRates => Frequency::Monthly,
            _ => Frequency::Daily,
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "trades" | "trade" => Some(DataType::Trades),
            "candlesticks" | "candles" | "klines" | "candle" => Some(DataType::Candlesticks),
            "funding" | "fundingrate" | "funding_rate" | "swaprate" | "swaprates" => {
                Some(DataType::Funding)
            }
            "borrowrates" | "borrow" | "borrowing" | "borrow_rates" => Some(DataType::BorrowRates),
            _ => None,
        }
    }
}

/// Identifies one OKX CDN archive file.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct OkxFile {
    pub market: Market,
    pub frequency: Frequency,
    pub data_type: DataType,
    /// Instrument id (`BTC-USDT`, `BTC-USDT-SWAP`) or currency (`BTC`) for borrow rates,
    /// or `ALL` for all-swap funding bundles.
    pub symbol: String,
    /// `YYYY-MM-DD` (daily) or `YYYY-MM` (monthly).
    pub period: String,
}

impl OkxFile {
    pub fn daily(
        market: Market,
        data_type: DataType,
        symbol: impl Into<String>,
        date: NaiveDate,
    ) -> Self {
        Self {
            market,
            frequency: Frequency::Daily,
            data_type,
            symbol: symbol.into(),
            period: date.format("%Y-%m-%d").to_string(),
        }
    }

    pub fn monthly(
        market: Market,
        data_type: DataType,
        symbol: impl Into<String>,
        year: i32,
        month: u32,
    ) -> Self {
        Self {
            market,
            frequency: Frequency::Monthly,
            data_type,
            symbol: symbol.into(),
            period: format!("{year:04}-{month:02}"),
        }
    }
}

/// Trade row from OKX trades CSV.
#[derive(Debug, Clone, PartialEq)]
pub struct TradeRow {
    pub instrument: String,
    pub trade_id: u64,
    pub side: String,
    pub price: f64,
    pub size: f64,
    pub timestamp_ms: i64,
}

/// Candlestick row from OKX candlesticks CSV.
#[derive(Debug, Clone, PartialEq)]
pub struct CandleRow {
    pub instrument: String,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub vol: f64,
    pub vol_ccy: f64,
    pub vol_quote: f64,
    pub open_time_ms: i64,
    pub confirm: i32,
}

/// Funding rate row.
#[derive(Debug, Clone, PartialEq)]
pub struct FundingRow {
    pub instrument: String,
    pub funding_rate: f64,
    pub funding_time_ms: i64,
}

/// Borrow rate row.
#[derive(Debug, Clone, PartialEq)]
pub struct BorrowRow {
    pub currency: String,
    pub borrow_rate: f64,
    pub time_ms: i64,
}

/// Aggregate mirror progress.
pub use historical_core::MirrorStatus;
