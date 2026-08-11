//! Aggregated trades CSV parser.

use crate::error::{Error, Result};
use crate::model::{AggTradeRow, Market};

pub fn parse_agg_trades_csv(csv_bytes: &[u8], market: Market) -> Result<Vec<AggTradeRow>> {
    let text = std::str::from_utf8(csv_bytes)
        .map_err(|e| Error::Parse(format!("aggTrades CSV is not UTF-8: {e}")))?;
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
    first.eq_ignore_ascii_case("agg_trade_id")
        || first.eq_ignore_ascii_case("aggregatetradeid")
        || first.parse::<u64>().is_err()
}

fn parse_row(record: &csv::StringRecord, market: Market) -> Result<AggTradeRow> {
    let cols: Vec<&str> = record.iter().collect();
    match market {
        Market::Spot => {
            if cols.len() < 8 {
                return Err(Error::Parse(format!(
                    "spot aggTrades expected 8 columns, got {}",
                    cols.len()
                )));
            }
            Ok(AggTradeRow {
                agg_trade_id: cols[0].parse().map_err(parse_err_int)?,
                price: cols[1].parse().map_err(parse_err_float)?,
                qty: cols[2].parse().map_err(parse_err_float)?,
                first_trade_id: cols[3].parse().map_err(parse_err_int)?,
                last_trade_id: cols[4].parse().map_err(parse_err_int)?,
                timestamp_ms: parse_timestamp(cols[5])?,
                is_buyer_maker: parse_bool(cols[6])?,
            })
        }
        Market::Um | Market::Cm => {
            if cols.len() < 7 {
                return Err(Error::Parse(format!(
                    "futures aggTrades expected 7 columns, got {}",
                    cols.len()
                )));
            }
            Ok(AggTradeRow {
                agg_trade_id: cols[0].parse().map_err(parse_err_int)?,
                price: cols[1].parse().map_err(parse_err_float)?,
                qty: cols[2].parse().map_err(parse_err_float)?,
                first_trade_id: cols[3].parse().map_err(parse_err_int)?,
                last_trade_id: cols[4].parse().map_err(parse_err_int)?,
                timestamp_ms: parse_timestamp(cols[5])?,
                is_buyer_maker: parse_bool(cols[6])?,
            })
        }
        Market::Option => Err(Error::Parse("aggTrades not supported for options".into())),
    }
}

fn parse_timestamp(raw: &str) -> Result<i64> {
    raw.trim()
        .parse()
        .map_err(|e| Error::Parse(format!("invalid timestamp '{raw}': {e}")))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skips_header_row() {
        let csv = b"agg_trade_id,price,quantity,first_trade_id,last_trade_id,transact_time,is_buyer_maker\n26129,0.01633102,4.70443515,27781,27781,1498793709153,true\n";
        let rows = parse_agg_trades_csv(csv, Market::Um).unwrap();
        assert_eq!(rows.len(), 1);
    }

    #[test]
    fn parses_um_row_without_header() {
        let csv = b"26129,0.01633102,4.70443515,27781,27781,1498793709153,true\n";
        let rows = parse_agg_trades_csv(csv, Market::Um).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].agg_trade_id, 26129);
        assert!(rows[0].is_buyer_maker);
    }

    #[test]
    fn parses_spot_row_with_best_match_column() {
        let csv = b"0,0.20000000,50.00000000,0,0,1735689600010866,False,True\n";
        let rows = parse_agg_trades_csv(csv, Market::Spot).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].timestamp_ms, 1735689600010866);
        assert!(!rows[0].is_buyer_maker);
    }
}
