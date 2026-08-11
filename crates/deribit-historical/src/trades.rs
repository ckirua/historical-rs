use chrono::{DateTime, Duration, NaiveDate, TimeZone, Utc};
use tracing::debug;

use crate::client::DeribitHttpClient;
use crate::error::Result;
use crate::model::TradeRow;

/// Inclusive UTC day bounds as `[start_ms, end_ms]` where `end_ms` is the last
/// millisecond of the day (23:59:59.999).
pub fn utc_day_bounds_ms(date: NaiveDate) -> (i64, i64) {
    let start = Utc.from_utc_datetime(&date.and_hms_opt(0, 0, 0).expect("midnight"));
    let end = start + Duration::days(1) - Duration::milliseconds(1);
    (start.timestamp_millis(), end.timestamp_millis())
}

pub fn expected_dates(start: NaiveDate, end: NaiveDate) -> impl Iterator<Item = NaiveDate> {
    std::iter::successors(Some(start), move |date| {
        let next = *date + Duration::days(1);
        (next <= end).then_some(next)
    })
}

/// Fetch all public trades for one instrument on one UTC day.
///
/// Strategy: first page via `get_last_trades_by_instrument_and_time`, then walk
/// `start_seq` until the day ends or `has_more` is false. Deduplicates by
/// `trade_seq`.
pub async fn fetch_trades_for_day(
    client: &DeribitHttpClient,
    instrument_name: &str,
    currency: &str,
    date: NaiveDate,
) -> Result<Vec<TradeRow>> {
    let (start_ms, end_ms) = utc_day_bounds_ms(date);
    let count = client.config().trade_page_count;
    let mut rows = Vec::new();
    let mut seen = std::collections::HashSet::new();

    let first = client
        .get_trades_page_by_time(instrument_name, start_ms, end_ms, count)
        .await?;
    let mut has_more = first.has_more;
    let mut next_seq = append_day_trades(
        &mut rows,
        &mut seen,
        first.trades,
        currency,
        start_ms,
        end_ms,
    );

    while has_more {
        let Some(start_seq) = next_seq else {
            break;
        };
        let page = client
            .get_trades_page_by_seq(instrument_name, start_seq, count)
            .await?;
        has_more = page.has_more;
        if page.trades.is_empty() {
            break;
        }
        // Entire page is after the UTC day — stop.
        if page.trades.iter().all(|trade| trade.timestamp_ms > end_ms) {
            break;
        }
        next_seq = append_day_trades(
            &mut rows,
            &mut seen,
            page.trades,
            currency,
            start_ms,
            end_ms,
        );
        // Page straddled the boundary (some kept, later discarded) and no further
        // in-range seq is expected once the last kept trade is at/after end_ms.
        if next_seq.is_none() {
            break;
        }
    }

    debug!(
        %instrument_name,
        %date,
        rows = rows.len(),
        "fetched Deribit trades for day"
    );
    Ok(rows)
}

fn append_day_trades(
    rows: &mut Vec<TradeRow>,
    seen: &mut std::collections::HashSet<i64>,
    trades: Vec<TradeRow>,
    currency: &str,
    start_ms: i64,
    end_ms: i64,
) -> Option<i64> {
    let mut max_seq = None;
    for mut trade in trades {
        max_seq = Some(max_seq.map_or(trade.trade_seq, |value: i64| value.max(trade.trade_seq)));
        if trade.timestamp_ms < start_ms || trade.timestamp_ms > end_ms {
            continue;
        }
        if !seen.insert(trade.trade_seq) {
            continue;
        }
        trade.currency = currency.to_ascii_uppercase();
        rows.push(trade);
    }
    max_seq.map(|seq| seq + 1)
}

/// Format a millisecond timestamp as RFC3339 UTC for diagnostics.
pub fn ms_to_rfc3339(ms: i64) -> String {
    DateTime::<Utc>::from_timestamp_millis(ms)
        .map(|dt| dt.to_rfc3339())
        .unwrap_or_else(|| ms.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::TradeRow;

    fn trade(seq: i64, ts: i64) -> TradeRow {
        TradeRow {
            instrument_name: "BTC-PERPETUAL".into(),
            currency: "BTC".into(),
            trade_id: seq.to_string(),
            trade_seq: seq,
            timestamp_ms: ts,
            direction: "buy".into(),
            price: 1.0,
            amount: 10.0,
            contracts: None,
            index_price: None,
            mark_price: None,
            tick_direction: None,
        }
    }

    #[test]
    fn day_bounds_cover_full_utc_day() {
        let date = NaiveDate::from_ymd_opt(2024, 1, 1).unwrap();
        let (start, end) = utc_day_bounds_ms(date);
        assert_eq!(start, 1_704_067_200_000);
        assert_eq!(end, 1_704_153_599_999);
    }

    #[test]
    fn append_dedupes_and_filters() {
        let (start, end) = utc_day_bounds_ms(NaiveDate::from_ymd_opt(2024, 1, 1).unwrap());
        let mut rows = Vec::new();
        let mut seen = std::collections::HashSet::new();
        let next = append_day_trades(
            &mut rows,
            &mut seen,
            vec![
                trade(1, start),
                trade(1, start),
                trade(2, end + 1),
                trade(3, end),
            ],
            "btc",
            start,
            end,
        );
        assert_eq!(next, Some(4));
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].trade_seq, 1);
        assert_eq!(rows[1].trade_seq, 3);
        assert_eq!(rows[0].currency, "BTC");
    }
}
