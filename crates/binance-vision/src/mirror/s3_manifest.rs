//! S3 manifest writer.

use std::sync::Arc;

use crate::error::Result;
use crate::mirror::manifest::ManifestEntry;
use crate::model::MirrorStatus;

pub const S3_MANIFEST_KEY: &str = "manifest/binance-vision/manifest.jsonl";

pub type S3ManifestWriter = historical_core::S3ManifestWriter<ManifestEntry>;

pub fn new_s3_manifest_writer(config: Arc<historical_core::S3Config>) -> S3ManifestWriter {
    S3ManifestWriter::new(config, S3_MANIFEST_KEY)
}

pub async fn mirror_status_s3(s3: &super::s3::S3Config) -> Result<MirrorStatus> {
    Ok(historical_core::s3_manifest::mirror_status_s3(s3.core(), S3_MANIFEST_KEY).await?)
}
