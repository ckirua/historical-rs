//! Deribit historical public trades + DVOL volatility index → S3 parquet mirror.

#![forbid(unsafe_code)]

pub mod auth;
pub mod client;
pub mod config;
pub mod error;
pub mod export;
pub mod instruments;
pub mod mirror;
pub mod model;
pub mod rate_limit;
pub mod trades;
pub mod volatility;

pub use client::DeribitHttpClient;
pub use config::{parse_volatility_resolution, Config};
pub use error::{Error, Result};
pub use export::{
    trade_batch, trade_schema, volatility_index_batch, volatility_index_schema,
    write_trades_parquet, write_trades_parquet_bytes, write_volatility_index_parquet,
};
pub use instruments::{
    default_currencies, filter_dvol_instruments, filter_kind, load_dvol_instruments,
    load_future_instruments, parse_currency_list, parse_volatility_currency_list, InstrumentsCache,
};
pub use mirror::{
    mirror_status_s3, mirror_volatility_status_s3, run_dvol_trades_s3_mirror, run_s3_mirror,
    run_volatility_index_s3_mirror, MirrorOptions, MirrorRunReport, S3Config,
    VolatilityMirrorOptions,
};
pub use model::{
    currency_from_instrument_name, manifest_key, storage_key, volatility_index_name,
    volatility_manifest_key, volatility_storage_key, DayJob, Instrument, ManifestEntry,
    MirrorStatus, TradeRow, VolDayJob, VolatilityIndexCandle, DATA_TYPE_TRADE,
    DATA_TYPE_VOLATILITY_INDEX, EXCHANGE, S3_DATA_PREFIX, S3_MANIFEST_KEY, S3_VOL_DATA_PREFIX,
    S3_VOL_MANIFEST_KEY,
};
pub use trades::{expected_dates, fetch_trades_for_day, utc_day_bounds_ms};
pub use volatility::fetch_volatility_index_for_day;
