use std::fs::File;
use std::io::{BufRead, BufReader, Cursor};
use std::path::Path;

use flate2::read::GzDecoder;
use serde_json::Value;

use crate::error::{Error, Result};
use crate::model::{OrderBookRow, TradeRow};
pub fn parse_line(line: &str, fallback_symbol: &str) -> Result<Vec<OrderBookRow>> {
    let value: Value = serde_json::from_str(line)?;
    let object = value
        .as_object()
        .ok_or_else(|| Error::InvalidRow("message is not an object".into()))?;
    let topic = object
        .get("topic")
        .and_then(Value::as_str)
        .ok_or_else(|| Error::InvalidRow("missing topic".into()))?
        .to_string();
    let message_type = object
        .get("type")
        .and_then(Value::as_str)
        .ok_or_else(|| Error::InvalidRow("missing type".into()))?
        .to_string();
    if message_type != "snapshot" && message_type != "delta" {
        return Err(Error::InvalidRow(format!(
            "unsupported type {message_type}"
        )));
    }
    let ts_ms = number_i64(object.get("ts"), "ts")?;
    let data = object
        .get("data")
        .and_then(Value::as_object)
        .ok_or_else(|| Error::InvalidRow("missing data".into()))?;
    let symbol = data
        .get("s")
        .and_then(Value::as_str)
        .unwrap_or(fallback_symbol)
        .to_string();
    let cts_ms = optional_i64(data.get("cts"))?;
    let update_id = optional_u64(data.get("u"))?;
    let cross_seq = optional_u64(data.get("seq"))?;
    let mut rows = Vec::new();
    for (side, key) in [("bid", "b"), ("ask", "a")] {
        let levels = data
            .get(key)
            .and_then(Value::as_array)
            .ok_or_else(|| Error::InvalidRow(format!("missing data.{key} level array")))?;
        for level in levels {
            let pair = level
                .as_array()
                .ok_or_else(|| Error::InvalidRow(format!("data.{key} level is not an array")))?;
            if pair.len() < 2 {
                return Err(Error::InvalidRow(format!(
                    "data.{key} level has fewer than two values"
                )));
            }
            rows.push(OrderBookRow {
                market: "spot".into(),
                symbol: symbol.clone(),
                topic: topic.clone(),
                message_type: message_type.clone(),
                ts_ms,
                cts_ms,
                update_id,
                cross_seq,
                side: side.into(),
                price: number_f64(&pair[0], "price")?,
                size: number_f64(&pair[1], "size")?,
            });
        }
    }
    Ok(rows)
}

pub fn parse_jsonl<R: BufRead>(reader: R, fallback_symbol: &str) -> Result<Vec<OrderBookRow>> {
    let mut rows = Vec::new();
    for (line_number, line) in reader.lines().enumerate() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        parse_line(&line, fallback_symbol)
            .map(|mut parsed| rows.append(&mut parsed))
            .map_err(|err| Error::InvalidRow(format!("line {}: {err}", line_number + 1)))?;
    }
    Ok(rows)
}

pub fn parse_zip_file(path: &Path, fallback_symbol: &str) -> Result<Vec<OrderBookRow>> {
    let file = File::open(path)?;
    let mut archive = zip::ZipArchive::new(file)?;
    let index = (0..archive.len())
        .find(|index| {
            archive
                .by_index(*index)
                .map(|member| !member.is_dir())
                .unwrap_or(false)
        })
        .ok_or_else(|| Error::InvalidRow("ZIP contains no JSONL member".into()))?;
    let member = archive.by_index(index)?;
    let reader = BufReader::new(member);
    parse_jsonl(reader, fallback_symbol)
}

pub fn parse_zip_file_batched<F>(
    path: &Path,
    fallback_symbol: &str,
    mut on_batch: F,
) -> Result<usize>
where
    F: FnMut(&[OrderBookRow]) -> Result<()>,
{
    let file = File::open(path)?;
    let mut archive = zip::ZipArchive::new(file)?;
    let index = (0..archive.len())
        .find(|index| {
            archive
                .by_index(*index)
                .map(|member| !member.is_dir())
                .unwrap_or(false)
        })
        .ok_or_else(|| Error::InvalidRow("ZIP contains no JSONL member".into()))?;
    let member = archive.by_index(index)?;
    let reader = BufReader::new(member);
    let mut batch = Vec::with_capacity(1024);
    let mut count = 0;
    for (line_number, line) in reader.lines().enumerate() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let mut rows = parse_line(&line, fallback_symbol)
            .map_err(|err| Error::InvalidRow(format!("line {}: {err}", line_number + 1)))?;
        count += rows.len();
        batch.append(&mut rows);
        if batch.len() >= 1024 {
            on_batch(&batch)?;
            batch.clear();
        }
    }
    if !batch.is_empty() {
        on_batch(&batch)?;
    }
    Ok(count)
}

