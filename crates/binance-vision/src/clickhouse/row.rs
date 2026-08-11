//! Row types for ClickHouse insert into `binance_vision.*`.

use chrono::{DateTime, Utc};
use clickhouse::Row;
use serde::Serialize;

#[derive(Debug, Row, Serialize)]
pub struct VisionTradeInsertRow {
    pub kind: String,
    pub symbol: String,
    pub id: u64,
    pub price: f64,
    pub qty: f64,
    pub quote_qty: f64,
    #[serde(with = "clickhouse::serde::chrono::datetime64::millis")]
    pub time: DateTime<Utc>,
    pub is_buyer_maker: bool,
}

#[derive(Debug, Row, Serialize)]
pub struct VisionKlineInsertRow {
    pub kind: String,
    pub symbol: String,
    pub interval: String,
    #[serde(with = "clickhouse::serde::chrono::datetime64::millis")]
    pub open_time: DateTime<Utc>,
    #[serde(with = "clickhouse::serde::chrono::datetime64::millis")]
    pub close_time: DateTime<Utc>,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
    pub quote_volume: f64,
    pub count: u32,
    pub taker_buy_volume: f64,
    pub taker_buy_quote_volume: f64,
}

#[derive(Debug, Row, Serialize)]
pub struct VisionBookDepthInsertRow {
    pub kind: String,
    pub symbol: String,
    #[serde(with = "clickhouse::serde::chrono::datetime")]
    pub timestamp: DateTime<Utc>,
    pub percentage: i8,
    pub depth: f64,
    pub notional: f64,
}

#[derive(Debug, Row, Serialize)]
pub struct VisionBvolInsertRow {
    pub symbol: String,
    pub base_asset: String,
    pub quote_asset: String,
    #[serde(with = "clickhouse::serde::chrono::datetime64::millis")]
    pub calc_time: DateTime<Utc>,
    pub index_value: f32,
}

#[derive(Debug, Row, Serialize)]
pub struct IngestLogInsertRow {
    pub s3_key: String,
    pub rows: u64,
    pub status: String,
    pub error: Option<String>,
}

/// Legacy insert type for idle `binance.agg_trades` (do not use for new work).
#[derive(Debug, Row, Serialize)]
pub struct AggTradeInsertRow {
    pub market: String,
    pub symbol: String,
    pub agg_trade_id: u64,
    pub price: f64,
    pub qty: f64,
    pub first_trade_id: u64,
    pub last_trade_id: u64,
    pub timestamp_ms: i64,
    pub is_buyer_maker: u8,
}
