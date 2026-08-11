# Kraken Historical

## Scope

The v1 product is an S3 archive in `mercat-kraken`, populated from public Google Drive data: quarterly and complete `trades` plus `ohlcvt` parquet, with local/S3 JSONL manifests for resume. The pipeline streams ZIPs to disk and does not load complete archives into RAM.

## Current status and deployment

- [Status and evidence](STATUS.md) — quarterly trades backfill recorded 4,016 parquet objects. OHLCVT and complete ZIP coverage were interrupted by Google Drive anonymous quota and are resume-later.
- [Deploy on a future ingest host](DEPLOY.md) — prerequisites, installation, resume commands, and monthly incremental cron. This VPS is cleaned; no sync, quota retry, tmux, or cron is running here.
- [Product goal](GOAL.md) — both trades and OHLCVT archive products must be mirrored with resume.

## Key commands

Run from the repository root on the designated future host:

```bash
./scripts/kraken/install.sh --global
./scripts/kraken/loop-status.sh

ENV_FILE=$HOME/.env SOURCE_MODE=both ./scripts/kraken/sync-s3.sh
./scripts/kraken/sync-s3-bg.sh
./scripts/kraken/retry-quota-s3.sh
./target/release/kraken-historical mirror status --s3
```

Use `retry-quota-s3.sh` only when Drive quota has cleared. Install monthly cron with `./scripts/kraken/install-cron.sh` only on the future ingest host.

## Boundaries

ClickHouse ingest is out of scope for v1, and REST gap-fill for the live quarter is deferred. Raw ZIPs must not be stored in S3, and Drive/ S3 backfills must not be restarted on this cleaned VPS; resume later from S3 with `--resume` on a clean host.
