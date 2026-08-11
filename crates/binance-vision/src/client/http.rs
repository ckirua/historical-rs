//! Async HTTP download client.

use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::{Path, PathBuf};

use crate::config::Config;
use crate::error::{Error, Result};
use crate::vision::VisionUrl;

#[derive(Clone)]
pub struct VisionHttpClient {
    client: reqwest::Client,
    config: Config,
    url_builder: VisionUrl,
}

impl VisionHttpClient {
    pub fn new(config: Config) -> Result<Self> {
        let client = reqwest::Client::builder()
            .user_agent(&config.user_agent)
            .timeout(config.timeout())
            .build()?;
        Ok(Self {
            client,
            url_builder: VisionUrl::new(&config.vision_base_url),
            config,
        })
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    pub fn url_builder(&self) -> &VisionUrl {
        &self.url_builder
    }

    pub async fn get_bytes(&self, url: &str) -> Result<Vec<u8>> {
        let policy = historical_core::http::RetryPolicy {
            max_retries: self.config.max_retries,
            retry_backoff_ms: self.config.retry_backoff_ms,
            retry_on_429: false,
            ..Default::default()
        };
        Ok(historical_core::http::get_bytes_with_retry(&self.client, url, policy).await?)
    }

    pub async fn download_to_file(
        &self,
        zip_url: &str,
        path: &Path,
        verify_checksum: bool,
    ) -> Result<bool> {
        let policy = historical_core::http::RetryPolicy {
            max_retries: self.config.max_retries,
            retry_backoff_ms: self.config.retry_backoff_ms,
            retry_on_429: false,
            ..Default::default()
        };
        historical_core::http::get_to_file_with_retry(&self.client, zip_url, policy, path).await?;
        if !verify_checksum {
            return Ok(false);
        }

        let checksum_url = self.url_builder.checksum_url(zip_url);
        let expected = match self.get_bytes(&checksum_url).await {
            Ok(checksum_body) => parse_checksum_file(&checksum_body)?,
            Err(Error::NotFound(_)) => return Ok(false),
            Err(err) => return Err(err),
        };
        let path = PathBuf::from(path);
        let actual = tokio::task::spawn_blocking(move || hex_sha256_file(&path))
            .await
            .map_err(|error| Error::Io(std::io::Error::other(error.to_string())))??;
        if expected.eq_ignore_ascii_case(&actual) {
            Ok(true)
        } else {
            Err(Error::ChecksumMismatch { expected, actual })
        }
    }

    pub async fn head_exists(&self, url: &str) -> Result<bool> {
        let response = self.client.head(url).send().await?;
        Ok(response.status().is_success())
    }

    pub async fn download_with_checksum(&self, zip_url: &str) -> Result<(Vec<u8>, bool)> {
        let bytes = self.get_bytes(zip_url).await?;
        let checksum_url = self.url_builder.checksum_url(zip_url);
        let verified = match self.get_bytes(&checksum_url).await {
            Ok(checksum_body) => {
                let expected = parse_checksum_file(&checksum_body)?;
                let actual = hex_sha256(&bytes);
                if expected.eq_ignore_ascii_case(&actual) {
                    true
                } else {
                    return Err(Error::ChecksumMismatch { expected, actual });
                }
            }
            Err(Error::NotFound(_)) => false,
            Err(err) => return Err(err),
        };
        Ok((bytes, verified))
    }
}

fn parse_checksum_file(body: &[u8]) -> Result<String> {
    let text = std::str::from_utf8(body)
        .map_err(|e| Error::Parse(format!("invalid CHECKSUM file: {e}")))?;
    let token = text
        .split_whitespace()
        .next()
        .ok_or_else(|| Error::Parse("empty CHECKSUM file".into()))?;
    Ok(token.to_string())
}

fn hex_sha256(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    format!("{digest:x}")
}

fn hex_sha256_file(path: &Path) -> Result<String> {
    let mut file = std::fs::File::open(path)?;
    let mut digest = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        digest.update(&buffer[..read]);
    }
    Ok(format!("{:x}", digest.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_checksum_line() {
        let body = b"abc123  BTCUSDT-aggTrades-2024-01-15.zip\n";
        assert_eq!(parse_checksum_file(body).unwrap(), "abc123");
    }

    #[test]
    fn hex_sha256_is_lowercase() {
        assert_eq!(
            hex_sha256(b"hello"),
            "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
        );
    }
}
