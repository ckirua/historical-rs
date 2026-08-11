//! Headerless trades CSV parser with column-order detection.

use crate::error::{Error, Result};
use crate::model::TradeRow;

/// Column order for Kraken trades CSV (no header).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TradeLayout {
    /// `[timestamp, price, volume]`
    TsPriceVol,
    /// `[price, volume, timestamp]` (legacy Kraken dump layout)
    PriceVolTs,
}

/// Detect layout from the first data row.
///
/// Heuristic: values `>= 1e9` are treated as unix timestamps (seconds or ms).
pub fn detect_trade_layout(a: f64, _b: f64, c: f64) -> TradeLayout {
    const TS_FLOOR: f64 = 1_000_000_000.0; // ~2001-09-09 in seconds
    let a_ts = a >= TS_FLOOR;
    let c_ts = c >= TS_FLOOR;
    match (a_ts, c_ts) {
        (true, false) => TradeLayout::TsPriceVol,
        (false, true) => TradeLayout::PriceVolTs,
        // Both or neither look like timestamps — prefer documented legacy layout.
        _ => TradeLayout::PriceVolTs,
    }
}

pub fn parse_trades_csv(csv_bytes: &[u8]) -> Result<Vec<TradeRow>> {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .from_reader(csv_bytes);

    let mut rows = Vec::new();
    let mut layout: Option<TradeLayout> = None;

    for (idx, record) in reader.records().enumerate() {
        let record = record?;
        if record.len() < 3 {
            continue;
        }
        // Skip accidental header row
        if idx == 0 {
            let first = record.get(0).unwrap_or("").to_ascii_lowercase();
            if first.contains("time") || first.contains("price") || first == "ts" {
                continue;
            }
        }

        let a: f64 = parse_f64(record.get(0).unwrap_or(""))?;
        let b: f64 = parse_f64(record.get(1).unwrap_or(""))?;
        let c: f64 = parse_f64(record.get(2).unwrap_or(""))?;

        let layout = *layout.get_or_insert_with(|| detect_trade_layout(a, b, c));
        let row = match layout {
            TradeLayout::TsPriceVol => TradeRow {
                ts: a,
                price: b,
                volume: c,
            },
            TradeLayout::PriceVolTs => TradeRow {
                ts: c,
                price: a,
                volume: b,
            },
        };
        rows.push(row);
    }

    Ok(rows)
}

fn parse_f64(s: &str) -> Result<f64> {
    s.trim()
        .parse()
        .map_err(|_| Error::Parse(format!("invalid float: {s}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_price_vol_ts() {
        assert_eq!(
            detect_trade_layout(0.00002, 0.02, 1_438_909_837.150_6),
            TradeLayout::PriceVolTs
        );
    }

    #[test]
    fn detect_ts_price_vol() {
        assert_eq!(
            detect_trade_layout(1_438_909_837.15, 42000.0, 0.5),
            TradeLayout::TsPriceVol
        );
    }

    #[test]
    fn parse_price_vol_ts_csv() {
        let csv = b"0.00002000,0.02000000,1438909837.1506\n42000.5,0.1,1700000000.0\n";
        let rows = parse_trades_csv(csv).unwrap();
        assert_eq!(rows.len(), 2);
        assert!((rows[0].price - 0.00002).abs() < 1e-12);
        assert!((rows[0].ts - 1_438_909_837.150_6).abs() < 1e-6);
    }

    #[test]
    fn parse_ts_price_vol_csv() {
        let csv = b"1438909837.1506,0.00002000,0.02000000\n";
        let rows = parse_trades_csv(csv).unwrap();
        assert_eq!(rows.len(), 1);
        assert!((rows[0].ts - 1_438_909_837.150_6).abs() < 1e-6);
        assert!((rows[0].volume - 0.02).abs() < 1e-12);
    }
}
