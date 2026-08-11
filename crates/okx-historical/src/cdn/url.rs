//! OKX CDN URL builder.

use crate::config::CDN_BASE_URL;
use crate::model::{DataType, Frequency, OkxFile};

/// Build download URLs for OKX historical CDN archives.
#[derive(Debug, Clone)]
pub struct CdnUrl {
    base_url: String,
}

impl Default for CdnUrl {
    fn default() -> Self {
        Self::new(CDN_BASE_URL)
    }
}

impl CdnUrl {
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into().trim_end_matches('/').to_string(),
        }
    }

    pub fn build(&self, file: &OkxFile) -> String {
        let freq = file.frequency.as_str();
        let period_compact = period_compact(&file.period, file.frequency);
        let filename = archive_filename(file);
        let cdn_dir = match file.data_type {
            DataType::Funding if file.symbol.eq_ignore_ascii_case("ALL") => "swaprates",
            DataType::Funding => "swaprate",
            other => other.cdn_dir(),
        };
        format!(
            "{base}/{cdn_dir}/{freq}/{period_compact}/{filename}",
            base = self.base_url
        )
    }
}

/// Archive zip filename on CDN.
pub fn archive_filename(file: &OkxFile) -> String {
    match file.data_type {
        DataType::Trades => format!("{}-trades-{}.zip", file.symbol, file.period),
        DataType::Candlesticks => format!("{}-candlesticks-{}.zip", file.symbol, file.period),
        DataType::Funding if file.symbol.eq_ignore_ascii_case("ALL") => {
            format!("allswap-fundingrates-{}.zip", file.period)
        }
        DataType::Funding => format!("{}-swaprate-{}.zip", file.symbol, file.period),
        DataType::BorrowRates => format!("{}-borrowrates-{}.zip", file.symbol, file.period),
    }
}

/// Storage key stem (no extension).
pub fn archive_stem(file: &OkxFile) -> String {
    archive_filename(file).trim_end_matches(".zip").to_string()
}

/// Prefix for objects inside the `mercat-okx` bucket.
pub const STORAGE_PREFIX: &str = "historical";

/// S3/local object key with `.parquet.zstd` extension.
pub fn storage_key(file: &OkxFile) -> String {
    let market = file.market.path_segment();
    let freq = file.frequency.as_str();
    let dtype = file.data_type.path_segment();
    let symbol = &file.symbol;
    let filename = format!("{}.parquet.zstd", archive_stem(file));
    format!("{STORAGE_PREFIX}/data/{market}/{freq}/{dtype}/{symbol}/{filename}")
}

fn period_compact(period: &str, frequency: Frequency) -> String {
    match frequency {
        Frequency::Daily => period.replace('-', ""),
        Frequency::Monthly => period.replace('-', ""),
    }
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;

    use super::*;
    use crate::model::{DataType, Frequency, Market, OkxFile};

    #[test]
    fn spot_trades_daily_url() {
        let builder = CdnUrl::default();
        let file = OkxFile::daily(
            Market::Spot,
            DataType::Trades,
            "BTC-USDT",
            NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
        );
        assert_eq!(
            builder.build(&file),
            "https://static.okx.com/cdn/okex/traderecords/trades/daily/20240101/BTC-USDT-trades-2024-01-01.zip"
        );
    }

    #[test]
    fn swap_candles_daily_url() {
        let builder = CdnUrl::default();
        let file = OkxFile::daily(
            Market::Swap,
            DataType::Candlesticks,
            "BTC-USDT-SWAP",
            NaiveDate::from_ymd_opt(2024, 7, 1).unwrap(),
        );
        assert_eq!(
            builder.build(&file),
            "https://static.okx.com/cdn/okex/traderecords/candlesticks/daily/20240701/BTC-USDT-SWAP-candlesticks-2024-07-01.zip"
        );
    }

    #[test]
    fn allswap_funding_url() {
        let builder = CdnUrl::default();
        let file = OkxFile {
            market: Market::Swap,
            frequency: Frequency::Daily,
            data_type: DataType::Funding,
            symbol: "ALL".into(),
            period: "2024-01-01".into(),
        };
        assert_eq!(
            builder.build(&file),
            "https://static.okx.com/cdn/okex/traderecords/swaprates/daily/20240101/allswap-fundingrates-2024-01-01.zip"
        );
    }

    #[test]
    fn borrow_monthly_url() {
        let builder = CdnUrl::default();
        let file = OkxFile::monthly(Market::Spot, DataType::BorrowRates, "BTC", 2024, 7);
        assert_eq!(
            builder.build(&file),
            "https://static.okx.com/cdn/okex/traderecords/borrowrates/monthly/202407/BTC-borrowrates-2024-07.zip"
        );
    }

    #[test]
    fn storage_key_layout() {
        let file = OkxFile::daily(
            Market::Spot,
            DataType::Trades,
            "BTC-USDT",
            NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
        );
        assert_eq!(
            storage_key(&file),
            "historical/data/spot/daily/trades/BTC-USDT/BTC-USDT-trades-2024-01-01.parquet.zstd"
        );
    }
}
