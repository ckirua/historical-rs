//! Individual trades CSV parser.

use crate::error::{Error, Result};
use crate::model::{Market, TradeRow};

pub fn parse_trades_csv(csv_bytes: &[u8], market: Market) -> Result<Vec<TradeRow>> {
    let text = std::str::from_utf8(csv_bytes)
        .map_err(|e| Error::Parse(format!("trades CSV is not UTF-8: {e}")))?;
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .from_reader(text.as_bytes());

    let mut rows = Vec::new();
    for record in reader.records() {
        let record = record?;
        if record.iter().all(|f| f.trim().is_empty()) {
            continue;
        }
        if is_header_row(&record) {
            continue;
        }
        rows.push(parse_row(&record, market)?);
    }
    Ok(rows)
}

fn is_header_row(record: &csv::StringRecord) -> bool {
    let Some(first) = record.get(0) else {
        return true;
    };
    first.eq_ignore_ascii_case("id")
        || first.eq_ignore_ascii_case("trade_id")
        || first.eq_ignore_ascii_case("agg_trade_id")
        || first.parse::<u64>().is_err()
}

fn parse_row(record: &csv::StringRecord, market: Market) -> Result<TradeRow> {
    let cols: Vec<&str> = record.iter().collect();
    match market {
        Market::Spot => {
            if cols.len() < 7 {
                return Err(Error::Parse(format!(
                    "spot trades expected 7 columns, got {}",
                    cols.len()
                )));
            }
            Ok(TradeRow {
                trade_id: cols[0].parse().map_err(parse_err_int)?,
                price: cols[1].parse().map_err(parse_err_float)?,
                qty: cols[2].parse().map_err(parse_err_float)?,
                quote_qty: Some(cols[3].parse().map_err(parse_err_float)?),
                base_qty: None,
                timestamp_ms: cols[4].parse().map_err(parse_err_int)?,
                is_buyer_maker: parse_bool(cols[5])?,
                is_best_match: Some(parse_bool(cols[6])?),
            })
        }
        Market::Um => {
            if cols.len() < 6 {
                return Err(Error::Parse(format!(
                    "um trades expected 6 columns, got {}",
                    cols.len()
                )));
            }
            Ok(TradeRow {
                trade_id: cols[0].parse().map_err(parse_err_int)?,
                price: cols[1].parse().map_err(parse_err_float)?,
                qty: cols[2].parse().map_err(parse_err_float)?,
                quote_qty: Some(cols[3].parse().map_err(parse_err_float)?),
                base_qty: None,
                timestamp_ms: cols[4].parse().map_err(parse_err_int)?,
                is_buyer_maker: parse_bool(cols[5])?,
                is_best_match: None,
            })
        }
        Market::Cm => {
            if cols.len() < 6 {
                return Err(Error::Parse(format!(
                    "cm trades expected 6 columns, got {}",
                    cols.len()
                )));
            }
            Ok(TradeRow {
                trade_id: cols[0].parse().map_err(parse_err_int)?,
                price: cols[1].parse().map_err(parse_err_float)?,
                qty: cols[2].parse().map_err(parse_err_float)?,
                quote_qty: None,
                base_qty: Some(cols[3].parse().map_err(parse_err_float)?),
                timestamp_ms: cols[4].parse().map_err(parse_err_int)?,
                is_buyer_maker: parse_bool(cols[5])?,
                is_best_match: None,
            })
        }
        Market::Option => Err(Error::Parse("trades not supported for options".into())),
    }
}

fn parse_bool(raw: &str) -> Result<bool> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "true" | "1" => Ok(true),
        "false" | "0" => Ok(false),
        other => Err(Error::Parse(format!("invalid bool '{other}'"))),
    }
}

fn parse_err_int(e: std::num::ParseIntError) -> Error {
    Error::Parse(format!("invalid integer: {e}"))
}

fn parse_err_float(e: std::num::ParseFloatError) -> Error {
    Error::Parse(format!("invalid float: {e}"))
}
