//! Error types for the Binance Vision library.

use thiserror::Error;

/// Result type alias.
pub type Result<T> = std::result::Result<T, Error>;

/// Errors that can occur when interacting with Binance Vision.
#[derive(Debug, Error)]
pub enum Error {
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("HTTP {status} for {url}")]
    HttpStatus { status: u16, url: String },

    #[error("resource not found: {0}")]
    NotFound(String),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("invalid URL: {0}")]
    Url(#[from] url::ParseError),

    #[error("CSV error: {0}")]
    Csv(#[from] csv::Error),

    #[cfg(feature = "export")]
    #[error("arrow error: {0}")]
    Arrow(#[from] arrow::error::ArrowError),

    #[cfg(feature = "export")]
    #[error("parquet error: {0}")]
    Parquet(#[from] parquet::errors::ParquetError),

    #[error("zip error: {0}")]
    Zip(String),

    #[error("parse error: {0}")]
    Parse(String),

    #[error("configuration error: {0}")]
    Config(String),

    #[error("checksum mismatch: expected {expected}, got {actual}")]
    ChecksumMismatch { expected: String, actual: String },

    #[cfg(feature = "s3")]
    #[error("S3 error: {0}")]
    S3(String),

    #[cfg(feature = "cache")]
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),

    #[cfg(feature = "clickhouse")]
    #[error("ClickHouse error: {0}")]
    ClickHouse(String),
}

impl From<historical_core::Error> for Error {
    fn from(err: historical_core::Error) -> Self {
        match err {
            historical_core::Error::Config(s) => Error::Config(s),
            historical_core::Error::Io(e) => Error::Io(e),
            historical_core::Error::Json(e) => Error::Json(e),
            historical_core::Error::Http(e) => Error::Http(e),
            historical_core::Error::HttpStatus { status, url } => Error::HttpStatus { status, url },
            historical_core::Error::NotFound(s) => Error::NotFound(s),
            #[cfg(feature = "s3")]
            historical_core::Error::S3(s) => Error::S3(s),
        }
    }
}
