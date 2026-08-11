//! Shared domain types.

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

/// Binance market segment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Market {
    Spot,
    Um,
    Cm,
    Option,
}

impl Market {
    pub fn path_segment(self) -> &'static str {
        match self {
            Market::Spot => "spot",
            Market::Um => "futures/um",
            Market::Cm => "futures/cm",
            Market::Option => "option",
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Market::Spot => "spot",
            Market::Um => "um",
            Market::Cm => "cm",
            Market::Option => "option",
        }
    }

    /// ClickHouse `kind` column used by `binance_vision.*` tables.
    pub fn kind_label(self) -> &'static str {
        match self {
            Market::Spot => "SPOT",
            Market::Um => "UM",
            Market::Cm => "CM",
            Market::Option => "OPTION",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "spot" => Some(Market::Spot),
            "um" | "futures_um" | "futures/um" => Some(Market::Um),
            "cm" | "futures_cm" | "futures/cm" => Some(Market::Cm),
            "option" | "options" => Some(Market::Option),
            _ => None,
        }
    }
}

/// File cadence on Binance Vision.
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

/// Vision data product type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DataType {
    AggTrades,
    Trades,
    Klines,
    BookDepth,
    BookTicker,
    PremiumIndexKlines,
    MarkPriceKlines,
    IndexPriceKlines,
    LiquidationSnapshot,
    Metrics,
    BvolIndex,
}

impl DataType {
    pub fn path_segment(self) -> &'static str {
        match self {
            DataType::AggTrades => "aggTrades",
            DataType::Trades => "trades",
            DataType::Klines => "klines",
            DataType::BookDepth => "bookDepth",
            DataType::BookTicker => "bookTicker",
            DataType::PremiumIndexKlines => "premiumIndexKlines",
            DataType::MarkPriceKlines => "markPriceKlines",
            DataType::IndexPriceKlines => "indexPriceKlines",
            DataType::LiquidationSnapshot => "liquidationSnapshot",
            DataType::Metrics => "metrics",
            DataType::BvolIndex => "BVOLIndex",
        }
    }

    pub fn file_label(self) -> &'static str {
        match self {
            DataType::BvolIndex => "BVOLIndex",
            other => other.path_segment(),
        }
    }

    pub fn requires_interval(self) -> bool {
        matches!(
            self,
            DataType::Klines
                | DataType::PremiumIndexKlines
                | DataType::MarkPriceKlines
                | DataType::IndexPriceKlines
        )
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "aggTrades" | "aggtrades" | "agg_trades" => Some(DataType::AggTrades),
            "trades" => Some(DataType::Trades),
            "klines" => Some(DataType::Klines),
            "bookDepth" | "bookdepth" | "book_depth" => Some(DataType::BookDepth),
            "bookTicker" | "bookticker" | "book_ticker" => Some(DataType::BookTicker),
            "premiumIndexKlines" | "premium_index_klines" => Some(DataType::PremiumIndexKlines),
            "markPriceKlines" | "mark_price_klines" => Some(DataType::MarkPriceKlines),
            "indexPriceKlines" | "index_price_klines" => Some(DataType::IndexPriceKlines),
            "liquidationSnapshot" | "liquidation_snapshot" => Some(DataType::LiquidationSnapshot),
            "metrics" => Some(DataType::Metrics),
            "BVOLIndex" | "bvolindex" | "bvol_index" => Some(DataType::BvolIndex),
            _ => None,
        }
    }
}

/// Identifies one Vision archive file.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct VisionFile {
    pub market: Market,
    pub frequency: Frequency,
    pub data_type: DataType,
    pub symbol: String,
    pub period: String,
    pub interval: Option<String>,
}

impl VisionFile {
    pub fn daily(
        market: Market,
        data_type: DataType,
        symbol: impl Into<String>,
        date: NaiveDate,
        interval: Option<&str>,
    ) -> Self {
        Self {
            market,
            frequency: Frequency::Daily,
            data_type,
            symbol: symbol.into(),
            period: date.format("%Y-%m-%d").to_string(),
            interval: interval.map(str::to_string),
        }
    }

    pub fn monthly(
        market: Market,
        data_type: DataType,
        symbol: impl Into<String>,
        year: i32,
        month: u32,
        interval: Option<&str>,
    ) -> Self {
        Self {
            market,
            frequency: Frequency::Monthly,
            data_type,
            symbol: symbol.into(),
            period: format!("{year:04}-{month:02}"),
            interval: interval.map(str::to_string),
        }
    }
}

/// Mirror progress summary.
pub use historical_core::MirrorStatus;

/// Parsed aggregated trade row (USD-M futures).
#[derive(Debug, Clone, PartialEq)]
pub struct AggTradeRow {
    pub agg_trade_id: u64,
    pub price: f64,
    pub qty: f64,
    pub first_trade_id: u64,
    pub last_trade_id: u64,
    pub timestamp_ms: i64,
    pub is_buyer_maker: bool,
}

/// Parsed individual trade row.
#[derive(Debug, Clone, PartialEq)]
pub struct TradeRow {
    pub trade_id: u64,
    pub price: f64,
    pub qty: f64,
    pub quote_qty: Option<f64>,
    pub base_qty: Option<f64>,
    pub timestamp_ms: i64,
    pub is_buyer_maker: bool,
    pub is_best_match: Option<bool>,
}

/// Parsed kline row.
#[derive(Debug, Clone, PartialEq)]
pub struct KlineRow {
    pub open_time: i64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
    pub close_time: i64,
    pub quote_volume: Option<f64>,
    pub base_volume: Option<f64>,
    pub num_trades: u64,
    pub taker_buy_volume: Option<f64>,
    pub taker_buy_quote_volume: Option<f64>,
    pub taker_buy_base_volume: Option<f64>,
    pub ignore: Option<f64>,
}
