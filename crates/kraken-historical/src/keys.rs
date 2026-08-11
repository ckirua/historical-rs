//! Storage key layout for local datasets and S3 objects.

use std::path::{Path, PathBuf};

use crate::model::OutputUnit;

/// Object key under the bucket (always starts with `historical/`).
pub fn dataset_object_key(unit: &OutputUnit) -> String {
    let source = unit.source.path_segment();
    match unit.data_type {
        crate::model::DataType::Trades => {
            format!("historical/data/trades/{source}/{}.parquet.zstd", unit.pair)
        }
        crate::model::DataType::Ohlcvt => {
            let interval = unit.interval.as_deref().unwrap_or("unknown");
            format!(
                "historical/data/ohlcvt/{source}/{}/{interval}.parquet.zstd",
                unit.pair
            )
        }
    }
}

pub fn local_dataset_path(base: &Path, unit: &OutputUnit) -> PathBuf {
    base.join(dataset_object_key(unit))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{OutputUnit, Source};

    #[test]
    fn trades_complete_key() {
        let unit = OutputUnit::trades(Source::Complete, "XBTUSD");
        assert_eq!(
            dataset_object_key(&unit),
            "historical/data/trades/complete/XBTUSD.parquet.zstd"
        );
    }

    #[test]
    fn trades_quarterly_key() {
        let unit = OutputUnit::trades(
            Source::Quarterly {
                quarter: "2024Q3".into(),
            },
            "XBTUSD",
        );
        assert_eq!(
            dataset_object_key(&unit),
            "historical/data/trades/quarterly/2024Q3/XBTUSD.parquet.zstd"
        );
    }

    #[test]
    fn ohlcvt_keys() {
        let complete = OutputUnit::ohlcvt(Source::Complete, "ETHUSD", "60");
        assert_eq!(
            dataset_object_key(&complete),
            "historical/data/ohlcvt/complete/ETHUSD/60.parquet.zstd"
        );
        let quarterly = OutputUnit::ohlcvt(
            Source::Quarterly {
                quarter: "2024Q1".into(),
            },
            "ETHUSD",
            "1",
        );
        assert_eq!(
            dataset_object_key(&quarterly),
            "historical/data/ohlcvt/quarterly/2024Q1/ETHUSD/1.parquet.zstd"
        );
    }
}
