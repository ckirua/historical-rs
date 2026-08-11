//! Rust library for Kraken historical Google Drive market data dumps.

pub mod client;
pub mod config;
pub mod drive;
pub mod error;
pub mod keys;
pub mod model;
pub mod parse;

#[cfg(feature = "export")]
pub mod export;

#[cfg(feature = "mirror")]
pub mod mirror;

pub use client::{fetch_asset_pairs, AssetPair, HttpClient};
pub use config::Config;
pub use drive::{discover, download_artifact, extract_drive_file_ids, parse_quarter_filename};
pub use error::{Error, Result};
pub use keys::{dataset_object_key, local_dataset_path};
pub use model::{
    parse_quarter, DataType, DriveArtifact, MirrorStatus, OhlcvtRow, OutputUnit, Source, TradeRow,
};

#[cfg(feature = "mirror")]
pub use mirror::{mirror_status, run_mirror, ManifestEntry, MirrorOptions, SourceMode};

#[cfg(feature = "s3")]
pub use mirror::{mirror_status_s3, run_s3_mirror, S3Config};
