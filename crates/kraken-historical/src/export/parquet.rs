//! Parquet/ZSTD export for parsed Kraken data.

use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::sync::Arc;

use arrow::array::{Float64Array, Int64Array, RecordBatch, StringArray};
use arrow::datatypes::{DataType, Field, Schema, SchemaRef};
use parquet::arrow::ArrowWriter;
use parquet::basic::Compression;
use parquet::file::properties::WriterProperties;

use crate::error::Result;
use crate::model::{DataType as KrakenDataType, OhlcvtRow, OutputUnit, TradeRow};
use crate::parse::{extract_csv_member, parse_ohlcvt_csv, parse_trades_csv, ZipMember};

pub fn export_trades_parquet(rows: &[TradeRow], pair: &str) -> Result<Vec<u8>> {
    write_zstd_parquet(&trades_batch(rows, pair)?)
}

fn trades_batch(rows: &[TradeRow], pair: &str) -> Result<RecordBatch> {
    let schema = Arc::new(Schema::new(vec![
        Field::new("pair", DataType::Utf8, false),
        Field::new("ts", DataType::Float64, false),
        Field::new("price", DataType::Float64, false),
        Field::new("volume", DataType::Float64, false),
    ]));
    Ok(RecordBatch::try_new(
        schema,
        vec![
            Arc::new(StringArray::from_iter_values(std::iter::repeat_n(
                pair,
                rows.len(),
            ))),
            Arc::new(Float64Array::from_iter_values(
                rows.iter().map(|row| row.ts),
            )),
            Arc::new(Float64Array::from_iter_values(
                rows.iter().map(|row| row.price),
            )),
            Arc::new(Float64Array::from_iter_values(
                rows.iter().map(|row| row.volume),
            )),
        ],
    )?)
}

pub fn export_ohlcvt_parquet(rows: &[OhlcvtRow], pair: &str, interval: &str) -> Result<Vec<u8>> {
    write_zstd_parquet(&ohlcvt_batch(rows, pair, interval)?)
}

fn ohlcvt_batch(rows: &[OhlcvtRow], pair: &str, interval: &str) -> Result<RecordBatch> {
    let schema = Arc::new(Schema::new(vec![
        Field::new("pair", DataType::Utf8, false),
        Field::new("interval", DataType::Utf8, false),
        Field::new("ts", DataType::Int64, false),
        Field::new("open", DataType::Float64, false),
        Field::new("high", DataType::Float64, false),
        Field::new("low", DataType::Float64, false),
        Field::new("close", DataType::Float64, false),
        Field::new("volume", DataType::Float64, false),
        Field::new("trades", DataType::Int64, false),
    ]));
    Ok(RecordBatch::try_new(
        schema,
        vec![
            Arc::new(StringArray::from_iter_values(std::iter::repeat_n(
                pair,
                rows.len(),
            ))),
            Arc::new(StringArray::from_iter_values(std::iter::repeat_n(
                interval,
                rows.len(),
            ))),
            Arc::new(Int64Array::from_iter_values(rows.iter().map(|row| row.ts))),
            Arc::new(Float64Array::from_iter_values(
                rows.iter().map(|row| row.open),
            )),
            Arc::new(Float64Array::from_iter_values(
                rows.iter().map(|row| row.high),
            )),
            Arc::new(Float64Array::from_iter_values(
                rows.iter().map(|row| row.low),
            )),
            Arc::new(Float64Array::from_iter_values(
                rows.iter().map(|row| row.close),
            )),
            Arc::new(Float64Array::from_iter_values(
                rows.iter().map(|row| row.volume),
            )),
            Arc::new(Int64Array::from_iter_values(
                rows.iter().map(|row| row.trades),
            )),
        ],
    )?)
}

/// Extract one ZIP member from disk, parse, and export parquet.zstd bytes.
pub fn convert_member_to_parquet(
    zip_path: &std::path::Path,
    member: &ZipMember,
    unit: &OutputUnit,
) -> Result<(Vec<u8>, usize)> {
    let csv = extract_csv_member(zip_path, member.index)?;
    match unit.data_type {
        KrakenDataType::Trades => {
            let rows = parse_trades_csv(&csv)?;
            let count = rows.len();
            if count == 0 {
                return Ok((Vec::new(), 0));
            }
            Ok((export_trades_parquet(&rows, &unit.pair)?, count))
        }
        KrakenDataType::Ohlcvt => {
            let interval = unit.interval.as_deref().unwrap_or("unknown");
            let rows = parse_ohlcvt_csv(&csv)?;
            let count = rows.len();
            if count == 0 {
                return Ok((Vec::new(), 0));
            }
            Ok((export_ohlcvt_parquet(&rows, &unit.pair, interval)?, count))
        }
    }
}