/// Parse a gzip-compressed Spot trade archive from disk.
pub fn parse_trade_gzip_file(path: &Path, fallback_symbol: &str) -> Result<Vec<TradeRow>> {
    let file = File::open(path)?;
    parse_trade_gzip_reader(file, fallback_symbol)
}

/// Parse a gzip-compressed Spot trade archive held in memory.
pub fn parse_trade_gzip_bytes(bytes: &[u8], fallback_symbol: &str) -> Result<Vec<TradeRow>> {
    parse_trade_gzip_reader(Cursor::new(bytes), fallback_symbol)
}

/// Parse a gzip-compressed Spot trade archive in batches.
pub fn parse_trade_gzip_file_batched<F>(
    path: &Path,
    fallback_symbol: &str,
    on_batch: F,
) -> Result<usize>
where
    F: FnMut(&[TradeRow]) -> Result<()>,
{
    let file = File::open(path)?;
    parse_trade_gzip_reader_batched(file, fallback_symbol, on_batch)
}

fn parse_trade_gzip_reader<R: std::io::Read>(
    reader: R,
    fallback_symbol: &str,
) -> Result<Vec<TradeRow>> {
    let mut rows = Vec::new();
    parse_trade_gzip_reader_batched(reader, fallback_symbol, |batch| {
        rows.extend_from_slice(batch);
        Ok(())
    })?;
    Ok(rows)
}

fn parse_trade_gzip_reader_batched<R: std::io::Read, F>(
    reader: R,
    fallback_symbol: &str,
    mut on_batch: F,
) -> Result<usize>
where
    F: FnMut(&[TradeRow]) -> Result<()>,
{
    let decoder = GzDecoder::new(reader);
    let mut csv_reader = csv::ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .trim(csv::Trim::All)
        .from_reader(decoder);
    let mut records = csv_reader.records();
    let first = records
        .next()
        .transpose()
        .map_err(|err| Error::InvalidRow(format!("CSV header: {err}")))?
        .ok_or_else(|| Error::InvalidRow("trade CSV is empty".into()))?;
    let header = detect_trade_header(&first);
    let layout = header
        .as_ref()
        .map(|headers| TradeLayout::from_headers(headers))
        .transpose()?;
    if let Some(layout) = layout.as_ref() {
        if layout.timestamp.is_none()
            || layout.side.is_none()
            || layout.price.is_none()
            || layout.size.is_none()
            || layout.trade_id.is_none()
        {
            return Err(Error::InvalidRow(
                "trade CSV header must include timestamp, side, price, size, and trade_id"
                    .into(),
            ));
        }
    }
    let mut batch = Vec::with_capacity(1024);
    let mut count = 0;
    let mut parse_record = |record: &csv::StringRecord, line_number: usize| -> Result<()> {
        let row = match layout.as_ref() {
            Some(layout) => parse_trade_record(record, layout, fallback_symbol, line_number)?,
            None => parse_trade_without_header(record, fallback_symbol, line_number)?,
        };
        count += 1;
        batch.push(row);
        if batch.len() >= 1024 {
            on_batch(&batch)?;
            batch.clear();
        }
        Ok(())
    };
    if layout.is_none() {
        parse_record(&first, 1)?;
    }
    for (index, record) in records.enumerate() {
        let record = record
            .map_err(|err| Error::InvalidRow(format!("line {}: {err}", index + 2)))?;
        if record.iter().all(|field| field.trim().is_empty()) {
            continue;
        }
        parse_record(&record, index + 2)?;
    }
    if !batch.is_empty() {
        on_batch(&batch)?;
    }
    Ok(count)
}

#[derive(Debug, Clone, Copy)]
struct TradeLayout {
    symbol: Option<usize>,
    timestamp: Option<usize>,
    side: Option<usize>,
    price: Option<usize>,
    size: Option<usize>,
    trade_id: Option<usize>,
    quote_qty: Option<usize>,
    is_buyer_maker: Option<usize>,
}

