//! Mirror progress manifest (JSONL).

use std::collections::HashSet;
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};

use serde::{Deserialize, Serialize};

use crate::config::Config;
use crate::error::Result;
use crate::model::{MirrorStatus, OutputUnit};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ManifestEntry {
    pub data_type: String,
    /// `complete` or `2024Q3`
    pub source: String,
    pub pair: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interval: Option<String>,
    pub file_id: String,
    pub status: String,
    pub path: Option<String>,
    pub rows: Option<usize>,
    pub error: Option<String>,
}

impl ManifestEntry {
    pub fn key(&self) -> String {
        manifest_key(
            &self.data_type,
            &self.source,
            &self.pair,
            self.interval.as_deref(),
            &self.file_id,
        )
    }
}

pub fn manifest_key(
    data_type: &str,
    source: &str,
    pair: &str,
    interval: Option<&str>,
    file_id: &str,
) -> String {
    let interval = interval.unwrap_or("-");
    format!("{data_type}:{source}:{pair}:{interval}:{file_id}")
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
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    serde_json::to_writer(&mut file, entry)?;
    file.write_all(b"\n")?;
    Ok(())
}

pub fn entry_from_unit(unit: &OutputUnit, file_id: &str) -> ManifestEntry {
    ManifestEntry {
        data_type: unit.data_type.path_segment().to_string(),
        source: unit.source.label(),
        pair: unit.pair.clone(),
        interval: unit.interval.clone(),
        file_id: file_id.to_string(),
        status: "pending".to_string(),
        path: None,
        rows: None,
        error: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{DataType, OutputUnit, Source};

    #[test]
    fn manifest_key_stable() {
        let unit = OutputUnit::ohlcvt(
            Source::Quarterly {
                quarter: "2024Q3".into(),
            },
            "XBTUSD",
            "1",
        );
        let entry = entry_from_unit(&unit, "file123");
        assert_eq!(entry.key(), "ohlcvt:2024Q3:XBTUSD:1:file123");
        let trades = entry_from_unit(&OutputUnit::trades(Source::Complete, "ETHUSD"), "abc");
        assert_eq!(trades.key(), "trades:complete:ETHUSD:-:abc");
        let _ = DataType::Trades;
    }

    #[test]
    fn status_from_str() {
        let content = r#"{"data_type":"trades","source":"complete","pair":"XBTUSD","file_id":"x","status":"ok","path":null,"rows":1,"error":null}
{"data_type":"trades","source":"complete","pair":"ETHUSD","file_id":"x","status":"failed","path":null,"rows":null,"error":"boom"}
"#;
        let status = mirror_status_from_str(content).unwrap();
        assert_eq!(status.total, 2);
        assert_eq!(status.done, 1);
        assert_eq!(status.failed, 1);
    }
}
