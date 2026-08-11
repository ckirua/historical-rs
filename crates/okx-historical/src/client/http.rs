//! Async HTTP download client.

use crate::cdn::CdnUrl;
use crate::config::Config;
use crate::error::Result;
use std::path::Path;

#[derive(Clone)]
pub struct OkxHttpClient {
    client: reqwest::Client,
    config: Config,
    url_builder: CdnUrl,
}

impl OkxHttpClient {
    pub fn new(config: Config) -> Result<Self> {
        let client = reqwest::Client::builder()
            .user_agent(&config.user_agent)
            .timeout(config.timeout())
            .build()?;
        Ok(Self {
            client,
            url_builder: CdnUrl::new(&config.cdn_base_url),
            config,
        })
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    pub fn url_builder(&self) -> &CdnUrl {
        &self.url_builder
    }

    pub fn client(&self) -> &reqwest::Client {
        &self.client
    }

    pub async fn get_bytes(&self, url: &str) -> Result<Vec<u8>> {
        let policy = historical_core::http::RetryPolicy {
            max_retries: self.config.max_retries,
            retry_backoff_ms: self.config.retry_backoff_ms,
            retry_on_429: true,
            ..Default::default()
        };
        Ok(historical_core::http::get_bytes_with_retry(&self.client, url, policy).await?)
    }

    pub async fn download_to_file(&self, url: &str, path: &Path) -> Result<()> {
        let policy = historical_core::http::RetryPolicy {
            max_retries: self.config.max_retries,
            retry_backoff_ms: self.config.retry_backoff_ms,
            retry_on_429: true,
            ..Default::default()
        };
        historical_core::http::get_to_file_with_retry(&self.client, url, policy, path).await?;
        Ok(())
    }

    pub async fn get_json<T: serde::de::DeserializeOwned>(&self, url: &str) -> Result<T> {
        let bytes = self.get_bytes(url).await?;
        Ok(serde_json::from_slice(&bytes)?)
    }

    pub async fn head_exists(&self, url: &str) -> Result<bool> {
        let response = self.client.head(url).send().await?;
        Ok(response.status().is_success())
    }
}
