//! Trades CSV parser.

use crate::error::{Error, Result};
use crate::model::TradeRow;

pub fn parse_trades_csv(csv_bytes: &[u8]) -> Result<Vec<TradeRow>> {
    let text = std::str::from_utf8(csv_bytes)
        .map_err(|e| Error::Parse(format!("trades CSV is not UTF-8: {e}")))?;
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .flexible(true)
        .from_reader(text.as_bytes());

    let mut rows = Vec::new();
    for record in reader.records() {
        let record = record?;
        if record.iter().all(|f| f.trim().is_empty()) {
            continue;
        }
        if record.len() < 6 {
            return Err(Error::Parse(format!(
                "trades expected >=6 columns, got {}",
                record.len()
            )));
        }
        rows.push(TradeRow {
            instrument: record[0].to_string(),
            trade_id: record[1].parse().map_err(parse_err)?,
            side: record[2].to_string(),
            price: record[3].parse().map_err(parse_err)?,
            size: record[4].parse().map_err(parse_err)?,
            timestamp_ms: record[5].parse().map_err(parse_err)?,
        });
    }
    Ok(rows)
}

fn parse_err<E: std::fmt::Display>(e: E) -> Error {
    Error::Parse(e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_sample_trades() {
        let csv = b"instrument_name,trade_id,side,price,size,created_time\nBTC-USDT,1,buy,100.0,0.5,1704038400000\n";
        let rows = parse_trades_csv(csv).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].trade_id, 1);
        assert_eq!(rows[0].side, "buy");
        assert_eq!(rows[0].price, 100.0);
    }
}
