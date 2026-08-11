//! Shared async HTTP client.

use crate::config::Config;
use crate::error::Result;

#[derive(Clone)]
pub struct HttpClient {
    client: reqwest::Client,
    config: Config,
}

impl HttpClient {
    pub fn new(config: Config) -> Result<Self> {
        let client = reqwest::Client::builder()
            .user_agent(&config.user_agent)
            .timeout(config.timeout())
            .build()?;
        Ok(Self { client, config })
    }

    pub fn config(&self) -> &Config {
        &self.config
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

    pub async fn get_json<T: serde::de::DeserializeOwned>(&self, url: &str) -> Result<T> {
        let bytes = self.get_bytes(url).await?;
        Ok(serde_json::from_slice(&bytes)?)
    }
}
