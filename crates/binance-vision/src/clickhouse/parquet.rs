//! Shared parquet → agg_trades row parsing.

use bytes::Bytes;
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;

use arrow::array::{
    Array, BooleanArray, Float64Array, Int32Array, Int64Array, LargeStringArray, StringArray,
    UInt64Array, UInt8Array,
};
use arrow::compute::cast;
use arrow::datatypes::DataType;

use super::row::AggTradeInsertRow;
use crate::error::{Error, Result};

/// Parse aggTrades parquet. When `market`/`symbol` are omitted, they must exist as columns.
pub fn parse_agg_trades_parquet(bytes: &[u8]) -> Result<Vec<AggTradeInsertRow>> {
    parse_agg_trades_parquet_with_context(bytes, None, None)
}

/// Parse aggTrades parquet, inferring `market`/`symbol` when columns are absent.
pub fn parse_agg_trades_parquet_with_context(
    bytes: &[u8],
    market: Option<&str>,
    symbol: Option<&str>,
) -> Result<Vec<AggTradeInsertRow>> {
    let reader = ParquetRecordBatchReaderBuilder::try_new(Bytes::copy_from_slice(bytes))?
        .build()
        .map_err(Error::Parquet)?;
    let mut out = Vec::new();

    for batch in reader {
        let batch = batch.map_err(Error::Arrow)?;
        let schema = batch.schema();
        let col = |name: &str| schema.fields().iter().position(|f| f.name() == name);
        let col_any = |names: &[&str]| names.iter().find_map(|name| col(name));

        let market_idx = col("market");
        let symbol_idx = col("symbol");
        let agg_idx = col_any(&["agg_trade_id"])
            .ok_or_else(|| Error::Parse("missing column agg_trade_id".into()))?;
        let price_idx =
            col_any(&["price"]).ok_or_else(|| Error::Parse("missing column price".into()))?;
        let qty_idx = col_any(&["qty", "quantity"])
            .ok_or_else(|| Error::Parse("missing column qty".into()))?;
        let first_idx = col_any(&["first_trade_id"])
            .ok_or_else(|| Error::Parse("missing column first_trade_id".into()))?;
        let last_idx = col_any(&["last_trade_id"])
            .ok_or_else(|| Error::Parse("missing column last_trade_id".into()))?;
        let ts_idx = col_any(&["timestamp_ms", "timestamp", "transact_time"])
            .ok_or_else(|| Error::Parse("missing column timestamp_ms".into()))?;
        let maker_idx = col_any(&["is_buyer_maker"])
            .ok_or_else(|| Error::Parse("missing column is_buyer_maker".into()))?;

        let market_default = if market_idx.is_none() {
            Some(
                market
                    .ok_or_else(|| Error::Parse("missing column market".into()))?
                    .to_string(),
            )
        } else {
            None
        };
        let symbol_default = if symbol_idx.is_none() {
            Some(
                symbol
                    .ok_or_else(|| Error::Parse("missing column symbol".into()))?
                    .to_string(),
            )
        } else {
            None
        };

        let agg_arr = batch.column(agg_idx);
        let price_arr = batch.column(price_idx);
        let qty_arr = batch.column(qty_idx);
        let first_arr = batch.column(first_idx);
        let last_arr = batch.column(last_idx);
        let ts_arr = batch.column(ts_idx);
        let maker_arr = batch.column(maker_idx);

        for row in 0..batch.num_rows() {
            let market = match market_idx {
                Some(idx) => read_string(batch.column(idx), row)?,
                None => market_default
                    .as_deref()
                    .ok_or_else(|| Error::Parse("missing market context".into()))?
                    .to_string(),
            };
            let symbol = match symbol_idx {
                Some(idx) => read_string(batch.column(idx), row)?,
                None => symbol_default
                    .as_deref()
                    .ok_or_else(|| Error::Parse("missing symbol context".into()))?
                    .to_string(),
            };
            out.push(AggTradeInsertRow {
                market,
                symbol,
                agg_trade_id: read_u64(agg_arr, row)?,
                price: read_f64(price_arr, row)?,
                qty: read_f64(qty_arr, row)?,
                first_trade_id: read_u64(first_arr, row)?,
                last_trade_id: read_u64(last_arr, row)?,
                timestamp_ms: read_i64(ts_arr, row)?,
                is_buyer_maker: read_bool(maker_arr, row)?,
            });
        }
    }
    Ok(out)
}

fn read_string(column: &dyn Array, row: usize) -> Result<String> {
    if matches!(column.data_type(), DataType::Dictionary(_, _)) {
        let casted = cast(column, &DataType::Utf8).map_err(|e| Error::Parse(e.to_string()))?;
        return read_string(casted.as_ref(), row);
    }
    if let Some(arr) = column.as_any().downcast_ref::<StringArray>() {
        return Ok(arr.value(row).to_string());
    }
    if let Some(arr) = column.as_any().downcast_ref::<LargeStringArray>() {
        return Ok(arr.value(row).to_string());
    }
    Err(Error::Parse(format!(
        "unsupported string column type {}",
        column.data_type()
    )))
}

