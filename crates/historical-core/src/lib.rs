//! Shared infrastructure for historical market-data mirror pipelines.
//!
//! Exchange crates keep parsers, URL builders, and CLI trees local.
//! This crate holds near-copy-paste plumbing: progress, S3 config/manifest,
//! status counters, tracing init, and HTTP retry helpers.

pub mod csv_batch;
pub mod env;
pub mod error;
pub mod logging;
pub mod status;

#[cfg(feature = "mirror")]
pub mod progress;

#[cfg(feature = "http")]
pub mod http;

#[cfg(feature = "s3")]
pub mod s3;

#[cfg(feature = "s3")]
pub mod s3_manifest;

pub use env::env_first;
pub use error::{Error, Result};
pub use logging::init_tracing;
pub use status::{mirror_status_from_jsonl, MirrorStatus};

#[cfg(feature = "mirror")]
pub use progress::{spawn_progress_reporter, MirrorProgress};

#[cfg(feature = "s3")]
pub use s3::S3Config;

#[cfg(feature = "s3")]
pub use s3_manifest::S3ManifestWriter;