/// Convert one ZIP member into a Parquet file using bounded CSV and Arrow batches.
pub fn convert_member_to_parquet_file(
    zip_path: &Path,
    member: &ZipMember,
    unit: &OutputUnit,
    output_path: &Path,
) -> Result<usize> {
    let input = File::open(zip_path)?;
    let mut archive =
        zip::ZipArchive::new(input).map_err(|error| crate::error::Error::Zip(error.to_string()))?;
    let csv = archive
        .by_index(member.index)
        .map_err(|error| crate::error::Error::Zip(error.to_string()))?;
    match unit.data_type {
        KrakenDataType::Trades => write_csv_batches(
            csv,
            output_path,
            trades_batch(&[], &unit.pair)?.schema(),
            parse_trades_csv,
            |rows| trades_batch(rows, &unit.pair),
        ),
        KrakenDataType::Ohlcvt => {
            let interval = unit.interval.as_deref().unwrap_or("unknown");
            write_csv_batches(
                csv,
                output_path,
                ohlcvt_batch(&[], &unit.pair, interval)?.schema(),
                parse_ohlcvt_csv,
                |rows| ohlcvt_batch(rows, &unit.pair, interval),
            )
        }
    }
}

fn write_csv_batches<R, Row, Parse, Batch>(
    reader: R,
    output_path: &Path,
    schema: SchemaRef,
    mut parse: Parse,
    mut batch: Batch,
) -> Result<usize>
where
    R: Read,
    Parse: FnMut(&[u8]) -> Result<Vec<Row>>,
    Batch: FnMut(&[Row]) -> Result<RecordBatch>,
{
    let output = File::create(output_path)?;
    let mut writer = ArrowWriter::try_new(output, schema, Some(writer_properties()))?;
    let mut row_count = 0usize;
    historical_core::csv_batch::for_each_csv_batch::<_, crate::error::Error, _>(
        reader,
        false,
        historical_core::csv_batch::DEFAULT_BATCH_ROWS,
        |chunk| {
            let rows = parse(chunk)?;
            row_count += rows.len();
            if !rows.is_empty() {
                writer.write(&batch(&rows)?)?;
            }
            Ok(())
        },
    )?;
    writer.close()?;
    Ok(row_count)
}

fn writer_properties() -> WriterProperties {
    WriterProperties::builder()
        .set_compression(Compression::ZSTD(Default::default()))
        .build()
}

fn write_zstd_parquet(batch: &RecordBatch) -> Result<Vec<u8>> {
    let mut buffer = Vec::new();
    let mut writer = ArrowWriter::try_new(&mut buffer, batch.schema(), Some(writer_properties()))?;
    writer.write(batch)?;
    writer.close()?;
    Ok(buffer)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::TradeRow;

    #[test]
    fn trades_parquet_nonempty() {
        let rows = vec![TradeRow {
            ts: 1.0,
            price: 100.0,
            volume: 0.1,
        }];
        let bytes = export_trades_parquet(&rows, "XBTUSD").unwrap();
        assert!(!bytes.is_empty());
    }

    #[test]
    fn streams_kraken_zip_member_to_parquet() {
        use std::io::Write;

        use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
        use zip::write::SimpleFileOptions;

        let mut zip = tempfile::NamedTempFile::new().unwrap();
        {
            let mut writer = zip::ZipWriter::new(zip.as_file_mut());
            writer
                .start_file("XBTUSD.csv", SimpleFileOptions::default())
                .unwrap();
            writeln!(writer, "0.00002000,0.02000000,1438909837.1506").unwrap();
            writeln!(writer, "42000.5,0.1,1700000000.0").unwrap();
            writer.finish().unwrap();
        }
        let member = ZipMember {
            name: "XBTUSD.csv".into(),
            index: 0,
            pair: "XBTUSD".into(),
            interval: None,
        };
        let unit = OutputUnit::trades(crate::model::Source::Complete, "XBTUSD");
        let output = tempfile::NamedTempFile::new().unwrap();

        assert_eq!(
            convert_member_to_parquet_file(zip.path(), &member, &unit, output.path()).unwrap(),
            2
        );
        let reader = ParquetRecordBatchReaderBuilder::try_new(output.reopen().unwrap())
            .unwrap()
            .build()
            .unwrap();
        assert_eq!(
            reader.map(|batch| batch.unwrap().num_rows()).sum::<usize>(),
            2
        );
    }
}