fn read_u64(column: &dyn Array, row: usize) -> Result<u64> {
    if let Some(arr) = column.as_any().downcast_ref::<UInt64Array>() {
        return Ok(arr.value(row));
    }
    if let Some(arr) = column.as_any().downcast_ref::<Int64Array>() {
        return Ok(arr.value(row) as u64);
    }
    if let Some(arr) = column.as_any().downcast_ref::<Int32Array>() {
        return Ok(arr.value(row) as u64);
    }
    Err(Error::Parse(format!(
        "unsupported integer column type {}",
        column.data_type()
    )))
}

fn read_i64(column: &dyn Array, row: usize) -> Result<i64> {
    if let Some(arr) = column.as_any().downcast_ref::<Int64Array>() {
        return Ok(arr.value(row));
    }
    if let Some(arr) = column.as_any().downcast_ref::<UInt64Array>() {
        return Ok(arr.value(row) as i64);
    }
    if let Some(arr) = column.as_any().downcast_ref::<Int32Array>() {
        return Ok(arr.value(row) as i64);
    }
    Err(Error::Parse(format!(
        "unsupported timestamp column type {}",
        column.data_type()
    )))
}

fn read_f64(column: &dyn Array, row: usize) -> Result<f64> {
    if let Some(arr) = column.as_any().downcast_ref::<Float64Array>() {
        return Ok(arr.value(row));
    }
    if let Some(arr) = column.as_any().downcast_ref::<Int64Array>() {
        return Ok(arr.value(row) as f64);
    }
    Err(Error::Parse(format!(
        "unsupported float column type {}",
        column.data_type()
    )))
}

fn read_bool(column: &dyn Array, row: usize) -> Result<u8> {
    if let Some(arr) = column.as_any().downcast_ref::<BooleanArray>() {
        return Ok(u8::from(arr.value(row)));
    }
    if let Some(arr) = column.as_any().downcast_ref::<UInt8Array>() {
        return Ok(arr.value(row));
    }
    if let Some(arr) = column.as_any().downcast_ref::<Int64Array>() {
        return Ok((arr.value(row) != 0) as u8);
    }
    Err(Error::Parse(format!(
        "unsupported boolean column type {}",
        column.data_type()
    )))
}

/// Infer `(market, symbol)` from a Vision-compatible S3 key.
pub fn market_symbol_from_s3_key(key: &str) -> Option<(String, String)> {
    // data/futures/um/daily/aggTrades/BTCUSDT/BTCUSDT-aggTrades-2024-01-15.parquet.zstd
    let parts: Vec<&str> = key.split('/').collect();
    if parts.len() < 6 || parts[0] != "data" {
        return None;
    }
    let market = match parts.get(1)? {
        &"spot" => "spot",
        &"futures" if parts.get(2)? == &"um" => "um",
        &"futures" if parts.get(2)? == &"cm" => "cm",
        &"option" => "option",
        other => other,
    };
    let symbol_idx = if parts[1] == "futures" { 5 } else { 4 };
    let symbol = (*parts.get(symbol_idx)?).to_string();
    Some((market.to_string(), symbol))
}

/// Parse the file period from a Vision-compatible S3 object key.
pub fn period_from_s3_key(key: &str) -> Option<chrono::NaiveDate> {
    let filename = key.rsplit('/').next()?;
    let stem = filename
        .strip_suffix(".parquet.zstd")
        .or_else(|| filename.strip_suffix(".parquet"))?;
    parse_period_from_stem(stem)
}

fn parse_period_from_stem(stem: &str) -> Option<chrono::NaiveDate> {
    use chrono::NaiveDate;
    let parts: Vec<&str> = stem.split('-').collect();
    if parts.len() >= 3 {
        if let (Ok(y), Ok(m), Ok(d)) = (
            parts[parts.len() - 3].parse::<i32>(),
            parts[parts.len() - 2].parse::<u32>(),
            parts[parts.len() - 1].parse::<u32>(),
        ) {
            if let Some(date) = NaiveDate::from_ymd_opt(y, m, d) {
                return Some(date);
            }
        }
    }
    if parts.len() >= 2 {
        if let (Ok(y), Ok(m)) = (
            parts[parts.len() - 2].parse::<i32>(),
            parts[parts.len() - 1].parse::<u32>(),
        ) {
            return NaiveDate::from_ymd_opt(y, m, 1);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;

    use super::*;

    #[test]
    fn infers_market_symbol_from_s3_key() {
        let (market, symbol) = market_symbol_from_s3_key(
            "data/futures/um/daily/aggTrades/BTCUSDT/BTCUSDT-aggTrades-2024-01-15.parquet.zstd",
        )
        .unwrap();
        assert_eq!(market, "um");
        assert_eq!(symbol, "BTCUSDT");
    }

    #[test]
    fn parses_daily_period_from_s3_key() {
        let date = period_from_s3_key(
            "data/futures/um/daily/aggTrades/BTCUSDT/BTCUSDT-aggTrades-2024-01-15.parquet.zstd",
        )
        .unwrap();
        assert_eq!(date, NaiveDate::from_ymd_opt(2024, 1, 15).unwrap());
    }
}
