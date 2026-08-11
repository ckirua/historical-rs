//! Klines CSV parser.

use crate::error::{Error, Result};
use crate::model::{KlineRow, Market};

pub fn parse_klines_csv(csv_bytes: &[u8], market: Market) -> Result<Vec<KlineRow>> {
    let text = std::str::from_utf8(csv_bytes)
        .map_err(|e| Error::Parse(format!("klines CSV is not UTF-8: {e}")))?;
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
    first.eq_ignore_ascii_case("open_time")
        || first.eq_ignore_ascii_case("opentime")
        || first.parse::<i64>().is_err()
}

fn parse_row(record: &csv::StringRecord, market: Market) -> Result<KlineRow> {
    let cols: Vec<&str> = record.iter().collect();
    if cols.len() < 11 {
        return Err(Error::Parse(format!(
            "klines expected at least 11 columns, got {}",
            cols.len()
        )));
    }

    match market {
        Market::Spot | Market::Um => Ok(KlineRow {
            open_time: cols[0].parse().map_err(parse_err_int)?,
            open: cols[1].parse().map_err(parse_err_float)?,
            high: cols[2].parse().map_err(parse_err_float)?,
            low: cols[3].parse().map_err(parse_err_float)?,
            close: cols[4].parse().map_err(parse_err_float)?,
            volume: cols[5].parse().map_err(parse_err_float)?,
            close_time: cols[6].parse().map_err(parse_err_int)?,
            quote_volume: Some(cols[7].parse().map_err(parse_err_float)?),
            base_volume: None,
            num_trades: cols[8].parse().map_err(parse_err_int)?,
            taker_buy_volume: Some(cols[9].parse().map_err(parse_err_float)?),
            taker_buy_quote_volume: Some(
                cols.get(10)
                    .ok_or_else(|| Error::Parse("missing taker_buy_quote_volume".into()))?
                    .parse()
                    .map_err(parse_err_float)?,
            ),
            taker_buy_base_volume: None,
            ignore: cols.get(11).and_then(|v| v.parse().ok()),
        }),
        Market::Cm => Ok(KlineRow {
            open_time: cols[0].parse().map_err(parse_err_int)?,
            open: cols[1].parse().map_err(parse_err_float)?,
            high: cols[2].parse().map_err(parse_err_float)?,
            low: cols[3].parse().map_err(parse_err_float)?,
            close: cols[4].parse().map_err(parse_err_float)?,
            volume: cols[5].parse().map_err(parse_err_float)?,
            close_time: cols[6].parse().map_err(parse_err_int)?,
            quote_volume: None,
            base_volume: Some(cols[7].parse().map_err(parse_err_float)?),
            num_trades: cols[8].parse().map_err(parse_err_int)?,
            taker_buy_volume: Some(cols[9].parse().map_err(parse_err_float)?),
            taker_buy_quote_volume: None,
            taker_buy_base_volume: Some(cols[10].parse().map_err(parse_err_float)?),
            ignore: cols.get(11).and_then(|v| v.parse().ok()),
        }),
        Market::Option => Err(Error::Parse("klines not supported for options".into())),
    }
}

fn parse_err_int(e: std::num::ParseIntError) -> Error {
    Error::Parse(format!("invalid integer: {e}"))
}

fn parse_err_float(e: std::num::ParseFloatError) -> Error {
    Error::Parse(format!("invalid float: {e}"))
}
