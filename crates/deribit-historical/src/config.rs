use std::time::Duration;

/// Default Deribit history REST root (public archive host).
pub const HISTORY_URL: &str = "https://history.deribit.com/api/v2";
/// Live REST root used for OAuth `public/auth` and `get_volatility_index_data`
/// (history host returns 400 for volatility index).
pub const AUTH_URL: &str = "https://www.deribit.com/api/v2";
pub const DEFAULT_USER_AGENT: &str = concat!("deribit-historical/", env!("CARGO_PKG_VERSION"));

/// Sustained request budget for authenticated non-matching credit refill (~20/s).
pub const DEFAULT_REQUESTS_PER_SEC: f64 = 12.0;
/// Conservative budget when `DERIBIT_CLIENT_*` are unset (per-IP public limits).
pub const UNAUTH_REQUESTS_PER_SEC: f64 = 2.0;
/// Custom Deribit limit for `public/get_instruments`.
pub const INSTRUMENTS_REQUESTS_PER_SEC: f64 = 1.0;
pub const DEFAULT_TRADE_PAGE_COUNT: u32 = 1000;
pub const DEFAULT_PARALLEL: usize = 2;
/// Supported `public/get_volatility_index_data` resolutions.
pub const VOLATILITY_RESOLUTIONS: &[&str] = &["1", "60", "3600", "43200", "1D"];

#[derive(Debug, Clone)]
pub struct Config {
    pub history_url: String,
    pub auth_url: String,
    pub timeout_secs: u64,
    pub max_retries: u32,
    pub retry_backoff_ms: u64,
    pub max_backoff_ms: u64,
    pub requests_per_sec: f64,
    pub instruments_requests_per_sec: f64,
    pub trade_page_count: u32,
    pub user_agent: String,
    /// Optional OAuth client credentials (preferred for production rate limits).
    pub client_id: Option<String>,
    pub client_secret: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            history_url: std::env::var("DERIBIT_HISTORY_URL")
                .unwrap_or_else(|_| HISTORY_URL.to_string()),
            auth_url: std::env::var("DERIBIT_AUTH_URL").unwrap_or_else(|_| AUTH_URL.to_string()),
            timeout_secs: 120,
            max_retries: 8,
            retry_backoff_ms: 500,
            max_backoff_ms: 60_000,
            requests_per_sec: DEFAULT_REQUESTS_PER_SEC,
            instruments_requests_per_sec: INSTRUMENTS_REQUESTS_PER_SEC,
            trade_page_count: DEFAULT_TRADE_PAGE_COUNT,
            user_agent: DEFAULT_USER_AGENT.to_string(),
            client_id: std::env::var("DERIBIT_CLIENT_ID").ok().filter(|v| !v.is_empty()),
            client_secret: std::env::var("DERIBIT_CLIENT_SECRET")
                .ok()
                .filter(|v| !v.is_empty()),
        }
    }
}

impl Config {
    pub fn timeout(&self) -> Duration {
        Duration::from_secs(self.timeout_secs)
    }

    pub fn has_credentials(&self) -> bool {
        self.client_id.is_some() && self.client_secret.is_some()
    }

    pub fn with_credentials(mut self, client_id: String, client_secret: String) -> Self {
        self.client_id = Some(client_id);
        self.client_secret = Some(client_secret);
        self
    }

    /// Prefer this for unauthenticated smokes (per-IP limits).
    pub fn for_unauthenticated_smoke(mut self) -> Self {
        if !self.has_credentials() {
            self.requests_per_sec = UNAUTH_REQUESTS_PER_SEC;
        }
        self
    }
}

pub fn parse_volatility_resolution(raw: &str) -> Result<String, String> {
    let value = raw.trim();
    if VOLATILITY_RESOLUTIONS.iter().any(|allowed| *allowed == value) {
        Ok(value.to_string())
    } else {
        Err(format!(
            "unsupported volatility resolution `{value}`; expected one of {}",
            VOLATILITY_RESOLUTIONS.join(", ")
        ))
    }
}
