# OKX Historical

## Scope

The v1 product is an S3-backed historical archive in `mercat-okx`, built from OKX CDN ZIP/CSV data and parquet exports with a resumable local/S3 manifest. The implemented primary types are trades, candlesticks, funding, and borrow rates for the currently supported instrument scope.

## Current status and deployment

- [Status and evidence](STATUS.md) — listing, parsing, export, local mirror, S3 mirror, and instrument discovery are implemented. Full four-type backfill is resume/catch-up; daily cron is installed on the ops host.
- [Deploy](DEPLOY.md) — installation, resume-safe catch-up, daily cron (02:00 host-local), and environment details.
- [Shared workspace architecture](../ARCHITECTURE.md) — crates, resume flow, and compatibility invariants.

## Key commands

Run from the repository root:

```bash
./scripts/okx/install.sh
./scripts/okx/loop-status.sh

LOG=/var/log/okx-historical-s3-catchup.log PID_FILE=/tmp/okx-historical-s3-catchup.pid \
  ./scripts/okx/sync-s3-bg.sh
tail -f /var/log/okx-historical-s3-catchup.log
./target/release/okx-historical mirror status --s3
./scripts/okx/install-cron.sh
```

The sync scripts pass `--resume` and skip existing S3 objects. Daily cron runs yesterday-only incremental at **02:00 host-local** (Europe/Berlin on ops host); log `/var/log/okx-historical-s3-daily.log`.

## Boundaries

Order books (L2), FUTURES and OPTION markets remain deferred. ClickHouse ingest is out of scope or resume-later on a dedicated host. S3 manifest 400/504 responses are noisy but non-blocking (HEAD fallback).
