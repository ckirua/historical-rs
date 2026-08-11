//! Parquet/ZSTD export for parsed Vision data.

use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::sync::Arc;

use arrow::array::{BooleanArray, Float64Array, Int64Array, RecordBatch, StringArray, UInt64Array};
use arrow::datatypes::{DataType, Field, Schema, SchemaRef};
use parquet::arrow::ArrowWriter;
use parquet::basic::Compression;
use parquet::file::properties::WriterProperties;

use crate::error::Result;
use crate::model::{
    AggTradeRow, DataType as VisionDataType, KlineRow, Market, TradeRow, VisionFile,
};

use crate::parse::GenericCsvTable;

pub fn dataset_object_key(file: &VisionFile) -> String {
    vision_storage_key(file)
}

pub fn local_dataset_path(base: &std::path::Path, file: &VisionFile) -> std::path::PathBuf {
    base.join(vision_storage_key(file))
}

use crate::vision::vision_storage_key;

pub fn export_agg_trades_parquet(
    rows: &[AggTradeRow],
    market: Market,
    symbol: &str,
) -> Result<Vec<u8>> {
    write_zstd_parquet(&agg_trades_batch(rows, market, symbol)?)
}

fn agg_trades_batch(rows: &[AggTradeRow], market: Market, symbol: &str) -> Result<RecordBatch> {
    let schema = Arc::new(Schema::new(vec![
        Field::new("market", DataType::Utf8, false),
        Field::new("symbol", DataType::Utf8, false),
        Field::new("agg_trade_id", DataType::UInt64, false),
        Field::new("price", DataType::Float64, false),
        Field::new("qty", DataType::Float64, false),
        Field::new("first_trade_id", DataType::UInt64, false),
        Field::new("last_trade_id", DataType::UInt64, false),
        Field::new("timestamp_ms", DataType::Int64, false),
        Field::new("is_buyer_maker", DataType::Boolean, false),
    ]));
    let n = rows.len();
    Ok(RecordBatch::try_new(
        schema,
        vec![
            Arc::new(StringArray::from_iter_values(std::iter::repeat_n(
                market.as_str(),
                n,
            ))),
            Arc::new(StringArray::from_iter_values(std::iter::repeat_n(
                symbol, n,
            ))),
            Arc::new(UInt64Array::from_iter_values(
                rows.iter().map(|row| row.agg_trade_id),
            )),
            Arc::new(Float64Array::from_iter_values(
                rows.iter().map(|row| row.price),
            )),
            Arc::new(Float64Array::from_iter_values(
                rows.iter().map(|row| row.qty),
            )),
            Arc::new(UInt64Array::from_iter_values(
                rows.iter().map(|row| row.first_trade_id),
            )),
            Arc::new(UInt64Array::from_iter_values(
                rows.iter().map(|row| row.last_trade_id),
            )),
            Arc::new(Int64Array::from_iter_values(
                rows.iter().map(|row| row.timestamp_ms),
            )),
            Arc::new(BooleanArray::from_iter(
                rows.iter().map(|row| Some(row.is_buyer_maker)),
            )),
        ],
    )?)
}

pub fn export_trades_parquet(rows: &[TradeRow], market: Market, symbol: &str) -> Result<Vec<u8>> {
    write_zstd_parquet(&trades_batch(rows, market, symbol)?)
}

fn trades_batch(rows: &[TradeRow], market: Market, symbol: &str) -> Result<RecordBatch> {
    let schema = Arc::new(Schema::new(vec![
        Field::new("market", DataType::Utf8, false),
        Field::new("symbol", DataType::Utf8, false),
        Field::new("trade_id", DataType::UInt64, false),
        Field::new("price", DataType::Float64, false),
        Field::new("qty", DataType::Float64, false),
        Field::new("quote_qty", DataType::Float64, true),
        Field::new("base_qty", DataType::Float64, true),
        Field::new("timestamp_ms", DataType::Int64, false),
        Field::new("is_buyer_maker", DataType::Boolean, false),
        Field::new("is_best_match", DataType::Boolean, true),
    ]));
    let n = rows.len();
    Ok(RecordBatch::try_new(
        schema,
        vec![
            Arc::new(StringArray::from_iter_values(std::iter::repeat_n(
                market.as_str(),
                n,
            ))),
            Arc::new(StringArray::from_iter_values(std::iter::repeat_n(
                symbol, n,
            ))),
            Arc::new(UInt64Array::from_iter_values(
                rows.iter().map(|row| row.trade_id),
            )),
            Arc::new(Float64Array::from_iter_values(
                rows.iter().map(|row| row.price),
            )),
            Arc::new(Float64Array::from_iter_values(
                rows.iter().map(|row| row.qty),
            )),
            Arc::new(Float64Array::from_iter(
                rows.iter().map(|row| row.quote_qty),
            )),
            Arc::new(Float64Array::from_iter(rows.iter().map(|row| row.base_qty))),
            Arc::new(Int64Array::from_iter_values(
                rows.iter().map(|row| row.timestamp_ms),
            )),
            Arc::new(BooleanArray::from_iter(
                rows.iter().map(|row| Some(row.is_buyer_maker)),
            )),
            Arc::new(BooleanArray::from_iter(
                rows.iter().map(|row| row.is_best_match),
            )),
        ],
    )?)
}

