//! Bulk mirror pipeline for Kraken historical Drive dumps.

mod convert;
mod local;
mod manifest;

#[cfg(feature = "s3")]
mod s3;
#[cfg(feature = "s3")]
mod s3_manifest;

pub use convert::{convert_output_unit, is_not_found};
pub use historical_core::{spawn_progress_reporter, MirrorProgress};
pub use local::{run_mirror, MirrorOptions, SourceMode};
pub use manifest::{
    append_manifest, entry_from_unit, load_completed_keys, mirror_status, mirror_status_from_str,
    ManifestEntry,
};

#[cfg(feature = "s3")]
pub use s3::{run_s3_mirror, S3Config};
#[cfg(feature = "s3")]
pub use s3_manifest::{mirror_status_s3, S3_MANIFEST_KEY};
