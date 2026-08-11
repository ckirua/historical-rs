# Deploy — Deribit historical trades

## Prerequisites

- Rust 1.88 (repo `rust-toolchain.toml`)
- `~/.env` with S3 keys (`S3_URL` / `S3_ACCESS_KEY` / `S3_SECRET_KEY` or `S3_DERIBIT_HISTORICAL_*`)
- Bucket `S3_DERIBIT_HISTORICAL_BUCKET` (default `mercat-deribit`)
- **Optional:** `DERIBIT_CLIENT_ID` + `DERIBIT_CLIENT_SECRET` for higher rate limits on large backfills

Daily incremental cron prefers **unauthenticated** public calls (no `DERIBIT_CLIENT_*`).
Do not run multi-year unauthenticated backfills on an IP shared with live Deribit feeds.

## Build

```bash
cargo build --release -p deribit-historical-cli
# or: ./scripts/deribit/install.sh
```

## One-day smoke

```bash
export S3_DERIBIT_HISTORICAL_BUCKET=mercat-deribit
# source ~/.env — do NOT set DERIBIT_CLIENT_* for tiny unauthenticated smokes
./target/release/deribit-historical mirror s3 \
  --currency BTC --instruments BTC-PERPETUAL \
  --from 2024-01-01 --to 2024-01-01 \
  --resume --parallel 1
# Second run should skip the existing object:
./target/release/deribit-historical mirror s3 \
  --currency BTC --instruments BTC-PERPETUAL \
  --from 2024-01-01 --to 2024-01-01 --resume --parallel 1
```

## DVOL volatility index + DVOL futures trades smoke

```bash
./target/release/deribit-historical mirror volatility-index \
  --currency BTC,ETH --resolution 3600 \
  --from 2026-08-03 --to 2026-08-05 --resume --parallel 2
./target/release/deribit-historical mirror dvol-trades \
  --from 2026-08-03 --to 2026-08-05 --resume --parallel 1
```

See [DVOL.md](./DVOL.md).

## Bounded backfill

```bash
FROM=2024-01-01 TO=2024-01-07 \
  CURRENCY=BTC,ETH,USDC PERPETUALS_ONLY=1 \
  DERIBIT_HISTORICAL_PARALLEL=2 \
  ./scripts/deribit/sync-s3.sh
```

Add dated futures with `INCLUDE_EXPIRED=1` only after credentials are set — instrument
listing is 1 req/s and expired sets are large.

## Cron (installed)

Daily incremental job keeps S3 current after the historical backfill.

```bash
./scripts/deribit/install-cron.sh
# ./scripts/deribit/install-cron.sh --uninstall
```

| Item | Value |
|------|-------|
| Schedule | `0 22 * * *` host-local (Europe/Berlin on ops host) |
| Window | `FROM=$(date -u -d '3 days ago' +%F)` → `TO=$(date -u -d yesterday +%F)` |
| Sync | BTC/ETH/USDC **perpetuals** public trades (`PERPETUALS_ONLY=1`) + DVOL volatility-index (`3600`) + DVOL futures trades |
| Auth | Unauthenticated preferred — does **not** require `DERIBIT_CLIENT_*` |
| Resume | `--resume` on all lanes |
| Log | `/var/log/deribit-historical-s3.log` (falls back to `~/logs/deribit/` if `/var/log` not writable) |
| Env | `ENV_FILE=$HOME/.env` (`S3_*` / `S3_DERIBIT_*`, bucket `mercat-deribit`) |

Staggered at **22:00 local** to avoid Binance Vision jobs at 06/10/14. Schedule TZ is
host-local; FROM/TO are UTC calendar days via `date -u`.