pub fn export_klines_parquet(rows: &[KlineRow], market: Market, symbol: &str) -> Result<Vec<u8>> {
    write_zstd_parquet(&klines_batch(rows, market, symbol)?)
}

fn klines_batch(rows: &[KlineRow], market: Market, symbol: &str) -> Result<RecordBatch> {
    let schema = Arc::new(Schema::new(vec![
        Field::new("market", DataType::Utf8, false),
        Field::new("symbol", DataType::Utf8, false),
        Field::new("open_time", DataType::Int64, false),
        Field::new("open", DataType::Float64, false),
        Field::new("high", DataType::Float64, false),
        Field::new("low", DataType::Float64, false),
        Field::new("close", DataType::Float64, false),
        Field::new("volume", DataType::Float64, false),
        Field::new("close_time", DataType::Int64, false),
        Field::new("quote_volume", DataType::Float64, true),
        Field::new("base_volume", DataType::Float64, true),
        Field::new("num_trades", DataType::UInt64, false),
        Field::new("taker_buy_volume", DataType::Float64, true),
        Field::new("taker_buy_quote_volume", DataType::Float64, true),
        Field::new("taker_buy_base_volume", DataType::Float64, true),
    ]));
    let n = rows.len();
    Ok(RecordBatch::try_new(
        schema,
        vec![
            Arc::new(StringArray::from_iter_values(std::iter::repeat_n(
                market.as_str(),
                n,
            ))),
            Arc::new(StringArray::from_iter_values(std::iter::repeat_n(
                symbol, n,
            ))),
            Arc::new(Int64Array::from_iter_values(
                rows.iter().map(|row| row.open_time),
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
                rows.iter().map(|row| row.volume),
            )),
            Arc::new(Int64Array::from_iter_values(
                rows.iter().map(|row| row.close_time),
            )),
            Arc::new(Float64Array::from_iter(
                rows.iter().map(|row| row.quote_volume),
            )),
            Arc::new(Float64Array::from_iter(
                rows.iter().map(|row| row.base_volume),
            )),
            Arc::new(UInt64Array::from_iter_values(
                rows.iter().map(|row| row.num_trades),
            )),
            Arc::new(Float64Array::from_iter(
                rows.iter().map(|row| row.taker_buy_volume),
            )),
            Arc::new(Float64Array::from_iter(
                rows.iter().map(|row| row.taker_buy_quote_volume),
            )),
            Arc::new(Float64Array::from_iter(
                rows.iter().map(|row| row.taker_buy_base_volume),
            )),
        ],
    )?)
}

pub fn export_generic_parquet(table: &GenericCsvTable) -> Result<Vec<u8>> {
    write_zstd_parquet(&generic_batch(table)?)
}

fn generic_batch(table: &GenericCsvTable) -> Result<RecordBatch> {
    let fields = table
        .headers
        .iter()
        .map(|name| Field::new(name, DataType::Utf8, true))
        .collect::<Vec<_>>();
    let schema = Arc::new(Schema::new(fields));
    let mut columns: Vec<Arc<dyn arrow::array::Array>> = Vec::with_capacity(table.headers.len());
    for col_idx in 0..table.headers.len() {
        columns.push(Arc::new(StringArray::from_iter(
            table
                .rows
                .iter()
                .map(|row| row.get(col_idx).map(String::as_str)),
        )));
    }
    Ok(RecordBatch::try_new(schema, columns)?)
}

