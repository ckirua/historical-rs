-- Binance Vision ClickHouse schema (v2 — optimized for tick queries + 3y hot tier)
-- Apply: clickhouse-client --multiquery < sql/clickhouse/001_schema.sql

CREATE DATABASE IF NOT EXISTS binance;

CREATE TABLE IF NOT EXISTS binance.agg_trades
(
    market LowCardinality(String),
    symbol LowCardinality(String),
    ts DateTime64(3) MATERIALIZED fromUnixTimestamp64Milli(timestamp_ms),
    agg_trade_id UInt64 CODEC(T64, ZSTD(1)),
    price Float64 CODEC(Gorilla, ZSTD(1)),
    qty Float64 CODEC(Gorilla, ZSTD(1)),
    first_trade_id UInt64 CODEC(T64, ZSTD(1)),
    last_trade_id UInt64 CODEC(T64, ZSTD(1)),
    timestamp_ms Int64 CODEC(Delta(8), ZSTD(1)),
    is_buyer_maker UInt8 CODEC(ZSTD(1)),
    ingested_at DateTime DEFAULT now() CODEC(Delta(4), ZSTD(1)),
    INDEX idx_symbol symbol TYPE bloom_filter(0.01) GRANULARITY 4,
    PROJECTION proj_market_symbol_time
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
    )
)
ENGINE = MergeTree
PARTITION BY toYYYYMM(ts)
ORDER BY (market, symbol, timestamp_ms, agg_trade_id)
TTL ts + INTERVAL 1095 DAY
SETTINGS index_granularity = 8192, ttl_only_drop_parts = 1;

CREATE TABLE IF NOT EXISTS binance.ingest_log
(
    s3_key String,
    rows UInt64,
    status LowCardinality(String),
    error Nullable(String),
    ingested_at DateTime DEFAULT now() CODEC(Delta(4), ZSTD(1))
)
ENGINE = ReplacingMergeTree(ingested_at)
ORDER BY (s3_key)
TTL ingested_at + INTERVAL 180 DAY
SETTINGS index_granularity = 8192;
