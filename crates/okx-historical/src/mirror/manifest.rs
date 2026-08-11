//! Mirror progress manifest (JSONL).

use std::collections::HashSet;
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};

use serde::{Deserialize, Serialize};

use crate::config::Config;
use crate::error::Result;
use crate::model::{DataType, Frequency, Market, MirrorStatus};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ManifestEntry {
    pub market: String,
    pub frequency: String,
    pub data_type: String,
    pub symbol: String,
    pub period: String,
    pub source_url: String,
    pub status: String,
    pub path: Option<String>,
    pub rows: Option<usize>,
    pub error: Option<String>,
}

impl ManifestEntry {
    pub fn key(&self) -> String {
        manifest_key(
            &self.market,
            &self.frequency,
            &self.data_type,
            &self.symbol,
            &self.period,
            &self.source_url,
        )
    }
}

pub fn manifest_key(
    market: &str,
    frequency: &str,
    data_type: &str,
    symbol: &str,
    period: &str,
    source_url: &str,
) -> String {
    format!("{market}:{frequency}:{data_type}:{symbol}:{period}:{source_url}")
}

pub fn mirror_status(config: &Config) -> Result<MirrorStatus> {
    let path = config.manifest_path()?;
    if !path.exists() {
        return Ok(MirrorStatus::default());
    }
    mirror_status_from_reader(BufReader::new(File::open(path)?))
}

pub fn mirror_status_from_str(content: &str) -> Result<MirrorStatus> {
    mirror_status_from_reader(content.as_bytes())
}

fn mirror_status_from_reader<R: BufRead>(reader: R) -> Result<MirrorStatus> {
    let mut status = MirrorStatus::default();
    for line in reader.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let entry: ManifestEntry = serde_json::from_str(&line)?;
        status.total += 1;
        match entry.status.as_str() {
            "ok" => status.done += 1,
            "skipped" => status.skipped += 1,
            "failed" => status.failed += 1,
            _ => status.pending += 1,
        }
    }
    Ok(status)
}

pub fn load_completed_keys(path: std::path::PathBuf) -> Result<HashSet<String>> {
    let mut keys = HashSet::new();
    if !path.exists() {
        return Ok(keys);
    }
    let reader = BufReader::new(File::open(path)?);
    for line in reader.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let entry: ManifestEntry = serde_json::from_str(&line)?;
        if entry.status == "ok" || entry.status == "skipped" {
            keys.insert(entry.key());
        }
    }
    Ok(keys)
}

pub fn append_manifest(config: &Config, entry: &ManifestEntry) -> Result<()> {
    let path = config.manifest_path()?;
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    serde_json::to_writer(&mut file, entry)?;
    file.write_all(b"\n")?;
    Ok(())
}

pub fn entry_from_file(
    market: Market,
    frequency: Frequency,
    data_type: DataType,
    symbol: &str,
    period: &str,
    source_url: &str,
) -> ManifestEntry {
    ManifestEntry {
        market: market.as_str().to_string(),
        frequency: frequency.as_str().to_string(),
        data_type: data_type.path_segment().to_string(),
        symbol: symbol.to_string(),
        period: period.to_string(),
        source_url: source_url.to_string(),
        status: "pending".to_string(),
        path: None,
        rows: None,
        error: None,
    }
}
