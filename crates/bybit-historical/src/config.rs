use std::time::Duration;

pub const PORTAL_URL: &str = "https://api2.bybit.com/quote/public/support/download/";
/// Bybit V5 public instruments endpoint used for current trading symbol discovery.
pub const INSTRUMENTS_INFO_URL: &str = "https://api.bybit.com/v5/market/instruments-info";
/// Backward-compatible alias for [`INSTRUMENTS_INFO_URL`].
pub const SPOT_INSTRUMENTS_INFO_URL: &str = INSTRUMENTS_INFO_URL;
pub const DEFAULT_USER_AGENT: &str = concat!("bybit-historical/", env!("CARGO_PKG_VERSION"));

#[derive(Debug, Clone)]
pub struct Config {
    pub portal_url: String,
    pub timeout_secs: u64,
    pub max_retries: u32,
    pub retry_backoff_ms: u64,
    /// Base backoff for `public.bybit.com` downloads (403 uses exponential growth).
    pub download_retry_backoff_ms: u64,
    /// Cap for download backoff (especially long 403 cool-downs).
    pub download_max_backoff_ms: u64,
    /// Trip the download circuit after this many consecutive exhausted HTTP 403 failures.
    pub download_403_circuit_threshold: u32,
    pub user_agent: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            portal_url: PORTAL_URL.to_string(),
            timeout_secs: 180,
            max_retries: 5,
            retry_backoff_ms: 500,
            download_retry_backoff_ms: 2_000,
            download_max_backoff_ms: 120_000,
            download_403_circuit_threshold: 24,
            user_agent: DEFAULT_USER_AGENT.to_string(),
        }
    }
}

impl Config {
    pub fn timeout(&self) -> Duration {
        Duration::from_secs(self.timeout_secs)
    }
}
