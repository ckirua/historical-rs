# Roadmap

## Phases 0–10 (complete)

| Phase | Description | Gate |
|-------|-------------|------|
| 0 | Cargo workspace scaffold | `cargo check -p binance-vision-cli` |
| 1 | URL builder + HTTP client | `cargo test -p binance-vision vision::` |
| 2 | aggTrades parser + zip extract | `cargo test -p binance-vision parse::` |
| 3 | parquet.zstd export | `cargo test -p binance-vision export::` |
| 4 | Local mirror + manifest | mirror 7d × 2 symbols, resume skips |
| 5 | S3 upload | `mirror s3` smoke (needs credentials) |
| 6 | Symbol discovery | `symbols refresh` returns >0 |
| 7 | All old-API data types + monthly | URL tests for each type |
| 8 | CHECKSUM verification | `--verify-checksum` on fetch |
| 9 | ClickHouse ingest (local) | `clickhouse ingest` after schema |
| 10 | Ops hardening | install.sh + DEPLOY.md + README |

## Phases 11–12 (legacy production path, complete)

| Phase | Description | Gate |
|-------|-------------|------|
| 11 | UM daily aggTrades backfill with S3 resume | Historical backfill completed |
| 12 | S3 → `binance.agg_trades` ingestion with `ingest_log` resume | Existing parser, Parquet schema, CLI, and scripts remain compatibility-tested |

The aggTrades implementation remains supported because existing production jobs and archives depend on it. It is not the active backfill target; do not start additional writers unless an existing production job requires recovery.

## Phases 13–17 (active)

| Phase | Description | Gate |
|-------|-------------|------|
| 13 | **trades + klines** — mirror scripts, Parquet, ClickHouse schemas | `binance_vision.trades` and `binance_vision.klines` production gates |
| 14 | **Multi-market** — spot, CM, and option sync plus symbol discovery gaps | One-day S3 and ClickHouse smoke per supported market |
| 15 | **bookDepth + BVOLIndex** — typed direct ingestion and S3 archive | `binance_vision.book_depth` and `binance_vision.bvol` production gates |
| 16 | **Production ops** — daily cron, failure replay, and alerts | Seven-day unattended daily run |
| 17 | **Integration** (optional) — rollups, cursor-data-feed, shared ops with eurostat | Project-specific |

## Loop

Automated phase driver: [LOOP.md](LOOP.md)

```bash
./scripts/binance/loop-status.sh
```

See [STATUS.md](STATUS.md) for current progress.
