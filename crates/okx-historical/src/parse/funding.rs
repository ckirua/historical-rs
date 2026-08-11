//! Funding rate CSV parser.

use crate::error::{Error, Result};
use crate::model::FundingRow;

pub fn parse_funding_csv(csv_bytes: &[u8]) -> Result<Vec<FundingRow>> {
    let text = std::str::from_utf8(csv_bytes)
        .map_err(|e| Error::Parse(format!("funding CSV is not UTF-8: {e}")))?;
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .flexible(true)
        .from_reader(text.as_bytes());

    let headers = reader
        .headers()
        .map(|h| {
            h.iter()
                .map(|s| s.trim().to_ascii_lowercase())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    let inst_idx = find_col(
        &headers,
        &["instrument_name", "instrument", "instid", "contract_type"],
    );
    let rate_idx = find_col(
        &headers,
        &["funding_rate", "fundingrate", "real_funding_rate"],
    );
    let time_idx = find_col(&headers, &["funding_time", "fundingtime", "time"]);

    let mut rows = Vec::new();
    for record in reader.records() {
        let record = record?;
        if record.iter().all(|f| f.trim().is_empty()) {
            continue;
        }
        let (inst, rate, time) = if let (Some(i), Some(r), Some(t)) = (inst_idx, rate_idx, time_idx)
        {
            (
                record.get(i).unwrap_or("").to_string(),
                record.get(r).unwrap_or("0"),
                record.get(t).unwrap_or("0"),
            )
        } else if record.len() >= 3 {
            (
                record[0].to_string(),
                record[1].as_ref(),
                record[2].as_ref(),
            )
        } else {
            return Err(Error::Parse(format!(
                "funding expected >=3 columns, got {}",
                record.len()
            )));
        };
        rows.push(FundingRow {
            instrument: inst,
            funding_rate: rate.parse().map_err(parse_err)?,
            funding_time_ms: time.parse().map_err(parse_err)?,
        });
    }
    Ok(rows)
}

fn find_col(headers: &[String], names: &[&str]) -> Option<usize> {
    headers.iter().position(|h| names.iter().any(|n| h == n))
}

fn parse_err<E: std::fmt::Display>(e: E) -> Error {
    Error::Parse(e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_allswap_funding() {
        let csv =
            b"instrument_name,funding_rate,funding_time\nBTC-USDT-SWAP,0.0001,1704038410000\n";
        let rows = parse_funding_csv(csv).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].funding_rate, 0.0001);
    }
}
