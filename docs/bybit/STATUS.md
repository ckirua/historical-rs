# Bybit Historical — status

Evidence-oriented status for the verified Bybit Spot/Contract catalog and S3
mirror. This is not a completion certificate.

## Implemented

- Workspace crates: `bybit-historical`, `bybit-historical-cli` (binary
  `bybit-historical`)
- Data-driven historical portal listing through
  `api2.bybit.com/quote/public/support/download/list-files`
- Verified catalog dimensions: `market`, `data_type`, `interval`, `symbol`,
  `date`, `url`, `filename`, and `size`
- Supported portal datasets: Spot and Contract `orderbook` / `trade`, all with
  `interval=daily` (`bizType=spot|contract`)
- Current Trading symbol discovery through Bybit V5
  `market/instruments-info` for `category=spot|linear|inverse`, filtering to
  instruments whose status is `Trading`
- Contract symbol discovery unions linear + inverse Trading symbols
- CLI `mirror symbols --market spot|contract`, which refreshes and prints one
  Trading symbol per line followed by `count=N`, without contacting S3
- CLI `mirror s3` with exactly one of explicit `--symbols` or `--all-symbols`;
  the latter prints the discovered count and mirrors the market-appropriate
  Trading catalog
- `--all-symbols` and `--symbols` cannot be combined, and omitting both is
  rejected
- Spot/Contract orderbook ZIP and trade `.csv.gz` download, conversion, and S3
  Parquet upload (Contract trade ids are UUID strings; Spot numeric ids are
  stringified into the same Utf8 Parquet column)
- Download path retries HTTP 403/429/5xx and transient transport errors with
  long exponential backoff on 403; a shared circuit breaker fails remaining
  downloads fast after sustained exhausted 403s
- End-of-run CLI `failure_histogram` of failed archive error classes (portal
  date gaps remain pending / non-failed)
- S3 Parquet mirror with JSONL manifests and dimension-aware
  storage/manifest keys (Spot orderbook keeps the legacy key layout)
- Deterministic public coverage reconciliation via
  `reconcile_coverage`, `CoverageCell`, and `CoverageReport`
- CLI `mirror status --s3`
- `--resume` defaults to `true`
- Helper: `scripts/bybit/download-orderbook.py` for raw local Spot/Contract
  catalog downloads
- Ops script `scripts/bybit/sync-s3.sh` mirrors `orderbook` and `trade`
  (default `DATA_TYPES=orderbook,trade`); supports `ALL_SYMBOLS=1` and
  `BYBIT_MARKET=spot|contract`
- Ops scripts: `scripts/bybit/install.sh`, `scripts/bybit/sync-s3.sh`,
  `scripts/bybit/install-cron.sh` (daily 18:00 host-local)
- Env template: `S3_BYBIT_HISTORICAL_*` in [`.env.example`](../../.env.example)

## Ops host (Europe/Berlin)

| Item | Value |
|------|-------|
| Daily cron | `0 18 * * *` host-local → `./scripts/bybit/sync-s3.sh` (FROM=TO=yesterday UTC, spot trade+orderbook, `--resume`) |
| Cron log | `/var/log/bybit-historical-s3-daily.log` |
| Cron install | `./scripts/bybit/install-cron.sh` |
| Catch-up log | `/var/log/bybit-historical-s3-catchup.log` |

**Catch-up gate (2025-04-29→2026-08-08, spot, `--all-symbols`, finished 2026-08-10):** trade
`failed=16` (HTTP 403 only; prior ~3618); orderbook `failed=0` (prior ~198); pending portal
gaps expected/non-failed; no parse/circuit storms. Daily cron installed after this gate.

## Coverage reconciliation

Reconciliation evaluates the requested dataset, the case-insensitive unique
symbol set, and the inclusive date range as deterministic symbol/date cells.
An `ok` object or a resume `skipped` object completes a cell. A cell with no
matching manifest entry is `missing`. The mirror planner records a portal date
that was not listed, or a failed portal listing, as `failed` with an
explanatory error; download, conversion, and upload errors are also `failed`.
Missing and unavailable portal dates are never successful objects, even when
the current Trading symbol catalog contains the symbol. Entries for another
dataset or malformed dates are ignored, and duplicate entries collapse in
favor of a successful object.

## Explicit boundaries

- The current Trading list is live and refresh-on-demand; it is not a frozen
  historical symbol universe.
- A current symbol does not imply an archive exists for every historical date.
- Verified markets are **Spot** and **Contract** (`bizType=spot|contract`).
- CLI `--market linear|inverse` is rejected; use `contract`.
- Options, klines/funding, and unknown market/product identifiers remain
  unsupported.
- Local Rust parquet mirror (S3-only in Rust today)
- ClickHouse ingest
- Live ignored integration smoke test (none checked in yet)

## Resume-later posture

Interrupted S3 mirrors should be resumed on a designated ingest host with the
same bucket credentials, selected `--market` / `--data-type`, and `--resume`
(default). Do not treat a quiet cleaned VPS, a missing portal listing, or an
unavailable catalog day as evidence that the Bybit archive is complete.

**Pending portal gaps** (often ~30k/type on full-window `--all-symbols` runs)
are sparse listings: the portal did not advertise those symbol/date archives.
They are expected and non-failed; `--resume` will not invent or backfill them.

**Orderbook null sequence:** JSON `u` / `seq` that are null, missing, or
non-numeric are stored as nullable Parquet nulls instead of failing conversion
with `invalid archive row … invalid sequence value`.
