# ClickHouse schema (Binance)

## Product path (`binance_vision`)

| Object | Purpose |
|--------|---------|
| `trades` | Vision daily trades (UM first) |
| `klines` | Vision daily klines |
| `book_depth` | Vision bookDepth |
| `bvol` | Vision option BVOLIndex |
| `ingest_log` | Direct/S3 ingest resume ledger |

Apply MergeTree DDL:

```bash
clickhouse-client --multiquery < sql/clickhouse/010_binance_vision_merge_tree.sql
```

Or let the CLI ensure schema on direct ingest. Resume interrupted fills with `--resume` on a future host — do not start long backfills on the cleaned VPS.

## Legacy path (`binance.agg_trades`) — avoid

| Object | Purpose |
|--------|---------|
| `agg_trades` | Hot tier — UM aggTrades ticks (~3y TTL); **not** the product |
| `ingest_log` | Legacy S3→agg_trades resume ledger |
| `v_agg_trades` / `v_ingest_status` | Legacy views |

```bash
clickhouse-client --multiquery < sql/clickhouse/001_schema.sql
clickhouse-client --multiquery < sql/clickhouse/002_optimize.sql   # existing installs
clickhouse-client --multiquery < sql/clickhouse/003_views.sql
```

Do **not** treat aggTrades as the ClickHouse or S3 product. See [docs/binance/GOAL.md](../../docs/binance/GOAL.md).

## Queries

See [queries.sql](queries.sql) for legacy aggTrades examples. Prefer querying `binance_vision.*` for product work.
