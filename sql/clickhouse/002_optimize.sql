-- Idempotent upgrades for existing installs (safe to re-run).
-- Runs in background where noted — does not block ingest.

-- Hot-tier retention (3 years of tick data in CH; S3 keeps full history).
ALTER TABLE binance.agg_trades
    MODIFY TTL fromUnixTimestamp64Milli(timestamp_ms) + INTERVAL 1095 DAY;

ALTER TABLE binance.agg_trades
    ADD COLUMN IF NOT EXISTS ts DateTime64(3) MATERIALIZED fromUnixTimestamp64Milli(timestamp_ms);

ALTER TABLE binance.agg_trades
    ADD INDEX IF NOT EXISTS idx_symbol symbol TYPE bloom_filter(0.01) GRANULARITY 4;

ALTER TABLE binance.agg_trades
    ADD PROJECTION IF NOT EXISTS proj_market_symbol_time
    (
        SELECT
            market,
            symbol,
            timestamp_ms,
            agg_trade_id,
            price,
            qty,
            first_trade_id,
            last_trade_id,
            is_buyer_maker
        ORDER BY market, symbol, timestamp_ms, agg_trade_id
    );

-- Materialize projection asynchronously (OK on large tables).
ALTER TABLE binance.agg_trades
    MATERIALIZE PROJECTION IF EXISTS proj_market_symbol_time
    SETTINGS mutations_sync = 0;

-- Column codecs (rewrite parts in background).
ALTER TABLE binance.agg_trades
    MODIFY COLUMN agg_trade_id UInt64 CODEC(T64, ZSTD(1)),
    MODIFY COLUMN price Float64 CODEC(Gorilla, ZSTD(1)),
    MODIFY COLUMN qty Float64 CODEC(Gorilla, ZSTD(1)),
    MODIFY COLUMN first_trade_id UInt64 CODEC(T64, ZSTD(1)),
    MODIFY COLUMN last_trade_id UInt64 CODEC(T64, ZSTD(1)),
    MODIFY COLUMN timestamp_ms Int64 CODEC(Delta(8), ZSTD(1));

ALTER TABLE binance.ingest_log
    MODIFY TTL ingested_at + INTERVAL 180 DAY;
