use std::fs::File;
use std::path::Path;
use std::sync::Arc;

use arrow::array::{Float64Array, Int64Array, RecordBatch, StringArray, UInt64Array};
use arrow::datatypes::{DataType, Field, Schema};
use parquet::arrow::ArrowWriter;
use parquet::basic::Compression;
use parquet::file::properties::WriterProperties;

use crate::error::Result;
use crate::model::{OrderBookRow, TradeRow};
use crate::parse::{parse_trade_gzip_file_batched, parse_zip_file_batched};
pub fn orderbook_schema() -> Arc<Schema> {
    Arc::new(Schema::new(vec![
        Field::new("market", DataType::Utf8, false),
        Field::new("symbol", DataType::Utf8, false),
        Field::new("topic", DataType::Utf8, false),
        Field::new("message_type", DataType::Utf8, false),
        Field::new("ts_ms", DataType::Int64, false),
        Field::new("cts_ms", DataType::Int64, true),
        Field::new("update_id", DataType::UInt64, true),
        Field::new("cross_seq", DataType::UInt64, true),
        Field::new("side", DataType::Utf8, false),
        Field::new("price", DataType::Float64, false),
        Field::new("size", DataType::Float64, false),
    ]))
}

/// Stable Parquet schema for Spot and Contract trade archives.
pub fn trade_schema() -> Arc<Schema> {
    Arc::new(Schema::new(vec![
        Field::new("symbol", DataType::Utf8, false),
        Field::new("timestamp_ms", DataType::Int64, false),
        Field::new("side", DataType::Utf8, false),
        Field::new("price", DataType::Float64, false),
        Field::new("size", DataType::Float64, false),
        Field::new("trade_id", DataType::Utf8, false),
        Field::new("quote_qty", DataType::Float64, true),
        Field::new("is_buyer_maker", DataType::Boolean, true),
    ]))
}

pub fn trade_batch(rows: &[TradeRow]) -> Result<RecordBatch> {
    Ok(RecordBatch::try_new(
        trade_schema(),
        vec![
            Arc::new(StringArray::from(
                rows.iter()
                    .map(|row| Some(row.symbol.as_str()))
                    .collect::<Vec<_>>(),
            )),
            Arc::new(Int64Array::from(
                rows.iter().map(|row| row.timestamp_ms).collect::<Vec<_>>(),
            )),
            Arc::new(StringArray::from(
                rows.iter()
                    .map(|row| Some(row.side.as_str()))
                    .collect::<Vec<_>>(),
            )),
            Arc::new(Float64Array::from(
                rows.iter().map(|row| row.price).collect::<Vec<_>>(),
            )),
            Arc::new(Float64Array::from(
                rows.iter().map(|row| row.size).collect::<Vec<_>>(),
            )),
            Arc::new(StringArray::from(
                rows.iter()
                    .map(|row| Some(row.trade_id.as_str()))
                    .collect::<Vec<_>>(),
            )),
            Arc::new(Float64Array::from(
                rows.iter().map(|row| row.quote_qty).collect::<Vec<_>>(),
            )),
            Arc::new(arrow::array::BooleanArray::from(
                rows.iter().map(|row| row.is_buyer_maker).collect::<Vec<_>>(),
            )),
        ],
    )?)
}

pub fn orderbook_batch(rows: &[OrderBookRow]) -> Result<RecordBatch> {
    Ok(RecordBatch::try_new(
        orderbook_schema(),
        vec![
            Arc::new(StringArray::from(
                rows.iter()
                    .map(|row| Some(row.market.as_str()))
                    .collect::<Vec<_>>(),
            )),
            Arc::new(StringArray::from(
                rows.iter()
                    .map(|row| Some(row.symbol.as_str()))
                    .collect::<Vec<_>>(),
            )),
            Arc::new(StringArray::from(
                rows.iter()
                    .map(|row| Some(row.topic.as_str()))
                    .collect::<Vec<_>>(),
            )),
            Arc::new(StringArray::from(
                rows.iter()
                    .map(|row| Some(row.message_type.as_str()))
                    .collect::<Vec<_>>(),
            )),
            Arc::new(Int64Array::from(
                rows.iter().map(|row| row.ts_ms).collect::<Vec<_>>(),
            )),
            Arc::new(Int64Array::from(
                rows.iter().map(|row| row.cts_ms).collect::<Vec<_>>(),
            )),
            Arc::new(UInt64Array::from(
                rows.iter().map(|row| row.update_id).collect::<Vec<_>>(),
            )),
            Arc::new(UInt64Array::from(
                rows.iter().map(|row| row.cross_seq).collect::<Vec<_>>(),
            )),
            Arc::new(StringArray::from(
                rows.iter()
                    .map(|row| Some(row.side.as_str()))
                    .collect::<Vec<_>>(),
            )),
            Arc::new(Float64Array::from(
                rows.iter().map(|row| row.price).collect::<Vec<_>>(),
            )),
            Arc::new(Float64Array::from(
                rows.iter().map(|row| row.size).collect::<Vec<_>>(),
            )),
        ],
    )?)
}

