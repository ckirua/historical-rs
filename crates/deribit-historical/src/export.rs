use std::fs::File;
use std::path::Path;
use std::sync::Arc;

use arrow::array::{Float64Array, Int32Array, Int64Array, RecordBatch, StringArray};
use arrow::datatypes::{DataType, Field, Schema};
use parquet::arrow::ArrowWriter;
use parquet::basic::Compression;
use parquet::file::properties::WriterProperties;

use crate::error::Result;
use crate::model::{TradeRow, VolatilityIndexCandle};

pub fn trade_schema() -> Arc<Schema> {
    Arc::new(Schema::new(vec![
        Field::new("instrument_name", DataType::Utf8, false),
        Field::new("currency", DataType::Utf8, false),
        Field::new("trade_id", DataType::Utf8, false),
        Field::new("trade_seq", DataType::Int64, false),
        Field::new("timestamp_ms", DataType::Int64, false),
        Field::new("direction", DataType::Utf8, false),
        Field::new("price", DataType::Float64, false),
        Field::new("amount", DataType::Float64, false),
        Field::new("contracts", DataType::Float64, true),
        Field::new("index_price", DataType::Float64, true),
        Field::new("mark_price", DataType::Float64, true),
        Field::new("tick_direction", DataType::Int32, true),
    ]))
}

pub fn trade_batch(rows: &[TradeRow]) -> Result<RecordBatch> {
    Ok(RecordBatch::try_new(
        trade_schema(),
        vec![
            Arc::new(StringArray::from(
                rows.iter()
                    .map(|row| Some(row.instrument_name.as_str()))
                    .collect::<Vec<_>>(),
            )),
            Arc::new(StringArray::from(
                rows.iter()
                    .map(|row| Some(row.currency.as_str()))
                    .collect::<Vec<_>>(),
            )),
            Arc::new(StringArray::from(
                rows.iter()
                    .map(|row| Some(row.trade_id.as_str()))
                    .collect::<Vec<_>>(),
            )),
            Arc::new(Int64Array::from(
                rows.iter().map(|row| row.trade_seq).collect::<Vec<_>>(),
            )),
            Arc::new(Int64Array::from(
                rows.iter().map(|row| row.timestamp_ms).collect::<Vec<_>>(),
            )),
            Arc::new(StringArray::from(
                rows.iter()
                    .map(|row| Some(row.direction.as_str()))
                    .collect::<Vec<_>>(),
            )),
            Arc::new(Float64Array::from(
                rows.iter().map(|row| row.price).collect::<Vec<_>>(),
            )),
            Arc::new(Float64Array::from(
                rows.iter().map(|row| row.amount).collect::<Vec<_>>(),
            )),
            Arc::new(Float64Array::from(
                rows.iter().map(|row| row.contracts).collect::<Vec<_>>(),
            )),
            Arc::new(Float64Array::from(
                rows.iter().map(|row| row.index_price).collect::<Vec<_>>(),
            )),
            Arc::new(Float64Array::from(
                rows.iter().map(|row| row.mark_price).collect::<Vec<_>>(),
            )),
            Arc::new(Int32Array::from(
                rows.iter().map(|row| row.tick_direction).collect::<Vec<_>>(),
            )),
        ],
    )?)
}

pub fn write_trades_parquet(rows: &[TradeRow], output_path: &Path) -> Result<usize> {
    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let output = File::create(output_path)?;
    let properties = WriterProperties::builder()
        .set_compression(Compression::ZSTD(Default::default()))
        .build();
    let mut writer = ArrowWriter::try_new(output, trade_schema(), Some(properties))?;
    if !rows.is_empty() {
        writer.write(&trade_batch(rows)?)?;
    } else {
        // Empty day still gets a valid parquet schema for resume/presence.
        writer.write(&trade_batch(&[])?)?;
    }
    writer.close()?;
    Ok(rows.len())
}

pub fn write_trades_parquet_bytes(rows: &[TradeRow]) -> Result<(Vec<u8>, usize)> {
    let output = tempfile::NamedTempFile::new()?;
    let count = write_trades_parquet(rows, output.path())?;
    Ok((std::fs::read(output.path())?, count))
}