impl TradeLayout {
    fn from_headers(headers: &[String]) -> Result<Self> {
        let find = |names: &[&str]| {
            headers
                .iter()
                .position(|header| names.iter().any(|name| header == name))
        };
        Ok(Self {
            symbol: find(&["symbol", "s", "pair", "instrument"]),
            timestamp: find(&[
                "timestamp",
                "timestampms",
                "time",
                "transacttime",
                "executiontime",
            ]),
            side: find(&["side", "direction"]),
            price: find(&["price", "p"]),
            size: find(&["size", "qty", "quantity", "volume", "q"]),
            trade_id: find(&[
                "trdmatchid",
                "tradeid",
                "trade_id",
                "id",
                "aggtradeid",
                "agg_trade_id",
            ]),
            quote_qty: find(&[
                "quoteqty",
                "quote_qty",
                "quotequantity",
                "foreignnotional",
            ]),
            is_buyer_maker: find(&["isbuyermaker", "is_buyer_maker", "buyermaker"]),
        })
    }
}

fn detect_trade_header(record: &csv::StringRecord) -> Option<Vec<String>> {
    let headers = record
        .iter()
        .map(normalize_header)
        .collect::<Vec<_>>();
    let has_named_column = headers.iter().any(|header| {
        matches!(
            header.as_str(),
            "symbol"
                | "s"
                | "pair"
                | "instrument"
                | "timestamp"
                | "timestampms"
                | "time"
                | "transacttime"
                | "side"
                | "price"
                | "size"
                | "qty"
                | "quantity"
                | "volume"
                | "tradeid"
                | "trade_id"
                | "trdmatchid"
                | "id"
                | "foreignnotional"
        )
    });
    has_named_column.then_some(headers)
}

fn normalize_header(value: &str) -> String {
    value
        .trim()
        .trim_start_matches('\u{feff}')
        .to_ascii_lowercase()
        .replace([' ', '-', '.'], "")
}

fn parse_trade_record(
    record: &csv::StringRecord,
    layout: &TradeLayout,
    fallback_symbol: &str,
    line_number: usize,
) -> Result<TradeRow> {
    let required = |index: Option<usize>, field: &str| -> Result<&str> {
        let index = index.ok_or_else(|| Error::InvalidRow(format!("missing {field} column")))?;
        record
            .get(index)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| Error::InvalidRow(format!("line {line_number}: missing {field}")))
    };
    let optional = |index: Option<usize>| {
        index
            .and_then(|index| record.get(index))
            .filter(|value| !value.is_empty())
    };
    let symbol = optional(layout.symbol).unwrap_or(fallback_symbol).to_string();
    Ok(TradeRow {
        symbol,
        timestamp_ms: parse_timestamp_ms(
            required(layout.timestamp, "timestamp")?,
            line_number,
        )?,
        side: required(layout.side, "side")?.to_string(),
        price: parse_f64(required(layout.price, "price")?, "price", line_number)?,
        size: parse_f64(required(layout.size, "size")?, "size", line_number)?,
        trade_id: required(layout.trade_id, "trade_id")?.to_string(),
        quote_qty: optional(layout.quote_qty)
            .map(|value| parse_f64(value, "quote_qty", line_number))
            .transpose()?,
        is_buyer_maker: optional(layout.is_buyer_maker)
            .map(|value| parse_bool(value, line_number))
            .transpose()?,
    })
}

fn parse_trade_without_header(
    record: &csv::StringRecord,
    fallback_symbol: &str,
    line_number: usize,
) -> Result<TradeRow> {
    let values = record.iter().collect::<Vec<_>>();
    if values.len() == 5
        && values.get(4).is_some_and(|value| is_side(value))
        && values[0].parse::<u64>().is_ok()
        && values[1].parse::<i64>().is_ok()
    {
        return parse_positional_trade(
            fallback_symbol,
            values[1],
            values[4],
            values[2],
            values[3],
            values[0],
            line_number,
        );
    }
    if values.len() == 5 && values.get(1).is_some_and(|value| is_side(value)) {
        return parse_positional_trade(
            fallback_symbol,
            values[0],
            values[1],
            values[2],
            values[3],
            values[4],
            line_number,
        );
    }
    if values.len() == 6 && values.get(2).is_some_and(|value| is_side(value)) {
        return parse_positional_trade(
            values[0],
            values[1],
            values[2],
            values[3],
            values[4],
            values[5],
            line_number,
        );
    }
    Err(Error::InvalidRow(format!(
        "line {line_number}: unsupported headerless trade row; expected timestamp,side,price,size,trade_id"
    )))
}

