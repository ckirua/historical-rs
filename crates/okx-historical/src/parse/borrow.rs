//! Borrow rate CSV parser.

use crate::error::{Error, Result};
use crate::model::BorrowRow;

pub fn parse_borrow_csv(csv_bytes: &[u8]) -> Result<Vec<BorrowRow>> {
    let text = std::str::from_utf8(csv_bytes)
        .map_err(|e| Error::Parse(format!("borrow CSV is not UTF-8: {e}")))?;
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
        if record.len() < 3 {
            return Err(Error::Parse(format!(
                "borrow expected >=3 columns, got {}",
                record.len()
            )));
        }
        rows.push(BorrowRow {
            currency: record[0].to_string(),
            borrow_rate: record[1].parse().map_err(parse_err)?,
            time_ms: record[2].parse().map_err(parse_err)?,
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
    fn parses_borrow_rates() {
        let csv = b"currency_name,borrow_rate,time\nBTC,0.01,1704038400000\n";
        let rows = parse_borrow_csv(csv).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].currency, "BTC");
    }
}
