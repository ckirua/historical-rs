# Status

**GOAL:** See [GOAL.md](GOAL.md) — fill **all** `binance_vision` tables (trades, klines, book_depth, bvol). Forbidden: more `binance.agg_trades` writes.

Last updated: 2026-08-03

**Host posture:** Historical S3 sync is treated as caught up for incremental ops. Daily cron via `scripts/binance/install-cron.sh` (rolling last 7 days → yesterday, `--resume`). Do **not** launch full historical backfills from cron. Interrupted CH table fills (klines/book_depth/bvol beyond daily) remain catch-up via explicit scripts if needed.

| Iteration | Status | Evidence |
|-----------|--------|----------|
| 0 Stop agg writers + GOAL.md | **done** | Killed S3→agg_trades ingest; [GOAL.md](GOAL.md) written |
| 1 Schema MergeTree migrate | **done** | All four tables `MergeTree` |
| 2 CH default `binance_vision` | **done** | `CLICKHOUSE_VISION_DATABASE` default; ensure_schema uses `010_*.sql` |
| 3 trades direct | **done** (UM) | finished 2026-07-17: total=246778 done=118189 skipped=128589 failed=0; ~931 GiB (at time of run) |
| 4 klines | **resume-later** | Interrupted mid-chain (was 1m then 1h/1d); restart with `scripts/binance/clickhouse-direct-klines.sh --resume` on future host |
| 5 book_depth | **resume-later** | Not started after klines; `scripts/binance/clickhouse-direct-book-depth.sh` |
| 6 bvol | **resume-later** | Not started after book_depth; `scripts/binance/clickhouse-direct-bvol.sh` |
| 7 S3 product archive | **resume-later** | Interrupted 2026-07-18; `DATA_TYPES=trades,klines,bookDepth,BVOLIndex ./scripts/binance/sync-s3.sh` |
| 8 docs/ops prod-freeze | **done** | [DEPLOY.md](DEPLOY.md) / [LOOP.md](LOOP.md) post-clean; paths under `historical-rs` + `scripts/binance/` |

## Ops

→ **[DEPLOY.md](DEPLOY.md)** — resume-safe start, SQL checks, daily incremental cron (`./scripts/binance/install-cron.sh`).

```bash
./scripts/binance/loop-status.sh
df -h /var/lib/clickhouse   # if CH is local on that host
```

## Notes

- Smoke trades (historical): BTCUSDT 2026-07-10→12 → ~5.6M rows
- Dropped `binance.agg_trades` (~1.1 TiB) on 2026-07-17
- Chain waiter bug: `pgrep -f` matched its own cmdline — fixed by starting klines directly
- Monorepo paths: use `scripts/binance/...` from the `historical-rs` root (not legacy `agent-*` checkouts)
