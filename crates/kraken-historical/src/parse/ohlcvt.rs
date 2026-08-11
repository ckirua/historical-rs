//! OHLCVT CSV parser (`ts,open,high,low,close,volume,trades`).

use crate::error::{Error, Result};
use crate::model::OhlcvtRow;

pub fn parse_ohlcvt_csv(csv_bytes: &[u8]) -> Result<Vec<OhlcvtRow>> {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .from_reader(csv_bytes);

    let mut rows = Vec::new();
    for (idx, record) in reader.records().enumerate() {
        let record = record?;
        if record.len() < 7 {
            continue;
        }
        if idx == 0 {
            let first = record.get(0).unwrap_or("").to_ascii_lowercase();
            if first == "timestamp"
                || first == "time"
                || first == "ts"
                || first == "date"
                || first.contains("open")
            {
                continue;
            }
        }

        let ts = parse_ts(record.get(0).unwrap_or(""))?;
        let open: f64 = parse_f64(record.get(1).unwrap_or(""))?;
        let high: f64 = parse_f64(record.get(2).unwrap_or(""))?;
        let low: f64 = parse_f64(record.get(3).unwrap_or(""))?;
        let close: f64 = parse_f64(record.get(4).unwrap_or(""))?;
        let volume: f64 = parse_f64(record.get(5).unwrap_or(""))?;
        let trades = parse_i64(record.get(6).unwrap_or(""))?;

        rows.push(OhlcvtRow {
            ts,
            open,
            high,
            low,
            close,
            volume,
            trades,
        });
    }

    if rows.is_empty() {
        return Ok(rows);
    }
    Ok(rows)
}

fn parse_f64(s: &str) -> Result<f64> {
    s.trim()
        .parse()
        .map_err(|_| Error::Parse(format!("invalid float: {s}")))
}

fn parse_i64(s: &str) -> Result<i64> {
    let t = s.trim();
    if let Ok(v) = t.parse::<i64>() {
        return Ok(v);
    }
    // Some dumps emit trades as float
    t.parse::<f64>()
        .map(|v| v as i64)
        .map_err(|_| Error::Parse(format!("invalid int: {s}")))
}

fn parse_ts(s: &str) -> Result<i64> {
    let t = s.trim();
    if let Ok(v) = t.parse::<i64>() {
        return Ok(v);
    }
    t.parse::<f64>()
        .map(|v| v as i64)
        .map_err(|_| Error::Parse(format!("invalid timestamp: {s}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_ohlcvt_rows() {
        let csv = b"1387267220,123.0,124.0,122.0,123.5,10.5,42\n1387267280,123.5,125.0,123.0,124.0,5.0,10\n";
        let rows = parse_ohlcvt_csv(csv).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].ts, 1_387_267_220);
        assert!((rows[0].open - 123.0).abs() < 1e-9);
        assert_eq!(rows[0].trades, 42);
    }

    #[test]
    fn skip_header() {
        let csv = b"timestamp,open,high,low,close,volume,trades\n1387267220,1,2,0.5,1.5,3,1\n";
        let rows = parse_ohlcvt_csv(csv).unwrap();
        assert_eq!(rows.len(), 1);
    }
}
