use chrono::NaiveDate;
use tracing::debug;

use crate::client::DeribitHttpClient;
use crate::error::Result;
use crate::model::VolatilityIndexCandle;
use crate::trades::utc_day_bounds_ms;

/// Fetch volatility-index OHLC candles for one UTC day, following `continuation`.
pub async fn fetch_volatility_index_for_day(
    client: &DeribitHttpClient,
    currency: &str,
    date: NaiveDate,
    resolution: &str,
) -> Result<Vec<VolatilityIndexCandle>> {
    let (start_ms, end_ms) = utc_day_bounds_ms(date);
    let mut candles = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let mut page_end = end_ms;
    // Guard against pathological continuation loops.
    for _ in 0..64 {
        let page = client
            .get_volatility_index_page(currency, start_ms, page_end, resolution)
            .await?;
        for candle in page.candles {
            if candle.timestamp_ms < start_ms || candle.timestamp_ms > end_ms {
                continue;
            }
            if seen.insert(candle.timestamp_ms) {
                candles.push(candle);
            }
        }
        match page.continuation {
            Some(next_end) if next_end < page_end && next_end >= start_ms => {
                page_end = next_end;
            }
            _ => break,
        }
    }
    candles.sort_by_key(|candle| candle.timestamp_ms);
    debug!(
        %currency,
        %date,
        %resolution,
        rows = candles.len(),
        "fetched Deribit volatility index for day"
    );
    Ok(candles)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn day_bounds_reexported_via_trades() {
        let date = NaiveDate::from_ymd_opt(2026, 8, 5).unwrap();
        let (start, end) = utc_day_bounds_ms(date);
        assert!(end > start);
    }
}
