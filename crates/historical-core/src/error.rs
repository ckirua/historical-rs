//! Shared error type for core helpers.

use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Error)]
pub enum Error {
    #[error("configuration error: {0}")]
    Config(String),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[cfg(feature = "http")]
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[cfg(feature = "http")]
    #[error("HTTP {status} for {url}")]
    HttpStatus { status: u16, url: String },

    #[cfg(feature = "http")]
    #[error("resource not found: {0}")]
    NotFound(String),

    #[cfg(feature = "s3")]
    #[error("S3 error: {0}")]
    S3(String),
}
