//! Client configuration.

use std::path::PathBuf;
use std::time::Duration;

use directories::BaseDirs;
use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

pub const REST_BASE_URL: &str = "https://api.kraken.com";
pub const DEFAULT_USER_AGENT: &str = concat!("kraken-historical/", env!("CARGO_PKG_VERSION"));
pub const DEFAULT_DATA_DIR: &str = ".data/kraken-historical";
pub const DEFAULT_PARALLEL: usize = 4;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub rest_base_url: String,
    pub timeout_secs: u64,
    pub max_retries: u32,
    pub retry_backoff_ms: u64,
    pub user_agent: String,
    pub drive_api_key: Option<String>,
    #[serde(skip)]
    pub data_dir: Option<PathBuf>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            rest_base_url: REST_BASE_URL.to_string(),
            timeout_secs: 600,
            max_retries: 3,
            retry_backoff_ms: 1000,
            user_agent: DEFAULT_USER_AGENT.to_string(),
            drive_api_key: std::env::var("KRAKEN_GOOGLE_DRIVE_API_KEY")
                .ok()
                .map(|v| v.trim().to_string())
                .filter(|v| !v.is_empty()),
            data_dir: default_data_dir(),
        }
    }
}

impl Config {
    pub fn load() -> Result<Self> {
        Ok(Self::default())
    }

    pub fn timeout(&self) -> Duration {
        Duration::from_secs(self.timeout_secs)
    }

    pub fn data_dir(&self) -> Result<PathBuf> {
        self.data_dir
            .clone()
            .or_else(default_data_dir)
            .ok_or_else(|| Error::Config("unable to resolve data directory".into()))
    }

    pub fn datasets_dir(&self) -> Result<PathBuf> {
        Ok(self.data_dir()?.join("data"))
    }

    pub fn zips_dir(&self) -> Result<PathBuf> {
        Ok(self.data_dir()?.join("zips"))
    }

    pub fn manifest_path(&self) -> Result<PathBuf> {
        Ok(self.data_dir()?.join("manifest.jsonl"))
    }

    pub fn ensure_data_dirs(&self) -> Result<()> {
        std::fs::create_dir_all(self.datasets_dir()?)?;
        std::fs::create_dir_all(self.zips_dir()?)?;
        Ok(())
    }

    pub fn drive_api_key(&self) -> Option<&str> {
        self.drive_api_key.as_deref()
    }

    /// Optional Drive API key (fallback if public folder scrape fails).
    #[allow(dead_code)]
    pub fn require_drive_api_key(&self) -> Result<&str> {
        self.drive_api_key().ok_or_else(|| {
            Error::Config(
                "KRAKEN_GOOGLE_DRIVE_API_KEY is not set (optional fallback for quarterly list)"
                    .into(),
            )
        })
    }
}

pub fn default_parallel() -> usize {
    std::env::var("KRAKEN_HISTORICAL_PARALLEL")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(DEFAULT_PARALLEL)
}

fn default_data_dir() -> Option<PathBuf> {
    if let Ok(value) = std::env::var("KRAKEN_HISTORICAL_DATA_DIR") {
        let path = PathBuf::from(value);
        if path.starts_with("~") {
            if let Some(home) = BaseDirs::new() {
                let stripped = path.strip_prefix("~").unwrap_or(&path);
                return Some(home.home_dir().join(stripped));
            }
        }
        return Some(path);
    }
    BaseDirs::new().map(|dirs| dirs.home_dir().join(DEFAULT_DATA_DIR))
}
