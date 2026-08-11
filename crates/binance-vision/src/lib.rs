//! Rust library for Binance Vision historical market data.

pub mod client;
pub mod config;
pub mod error;
pub mod model;
pub mod parse;
pub mod vision;

#[cfg(feature = "export")]
pub mod export;

#[cfg(feature = "mirror")]
pub mod mirror;

#[cfg(feature = "cache")]
pub mod symbols;

#[cfg(feature = "clickhouse")]
pub mod clickhouse;

pub use client::VisionHttpClient;
pub use config::Config;
pub use error::{Error, Result};
pub use model::{
    AggTradeRow, DataType, Frequency, KlineRow, Market, MirrorStatus, TradeRow, VisionFile,
};
pub use vision::VisionUrl;

#[cfg(feature = "mirror")]
pub use mirror::{mirror_status, run_mirror, ManifestEntry, MirrorOptions};

#[cfg(feature = "s3")]
pub use mirror::{mirror_status_s3, run_s3_mirror, S3Config};

#[cfg(feature = "cache")]
pub use client::{BinanceRestClient, ExchangeInfoSymbol};
#[cfg(feature = "cache")]
pub use symbols::SymbolCache;

#[cfg(feature = "clickhouse")]
pub use clickhouse::{
    run_fetch_ingest, run_ingest, run_s3_ingest, ClickHouseClient, ClickHouseConfig,
    FetchIngestOptions, IngestOptions, IngestStatus,
};
