//! Parquet/ZSTD export for parsed OKX data.

use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::sync::Arc;

use arrow::array::{Float64Array, Int32Array, Int64Array, RecordBatch, StringArray, UInt64Array};
use arrow::datatypes::{DataType, Field, Schema, SchemaRef};
use parquet::arrow::ArrowWriter;
use parquet::basic::Compression;
use parquet::file::properties::WriterProperties;

use crate::cdn::storage_key;
use crate::error::Result;
use crate::model::{
    BorrowRow, CandleRow, DataType as OkxDataType, FundingRow, Market, OkxFile, TradeRow,
};
use crate::parse::{
    extract_csv_from_zip, parse_borrow_csv, parse_candles_csv, parse_funding_csv, parse_trades_csv,
};

pub fn dataset_object_key(file: &OkxFile) -> String {
    storage_key(file)
}

pub fn local_dataset_path(base: &std::path::Path, file: &OkxFile) -> std::path::PathBuf {
    base.join(storage_key(file))
}

pub fn export_trades_parquet(rows: &[TradeRow], market: Market) -> Result<Vec<u8>> {
    write_zstd_parquet(&trades_batch(rows, market)?)
}

fn trades_batch(rows: &[TradeRow], market: Market) -> Result<RecordBatch> {
    let schema = Arc::new(Schema::new(vec![
        Field::new("market", DataType::Utf8, false),
        Field::new("instrument", DataType::Utf8, false),
        Field::new("trade_id", DataType::UInt64, false),
        Field::new("side", DataType::Utf8, false),
        Field::new("price", DataType::Float64, false),
        Field::new("size", DataType::Float64, false),
        Field::new("timestamp_ms", DataType::Int64, false),
    ]));
    Ok(RecordBatch::try_new(
        schema,
        vec![
            Arc::new(StringArray::from_iter_values(std::iter::repeat_n(
                market.as_str(),
                rows.len(),
            ))),
            Arc::new(StringArray::from_iter_values(
                rows.iter().map(|row| row.instrument.as_str()),
            )),
            Arc::new(UInt64Array::from_iter_values(
                rows.iter().map(|row| row.trade_id),
            )),
            Arc::new(StringArray::from_iter_values(
                rows.iter().map(|row| row.side.as_str()),
            )),
            Arc::new(Float64Array::from_iter_values(
                rows.iter().map(|row| row.price),
            )),
            Arc::new(Float64Array::from_iter_values(
                rows.iter().map(|row| row.size),
            )),
            Arc::new(Int64Array::from_iter_values(
                rows.iter().map(|row| row.timestamp_ms),
            )),
        ],
    )?)
}

pub fn export_candles_parquet(rows: &[CandleRow], market: Market) -> Result<Vec<u8>> {
    write_zstd_parquet(&candles_batch(rows, market)?)
}

fn candles_batch(rows: &[CandleRow], market: Market) -> Result<RecordBatch> {
    let schema = Arc::new(Schema::new(vec![
        Field::new("market", DataType::Utf8, false),
        Field::new("instrument", DataType::Utf8, false),
        Field::new("open", DataType::Float64, false),
        Field::new("high", DataType::Float64, false),
        Field::new("low", DataType::Float64, false),
        Field::new("close", DataType::Float64, false),
        Field::new("vol", DataType::Float64, false),
        Field::new("vol_ccy", DataType::Float64, false),
        Field::new("vol_quote", DataType::Float64, false),
        Field::new("open_time_ms", DataType::Int64, false),
        Field::new("confirm", DataType::Int32, false),
    ]));
    Ok(RecordBatch::try_new(
        schema,
        vec![
            Arc::new(StringArray::from_iter_values(std::iter::repeat_n(
                market.as_str(),
                rows.len(),
            ))),
            Arc::new(StringArray::from_iter_values(
                rows.iter().map(|row| row.instrument.as_str()),
            )),
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
                rows.iter().map(|row| row.vol),
            )),
            Arc::new(Float64Array::from_iter_values(
                rows.iter().map(|row| row.vol_ccy),
            )),
            Arc::new(Float64Array::from_iter_values(
                rows.iter().map(|row| row.vol_quote),
            )),
            Arc::new(Int64Array::from_iter_values(
                rows.iter().map(|row| row.open_time_ms),
            )),
            Arc::new(Int32Array::from_iter_values(
                rows.iter().map(|row| row.confirm),
            )),
        ],
    )?)
}

