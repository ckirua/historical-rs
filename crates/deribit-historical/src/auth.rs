use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::Deserialize;
use tokio::sync::Mutex;
use tracing::{debug, info};

use crate::config::Config;
use crate::error::{Error, Result};
use crate::rate_limit::TokenBucket;

#[derive(Debug, Clone)]
struct CachedToken {
    access_token: String,
    expires_at: Instant,
}

/// Single-flight OAuth client_credentials token cache.
#[derive(Clone)]
pub struct AuthToken {
    client: reqwest::Client,
    config: Config,
    limiter: Arc<TokenBucket>,
    cached: Arc<Mutex<Option<CachedToken>>>,
}

impl AuthToken {
    pub fn new(client: reqwest::Client, config: Config, limiter: Arc<TokenBucket>) -> Self {
        Self {
            client,
            config,
            limiter,
            cached: Arc::new(Mutex::new(None)),
        }
    }

    pub fn enabled(&self) -> bool {
        self.config.has_credentials()
    }

    /// Return a bearer token, refreshing when absent or near expiry.
    pub async fn bearer(&self) -> Result<Option<String>> {
        if !self.enabled() {
            return Ok(None);
        }
        {
            let guard = self.cached.lock().await;
            if let Some(token) = guard.as_ref() {
                if Instant::now() + Duration::from_secs(60) < token.expires_at {
                    return Ok(Some(token.access_token.clone()));
                }
            }
        }
        self.refresh().await
    }

    async fn refresh(&self) -> Result<Option<String>> {
        let mut guard = self.cached.lock().await;
        if let Some(token) = guard.as_ref() {
            if Instant::now() + Duration::from_secs(60) < token.expires_at {
                return Ok(Some(token.access_token.clone()));
            }
        }
        let client_id = self
            .config
            .client_id
            .as_deref()
            .ok_or_else(|| Error::Config("DERIBIT_CLIENT_ID is not set".into()))?;
        let client_secret = self
            .config
            .client_secret
            .as_deref()
            .ok_or_else(|| Error::Config("DERIBIT_CLIENT_SECRET is not set".into()))?;

        self.limiter.acquire().await;
        let url = format!("{}/public/auth", self.config.auth_url.trim_end_matches('/'));
        debug!(%url, "requesting Deribit OAuth token");
        let response = self
            .client
            .get(&url)
            .query(&[
                ("grant_type", "client_credentials"),
                ("client_id", client_id),
                ("client_secret", client_secret),
            ])
            .send()
            .await?;
        let status = response.status();
        let bytes = response.bytes().await?;
        if !status.is_success() {
            return Err(Error::HttpStatus {
                status: status.as_u16(),
                url,
            });
        }
        let parsed: AuthResponse = serde_json::from_slice(&bytes)?;
        if let Some(error) = parsed.error {
            return Err(Error::Api {
                code: error.code,
                message: error.message,
            });
        }
        let result = parsed.result.ok_or_else(|| {
            Error::Config("Deribit auth response missing result".into())
        })?;
        let expires_in = result.expires_in.max(120);
        let token = CachedToken {
            access_token: result.access_token.clone(),
            expires_at: Instant::now() + Duration::from_secs(expires_in as u64),
        };
        info!(expires_in, "Deribit OAuth token refreshed");
        *guard = Some(token);
        Ok(Some(result.access_token))
    }

    pub async fn invalidate(&self) {
        let mut guard = self.cached.lock().await;
        *guard = None;
    }
}

#[derive(Debug, Deserialize)]
struct AuthResponse {
    result: Option<AuthResult>,
    error: Option<ApiErrorBody>,
}

#[derive(Debug, Deserialize)]
struct AuthResult {
    access_token: String,
    expires_in: i64,
}

#[derive(Debug, Deserialize)]
struct ApiErrorBody {
    code: i64,
    message: String,
}
