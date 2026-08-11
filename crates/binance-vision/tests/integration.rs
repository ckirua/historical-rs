//! Integration tests against live Binance Vision (network required).

use binance_vision::{
    parse::{extract_csv_from_zip, parse_agg_trades_csv},
    Config, DataType, Market, VisionFile, VisionHttpClient,
};
use chrono::NaiveDate;

#[tokio::test]
#[ignore = "requires network access to data.binance.vision"]
async fn downloads_um_aggtrades_for_btcusdt() {
    let config = Config::default();
    let http = VisionHttpClient::new(config).expect("client");
    let file = VisionFile::daily(
        Market::Um,
        DataType::AggTrades,
        "BTCUSDT",
        NaiveDate::from_ymd_opt(2024, 1, 15).unwrap(),
        None,
    );
    let url = http.url_builder().build(&file);
    let zip = http.get_bytes(&url).await.expect("download");
    let csv = extract_csv_from_zip(&zip).expect("extract");
    let rows = parse_agg_trades_csv(&csv, Market::Um).expect("parse");
    assert!(!rows.is_empty(), "expected aggTrades rows");
}