pub fn export_funding_parquet(rows: &[FundingRow], market: Market) -> Result<Vec<u8>> {
    write_zstd_parquet(&funding_batch(rows, market)?)
}

fn funding_batch(rows: &[FundingRow], market: Market) -> Result<RecordBatch> {
    let schema = Arc::new(Schema::new(vec![
        Field::new("market", DataType::Utf8, false),
        Field::new("instrument", DataType::Utf8, false),
        Field::new("funding_rate", DataType::Float64, false),
        Field::new("funding_time_ms", DataType::Int64, false),
    ]));
    Ok(RecordBatch::try_new(
        schema,
        vec![
            Arc::new(StringArray::from_iter_values(std::iter::repeat_n(
                market.as_str(),
                rows.len(),
            ))),
            Arc::new(StringArray::from_iter_values(
                rows.iter().map(|row| row.instrument.as_str()),
            )),
            Arc::new(Float64Array::from_iter_values(
                rows.iter().map(|row| row.funding_rate),
            )),
            Arc::new(Int64Array::from_iter_values(
                rows.iter().map(|row| row.funding_time_ms),
            )),
        ],
    )?)
}

pub fn export_borrow_parquet(rows: &[BorrowRow], market: Market) -> Result<Vec<u8>> {
    write_zstd_parquet(&borrow_batch(rows, market)?)
}

fn borrow_batch(rows: &[BorrowRow], market: Market) -> Result<RecordBatch> {
    let schema = Arc::new(Schema::new(vec![
        Field::new("market", DataType::Utf8, false),
        Field::new("currency", DataType::Utf8, false),
        Field::new("borrow_rate", DataType::Float64, false),
        Field::new("time_ms", DataType::Int64, false),
    ]));
    Ok(RecordBatch::try_new(
        schema,
        vec![
            Arc::new(StringArray::from_iter_values(std::iter::repeat_n(
                market.as_str(),
                rows.len(),
            ))),
            Arc::new(StringArray::from_iter_values(
                rows.iter().map(|row| row.currency.as_str()),
            )),
            Arc::new(Float64Array::from_iter_values(
                rows.iter().map(|row| row.borrow_rate),
            )),
            Arc::new(Int64Array::from_iter_values(
                rows.iter().map(|row| row.time_ms),
            )),
        ],
    )?)
}

