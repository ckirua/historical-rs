# Deploy

How to run **historical-rs (kraken)** on a **future** host: build, mirror Drive → S3 with resume, and (optionally) install monthly cron.

**This VPS is cleaned.** Do **not** start S3 backfills, tmux sessions, quota-retry loops, or cron here. Resume later from S3 with `--resume` after a clean redeploy.

Repo root: monorepo `historical-rs` (scripts live under `scripts/kraken/`).

---

## Prerequisites

- Rust 1.85+
- Hetzner/S3 credentials in `~/.env` (`S3_KRAKEN_HISTORICAL_*`)
- Bucket `mercat-kraken` (objects under `historical/`)
- Drive API key optional (public folder scrape + confirm download by default)

---

## Install (future host)

```bash
cd /path/to/historical-rs
./scripts/kraken/install.sh --global
# Optional monthly cron — only on the dedicated ingest host, never on this cleaned VPS:
# ./scripts/kraken/install.sh --cron
```

This builds `target/release/kraken-historical`, optionally installs onto `PATH`, and can register monthly cron via `scripts/kraken/install-cron.sh`.

---

## Monthly incremental cron (future host only)

Managed by `scripts/kraken/install-cron.sh` (do not hand-edit the marked block):

```cron
# Monthly 1st 08:00 UTC — Kraken Drive → S3 (resume; pick up new quarters)
0 8 1 * * cd /path/to/historical-rs && ENV_FILE=$HOME/.env SOURCE_MODE=both ./scripts/kraken/sync-s3.sh >> /var/log/kraken-historical-s3-monthly.log 2>&1
```

Kraken publishes new quarterly ZIPs end-of-quarter; daily sync is pointless.

```bash
./scripts/kraken/install-cron.sh
./scripts/kraken/install-cron.sh --uninstall
```

---

## Resume backfill (future host)

Interrupted ClickHouse/S3 work from the prior VPS is **resume-later**, not running here. On a dedicated host:

```bash
# Foreground resume (safe: --resume / S3 HEAD)
ENV_FILE=$HOME/.env SOURCE_MODE=both ./scripts/kraken/sync-s3.sh

# Or background
./scripts/kraken/sync-s3-bg.sh
tail -f /tmp/kraken-historical-s3.log
```

OHLCVT / complete ZIPs previously blocked by Google Drive anonymous quota — retry with `scripts/kraken/retry-quota-s3.sh` only on the ingest host when quota has cleared.

---

## Environment

Copy monorepo `.env.example` into `~/.env`:

| Variable | Purpose |
|----------|---------|
| `KRAKEN_GOOGLE_DRIVE_API_KEY` | optional fallback for quarterly list |
| `S3_KRAKEN_HISTORICAL_BUCKET` | `mercat-kraken` |
| `S3_KRAKEN_HISTORICAL_ENDPOINT` | object storage endpoint |
| `S3_KRAKEN_HISTORICAL_REGION` | `fsn1` |
| `S3_KRAKEN_HISTORICAL_ACCESS_KEY` / `SECRET_KEY` | credentials |
| `KRAKEN_HISTORICAL_PARALLEL` | ZIP-member workers (default 4) |

---

## Validation

```bash
./scripts/kraken/install.sh --dir .
./target/release/kraken-historical discover --data-type trades
FROM_QUARTER=2024Q1 TO_QUARTER=2024Q1 SOURCE_MODE=quarterly PAIRS=XBTUSD ./scripts/kraken/sync-s3.sh
./target/release/kraken-historical mirror status --s3
./scripts/kraken/loop-status.sh
```

## Docs

See [STATUS.md](STATUS.md) for checklist truthfulness and [LOOP.md](LOOP.md) for resume-later posture.
