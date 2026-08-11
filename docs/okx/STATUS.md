# Status — historical-rs (okx)

Ops host posture (2026-08-07). Mark boxes only with evidence.

**Host posture:** Daily incremental cron **installed** (02:00 local / Europe/Berlin). Catch-up resume runs supervised via `sync-s3-bg.sh` / nohup. S3 manifest GET/PUT 400/504 noise may still appear; mirror falls back to HEAD checks and does not block cron.

## PRIMARY (S3 only — no DB)

- [x] CDN URL builder + listing client (trades, candlesticks, funding, borrowrates)
- [x] Zip → CSV parse → parquet.zstd export
- [x] Local + S3 mirror with resume manifest (`historical/` prefix on `mercat-okx`)
- [x] Instrument discovery cache (SPOT/SWAP)
- [x] `scripts/okx/sync-s3.sh` + `sync-s3-bg.sh` full backfill helpers
- [x] `scripts/okx/install.sh` + `install-cron.sh` daily incremental
- [ ] Full backfill completes for all four types — **in progress / resume** (candlesticks + funding gaps; trades mostly OK; borrowrates often skipped)
- [x] Daily cron installed — `0 2 * * *` → `/var/log/okx-historical-s3-daily.log` (yesterday UTC window)
- [ ] Order books (L2) — deferred
- [ ] FUTURES / OPTION markets — deferred
- [ ] ClickHouse ingest — out of scope / resume-later on a dedicated host

## Ops

| Item | Command / path |
|------|----------------|
| Daily cron | `0 2 * * *` host-local → `./scripts/okx/sync-s3.sh` (FROM=TO=yesterday UTC) |
| Cron log | `/var/log/okx-historical-s3-daily.log` |
| Catch-up / resume | `LOG=... PID_FILE=... ./scripts/okx/sync-s3-bg.sh` |
| Follow catch-up | `tail -f /var/log/okx-historical-s3-catchup.log` (or `/root/logs/okx/`) |
| Loop snapshot | `./scripts/okx/loop-status.sh` |
| Cron install | `./scripts/okx/install-cron.sh` |
| S3 status | `./target/release/okx-historical mirror status --s3` |

## Per-iteration checklist

1. Quote unfinished PRIMARY boxes
2. Run `./scripts/okx/loop-status.sh`
3. Do **not** kill Bybit / Binance / Deribit jobs
4. Prefer `--resume`; note manifest 400/504 without blocking ops
5. Mark STATUS boxes only with evidence
