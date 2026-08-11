use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use tracing::{debug, warn};

use crate::config::Config;
use crate::error::{Error, Result};
use crate::model::Instrument;
use crate::DeribitHttpClient;

const CACHE_TTL: Duration = Duration::from_secs(6 * 60 * 60);

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CacheFile {
    fetched_at_unix_secs: i64,
    instruments: Vec<Instrument>,
}

/// File-backed instruments cache. Respects Deribit's 1 req/s get_instruments limit.
#[derive(Clone)]
pub struct InstrumentsCache {
    path: PathBuf,
    ttl: Duration,
    memory: Arc<Mutex<HashMap<String, (Instant, Vec<Instrument>)>>>,
}

impl InstrumentsCache {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            ttl: CACHE_TTL,
            memory: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn default_path() -> PathBuf {
        directories::ProjectDirs::from("com", "arathi", "deribit-historical")
            .map(|dirs| dirs.cache_dir().join("instruments.json"))
            .unwrap_or_else(|| PathBuf::from(".deribit-instruments-cache.json"))
    }

    pub async fn get_futures(
        &self,
        client: &DeribitHttpClient,
        currencies: &[String],
        include_expired: bool,
    ) -> Result<Vec<Instrument>> {
        let mut all = Vec::new();
        for currency in currencies {
            let currency = currency.trim().to_ascii_uppercase();
            if currency.is_empty() {
                continue;
            }
            let key = format!("{currency}:future:expired={include_expired}");
            if let Some(hit) = self.memory_hit(&key).await {
                all.extend(hit);
                continue;
            }
            if let Some(hit) = self.disk_hit(&key)? {
                self.memory_store(&key, hit.clone()).await;
                all.extend(hit);
                continue;
            }
            let fetched = client
                .get_instruments(&currency, "future", include_expired)
                .await?;
            self.persist(&key, &fetched)?;
            self.memory_store(&key, fetched.clone()).await;
            all.extend(fetched);
        }
        all.sort_by(|a, b| a.instrument_name.cmp(&b.instrument_name));
        all.dedup_by(|a, b| a.instrument_name == b.instrument_name);
        Ok(all)
    }

    async fn memory_hit(&self, key: &str) -> Option<Vec<Instrument>> {
        let guard = self.memory.lock().await;
        guard.get(key).and_then(|(at, rows)| {
            (at.elapsed() < self.ttl).then(|| rows.clone())
        })
    }

    async fn memory_store(&self, key: &str, rows: Vec<Instrument>) {
        let mut guard = self.memory.lock().await;
        guard.insert(key.to_string(), (Instant::now(), rows));
    }

    fn disk_hit(&self, key: &str) -> Result<Option<Vec<Instrument>>> {
        let path = self.key_path(key);
        if !path.exists() {
            return Ok(None);
        }
        let bytes = std::fs::read(&path)?;
        let parsed: CacheFile = match serde_json::from_slice(&bytes) {
            Ok(parsed) => parsed,
            Err(error) => {
                warn!(%error, path = %path.display(), "ignoring corrupt instruments cache");
                return Ok(None);
            }
        };
        let age = chrono::Utc::now().timestamp() - parsed.fetched_at_unix_secs;
        if age < 0 || age as u64 > self.ttl.as_secs() {
            return Ok(None);
        }
        debug!(
            path = %path.display(),
            count = parsed.instruments.len(),
            "instruments cache hit"
        );
        Ok(Some(parsed.instruments))
    }