/// Convert a Vision ZIP file to Parquet without retaining the archive, CSV, or full row set.
pub fn convert_vision_archive(
    file: &VisionFile,
    zip_path: &Path,
    output_path: &Path,
) -> Result<usize> {
    use crate::parse::{
        parse_agg_trades_csv, parse_generic_csv, parse_klines_csv, parse_trades_csv,
    };

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
    let symbol = file.symbol.as_str();

    match file.data_type {
        VisionDataType::AggTrades => write_typed_csv_batches(
            csv,
            output_path,
            agg_trades_batch(&[], market, symbol)?.schema(),
            |chunk| parse_agg_trades_csv(chunk, market),
            |rows| agg_trades_batch(rows, market, symbol),
        ),
        VisionDataType::Trades => write_typed_csv_batches(
            csv,
            output_path,
            trades_batch(&[], market, symbol)?.schema(),
            |chunk| parse_trades_csv(chunk, market),
            |rows| trades_batch(rows, market, symbol),
        ),
        VisionDataType::Klines
        | VisionDataType::PremiumIndexKlines
        | VisionDataType::MarkPriceKlines
        | VisionDataType::IndexPriceKlines => write_typed_csv_batches(
            csv,
            output_path,
            klines_batch(&[], market, symbol)?.schema(),
            |chunk| parse_klines_csv(chunk, market),
            |rows| klines_batch(rows, market, symbol),
        ),
        VisionDataType::BookDepth
        | VisionDataType::BookTicker
        | VisionDataType::LiquidationSnapshot
        | VisionDataType::Metrics
        | VisionDataType::BvolIndex => {
            write_generic_csv_batches(csv, output_path, parse_generic_csv)
        }
    }
}

