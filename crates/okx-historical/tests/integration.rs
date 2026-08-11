//! Integration / live smoke tests.

use chrono::NaiveDate;
use okx_historical::{CdnUrl, Config, DataType, Market, OkxFile, OkxHttpClient};

#[test]
fn cdn_url_matches_known_archive() {
    let url = CdnUrl::default().build(&OkxFile::daily(
        Market::Spot,
        DataType::Trades,
        "BTC-USDT",
        NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
    ));
    assert!(url.contains("/trades/daily/20240101/BTC-USDT-trades-2024-01-01.zip"));
}

#[tokio::test]
#[ignore = "live OKX CDN download"]
async fn live_download_spot_trades_day() {
    let config = Config::default();
    let http = OkxHttpClient::new(config).unwrap();
    let file = OkxFile::daily(
        Market::Spot,
        DataType::Trades,
        "BTC-USDT",
        NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
    );
    let url = http.url_builder().build(&file);
    let bytes = http.get_bytes(&url).await.expect("download");
    assert!(bytes.len() > 100, "expected non-trivial zip");
}
