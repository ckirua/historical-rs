use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("configuration error: {0}")]
    Config(String),
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("HTTP status {status} for {url}")]
    HttpStatus { status: u16, url: String },
    #[error("not found: {0}")]
    NotFound(String),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("ZIP error: {0}")]
    Zip(#[from] zip::result::ZipError),
    #[error("Parquet error: {0}")]
    Parquet(#[from] parquet::errors::ParquetError),
    #[error("Arrow error: {0}")]
    Arrow(#[from] arrow::error::ArrowError),
    #[error("S3 error: {0}")]
    S3(String),
    #[error("invalid archive row: {0}")]
    InvalidRow(String),
    #[error("shared infrastructure error: {0}")]
    Core(#[from] historical_core::Error),
}

pub type Result<T> = std::result::Result<T, Error>;
