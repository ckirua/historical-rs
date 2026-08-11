//! ClickHouse ingest for mirrored parquet datasets.

mod client;
mod config;
mod fetch_ingest;
mod ingest;
#[cfg(feature = "s3")]
mod ingest_s3;
mod parquet;
mod row;

pub use client::ClickHouseClient;
pub use config::ClickHouseConfig;
pub use fetch_ingest::{direct_source_key, run_fetch_ingest, FetchIngestOptions};
pub use ingest::{run_ingest, IngestOptions, IngestStatus};
#[cfg(feature = "s3")]
pub use ingest_s3::run_s3_ingest;
