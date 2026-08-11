//! Client configuration.

use std::path::PathBuf;
use std::time::Duration;

use directories::BaseDirs;
use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::model::Market;

pub const VISION_BASE_URL: &str = "https://data.binance.vision/data";
pub const SPOT_API_URL: &str = "https://api.binance.com/api/v3/exchangeInfo";
pub const UM_API_URL: &str = "https://fapi.binance.com/fapi/v1/exchangeInfo";
pub const CM_API_URL: &str = "https://dapi.binance.com/dapi/v1/exchangeInfo";
pub const DEFAULT_USER_AGENT: &str = concat!("binance-vision/", env!("CARGO_PKG_VERSION"));
pub const DEFAULT_DATA_DIR: &str = ".data/binance-vision";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub vision_base_url: String,
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
            vision_base_url: VISION_BASE_URL.to_string(),
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

    /// Per-market SQLite symbol cache path (`symbols-{market}.db`).
    ///
    /// Separate files avoid writer lock contention when UM/spot/CM crons overlap.
    pub fn symbols_db_path(&self, market: Market) -> Result<PathBuf> {
        Ok(self
            .data_dir()?
            .join(format!("symbols-{}.db", market.as_str())))
    }

    pub fn ensure_data_dirs(&self) -> Result<()> {
        std::fs::create_dir_all(self.datasets_dir()?)?;
        Ok(())
    }
}

fn default_data_dir() -> Option<PathBuf> {
    if let Ok(value) = std::env::var("BINANCE_VISION_DATA_DIR") {
        return Some(PathBuf::from(value));
    }
    BaseDirs::new().map(|dirs| dirs.home_dir().join(DEFAULT_DATA_DIR))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn symbols_db_path_is_per_market() {
        let mut config = Config::default();
        config.data_dir = Some(PathBuf::from("/tmp/binance-vision-test"));
        assert_eq!(
            config.symbols_db_path(Market::Um).unwrap(),
            PathBuf::from("/tmp/binance-vision-test/symbols-um.db")
        );
        assert_eq!(
            config.symbols_db_path(Market::Spot).unwrap(),
            PathBuf::from("/tmp/binance-vision-test/symbols-spot.db")
        );
        assert_eq!(
            config.symbols_db_path(Market::Cm).unwrap(),
            PathBuf::from("/tmp/binance-vision-test/symbols-cm.db")
        );
    }
}
