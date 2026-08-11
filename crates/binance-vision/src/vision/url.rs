//! Binance Vision URL builder (ports legacy Cython BinanceVisionHTTPAPI).

use crate::config::VISION_BASE_URL;
use crate::model::VisionFile;

/// Build download URLs for Binance Vision archives.
#[derive(Debug, Clone)]
pub struct VisionUrl {
    base_url: String,
}

impl Default for VisionUrl {
    fn default() -> Self {
        Self::new(VISION_BASE_URL)
    }
}

impl VisionUrl {
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into().trim_end_matches('/').to_string(),
        }
    }

    pub fn build(&self, file: &VisionFile) -> String {
        let market = file.market.path_segment();
        let freq = file.frequency.as_str();
        let dtype = file.data_type.path_segment();
        let label = file.data_type.file_label();
        let symbol = &file.symbol;

        let filename = if file.data_type.requires_interval() {
            let interval = file
                .interval
                .as_deref()
                .expect("interval required for kline data types");
            format!("{symbol}-{interval}-{}", file.period)
        } else {
            format!("{symbol}-{label}-{}.zip", file.period)
        };

        let filename = if file.data_type.requires_interval() {
            format!("{filename}.zip")
        } else {
            filename
        };

        if file.data_type.requires_interval() {
            let interval = file.interval.as_deref().unwrap();
            format!(
                "{base}/{market}/{freq}/{dtype}/{symbol}/{interval}/{filename}",
                base = self.base_url
            )
        } else {
            format!(
                "{base}/{market}/{freq}/{dtype}/{symbol}/{filename}",
                base = self.base_url
            )
        }
    }

    pub fn checksum_url(&self, zip_url: &str) -> String {
        format!("{zip_url}.CHECKSUM")
    }
}

/// Archive filename stem (no extension), matching Binance Vision zip naming.
pub fn vision_archive_stem(file: &VisionFile) -> String {
    let label = file.data_type.file_label();
    let symbol = &file.symbol;
    if file.data_type.requires_interval() {
        let interval = file
            .interval
            .as_deref()
            .expect("interval required for kline data types");
        format!("{symbol}-{interval}-{}", file.period)
    } else {
        format!("{symbol}-{label}-{}", file.period)
    }
}

/// S3/local object key mirroring Binance Vision layout with `.parquet.zstd` extension.
pub fn vision_storage_key(file: &VisionFile) -> String {
    let market = file.market.path_segment();
    let freq = file.frequency.as_str();
    let dtype = file.data_type.path_segment();
    let symbol = &file.symbol;
    let filename = format!("{}.parquet.zstd", vision_archive_stem(file));

    if file.data_type.requires_interval() {
        let interval = file.interval.as_deref().unwrap();
        format!("data/{market}/{freq}/{dtype}/{symbol}/{interval}/{filename}")
    } else {
        format!("data/{market}/{freq}/{dtype}/{symbol}/{filename}")
    }
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;

    use super::*;
    use crate::model::{DataType, Frequency, Market};

    #[test]
    fn futures_um_daily_aggtrades_matches_legacy_api() {
        let builder = VisionUrl::default();
        let file = VisionFile {
            market: Market::Um,
            frequency: Frequency::Daily,
            data_type: DataType::AggTrades,
            symbol: "BTCUSDT".into(),
            period: "2024-01-15".into(),
            interval: None,
        };
        assert_eq!(
            builder.build(&file),
            "https://data.binance.vision/data/futures/um/daily/aggTrades/BTCUSDT/BTCUSDT-aggTrades-2024-01-15.zip"
        );
    }

    #[test]
    fn spot_daily_klines_includes_interval() {
        let builder = VisionUrl::default();
        let file = VisionFile {
            market: Market::Spot,
            frequency: Frequency::Daily,
            data_type: DataType::Klines,
            symbol: "BTCUSDT".into(),
            period: "2024-01-15".into(),
            interval: Some("1h".into()),
        };
        assert_eq!(
            builder.build(&file),
            "https://data.binance.vision/data/spot/daily/klines/BTCUSDT/1h/BTCUSDT-1h-2024-01-15.zip"
        );
    }

    #[test]
    fn monthly_aggtrades_uses_month_period() {
        let builder = VisionUrl::default();
        let file = VisionFile {
            market: Market::Um,
            frequency: Frequency::Monthly,
            data_type: DataType::AggTrades,
            symbol: "ETHUSDT".into(),
            period: "2023-01".into(),
            interval: None,
        };
        assert_eq!(
            builder.build(&file),
            "https://data.binance.vision/data/futures/um/monthly/aggTrades/ETHUSDT/ETHUSDT-aggTrades-2023-01.zip"
        );
    }

    #[test]
    fn option_bvolindex_path() {
        let builder = VisionUrl::default();
        let file = VisionFile {
            market: Market::Option,
            frequency: Frequency::Daily,
            data_type: DataType::BvolIndex,
            symbol: "BTCUSDT".into(),
            period: "2024-01-15".into(),
            interval: None,
        };
        assert_eq!(
            builder.build(&file),
            "https://data.binance.vision/data/option/daily/BVOLIndex/BTCUSDT/BTCUSDT-BVOLIndex-2024-01-15.zip"
        );
    }

    #[test]
    fn vision_storage_key_matches_existing_bucket_layout() {
        let file = VisionFile {
            market: Market::Um,
            frequency: Frequency::Daily,
            data_type: DataType::AggTrades,
            symbol: "BTCUSDT".into(),
            period: "2024-01-15".into(),
            interval: None,
        };
        assert_eq!(
            vision_storage_key(&file),
            "data/futures/um/daily/aggTrades/BTCUSDT/BTCUSDT-aggTrades-2024-01-15.parquet.zstd"
        );
    }

    #[test]
    fn daily_from_naive_date_helper() {
        let file = VisionFile::daily(
            Market::Um,
            DataType::AggTrades,
            "BTCUSDT",
            NaiveDate::from_ymd_opt(2024, 1, 15).unwrap(),
            None,
        );
        assert_eq!(file.period, "2024-01-15");
    }
}
