//! Bybit historical Spot and Contract archive catalog and S3 mirror.

pub mod client;
pub mod config;
pub mod error;
pub mod export;
pub mod mirror;
pub mod model;
pub mod parse;

pub use client::BybitHttpClient;
pub use config::Config;
pub use error::{Error, Result};
pub use mirror::{
    classify_failure, failure_histogram_from_errors, mirror_status_s3, mirror_status_s3_for,
    reconcile_coverage, run_s3_mirror, run_s3_mirror_for, MirrorOptions, MirrorRunReport, S3Config,
    S3_MANIFEST_KEY,
};
pub use model::{
    manifest_key_for, storage_key, storage_key_for, Archive, CoverageCell, CoverageReport, DataType,
    Dataset, Interval, Market, ManifestEntry, MirrorStatus, OrderBookRow, DATA_TYPE_ORDERBOOK,
    DATA_TYPE_TRADE,
};
pub use export::{
    convert_archive, convert_archive_bytes, convert_trade_archive, convert_trade_archive_bytes,
    orderbook_batch, orderbook_schema, trade_batch, trade_schema,
};
pub use parse::{
    parse_trade_gzip_bytes, parse_trade_gzip_file, parse_trade_gzip_file_batched,
};
