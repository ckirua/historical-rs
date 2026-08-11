# Deploy — historical-rs (OKX)

Ops guide for the historical-rs OKX CDN → S3 mirror on the ops host (`Europe/Berlin` cron TZ).

Repo root example: `/root/apps/historical-rs` (adjust for your checkout).

---

## Prerequisites

- Rust 1.85+
- Credentials in `~/.env`: `S3_OKX_HISTORICAL_*` **or** alias `S3_OKX_BUCKET` plus shared `S3_URL` / `S3_ACCESS_KEY` / `S3_SECRET_KEY` / `S3_REGION`
- Bucket `mercat-okx` (objects under `historical/`)

---

## Install

```bash
cd /root/apps/historical-rs   # or your checkout
./scripts/okx/install.sh            # builds target/release/okx-historical
./scripts/okx/install.sh --global   # optional: cargo install onto PATH
./scripts/okx/install-cron.sh       # daily incremental (host-local 02:00)
```

---

## Daily incremental cron

Managed by `scripts/okx/install-cron.sh` (do not hand-edit the marked block).

**Schedule times** follow the host’s local timezone (not UTC). On this host (`Europe/Berlin`) that is CEST/CET wall clock. **FROM/TO** use `date -u` (UTC calendar days).

Staggered **before** Binance Vision 06/10/14 and Deribit 22:00 local.

```cron
# Daily 02:00 local — yesterday's CDN → S3 (all types, all symbols)
0 2 * * * cd /root/apps/historical-rs && ENV_FILE=$HOME/.env FROM=$(date -u -d yesterday +%F) TO=$(date -u -d yesterday +%F) ./scripts/okx/sync-s3.sh >> /var/log/okx-historical-s3-daily.log 2>&1

# Sunday 04:00 local — refresh instrument cache
0 4 * * 0 cd /root/apps/historical-rs && ENV_FILE=$HOME/.env ./target/release/okx-historical symbols refresh >> /var/log/okx-historical-s3-daily.log 2>&1
```

Install / update / remove:

```bash
./scripts/okx/install-cron.sh
./scripts/okx/install-cron.sh --uninstall
```

**Log:** `/var/log/okx-historical-s3-daily.log`

---

## Catch-up / full backfill — resume from S3

Scripts already pass `--resume`. Existing S3 objects are skipped (HEAD fallback if manifest GET/PUT returns 400/504).

```bash
# Default window: FROM=2021-09-01 → yesterday; types=trades,candlesticks,funding,borrowrates
LOG=/var/log/okx-historical-s3-catchup.log PID_FILE=/var/run/okx-historical-s3-catchup.pid \
  ./scripts/okx/sync-s3-bg.sh

# Narrower gap focus (example):
FROM=2024-01-01 TO=$(date -u -d yesterday +%F) \
  DATA_TYPES=trades,candlesticks,funding,borrowrates \
  LOG=/var/log/okx-historical-s3-catchup.log PID_FILE=/tmp/okx-historical-s3-catchup.pid \
  ./scripts/okx/sync-s3-bg.sh

tail -f /var/log/okx-historical-s3-catchup.log
./target/release/okx-historical mirror status --s3
```

Do **not** kill concurrent Bybit / Binance / Deribit historical jobs.

---

## Environment

Copy [`.env.example`](../../.env.example) into `~/.env` (OKX section):

| Variable | Purpose |
|----------|---------|
| `S3_OKX_HISTORICAL_BUCKET` / `S3_OKX_BUCKET` | `mercat-okx` |
| `S3_OKX_HISTORICAL_ENDPOINT` / `S3_URL` | object storage endpoint |
| `S3_OKX_HISTORICAL_REGION` / `S3_REGION` | e.g. `fsn1` |
| `S3_OKX_HISTORICAL_ACCESS_KEY` / `S3_ACCESS_KEY` | credentials |
| `S3_OKX_HISTORICAL_SECRET_KEY` / `S3_SECRET_KEY` | credentials |
| `OKX_HISTORICAL_PARALLEL` | mirror workers (default 12–16) |

---

## Validation

```bash
./scripts/okx/install.sh --dir .
cargo test -p okx-historical --all-features
cargo test -p okx-historical-cli --all-features
# Optional one-day smoke:
# FROM=2024-01-01 TO=2024-01-01 SYMBOLS=BTC-USDT ./scripts/okx/sync-s3.sh
./scripts/okx/loop-status.sh
crontab -l | sed -n '/OKX-HISTORICAL-CRON/,/OKX-HISTORICAL-CRON-END/p'
```

---

## Scripts map

| Script | Role |
|--------|------|
| `scripts/okx/install.sh` | Release build; optional `--global` / `--cron` |
| `scripts/okx/install-cron.sh` | Daily cron block (02:00 local) |
| `scripts/okx/sync-s3.sh` | CDN → parquet → S3 (`--resume`) |
| `scripts/okx/sync-s3-bg.sh` | Background full/catch-up window via `sync-s3.sh` |
| `scripts/okx/mirror-local.sh` | Local parquet mirror |
| `scripts/okx/loop-status.sh` | Ops snapshot (STATUS + sync/cron) |

---

## Docs index

| Doc | Use when |
|-----|----------|
| [STATUS.md](STATUS.md) | Checklist + ops posture |
| [LOOP.md](LOOP.md) | Agent / manual tick protocol |
| [ARCHITECTURE.md](ARCHITECTURE.md) | Crate layout and data products |
| [DEPLOY.md](DEPLOY.md) | This file |