/// Convert an OKX ZIP file to Parquet with bounded CSV and Arrow batches.
pub fn convert_okx_archive(file: &OkxFile, zip_path: &Path, output_path: &Path) -> Result<usize> {
    let input = File::open(zip_path)?;
    let mut archive =
        zip::ZipArchive::new(input).map_err(|error| crate::error::Error::Zip(error.to_string()))?;
    let mut csv_index = None;
    for index in 0..archive.len() {
        let member = archive
            .by_index(index)
            .map_err(|error| crate::error::Error::Zip(error.to_string()))?;
        if !member.is_dir() && member.name().to_ascii_lowercase().ends_with(".csv") {
            csv_index = Some(index);
            break;
        }
    }
    let index =
        csv_index.ok_or_else(|| crate::error::Error::Zip("no CSV file in archive".into()))?;
    let csv = archive
        .by_index(index)
        .map_err(|error| crate::error::Error::Zip(error.to_string()))?;
    let market = file.market;

    match file.data_type {
        OkxDataType::Trades => write_csv_batches(
            csv,
            output_path,
            trades_batch(&[], market)?.schema(),
            parse_trades_csv,
            |rows| trades_batch(rows, market),
        ),
        OkxDataType::Candlesticks => write_csv_batches(
            csv,
            output_path,
            candles_batch(&[], market)?.schema(),
            parse_candles_csv,
            |rows| candles_batch(rows, market),
        ),
        OkxDataType::Funding => write_csv_batches(
            csv,
            output_path,
            funding_batch(&[], market)?.schema(),
            parse_funding_csv,
            |rows| funding_batch(rows, market),
        ),
        OkxDataType::BorrowRates => write_csv_batches(
            csv,
            output_path,
            borrow_batch(&[], market)?.schema(),
            parse_borrow_csv,
            |rows| borrow_batch(rows, market),
        ),
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
        true,
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

pub fn convert_okx_bytes(file: &OkxFile, zip_bytes: &[u8]) -> Result<(Vec<u8>, usize)> {
    let csv = extract_csv_from_zip(zip_bytes)?;
    let market = file.market;
    match file.data_type {
        OkxDataType::Trades => {
            let rows = parse_trades_csv(&csv)?;
            let count = rows.len();
            Ok((export_trades_parquet(&rows, market)?, count))
        }
        OkxDataType::Candlesticks => {
            let rows = parse_candles_csv(&csv)?;
            let count = rows.len();
            Ok((export_candles_parquet(&rows, market)?, count))
        }
        OkxDataType::Funding => {
            let rows = parse_funding_csv(&csv)?;
            let count = rows.len();
            Ok((export_funding_parquet(&rows, market)?, count))
        }
        OkxDataType::BorrowRates => {
            let rows = parse_borrow_csv(&csv)?;
            let count = rows.len();
            Ok((export_borrow_parquet(&rows, market)?, count))
        }
    }
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
    use chrono::NaiveDate;

    use super::*;
    use crate::model::{DataType, Market, OkxFile, TradeRow};

    #[test]
    fn dataset_object_key_layout() {
        let file = OkxFile::daily(
            Market::Spot,
            DataType::Trades,
            "BTC-USDT",
            NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
        );
        assert_eq!(
            dataset_object_key(&file),
            "historical/data/spot/daily/trades/BTC-USDT/BTC-USDT-trades-2024-01-01.parquet.zstd"
        );
    }

    #[test]
    fn trades_parquet_nonempty() {
        let rows = vec![TradeRow {
            instrument: "BTC-USDT".into(),
            trade_id: 1,
            side: "buy".into(),
            price: 100.0,
            size: 0.1,
            timestamp_ms: 1,
        }];
        let bytes = export_trades_parquet(&rows, Market::Spot).unwrap();
        assert!(!bytes.is_empty());
    }

    #[test]
    fn streams_okx_zip_without_dropping_repeated_batch_headers() {
        use std::io::Write;

        use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
        use zip::write::SimpleFileOptions;

        let mut zip = tempfile::NamedTempFile::new().unwrap();
        {
            let mut writer = zip::ZipWriter::new(zip.as_file_mut());
            writer
                .start_file("BTC-USDT-trades.csv", SimpleFileOptions::default())
                .unwrap();
            writeln!(
                writer,
                "instrument_name,trade_id,side,price,size,created_time"
            )
            .unwrap();
            writeln!(writer, "BTC-USDT,1,buy,100,0.5,1704038400000").unwrap();
            writeln!(writer, "BTC-USDT,2,sell,101,0.4,1704038400001").unwrap();
            writer.finish().unwrap();
        }
        let output = tempfile::NamedTempFile::new().unwrap();
        let file = OkxFile::daily(
            Market::Spot,
            DataType::Trades,
            "BTC-USDT",
            NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
        );

        assert_eq!(
            convert_okx_archive(&file, zip.path(), output.path()).unwrap(),
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