pub fn convert_archive(zip_path: &Path, symbol: &str, output_path: &Path) -> Result<usize> {
    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let output = File::create(output_path)?;
    let properties = WriterProperties::builder()
        .set_compression(Compression::ZSTD(Default::default()))
        .build();
    let mut writer = ArrowWriter::try_new(output, orderbook_schema(), Some(properties))?;
    let rows = parse_zip_file_batched(zip_path, symbol, |batch| {
        writer.write(&orderbook_batch(batch)?)?;
        Ok(())
    })?;
    writer.close()?;
    Ok(rows)
}


/// Convert a gzip-compressed Spot trade archive into deterministic Parquet.
pub fn convert_trade_archive(
    gzip_path: &Path,
    symbol: &str,
    output_path: &Path,
) -> Result<usize> {
    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let output = File::create(output_path)?;
    let properties = WriterProperties::builder()
        .set_compression(Compression::ZSTD(Default::default()))
        .build();
    let mut writer = ArrowWriter::try_new(output, trade_schema(), Some(properties))?;
    let rows = parse_trade_gzip_file_batched(gzip_path, symbol, |batch| {
        writer.write(&trade_batch(batch)?)?;
        Ok(())
    })?;
    writer.close()?;
    Ok(rows)
}

pub fn convert_trade_archive_bytes(
    gzip_path: &Path,
    symbol: &str,
) -> Result<(Vec<u8>, usize)> {
    let output_path = tempfile::NamedTempFile::new()?.into_temp_path();
    let count = convert_trade_archive(gzip_path, symbol, &output_path)?;
    Ok((std::fs::read(output_path)?, count))
}
pub fn convert_archive_bytes(zip_path: &Path, symbol: &str) -> Result<(Vec<u8>, usize)> {
    let output_path = tempfile::NamedTempFile::new()?.into_temp_path();
    convert_archive(zip_path, symbol, &output_path)?;
    Ok((
        std::fs::read(output_path)?,
        parse_rows_for_count(zip_path, symbol)?,
    ))
}

fn parse_rows_for_count(zip_path: &Path, symbol: &str) -> Result<usize> {
    let mut count = 0;
    crate::parse::parse_zip_file_batched(zip_path, symbol, |rows| {
        count += rows.len();
        Ok(())
    })?;
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trade_batch_uses_stable_nullable_optional_columns() {
        let rows = [TradeRow {
            symbol: "BTCUSDT".into(),
            timestamp_ms: 1_700_000_000_123,
            side: "Buy".into(),
            price: 100.25,
            size: 0.5,
            trade_id: "42".into(),
            quote_qty: Some(50.125),
            is_buyer_maker: None,
        }];
        let batch = trade_batch(&rows).unwrap();
        assert_eq!(batch.num_rows(), 1);
        assert_eq!(batch.schema(), trade_schema());
        assert_eq!(batch.column(6).null_count(), 0);
        assert_eq!(batch.column(7).null_count(), 1);
    }

    #[test]
    fn converts_gzip_trade_archive_to_parquet() {
        use flate2::write::GzEncoder;
        use flate2::Compression;
        use std::io::Write;

        let mut gzip = GzEncoder::new(Vec::new(), Compression::default());
        gzip.write_all(b"timestamp,side,price,size,trade_id\n1700000000000,Buy,100,0.1,1\n")
            .unwrap();
        let input = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(input.path(), gzip.finish().unwrap()).unwrap();
        let output = tempfile::NamedTempFile::new().unwrap();
        assert_eq!(convert_trade_archive(input.path(), "BTCUSDT", output.path()).unwrap(), 1);
        assert!(std::fs::metadata(output.path()).unwrap().len() > 4);
    }
}
