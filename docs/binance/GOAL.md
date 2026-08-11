# Goal lock — historical-rs Binance ClickHouse

Read this at the start of every iteration. If the work does not advance an unfinished PRIMARY table, stop.

```
PRIMARY DELIVERABLE — database binance_vision (NOT binance):

  1. trades      ← Vision daily trades     (UM all symbols first; then CM/SPOT)
  2. klines      ← Vision daily klines     (UM all symbols; intervals: 1m,1h,1d at minimum)
  3. book_depth  ← Vision bookDepth        (UM all symbols)
  4. bvol        ← Vision option BVOLIndex

PATH (ClickHouse): Vision HTTP → parse → ClickHouse direct insert (resume via binance_vision.ingest_log)
PATH (S3 archive): Vision HTTP → parquet.zstd → S3 under Vision-shaped keys for the SAME four types
WINDOW default: ~18 months (CLICKHOUSE_FETCH_SINCE_DAYS=548); S3 may keep longer
SCHEMA: MergeTree (NOT ReplacingMergeTree) — append-only + ingest_log dedupe

FORBIDDEN until all four table gates are green:
  - Writing more rows to binance.agg_trades
  - Treating aggTrades as the ClickHouse or S3 product
  - Inventing new DB names when binance_vision.* already exists
  - Shipping only trades and calling the job done
  - Running S3 sync for aggTrades when the product types are unfinished
```

## kind mapping

| Market | `kind` |
|--------|--------|
| Spot | `SPOT` |
| Um | `UM` |
| Cm | `CM` |

## Per-iteration checklist

1. Quote this PRIMARY block
2. Which table this iteration advanced
3. Gate SQL/CLI evidence
4. Confirm zero writes to `binance.agg_trades`
5. Mark done only if that iteration’s gate passes
