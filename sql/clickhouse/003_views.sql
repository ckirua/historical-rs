-- Query-friendly views (cheap at read time; no extra storage for rollups).

CREATE OR REPLACE VIEW binance.v_agg_trades AS
SELECT
    market,
    symbol,
    ts,
    timestamp_ms,
    agg_trade_id,
    price,
    qty,
    price * qty AS notional,
    first_trade_id,
    last_trade_id,
    is_buyer_maker,
    if(is_buyer_maker = 1, 'sell', 'buy') AS aggressor_side
FROM binance.agg_trades;

-- Latest ingest status per S3 object (works with plain or ReplacingMergeTree logs).
CREATE OR REPLACE VIEW binance.v_ingest_status AS
SELECT
    s3_key,
    argMax(status, ingested_at) AS status,
    argMax(rows, ingested_at) AS rows,
    argMax(error, ingested_at) AS error,
    max(ingested_at) AS last_ingested_at
FROM binance.ingest_log
GROUP BY s3_key;