fn write_typed_csv_batches<R, Row, Parse, Batch>(
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

fn write_generic_csv_batches<R, Parse>(
    reader: R,
    output_path: &Path,
    mut parse: Parse,
) -> Result<usize>
where
    R: Read,
    Parse: FnMut(&[u8]) -> Result<GenericCsvTable>,
{
    let mut output = Some(File::create(output_path)?);
    let mut writer = None;
    let mut row_count = 0usize;
    historical_core::csv_batch::for_each_csv_batch::<_, crate::error::Error, _>(
        reader,
        false,
        historical_core::csv_batch::DEFAULT_BATCH_ROWS,
        |chunk| {
            let table = parse(chunk)?;
            row_count += table.rows.len();
            let batch = generic_batch(&table)?;
            if writer.is_none() {
                let file = output.take().ok_or_else(|| {
                    crate::error::Error::Io(std::io::Error::other(
                        "Parquet output was already consumed",
                    ))
                })?;
                writer = Some(ArrowWriter::try_new(
                    file,
                    batch.schema(),
                    Some(writer_properties()),
                )?);
            }
            let parquet = writer.as_mut().ok_or_else(|| {
                crate::error::Error::Io(std::io::Error::other(
                    "Parquet writer failed to initialize",
                ))
            })?;
            if batch.num_rows() > 0 {
                parquet.write(&batch)?;
            }
            Ok(())
        },
    )?;
    if let Some(writer) = writer {
        writer.close()?;
    } else {
        let file = output.ok_or_else(|| {
            crate::error::Error::Io(std::io::Error::other("Parquet output was already consumed"))
        })?;
        ArrowWriter::try_new(file, Arc::new(Schema::empty()), Some(writer_properties()))?
            .close()?;
    }
    Ok(row_count)
}

pub fn convert_vision_bytes(file: &VisionFile, zip_bytes: &[u8]) -> Result<(Vec<u8>, usize)> {
    use crate::parse::{
        extract_csv_from_zip, parse_agg_trades_csv, parse_generic_csv, parse_klines_csv,
        parse_trades_csv,
    };

    let csv = extract_csv_from_zip(zip_bytes)?;
    let market = file.market;
    let symbol = file.symbol.as_str();
    match file.data_type {
        VisionDataType::AggTrades => {
            let rows = parse_agg_trades_csv(&csv, market)?;
            let count = rows.len();
            Ok((export_agg_trades_parquet(&rows, market, symbol)?, count))
        }
        VisionDataType::Trades => {
            let rows = parse_trades_csv(&csv, market)?;
            let count = rows.len();
            Ok((export_trades_parquet(&rows, market, symbol)?, count))
        }
        VisionDataType::Klines
        | VisionDataType::PremiumIndexKlines
        | VisionDataType::MarkPriceKlines
        | VisionDataType::IndexPriceKlines => {
            let rows = parse_klines_csv(&csv, market)?;
            let count = rows.len();
            Ok((export_klines_parquet(&rows, market, symbol)?, count))
        }
        VisionDataType::BookDepth
        | VisionDataType::BookTicker
        | VisionDataType::LiquidationSnapshot
        | VisionDataType::Metrics
        | VisionDataType::BvolIndex => {
            let table = parse_generic_csv(&csv)?;
            let count = table.rows.len();
            Ok((export_generic_parquet(&table)?, count))
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
    use crate::model::{AggTradeRow, DataType as VisionDataType, Frequency, Market, VisionFile};

    use super::*;

    #[test]
    fn dataset_object_key_layout() {
        let file = VisionFile {
            market: Market::Um,
            frequency: Frequency::Daily,
            data_type: VisionDataType::AggTrades,
            symbol: "BTCUSDT".into(),
            period: "2024-01-15".into(),
            interval: None,
        };
        assert_eq!(
            dataset_object_key(&file),
            "data/futures/um/daily/aggTrades/BTCUSDT/BTCUSDT-aggTrades-2024-01-15.parquet.zstd"
        );
    }

    #[test]
    fn agg_trades_parquet_round_trip_row_count() {
        let rows = vec![AggTradeRow {
            agg_trade_id: 1,
            price: 100.0,
            qty: 0.5,
            first_trade_id: 1,
            last_trade_id: 2,
            timestamp_ms: 1_498_793_709_153,
            is_buyer_maker: true,
        }];
        let bytes = export_agg_trades_parquet(&rows, Market::Um, "BTCUSDT").unwrap();
        assert!(!bytes.is_empty());
    }

    #[test]
    fn agg_trades_parquet_schema_stays_production_compatible() {
        use std::io::{Seek, Write};

        use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;

        let rows = vec![AggTradeRow {
            agg_trade_id: 1,
            price: 100.0,
            qty: 0.5,
            first_trade_id: 1,
            last_trade_id: 2,
            timestamp_ms: 1_498_793_709_153,
            is_buyer_maker: true,
        }];
        let bytes = export_agg_trades_parquet(&rows, Market::Um, "BTCUSDT").unwrap();
        let mut file = tempfile::tempfile().unwrap();
        file.write_all(&bytes).unwrap();
        file.rewind().unwrap();

        let reader = ParquetRecordBatchReaderBuilder::try_new(file).unwrap();
        let fields = reader
            .schema()
            .fields()
            .iter()
            .map(|field| {
                (
                    field.name().as_str(),
                    field.data_type().clone(),
                    field.is_nullable(),
                )
            })
            .collect::<Vec<_>>();

        assert_eq!(
            fields,
            vec![
                ("market", DataType::Utf8, false),
                ("symbol", DataType::Utf8, false),
                ("agg_trade_id", DataType::UInt64, false),
                ("price", DataType::Float64, false),
                ("qty", DataType::Float64, false),
                ("first_trade_id", DataType::UInt64, false),
                ("last_trade_id", DataType::UInt64, false),
                ("timestamp_ms", DataType::Int64, false),
                ("is_buyer_maker", DataType::Boolean, false),
            ]
        );
    }

    #[test]
    fn streams_archive_across_multiple_record_batches() {
        use std::io::Write;

        use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
        use zip::write::SimpleFileOptions;

        let mut zip = tempfile::NamedTempFile::new().unwrap();
        {
            let mut writer = zip::ZipWriter::new(zip.as_file_mut());
            writer
                .start_file("BTCUSDT-aggTrades.csv", SimpleFileOptions::default())
                .unwrap();
            writeln!(
                writer,
                "agg_trade_id,price,quantity,first_trade_id,last_trade_id,transact_time,is_buyer_maker"
            )
            .unwrap();
            for id in 0..=historical_core::csv_batch::DEFAULT_BATCH_ROWS {
                writeln!(writer, "{id},100,0.5,{id},{id},1498793709153,true").unwrap();
            }
            writer.finish().unwrap();
        }
        let output = tempfile::NamedTempFile::new().unwrap();
        let file = VisionFile {
            market: Market::Um,
            frequency: Frequency::Daily,
            data_type: VisionDataType::AggTrades,
            symbol: "BTCUSDT".into(),
            period: "2024-01-15".into(),
            interval: None,
        };

        let rows = convert_vision_archive(&file, zip.path(), output.path()).unwrap();
        assert_eq!(rows, historical_core::csv_batch::DEFAULT_BATCH_ROWS + 1);

        let reader = ParquetRecordBatchReaderBuilder::try_new(output.reopen().unwrap())
            .unwrap()
            .build()
            .unwrap();
        assert_eq!(
            reader.map(|batch| batch.unwrap().num_rows()).sum::<usize>(),
            rows
        );
    }
}