pub fn volatility_index_schema() -> Arc<Schema> {
    Arc::new(Schema::new(vec![
        Field::new("currency", DataType::Utf8, false),
        Field::new("index_name", DataType::Utf8, false),
        Field::new("resolution", DataType::Utf8, false),
        Field::new("timestamp_ms", DataType::Int64, false),
        Field::new("open", DataType::Float64, false),
        Field::new("high", DataType::Float64, false),
        Field::new("low", DataType::Float64, false),
        Field::new("close", DataType::Float64, false),
    ]))
}

pub fn volatility_index_batch(rows: &[VolatilityIndexCandle]) -> Result<RecordBatch> {
    Ok(RecordBatch::try_new(
        volatility_index_schema(),
        vec![
            Arc::new(StringArray::from(
                rows.iter()
                    .map(|row| Some(row.currency.as_str()))
                    .collect::<Vec<_>>(),
            )),
            Arc::new(StringArray::from(
                rows.iter()
                    .map(|row| Some(row.index_name.as_str()))
                    .collect::<Vec<_>>(),
            )),
            Arc::new(StringArray::from(
                rows.iter()
                    .map(|row| Some(row.resolution.as_str()))
                    .collect::<Vec<_>>(),
            )),
            Arc::new(Int64Array::from(
                rows.iter().map(|row| row.timestamp_ms).collect::<Vec<_>>(),
            )),
            Arc::new(Float64Array::from(
                rows.iter().map(|row| row.open).collect::<Vec<_>>(),
            )),
            Arc::new(Float64Array::from(
                rows.iter().map(|row| row.high).collect::<Vec<_>>(),
            )),
            Arc::new(Float64Array::from(
                rows.iter().map(|row| row.low).collect::<Vec<_>>(),
            )),
            Arc::new(Float64Array::from(
                rows.iter().map(|row| row.close).collect::<Vec<_>>(),
            )),
        ],
    )?)
}

pub fn write_volatility_index_parquet(
    rows: &[VolatilityIndexCandle],
    output_path: &Path,
) -> Result<usize> {
    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let output = File::create(output_path)?;
    let properties = WriterProperties::builder()
        .set_compression(Compression::ZSTD(Default::default()))
        .build();
    let mut writer = ArrowWriter::try_new(output, volatility_index_schema(), Some(properties))?;
    writer.write(&volatility_index_batch(rows)?)?;
    writer.close()?;
    Ok(rows.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_non_empty_parquet() {
        let rows = [TradeRow {
            instrument_name: "BTC-PERPETUAL".into(),
            currency: "BTC".into(),
            trade_id: "1".into(),
            trade_seq: 1,
            timestamp_ms: 1_704_067_200_000,
            direction: "buy".into(),
            price: 42000.0,
            amount: 10.0,
            contracts: Some(1.0),
            index_price: Some(41990.0),
            mark_price: Some(42001.0),
            tick_direction: Some(1),
        }];
        let output = tempfile::NamedTempFile::new().unwrap();
        assert_eq!(write_trades_parquet(&rows, output.path()).unwrap(), 1);
        assert!(std::fs::metadata(output.path()).unwrap().len() > 4);
        let batch = trade_batch(&rows).unwrap();
        assert_eq!(batch.schema(), trade_schema());
        assert_eq!(batch.num_rows(), 1);
    }

    #[test]
    fn writes_empty_parquet() {
        let output = tempfile::NamedTempFile::new().unwrap();
        assert_eq!(write_trades_parquet(&[], output.path()).unwrap(), 0);
        assert!(std::fs::metadata(output.path()).unwrap().len() > 4);
    }

    #[test]
    fn writes_volatility_index_parquet() {
        let rows = [VolatilityIndexCandle {
            currency: "BTC".into(),
            index_name: "btcdvol_usdc".into(),
            resolution: "3600".into(),
            timestamp_ms: 1_785_772_800_000,
            open: 35.08,
            high: 35.1,
            low: 34.98,
            close: 35.0,
        }];
        let output = tempfile::NamedTempFile::new().unwrap();
        assert_eq!(
            write_volatility_index_parquet(&rows, output.path()).unwrap(),
            1
        );
        assert!(std::fs::metadata(output.path()).unwrap().len() > 4);
    }
}
