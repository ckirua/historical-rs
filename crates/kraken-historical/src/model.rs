//! Shared domain types for Kraken historical Drive dumps.

use serde::{Deserialize, Serialize};

/// Kraken historical product type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DataType {
    Trades,
    Ohlcvt,
}

impl DataType {
    pub fn path_segment(self) -> &'static str {
        match self {
            DataType::Trades => "trades",
            DataType::Ohlcvt => "ohlcvt",
        }
    }

    pub fn as_str(self) -> &'static str {
        self.path_segment()
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "trades" | "trade" => Some(DataType::Trades),
            "ohlcvt" | "ohlc" | "candles" | "candlesticks" => Some(DataType::Ohlcvt),
            _ => None,
        }
    }
}

/// Archive source: full history dump or a calendar quarter.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Source {
    Complete,
    Quarterly { quarter: String },
}

impl Source {
    pub fn path_segment(&self) -> String {
        match self {
            Source::Complete => "complete".to_string(),
            Source::Quarterly { quarter } => format!("quarterly/{quarter}"),
        }
    }

    pub fn label(&self) -> String {
        match self {
            Source::Complete => "complete".to_string(),
            Source::Quarterly { quarter } => quarter.clone(),
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        let lower = s.to_ascii_lowercase();
        if lower == "complete" || lower == "full" {
            return Some(Source::Complete);
        }
        parse_quarter(&lower).map(|quarter| Source::Quarterly { quarter })
    }
}

/// Normalize `2024Q3`, `2024q3`, `Q3_2024` → `2024Q3`.
pub fn parse_quarter(s: &str) -> Option<String> {
    let s = s.trim();
    // YYYYQn
    if s.len() == 6 {
        let year = &s[0..4];
        let q = &s[4..];
        if year.chars().all(|c| c.is_ascii_digit())
            && matches!(q, "Q1" | "Q2" | "Q3" | "Q4" | "q1" | "q2" | "q3" | "q4")
        {
            return Some(format!("{}{}", year, q.to_ascii_uppercase()));
        }
    }
    // Qn_YYYY (from some zip names after parse)
    if s.len() >= 6 && s.starts_with('Q') || s.starts_with('q') {
        // handled below via filename parsers
    }
    None
}

/// One Google Drive file (complete dump or quarterly ZIP).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DriveArtifact {
    pub data_type: DataType,
    pub source: Source,
    pub file_id: String,
    pub name: String,
    pub size_hint: Option<u64>,
}

/// Headerless trades CSV row (`ts`, `price`, `volume`).
#[derive(Debug, Clone, PartialEq)]
pub struct TradeRow {
    pub ts: f64,
    pub price: f64,
    pub volume: f64,
}

/// OHLCVT CSV row for `{PAIR}_{interval}.csv`.
#[derive(Debug, Clone, PartialEq)]
pub struct OhlcvtRow {
    pub ts: i64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
    pub trades: i64,
}

/// One planned parquet output (pair × source, plus interval for OHLCVT).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct OutputUnit {
    pub data_type: DataType,
    pub source: Source,
    pub pair: String,
    /// Present for OHLCVT (`1`, `5`, `15`, `60`, `720`, `1440`, …).
    pub interval: Option<String>,
}

impl OutputUnit {
    pub fn trades(source: Source, pair: impl Into<String>) -> Self {
        Self {
            data_type: DataType::Trades,
            source,
            pair: pair.into(),
            interval: None,
        }
    }

    pub fn ohlcvt(source: Source, pair: impl Into<String>, interval: impl Into<String>) -> Self {
        Self {
            data_type: DataType::Ohlcvt,
            source,
            pair: pair.into(),
            interval: Some(interval.into()),
        }
    }
}

/// Aggregate mirror progress.
pub use historical_core::MirrorStatus;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_quarter_yyyyqn() {
        assert_eq!(parse_quarter("2024Q3").as_deref(), Some("2024Q3"));
        assert_eq!(parse_quarter("2024q1").as_deref(), Some("2024Q1"));
        assert!(parse_quarter("2024Q5").is_none());
        assert!(parse_quarter("complete").is_none());
    }

    #[test]
    fn source_parse() {
        assert_eq!(Source::parse("complete"), Some(Source::Complete));
        assert_eq!(
            Source::parse("2024Q3"),
            Some(Source::Quarterly {
                quarter: "2024Q3".into()
            })
        );
    }

    #[test]
    fn data_type_parse() {
        assert_eq!(DataType::parse("trades"), Some(DataType::Trades));
        assert_eq!(DataType::parse("ohlcvt"), Some(DataType::Ohlcvt));
        assert_eq!(DataType::parse("ohlc"), Some(DataType::Ohlcvt));
    }
}
