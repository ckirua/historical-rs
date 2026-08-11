-- ClickHouse query cookbook for binance.agg_trades
-- Replace :symbol / :market / dates as needed.

-- ---------------------------------------------------------------------------
-- Coverage / health
-- ---------------------------------------------------------------------------

-- Row counts and date range per symbol (last 3y TTL window)
SELECT
    market,
    symbol,
    count() AS trades,
    min(ts) AS first_ts,
    max(ts) AS last_ts,
    formatReadableSize(sum(price * qty)) AS notional
FROM binance.agg_trades
GROUP BY market, symbol
ORDER BY trades DESC
LIMIT 50;

-- ---------------------------------------------------------------------------
-- Time-range scans (uses ORDER BY market, symbol, timestamp_ms)
-- ---------------------------------------------------------------------------

-- Recent trades for one symbol
SELECT ts, price, qty, is_buyer_maker
FROM binance.agg_trades
WHERE market = 'um'
  AND symbol = 'BTCUSDT'
  AND ts >= now() - INTERVAL 1 DAY
ORDER BY ts DESC
LIMIT 100;

-- Point lookup by agg id (primary key suffix)
SELECT *
FROM binance.agg_trades
WHERE market = 'um'
  AND symbol = 'BTCUSDT'
  AND agg_trade_id = 1234567890;

-- ---------------------------------------------------------------------------
-- Aggregations (prefer hourly/daily buckets over raw ticks when possible)
-- ---------------------------------------------------------------------------

-- Hourly OHLCV + buy/sell volume split
SELECT
    toStartOfHour(ts) AS hour,
    argMin(price, ts) AS open,
    max(price) AS high,
    min(price) AS low,
    argMax(price, ts) AS close,
    sum(qty) AS volume,
    sumIf(qty, is_buyer_maker = 0) AS taker_buy_volume,
    sumIf(qty, is_buyer_maker = 1) AS taker_sell_volume,
    count() AS trades
FROM binance.agg_trades
WHERE market = 'um'
  AND symbol = 'BTCUSDT'
  AND ts >= today() - 30
GROUP BY hour
ORDER BY hour;

-- Daily VWAP
SELECT
    toDate(ts) AS day,
    sum(price * qty) / sum(qty) AS vwap,
    sum(qty) AS volume,
    count() AS trades
FROM binance.agg_trades
WHERE market = 'um'
  AND symbol = 'ETHUSDT'
  AND ts >= today() - 90
GROUP BY day
ORDER BY day;

-- ---------------------------------------------------------------------------
-- Ingest ledger
-- ---------------------------------------------------------------------------

-- Failed S3 objects (retry candidates)
SELECT s3_key, error, last_ingested_at
FROM binance.v_ingest_status
WHERE status = 'failed'
ORDER BY last_ingested_at DESC
LIMIT 100;
