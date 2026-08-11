//! Shared HTTP GET-with-retry helpers.

use std::time::Duration;

use reqwest::StatusCode;
use tracing::debug;

use crate::error::{Error, Result};

/// Retry policy for exchange download clients.
#[derive(Debug, Clone, Copy)]
pub struct RetryPolicy {
    pub max_retries: u32,
    pub retry_backoff_ms: u64,
    /// When true, also retry HTTP 429 (rate limit).
    pub retry_on_429: bool,
    /// When true, also retry HTTP 403 (edge throttle / WAF) with exponential backoff.
    pub retry_on_403: bool,
    /// Cap applied to computed backoff delays.
    pub max_backoff_ms: u64,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_retries: 3,
            retry_backoff_ms: 500,
            retry_on_429: true,
            retry_on_403: false,
            max_backoff_ms: 60_000,
        }
    }
}

fn is_retryable_status(status: StatusCode, policy: RetryPolicy) -> bool {
    status.is_server_error()
        || (policy.retry_on_429 && status.as_u16() == 429)
        || (policy.retry_on_403 && status.as_u16() == 403)
}

fn is_retryable_transport(error: &reqwest::Error) -> bool {
    error.is_timeout() || error.is_connect()
}

fn backoff_delay(policy: RetryPolicy, attempt: u32, status: Option<u16>) -> Duration {
    let millis = if status == Some(403) && policy.retry_on_403 {
        // Long exponential backoff for edge/WAF 403: base, 2x, 4x, ...
        let shift = attempt.saturating_sub(1).min(10);
        policy
            .retry_backoff_ms
            .saturating_mul(1u64 << shift)
    } else {
        policy.retry_backoff_ms.saturating_mul(attempt as u64)
    };
    let capped = millis.min(policy.max_backoff_ms.max(policy.retry_backoff_ms));
    Duration::from_millis(capped)
}

/// GET `url` as bytes, treating 404 as [`Error::NotFound`] and retrying server errors.
pub async fn get_bytes_with_retry(
    client: &reqwest::Client,
    url: &str,
    policy: RetryPolicy,
) -> Result<Vec<u8>> {
    let mut attempt = 0u32;
    loop {
        let response = match client.get(url).send().await {
            Ok(response) => response,
            Err(error) if is_retryable_transport(&error) && attempt < policy.max_retries => {
                attempt += 1;
                let delay = backoff_delay(policy, attempt, None);
                debug!(url, attempt, ?delay, error = %error, "retrying HTTP transport error");
                tokio::time::sleep(delay).await;
                continue;
            }
            Err(error) => return Err(error.into()),
        };
        let status = response.status();
        if status == StatusCode::NOT_FOUND {
            return Err(Error::NotFound(url.to_string()));
        }
        if status.is_success() {
            return Ok(response.bytes().await?.to_vec());
        }
        if is_retryable_status(status, policy) && attempt < policy.max_retries {
            attempt += 1;
            let delay = backoff_delay(policy, attempt, Some(status.as_u16()));
            debug!(url, attempt, ?delay, status = status.as_u16(), "retrying HTTP request");
            tokio::time::sleep(delay).await;
            continue;
        }
        return Err(Error::HttpStatus {
            status: status.as_u16(),
            url: url.to_string(),
        });
    }
}

/// GET `url` into a file, bounding memory to one response chunk.
pub async fn get_to_file_with_retry(
    client: &reqwest::Client,
    url: &str,
    policy: RetryPolicy,
    path: &std::path::Path,
) -> Result<u64> {
    use tokio::io::AsyncWriteExt;

    let mut attempt = 0u32;
    loop {
        let mut response = match client.get(url).send().await {
            Ok(response) => response,
            Err(error) if is_retryable_transport(&error) && attempt < policy.max_retries => {
                attempt += 1;
                let delay = backoff_delay(policy, attempt, None);
                debug!(url, attempt, ?delay, error = %error, "retrying HTTP transport error");
                tokio::time::sleep(delay).await;
                continue;
            }
            Err(error) => return Err(error.into()),
        };
        let status = response.status();
        if status == StatusCode::NOT_FOUND {
            return Err(Error::NotFound(url.to_string()));
        }
        if status.is_success() {
            let mut output = tokio::fs::File::create(path).await?;
            let mut written = 0u64;
            while let Some(chunk) = response.chunk().await? {
                output.write_all(&chunk).await?;
                written += chunk.len() as u64;
            }
            output.flush().await?;
            return Ok(written);
        }
        if is_retryable_status(status, policy) && attempt < policy.max_retries {
            attempt += 1;
            let delay = backoff_delay(policy, attempt, Some(status.as_u16()));
            debug!(url, attempt, ?delay, status = status.as_u16(), "retrying HTTP request");
            tokio::time::sleep(delay).await;
            continue;
        }
        return Err(Error::HttpStatus {
            status: status.as_u16(),
            url: url.to_string(),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backoff_for_403_grows_exponentially_and_caps() {
        let policy = RetryPolicy {
            retry_backoff_ms: 2_000,
            retry_on_403: true,
            max_backoff_ms: 10_000,
            ..Default::default()
        };
        assert_eq!(backoff_delay(policy, 1, Some(403)), Duration::from_millis(2_000));
        assert_eq!(backoff_delay(policy, 2, Some(403)), Duration::from_millis(4_000));
        assert_eq!(backoff_delay(policy, 3, Some(403)), Duration::from_millis(8_000));
        assert_eq!(backoff_delay(policy, 4, Some(403)), Duration::from_millis(10_000));
    }

    #[test]
    fn backoff_for_5xx_stays_linear() {
        let policy = RetryPolicy {
            retry_backoff_ms: 500,
            ..Default::default()
        };
        assert_eq!(backoff_delay(policy, 1, Some(503)), Duration::from_millis(500));
        assert_eq!(backoff_delay(policy, 3, Some(503)), Duration::from_millis(1_500));
    }

    #[test]
    fn retryable_status_respects_flags() {
        let base = RetryPolicy::default();
        assert!(is_retryable_status(StatusCode::INTERNAL_SERVER_ERROR, base));
        assert!(is_retryable_status(StatusCode::TOO_MANY_REQUESTS, base));
        assert!(!is_retryable_status(StatusCode::FORBIDDEN, base));

        let with_403 = RetryPolicy {
            retry_on_403: true,
            ..base
        };
        assert!(is_retryable_status(StatusCode::FORBIDDEN, with_403));
        assert!(!is_retryable_status(StatusCode::UNAUTHORIZED, with_403));
    }
}
