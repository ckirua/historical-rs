//! Candlesticks CSV parser.

use crate::error::{Error, Result};
use crate::model::CandleRow;

pub fn parse_candles_csv(csv_bytes: &[u8]) -> Result<Vec<CandleRow>> {
    let text = std::str::from_utf8(csv_bytes)
        .map_err(|e| Error::Parse(format!("candles CSV is not UTF-8: {e}")))?;
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
        if record.len() < 10 {
            return Err(Error::Parse(format!(
                "candles expected >=10 columns, got {}",
                record.len()
            )));
        }
        rows.push(CandleRow {
            instrument: record[0].to_string(),
            open: record[1].parse().map_err(parse_err)?,
            high: record[2].parse().map_err(parse_err)?,
            low: record[3].parse().map_err(parse_err)?,
            close: record[4].parse().map_err(parse_err)?,
            vol: record[5].parse().map_err(parse_err)?,
            vol_ccy: record[6].parse().map_err(parse_err)?,
            vol_quote: record[7].parse().map_err(parse_err)?,
            open_time_ms: record[8].parse().map_err(parse_err)?,
            confirm: record[9].parse().unwrap_or(0),
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
    fn parses_sample_candles() {
        let csv = b"instrument_name,open,high,low,close,vol,vol_ccy,vol_quote,open_time,confirm\nBTC-USDT,1,2,0.5,1.5,10,100,100,1704038400000,0\n";
        let rows = parse_candles_csv(csv).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].open, 1.0);
        assert_eq!(rows[0].vol_quote, 100.0);
    }
}
