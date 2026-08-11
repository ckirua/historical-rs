//! Disk-backed ZIP member listing and CSV extraction.

use std::fs::File;
use std::io::Read;
use std::path::Path;

use zip::read::ZipArchive;

use crate::error::{Error, Result};
use crate::model::DataType;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZipMember {
    pub name: String,
    pub index: usize,
    pub pair: String,
    pub interval: Option<String>,
}

/// List CSV members and map them to pair / interval.
pub fn list_csv_members(zip_path: &Path, data_type: DataType) -> Result<Vec<ZipMember>> {
    let file = File::open(zip_path)?;
    let mut archive = ZipArchive::new(file)
        .map_err(|e| Error::Zip(format!("open {}: {e}", zip_path.display())))?;
    let mut members = Vec::new();
    for i in 0..archive.len() {
        let entry = archive
            .by_index(i)
            .map_err(|e| Error::Zip(format!("zip index {i}: {e}")))?;
        if entry.is_dir() {
            continue;
        }
        let name = entry.name().to_string();
        if !name.to_ascii_lowercase().ends_with(".csv") {
            continue;
        }
        let file_name = name.rsplit('/').next().unwrap_or(&name);
        match data_type {
            DataType::Trades => {
                if let Some(pair) = parse_trades_member_name(file_name) {
                    members.push(ZipMember {
                        name,
                        index: i,
                        pair,
                        interval: None,
                    });
                }
            }
            DataType::Ohlcvt => {
                if let Some((pair, interval)) = parse_ohlcvt_member_name(file_name) {
                    members.push(ZipMember {
                        name,
                        index: i,
                        pair,
                        interval: Some(interval),
                    });
                }
            }
        }
    }
    members.sort_by(|a, b| (&a.pair, &a.interval).cmp(&(&b.pair, &b.interval)));
    Ok(members)
}

/// Extract one CSV member by archive index (opens ZIP from disk).
pub fn extract_csv_member(zip_path: &Path, index: usize) -> Result<Vec<u8>> {
    let file = File::open(zip_path)?;
    let mut archive = ZipArchive::new(file)
        .map_err(|e| Error::Zip(format!("open {}: {e}", zip_path.display())))?;
    let mut entry = archive
        .by_index(index)
        .map_err(|e| Error::Zip(format!("zip index {index}: {e}")))?;
    let mut buf = Vec::new();
    entry
        .read_to_end(&mut buf)
        .map_err(|e| Error::Zip(format!("read member {index}: {e}")))?;
    Ok(buf)
}

/// `{PAIR}.csv` → pair (also accepts nested paths already stripped).
pub fn parse_trades_member_name(file_name: &str) -> Option<String> {
    let lower = file_name.to_ascii_lowercase();
    if !lower.ends_with(".csv") {
        return None;
    }
    let stem = &file_name[..file_name.len() - 4];
    if stem.is_empty() || stem.contains('_') {
        // OHLCVT-style names are not trades members
        // Exception: some dumps may use PAIR only
        if stem.contains('_') {
            return None;
        }
    }
    // Reject obvious non-pair names
    if stem.eq_ignore_ascii_case("readme") || stem.eq_ignore_ascii_case("metadata") {
        return None;
    }
    Some(stem.to_string())
}

/// `{PAIR}_{interval}.csv` → (pair, interval).
pub fn parse_ohlcvt_member_name(file_name: &str) -> Option<(String, String)> {
    let lower = file_name.to_ascii_lowercase();
    if !lower.ends_with(".csv") {
        return None;
    }
    let stem = &file_name[..file_name.len() - 4];
    let (pair, interval) = stem.rsplit_once('_')?;
    if pair.is_empty() || interval.is_empty() {
        return None;
    }
    // interval should be numeric minutes (1, 5, 15, 60, 720, 1440, …)
    if !interval.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    Some((pair.to_string(), interval.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trades_member_names() {
        assert_eq!(
            parse_trades_member_name("XBTUSD.csv").as_deref(),
            Some("XBTUSD")
        );
        assert_eq!(
            parse_trades_member_name("XXBTZUSD.csv").as_deref(),
            Some("XXBTZUSD")
        );
        assert!(parse_trades_member_name("XBTUSD_1.csv").is_none());
    }

    #[test]
    fn ohlcvt_member_names() {
        assert_eq!(
            parse_ohlcvt_member_name("XBTUSD_1.csv"),
            Some(("XBTUSD".into(), "1".into()))
        );
        assert_eq!(
            parse_ohlcvt_member_name("ETHUSD_1440.csv"),
            Some(("ETHUSD".into(), "1440".into()))
        );
        assert!(parse_ohlcvt_member_name("XBTUSD.csv").is_none());
    }
}
