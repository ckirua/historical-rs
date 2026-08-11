-- binance_vision MergeTree schemas (append-only; resume via ingest_log).
-- Do NOT use ReplacingMergeTree for Vision backfills.

CREATE DATABASE IF NOT EXISTS binance_vision;

CREATE TABLE IF NOT EXISTS binance_vision.ingest_log
(
    s3_key String,
    rows UInt64,
    status LowCardinality(String),
    error Nullable(String),
    ingested_at DateTime DEFAULT now() CODEC(Delta(4), ZSTD(1))
)
ENGINE = MergeTree
ORDER BY (s3_key, ingested_at)
TTL ingested_at + INTERVAL 180 DAY
SETTINGS index_granularity = 8192;

-- New MergeTree targets (migration EXCHANGEs these into place).

CREATE TABLE IF NOT EXISTS binance_vision.trades_mt
(
    kind LowCardinality(String),
    symbol LowCardinality(String),
    id UInt64 CODEC(Delta(8), ZSTD(3)),
    price Float64 CODEC(Gorilla(8), ZSTD(3)),
    qty Float64 CODEC(Gorilla(8), ZSTD(3)),
    quote_qty Float64 CODEC(Gorilla(8), ZSTD(3)),
    time DateTime64(3) CODEC(Delta(8), ZSTD(3)),
    is_buyer_maker Bool CODEC(ZSTD(3)),
    INDEX idx_symbol symbol TYPE bloom_filter(0.01) GRANULARITY 4
)
ENGINE = MergeTree
PARTITION BY toYYYYMM(time)
ORDER BY (kind, symbol, time, id)
TTL time + INTERVAL 1095 DAY
SETTINGS index_granularity = 8192, ttl_only_drop_parts = 1;

CREATE TABLE IF NOT EXISTS binance_vision.klines_mt
(
    kind LowCardinality(String),
    symbol LowCardinality(String),
    interval LowCardinality(String),
    open_time DateTime64(3) CODEC(Delta(8), ZSTD(3)),
    close_time DateTime64(3) CODEC(Delta(8), ZSTD(3)),
    open Float64 CODEC(Gorilla(8), ZSTD(3)),
    high Float64 CODEC(Gorilla(8), ZSTD(3)),
    low Float64 CODEC(Gorilla(8), ZSTD(3)),
    close Float64 CODEC(Gorilla(8), ZSTD(3)),
    volume Float64 CODEC(Gorilla(8), ZSTD(3)),
    quote_volume Float64 CODEC(Gorilla(8), ZSTD(3)),
    count UInt32 CODEC(Delta(4), ZSTD(3)),
    taker_buy_volume Float64 CODEC(Gorilla(8), ZSTD(3)),
    taker_buy_quote_volume Float64 CODEC(Gorilla(8), ZSTD(3)),
    INDEX idx_symbol symbol TYPE bloom_filter(0.01) GRANULARITY 4
)
ENGINE = MergeTree
PARTITION BY toYYYYMM(open_time)
ORDER BY (kind, symbol, interval, open_time)
TTL open_time + INTERVAL 1095 DAY
SETTINGS index_granularity = 8192, ttl_only_drop_parts = 1;

CREATE TABLE IF NOT EXISTS binance_vision.book_depth_mt
(
    kind LowCardinality(String),
    symbol LowCardinality(String),
    timestamp DateTime CODEC(Delta(4), ZSTD(3)),
    percentage Int8 CODEC(ZSTD(3)),
    depth Float64 CODEC(Gorilla(8), ZSTD(3)),
    notional Float64 CODEC(Gorilla(8), ZSTD(3)),
    INDEX idx_symbol symbol TYPE bloom_filter(0.01) GRANULARITY 4
)
ENGINE = MergeTree
PARTITION BY toYYYYMM(timestamp)
ORDER BY (kind, symbol, timestamp, percentage)
TTL timestamp + INTERVAL 1095 DAY
SETTINGS index_granularity = 8192, ttl_only_drop_parts = 1;

CREATE TABLE IF NOT EXISTS binance_vision.bvol_mt
(
    symbol LowCardinality(String),
    base_asset LowCardinality(String),
    quote_asset LowCardinality(String),
    calc_time DateTime64(3) CODEC(Delta(8), ZSTD(3)),
    index_value Float32 CODEC(Gorilla(4), ZSTD(3)),
    INDEX idx_symbol symbol TYPE bloom_filter(0.01) GRANULARITY 4
)
ENGINE = MergeTree
PARTITION BY toYYYYMM(calc_time)
ORDER BY (symbol, calc_time)
TTL calc_time + INTERVAL 1095 DAY
SETTINGS index_granularity = 8192, ttl_only_drop_parts = 1;
