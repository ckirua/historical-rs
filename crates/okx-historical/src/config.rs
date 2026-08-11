//! Client configuration.

use std::path::PathBuf;
use std::time::Duration;

use directories::BaseDirs;
use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

pub const CDN_BASE_URL: &str = "https://static.okx.com/cdn/okex/traderecords";
pub const REST_BASE_URL: &str = "https://www.okx.com";
pub const DEFAULT_USER_AGENT: &str = concat!("okx-historical/", env!("CARGO_PKG_VERSION"));
pub const DEFAULT_DATA_DIR: &str = ".data/okx-historical";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub cdn_base_url: String,
    pub rest_base_url: String,
    pub timeout_secs: u64,
    pub max_retries: u32,
    pub retry_backoff_ms: u64,
    pub user_agent: String,
    #[serde(skip)]
    pub data_dir: Option<PathBuf>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            cdn_base_url: CDN_BASE_URL.to_string(),
            rest_base_url: REST_BASE_URL.to_string(),
            timeout_secs: 120,
            max_retries: 3,
            retry_backoff_ms: 500,
            user_agent: DEFAULT_USER_AGENT.to_string(),
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
        Ok(self.data_dir()?.join("datasets"))
    }

    pub fn manifest_path(&self) -> Result<PathBuf> {
        Ok(self.data_dir()?.join("manifest.jsonl"))
    }

    pub fn symbols_db_path(&self) -> Result<PathBuf> {
        Ok(self.data_dir()?.join("symbols.db"))
    }

    pub fn ensure_data_dirs(&self) -> Result<()> {
        std::fs::create_dir_all(self.datasets_dir()?)?;
        Ok(())
    }
}

fn default_data_dir() -> Option<PathBuf> {
    if let Ok(value) = std::env::var("OKX_HISTORICAL_DATA_DIR") {
        return Some(PathBuf::from(value));
    }
    BaseDirs::new().map(|dirs| dirs.home_dir().join(DEFAULT_DATA_DIR))
}