fn parse_positional_trade(
    symbol: &str,
    timestamp: &str,
    side: &str,
    price: &str,
    size: &str,
    trade_id: &str,
    line_number: usize,
) -> Result<TradeRow> {
    Ok(TradeRow {
        symbol: if symbol.is_empty() {
            String::new()
        } else {
            symbol.to_string()
        },
        timestamp_ms: parse_timestamp_ms(timestamp, line_number)?,
        side: side.to_string(),
        price: parse_f64(price, "price", line_number)?,
        size: parse_f64(size, "size", line_number)?,
        trade_id: trade_id.to_string(),
        quote_qty: None,
        is_buyer_maker: None,
    })
}

fn is_side(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "buy" | "sell" | "bid" | "ask"
    )
}

fn parse_timestamp_ms(value: &str, line_number: usize) -> Result<i64> {
    let value = value.trim();
    if value.contains('.') {
        let seconds: f64 = value.parse().map_err(|_| {
            Error::InvalidRow(format!("line {line_number}: invalid timestamp"))
        })?;
        return Ok((seconds * 1000.0).round() as i64);
    }
    let number: i64 = value
        .parse()
        .map_err(|_| Error::InvalidRow(format!("line {line_number}: invalid timestamp")))?;
    // Contract archives may emit whole-second epochs; Spot uses millisecond ints.
    if number.abs() < 100_000_000_000 {
        Ok(number.saturating_mul(1000))
    } else {
        Ok(number)
    }
}

fn parse_f64(value: &str, field: &str, line_number: usize) -> Result<f64> {
    value
        .parse()
        .map_err(|_| Error::InvalidRow(format!("line {line_number}: invalid {field}")))
}

fn parse_bool(value: &str, line_number: usize) -> Result<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "true" | "1" | "yes" => Ok(true),
        "false" | "0" | "no" => Ok(false),
        _ => Err(Error::InvalidRow(format!(
            "line {line_number}: invalid is_buyer_maker"
        ))),
    }
}

fn number_f64(value: &Value, field: &str) -> Result<f64> {
    value
        .as_f64()
        .or_else(|| value.as_str().and_then(|text| text.parse().ok()))
        .ok_or_else(|| Error::InvalidRow(format!("invalid {field}")))
}

fn number_i64(value: Option<&Value>, field: &str) -> Result<i64> {
    let value = value.ok_or_else(|| Error::InvalidRow(format!("missing {field}")))?;
    value
        .as_i64()
        .or_else(|| value.as_u64().and_then(|number| i64::try_from(number).ok()))
        .or_else(|| value.as_str().and_then(|text| text.parse().ok()))
        .ok_or_else(|| Error::InvalidRow(format!("invalid {field}")))
}

fn optional_i64(value: Option<&Value>) -> Result<Option<i64>> {
    value
        .map(|value| number_i64(Some(value), "cts"))
        .transpose()
}