    fn persist(&self, key: &str, instruments: &[Instrument]) -> Result<()> {
        let path = self.key_path(key);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let payload = CacheFile {
            fetched_at_unix_secs: chrono::Utc::now().timestamp(),
            instruments: instruments.to_vec(),
        };
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_vec_pretty(&payload)?)?;
        std::fs::rename(&tmp, &path)?;
        Ok(())
    }

    fn key_path(&self, key: &str) -> PathBuf {
        let safe = key.replace(['/', ':', '='], "_");
        if self.path.is_dir() {
            self.path.join(format!("{safe}.json"))
        } else if let Some(parent) = self.path.parent() {
            parent.join(format!("{safe}.json"))
        } else {
            PathBuf::from(format!("{safe}.json"))
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Default for InstrumentsCache {
    fn default() -> Self {
        Self::new(Self::default_path())
    }
}

/// Resolve default currencies from config / CLI defaults.
pub fn default_currencies() -> Vec<String> {
    vec!["BTC".into(), "ETH".into()]
}

/// Currencies accepted by `public/get_volatility_index_data` for DVOL (v1: BTC+ETH).
pub fn parse_volatility_currency_list(raw: &str) -> Result<Vec<String>> {
    let mut out = Vec::new();
    for part in raw.split(',') {
        let value = part.trim().to_ascii_uppercase();
        if value.is_empty() {
            continue;
        }
        if !matches!(value.as_str(), "BTC" | "ETH") {
            return Err(Error::Config(format!(
                "unsupported volatility currency `{value}`; expected BTC and/or ETH \
                 (Deribit DVOL index params — not BTCUSD/ETHUSD instrument names)"
            )));
        }
        out.push(value);
    }
    out.sort();
    out.dedup();
    if out.is_empty() {
        return Err(Error::Config(
            "at least one volatility currency is required".into(),
        ));
    }
    Ok(out)
}

pub fn parse_currency_list(raw: &str) -> Result<Vec<String>> {
    let mut out = Vec::new();
    for part in raw.split(',') {
        let value = part.trim().to_ascii_uppercase();
        if value.is_empty() {
            continue;
        }
        if !matches!(value.as_str(), "BTC" | "ETH" | "USDC") {
            return Err(Error::Config(format!(
                "unsupported currency `{value}` in v1; expected BTC, ETH, and/or USDC"
            )));
        }
        out.push(value);
    }
    out.sort();
    out.dedup();
    if out.is_empty() {
        return Err(Error::Config("at least one currency is required".into()));
    }
    Ok(out)
}

pub fn filter_kind(instruments: Vec<Instrument>, kind: &str) -> Result<Vec<Instrument>> {
    let kind = kind.trim().to_ascii_lowercase();
    if kind != "future" {
        return Err(Error::Config(format!(
            "unsupported kind `{kind}` in v1; only `future` (includes perpetuals) is enabled"
        )));
    }
    Ok(instruments
        .into_iter()
        .filter(|instrument| instrument.kind.eq_ignore_ascii_case("future"))
        .collect())
}

/// Keep futures whose instrument name contains `DVOL` (e.g. `BTCDVOL_USDC-26AUG26`).
pub fn filter_dvol_instruments(instruments: Vec<Instrument>) -> Vec<Instrument> {
    instruments
        .into_iter()
        .filter(|instrument| {
            instrument
                .instrument_name
                .to_ascii_uppercase()
                .contains("DVOL")
        })
        .collect()
}

/// Convenience: load BTC/ETH futures with optional expired instruments for dated backfill.
pub async fn load_future_instruments(
    client: &DeribitHttpClient,
    cache: &InstrumentsCache,
    currencies: &[String],
    include_expired: bool,
) -> Result<Vec<Instrument>> {
    let _ = Config::default();
    let instruments = cache
        .get_futures(client, currencies, include_expired)
        .await?;
    filter_kind(instruments, "future")
}

/// Discover tradeable DVOL futures (listed under settlement currency `USDC`).
pub async fn load_dvol_instruments(
    client: &DeribitHttpClient,
    cache: &InstrumentsCache,
    include_expired: bool,
) -> Result<Vec<Instrument>> {
    let instruments = load_future_instruments(
        client,
        cache,
        &["USDC".into()],
        include_expired,
    )
    .await?;
    Ok(filter_dvol_instruments(instruments))
}
