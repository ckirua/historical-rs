//! URL builder tests covering all legacy Cython API endpoints.

use binance_vision::{DataType, Frequency, Market, VisionFile, VisionUrl};

fn url(file: VisionFile) -> String {
    VisionUrl::default().build(&file)
}

#[test]
fn all_daily_um_types() {
    let base = |dtype| VisionFile {
        market: Market::Um,
        frequency: Frequency::Daily,
        data_type: dtype,
        symbol: "BTCUSDT".into(),
        period: "2024-01-15".into(),
        interval: None,
    };
    assert!(url(base(DataType::AggTrades)).contains("/aggTrades/"));
    assert!(url(base(DataType::Trades)).contains("/trades/"));
    assert!(url(base(DataType::BookDepth)).contains("/bookDepth/"));
    assert!(url(base(DataType::BookTicker)).contains("/bookTicker/"));
    assert!(url(base(DataType::Metrics)).contains("/metrics/"));
}

#[test]
fn kline_types_require_interval_in_path() {
    for dtype in [
        DataType::Klines,
        DataType::PremiumIndexKlines,
        DataType::MarkPriceKlines,
        DataType::IndexPriceKlines,
    ] {
        let file = VisionFile {
            market: Market::Um,
            frequency: Frequency::Daily,
            data_type: dtype,
            symbol: "BTCUSDT".into(),
            period: "2024-01-15".into(),
            interval: Some("1h".into()),
        };
        let u = url(file);
        assert!(u.contains("/1h/"), "{u}");
    }
}

#[test]
fn cm_liquidation_snapshot() {
    let file = VisionFile {
        market: Market::Cm,
        frequency: Frequency::Daily,
        data_type: DataType::LiquidationSnapshot,
        symbol: "BTCUSD_PERP".into(),
        period: "2024-01-15".into(),
        interval: None,
    };
    assert!(url(file).contains("/liquidationSnapshot/"));
}

#[test]
fn spot_and_option_paths() {
    let spot = VisionFile {
        market: Market::Spot,
        frequency: Frequency::Daily,
        data_type: DataType::AggTrades,
        symbol: "BTCUSDT".into(),
        period: "2024-01-15".into(),
        interval: None,
    };
    assert!(url(spot).contains("/spot/daily/"));

    let option = VisionFile {
        market: Market::Option,
        frequency: Frequency::Daily,
        data_type: DataType::BvolIndex,
        symbol: "BTCUSDT".into(),
        period: "2024-01-15".into(),
        interval: None,
    };
    assert!(url(option).contains("/BVOLIndex/"));
}

#[test]
fn monthly_filename_uses_year_month() {
    let file = VisionFile {
        market: Market::Um,
        frequency: Frequency::Monthly,
        data_type: DataType::AggTrades,
        symbol: "ETHUSDT".into(),
        period: "2023-01".into(),
        interval: None,
    };
    assert!(url(file).ends_with("ETHUSDT-aggTrades-2023-01.zip"));
}
