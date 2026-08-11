# Status — historical-rs (kraken)

Post-VPS-clean freeze. **No live tmux, cron, or sync on this host.** Interrupted S3/Drive work is resume-later on a future ingest host (`--resume`).

Last updated: 2026-07-19

## PRIMARY (S3 only — no DB)

- [x] Drive discovery + streaming download (public folder scrape + confirm; API key optional)
- [x] Disk-backed ZIP → CSV parse → parquet.zstd export
- [x] Local + S3 mirror with resume manifest (`historical/` prefix on `mercat-kraken`)
- [x] CLI: discover / fetch / mirror / pairs
- [x] `scripts/kraken/sync-s3.sh` + `sync-s3-bg.sh` full backfill helpers
- [x] `scripts/kraken/install.sh` + `install-cron.sh` (for a **future** host; not enabled here)
- [x] Full backfill trades quarterly (2025Q3/Q4 + 2026Q1 → **4016** parquet objects on `mercat-kraken`; evidence 2026-07-17)
- [ ] OHLCVT + complete ZIPs — **interrupted / resume-later** (was blocked by Google Drive anonymous quota; do not retry on this cleaned VPS)
- [x] Monthly cron script validated historically (1st 08:00 UTC); **not installed on this VPS**
- [x] Quarterly smoke S3 (2025Q3 trades XBTUSD+ETHUSD → 2 objects; evidence 2026-07-17)
- [x] Resume validated (re-run smoke skipped=2; evidence 2026-07-17)
- [ ] ClickHouse ingest — out of scope (v1)
- [ ] REST gap-fill for live quarter — deferred

## Ops (future host)

| Item | Command / path |
|------|----------------|
| Foreground S3 sync | `./scripts/kraken/sync-s3.sh` |
| Background full sync | `./scripts/kraken/sync-s3-bg.sh` |
| Quota retry (when Drive clears) | `./scripts/kraken/retry-quota-s3.sh` |
| Follow log | `tail -f /tmp/kraken-historical-s3.log` |
| Loop snapshot | `./scripts/kraken/loop-status.sh` |
| Cron (future host only) | `./scripts/kraken/install-cron.sh` |
| S3 status | `./target/release/kraken-historical mirror status --s3` |

## Smoke evidence (2026-07-17)

- ZIP: `~/.data/kraken-historical/zips/trades/2025Q3_Kraken_Trading_History_Q3_2025.zip` (~788MB)
- S3: `historical/data/trades/quarterly/2025Q3/{XBTUSD,ETHUSD}.parquet.zstd`
- CLI: `total=2 done=2 skipped=0 failed=0`

## Full trades quarterly evidence (2026-07-17)

| Prefix | Objects |
|--------|---------|
| `trades/quarterly/2025Q3` | 1194 |
| `trades/quarterly/2025Q4` | 1355 |
| `trades/quarterly/2026Q1` | 1467 |
| **total** | **4016** |

Empty-pair CSVs counted as skipped (not failed). Complete trades ZIP skipped: Drive anonymous quota — resume later off this VPS.
