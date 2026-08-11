# Deribit historical — status

| Item | State |
|------|-------|
| Crates | `deribit-historical` + `deribit-historical-cli` |
| Product scope | BTC/ETH/USDC futures public trades + BTC/ETH DVOL volatility-index OHLC |
| Auth | Optional OAuth; daily cron runs **unauthenticated** |
| S3 trade layout | `historical/data/trade/{currency}/{instrument}/{date}.parquet.zstd` |
| S3 vol layout | `historical/data/volatility_index/{currency}/{resolution}/{date}.parquet.zstd` |
| Resume | Data-prefix listing + `--resume` |
| Cron | **Installed** — daily 22:00 local via `scripts/deribit/install-cron.sh` |
| Credentials on ops host | Unauthenticated preferred; see [RATE-LIMITS.md](./RATE-LIMITS.md) |

## Smoke checklist

- [x] Bucket `mercat-deribit` exists (shared `S3_*` credentials)
- [x] One-day `BTC-PERPETUAL` mirror → `ok` (`2019-01-02` and `2024-01-01` objects present)
- [x] Second `--resume` run → `skipped=1`
- [ ] `DERIBIT_CLIENT_*` unset on this host (intentional for daily incremental)
- [x] BTC+ETH volatility-index bounded window → S3 (`2026-08-03`..`2026-08-05`, resolution `3600`; resume skipped=6)
- [x] `BTCDVOL_USDC-26AUG26` trades bounded window → S3 (same dates; resume skipped=3)
- [x] Daily cron installed (`0 22 * * *` → `/var/log/deribit-historical-s3.log`)

## Explicit boundaries

- No options / spot / combos
- Volatility index is candles only (not trades); DVOL futures trades are separate
- No ClickHouse ingest
- Daily cron covers perpetuals + DVOL only (not full dated-futures universe)

See [DVOL.md](./DVOL.md) for instrument/index names and endpoints.
See [DEPLOY.md](./DEPLOY.md) for cron install / schedule.
