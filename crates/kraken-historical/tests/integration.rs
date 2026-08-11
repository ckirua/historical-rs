//! Integration / live smoke tests.

use std::io::Write;

use kraken_historical::parse::{
    extract_csv_member, list_csv_members, parse_ohlcvt_csv, parse_trades_csv,
};
use kraken_historical::{
    dataset_object_key, extract_drive_file_ids, parse_quarter_filename, DataType, OutputUnit,
    Source,
};
use zip::write::SimpleFileOptions;
use zip::ZipWriter;

fn make_zip(entries: &[(&str, &[u8])]) -> tempfile::NamedTempFile {
    let mut tmp = tempfile::NamedTempFile::new().unwrap();
    {
        let mut writer = ZipWriter::new(tmp.as_file_mut());
        for (name, data) in entries {
            writer
                .start_file(*name, SimpleFileOptions::default())
                .unwrap();
            writer.write_all(data).unwrap();
        }
        writer.finish().unwrap();
    }
    tmp
}

#[test]
fn zip_members_and_roundtrip_trades() {
    let csv = b"0.00002000,0.02000000,1438909837.1506\n";
    let tmp = make_zip(&[("XBTUSD.csv", csv)]);
    let members = list_csv_members(tmp.path(), DataType::Trades).unwrap();
    assert_eq!(members.len(), 1);
    assert_eq!(members[0].pair, "XBTUSD");
    let extracted = extract_csv_member(tmp.path(), members[0].index).unwrap();
    let rows = parse_trades_csv(&extracted).unwrap();
    assert_eq!(rows.len(), 1);
}

#[test]
fn zip_members_ohlcvt() {
    let csv = b"1387267220,123.0,124.0,122.0,123.5,10.5,42\n";
    let tmp = make_zip(&[("ETHUSD_60.csv", csv)]);
    let members = list_csv_members(tmp.path(), DataType::Ohlcvt).unwrap();
    assert_eq!(members.len(), 1);
    assert_eq!(members[0].interval.as_deref(), Some("60"));
    let extracted = extract_csv_member(tmp.path(), members[0].index).unwrap();
    let rows = parse_ohlcvt_csv(&extracted).unwrap();
    assert_eq!(rows[0].trades, 42);
}

#[test]
fn storage_keys_and_filenames() {
    assert_eq!(
        parse_quarter_filename("Kraken_Trades_Q2_2024.zip"),
        Some((DataType::Trades, "2024Q2".into()))
    );
    let unit = OutputUnit::ohlcvt(
        Source::Quarterly {
            quarter: "2024Q2".into(),
        },
        "XBTUSD",
        "1",
    );
    assert_eq!(
        dataset_object_key(&unit),
        "historical/data/ohlcvt/quarterly/2024Q2/XBTUSD/1.parquet.zstd"
    );
}

#[test]
fn scrape_fixture_html() {
    let html = include_str!("fixtures/support_trades_snippet.html");
    let ids = extract_drive_file_ids(html);
    assert!(ids
        .iter()
        .any(|id| id == "10zh3tDpqANYvVtYVgczwVz3UZFRUb1el"));
}

/// Live smoke: discover trades via public Drive folder scrape (no API key).
#[tokio::test]
#[ignore = "requires network access to drive.google.com"]
async fn live_discover_quarterly_trades() {
    let config = kraken_historical::Config::load().unwrap();
    let artifacts = kraken_historical::discover(&config, Some(DataType::Trades))
        .await
        .unwrap();
    assert!(
        artifacts
            .iter()
            .any(|a| matches!(a.source, Source::Complete)),
        "expected complete trades artifact"
    );
    assert!(
        artifacts
            .iter()
            .any(|a| matches!(a.source, Source::Quarterly { .. })),
        "expected at least one quarterly trades artifact from public folder scrape"
    );
}
