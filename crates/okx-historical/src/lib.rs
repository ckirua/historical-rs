//! Rust library for OKX historical market data CDN archives.

pub mod cdn;
pub mod client;
pub mod config;
pub mod error;
pub mod model;
pub mod parse;

#[cfg(feature = "export")]
pub mod export;

#[cfg(feature = "mirror")]
pub mod mirror;

#[cfg(feature = "cache")]
pub mod symbols;

pub use cdn::CdnUrl;
pub use client::OkxHttpClient;
pub use config::Config;
pub use error::{Error, Result};
pub use model::{
    BorrowRow, CandleRow, DataType, Frequency, FundingRow, Market, MirrorStatus, OkxFile, TradeRow,
};

#[cfg(feature = "mirror")]
pub use mirror::{mirror_status, run_mirror, ManifestEntry, MirrorOptions};

#[cfg(feature = "s3")]
pub use mirror::{mirror_status_s3, run_s3_mirror, S3Config};

#[cfg(feature = "cache")]
pub use client::{InstrumentInfo, OkxRestClient};
#[cfg(feature = "cache")]
pub use symbols::SymbolCache;
