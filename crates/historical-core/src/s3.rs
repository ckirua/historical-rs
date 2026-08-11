//! Shared S3-compatible bucket configuration.

use s3::bucket::Bucket;
use s3::creds::Credentials;
use s3::Region;

use crate::env::env_first;
use crate::error::{Error, Result};

#[derive(Clone)]
pub struct S3Config {
    pub bucket: String,
    pub endpoint: String,
    pub region: String,
    pub access_key: String,
    pub secret_key: String,
}

impl std::fmt::Debug for S3Config {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("S3Config")
            .field("bucket", &self.bucket)
            .field("endpoint", &self.endpoint)
            .field("region", &self.region)
            .field("access_key", &"<redacted>")
            .field("secret_key", &"<redacted>")
            .finish()
    }
}

impl S3Config {
    /// Build from env keys. `bucket_keys[0]` is preferred; `default_bucket` if unset.
    /// Endpoint / access / secret must be set (first matching key).
    pub fn from_env_keys(
        bucket_keys: &[&str],
        endpoint_keys: &[&str],
        region_keys: &[&str],
        access_keys: &[&str],
        secret_keys: &[&str],
        default_bucket: &str,
        default_region: &str,
    ) -> Result<Self> {
        let bucket = env_first(bucket_keys).unwrap_or_else(|| default_bucket.to_string());
        let endpoint = env_first(endpoint_keys).ok_or_else(|| {
            Error::Config(format!(
                "{} is not set",
                endpoint_keys.first().copied().unwrap_or("S3_ENDPOINT")
            ))
        })?;
        let region = env_first(region_keys).unwrap_or_else(|| default_region.to_string());
        let access_key = env_first(access_keys).ok_or_else(|| {
            Error::Config(format!(
                "{} is not set",
                access_keys.first().copied().unwrap_or("S3_ACCESS_KEY")
            ))
        })?;
        let secret_key = env_first(secret_keys).ok_or_else(|| {
            Error::Config(format!(
                "{} is not set",
                secret_keys.first().copied().unwrap_or("S3_SECRET_KEY")
            ))
        })?;
        Ok(Self {
            bucket: bucket.trim().to_string(),
            endpoint: endpoint.trim().trim_end_matches('/').to_string(),
            region: region.trim().to_string(),
            access_key: access_key.trim().to_string(),
            secret_key: secret_key.trim().to_string(),
        })
    }

    /// Path-style bucket handle for Hetzner / MinIO-style endpoints.
    pub fn bucket(&self) -> Result<Box<Bucket>> {
        let credentials = Credentials::new(
            Some(&self.access_key),
            Some(&self.secret_key),
            None,
            None,
            None,
        )
        .map_err(|e| Error::S3(e.to_string()))?;
        let region = Region::Custom {
            region: self.region.clone(),
            endpoint: self.endpoint.clone(),
        };
        let mut bucket =
            Bucket::new(&self.bucket, region, credentials).map_err(|e| Error::S3(e.to_string()))?;
        bucket.set_path_style();
        Ok(bucket)
    }
}
