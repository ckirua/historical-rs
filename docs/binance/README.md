# Binance Vision

## Scope

The Binance product is the `binance_vision` ClickHouse database and a matching Vision-shaped S3 parquet archive. The declared data types are `trades`, `klines`, `book_depth`, and `bvol`; the default direct ClickHouse window is about 18 months. The legacy `binance.agg_trades` path is not the product.

## Current status and deployment

- [Status and evidence](STATUS.md) — UM trades completed for the recorded run; klines and the S3 product archive were interrupted; book depth and BVOL remain resume-later/not started after the interruption.
- [Deploy and operate on a future host](DEPLOY.md) — setup, schema checks, resume-safe jobs, and optional cron. This cleaned VPS has no live ingest, tmux, or cron; do not restart work here.
- [Product goal](GOAL.md) — all four table gates must be green before calling the product complete.

## Key commands

Run from the repository root on the designated future host:

```bash
./scripts/binance/install.sh --dir .
./scripts/binance/loop-status.sh

# Direct Vision → ClickHouse product tables
./scripts/binance/clickhouse-direct-trades.sh
./scripts/binance/clickhouse-direct-klines.sh --resume
./scripts/binance/clickhouse-direct-book-depth.sh
./scripts/binance/clickhouse-direct-bvol.sh

# Vision → S3 product archive
DATA_TYPES=trades,klines,bookDepth,BVOLIndex ./scripts/binance/sync-s3.sh
```

The direct jobs resume through `ingest_log`; S3 sync resumes from existing objects. Use the deployment guide for environment, host, and monitoring details.

## Boundaries

Do not write to `binance.agg_trades`, use the legacy S3-to-aggTrades scripts, or describe a trades-only run as completion. Klines, book depth, BVOL, and interrupted S3 coverage remain resume-later. Daily cron is for a future host only, after the initial backfill is green.