fn optional_u64(value: Option<&Value>) -> Result<Option<u64>> {
    // Null, missing, and unparseable sequence fields are nullable — do not fail the row.
    Ok(match value {
        None | Some(Value::Null) => None,
        Some(value) => value
            .as_u64()
            .or_else(|| value.as_i64().and_then(|number| u64::try_from(number).ok()))
            .or_else(|| value.as_str().and_then(|text| text.parse().ok())),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_snapshot_levels_to_rows() {
        let line = r#"{"topic":"orderbook.1.BTCUSDT","ts":1700000000000,"type":"snapshot","data":{"s":"BTCUSDT","b":[["100.0","2"]],"a":[["101.0","3"]],"u":7,"seq":9,"cts":1699999999999}}"#;
        let rows = parse_line(line, "BTCUSDT").unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].side, "bid");
        assert_eq!(rows[1].side, "ask");
        assert_eq!(rows[0].update_id, Some(7));
        assert_eq!(rows[0].cross_seq, Some(9));
    }

    #[test]
    fn treats_null_and_unparseable_sequence_fields_as_none() {
        let null_seq = r#"{"topic":"orderbook.1.BTCUSDT","ts":1700000000000,"type":"snapshot","data":{"s":"BTCUSDT","b":[["100.0","2"]],"a":[["101.0","3"]],"u":null,"seq":null}}"#;
        let rows = parse_line(null_seq, "BTCUSDT").unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].update_id, None);
        assert_eq!(rows[0].cross_seq, None);

        let bad_seq = r#"{"topic":"orderbook.1.BTCUSDT","ts":1700000000000,"type":"delta","data":{"s":"BTCUSDT","b":[["100.0","2"]],"a":[],"u":"not-a-number","seq":"also-bad"}}"#;
        let rows = parse_line(bad_seq, "BTCUSDT").unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].update_id, None);
        assert_eq!(rows[0].cross_seq, None);

        let missing_seq = r#"{"topic":"orderbook.1.BTCUSDT","ts":1700000000000,"type":"snapshot","data":{"s":"BTCUSDT","b":[["100.0","1"]],"a":[["101.0","1"]]}}"#;
        let rows = parse_line(missing_seq, "BTCUSDT").unwrap();
        assert_eq!(rows[0].update_id, None);
        assert_eq!(rows[0].cross_seq, None);
    }

    #[test]
    fn parses_gzip_trade_header_and_optional_columns() {
        use flate2::write::GzEncoder;
        use flate2::Compression;
        use std::io::Write;

        let csv = b"symbol,timestamp,side,price,size,trade_id,quote_qty,is_buyer_maker\nBTCUSDT,1700000000123,Buy,100.25,0.5,42,50.125,false\n";
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(csv).unwrap();
        let rows = parse_trade_gzip_bytes(&encoder.finish().unwrap(), "ETHUSDT").unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].symbol, "BTCUSDT");
        assert_eq!(rows[0].timestamp_ms, 1_700_000_000_123);
        assert_eq!(rows[0].trade_id, "42");
        assert_eq!(rows[0].quote_qty, Some(50.125));
        assert_eq!(rows[0].is_buyer_maker, Some(false));
    }

    #[test]
    fn parses_gzip_trade_without_header() {
        use flate2::write::GzEncoder;
        use flate2::Compression;
        use std::io::Write;

        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder
            .write_all(b"1700000000123,Sell,99.5,2.25,7\n")
            .unwrap();
        let rows = parse_trade_gzip_bytes(&encoder.finish().unwrap(), "BTCUSDT").unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].symbol, "BTCUSDT");
        assert_eq!(rows[0].side, "Sell");
        assert_eq!(rows[0].price, 99.5);
        assert_eq!(rows[0].size, 2.25);
        assert_eq!(rows[0].trade_id, "7");
    }

    #[test]
    fn parses_bybit_spot_archive_header_order() {
        use flate2::write::GzEncoder;
        use flate2::Compression;
        use std::io::Write;

        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder
            .write_all(b"id,timestamp,price,volume,side\n1,1668038400525,15910.61,0.003383,sell\n")
            .unwrap();
        let rows = parse_trade_gzip_bytes(&encoder.finish().unwrap(), "BTCUSDT").unwrap();
        assert_eq!(rows[0].trade_id, "1");
        assert_eq!(rows[0].timestamp_ms, 1_668_038_400_525);
        assert_eq!(rows[0].size, 0.003383);
        assert_eq!(rows[0].side, "sell");
    }

    #[test]
    fn parses_bybit_contract_archive_header_layout() {
        use flate2::write::GzEncoder;
        use flate2::Compression;
        use std::io::Write;

        let csv = concat!(
            "timestamp,symbol,side,size,price,tickDirection,trdMatchID,grossValue,homeNotional,foreignNotional\n",
            "1668038400.525,BTCUSDT,Sell,0.01,15910.5,PlusTick,",
            "a1b2c3d4-e5f6-7890-abcd-ef1234567890,159.105,0.01,159.105\n"
        );
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(csv.as_bytes()).unwrap();
        let rows = parse_trade_gzip_bytes(&encoder.finish().unwrap(), "ETHUSDT").unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].symbol, "BTCUSDT");
        assert_eq!(rows[0].side, "Sell");
        assert_eq!(rows[0].size, 0.01);
        assert_eq!(rows[0].price, 15910.5);
        assert_eq!(rows[0].timestamp_ms, 1_668_038_400_525);
        assert_eq!(
            rows[0].trade_id,
            "a1b2c3d4-e5f6-7890-abcd-ef1234567890"
        );
        assert_eq!(rows[0].quote_qty, Some(159.105));
    }

    #[test]
    fn parses_bybit_archive_without_header_order() {
        use flate2::write::GzEncoder;
        use flate2::Compression;
        use std::io::Write;

        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder
            .write_all(b"1,1668038400525,15910.61,0.003383,sell\n")
            .unwrap();
        let rows = parse_trade_gzip_bytes(&encoder.finish().unwrap(), "BTCUSDT").unwrap();
        assert_eq!(rows[0].trade_id, "1");
        assert_eq!(rows[0].timestamp_ms, 1_668_038_400_525);
        assert_eq!(rows[0].size, 0.003383);
        assert_eq!(rows[0].side, "sell");
    }
}
