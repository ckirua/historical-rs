# Next Steps

This file tracks what remains before `historical-rs` can be considered broadly production-ready.

How to build and run today: [`docs/USAGE.md`](docs/USAGE.md).

## Current position

The repository is suitable for controlled staging or a narrowly scoped, manually supervised pilot. It is not yet ready for a broad launch or unattended operation.

The workspace includes **four** exchange lanes (Binance, Kraken, OKX, Bybit). The current host is intentionally cleaned: no live backfills, cron jobs, tmux sessions, or S3/ClickHouse syncs should be started here unless you explicitly own that decision.

## 1. Complete the data products

### Binance Vision

The declared product is:

- `binance_vision.trades`
- `binance_vision.klines`
- `binance_vision.book_depth`
- `binance_vision.bvol`
- Matching Vision-shaped S3 archive

Remaining work:

- [ ] Finish the interrupted klines backfill.
- [ ] Run the book-depth backfill.
- [ ] Run the BVOL backfill.
- [ ] Finish the interrupted S3 product archive.
- [ ] Reconcile symbols, date bounds, row counts, object counts, and failed keys.
- [ ] Prove that no writes use legacy `binance.agg_trades` paths.

Current evidence records only UM trades as complete. Resume from a dedicated future host using the procedures in [`docs/binance/DEPLOY.md`](docs/binance/DEPLOY.md). Read [`docs/binance/STATUS.md`](docs/binance/STATUS.md) and [`docs/binance/GOAL.md`](docs/binance/GOAL.md) before starting.

### Kraken Historical

Remaining work:

- [ ] Complete OHLCVT coverage.
- [ ] Complete ZIP coverage.
- [ ] Resume work after the Google Drive quota issue clears.
- [ ] Reconcile complete and quarterly datasets.

Explicitly deferred or out of scope:

- ClickHouse ingest.
- REST live-quarter gap fill.

Use [`docs/kraken/DEPLOY.md`](docs/kraken/DEPLOY.md) and [`docs/kraken/STATUS.md`](docs/kraken/STATUS.md). Do not retry the interrupted work on the cleaned current host.

### OKX Historical

Remaining work:

- [ ] Complete the full four-type backfill: trades, candlesticks, funding, and borrow rates.
- [ ] Validate daily incremental cron on a real deployment host.
- [ ] Reconcile destination objects and manifests.

Explicitly deferred:

- L2 order books.
- Futures and options.
- ClickHouse ingest.

Use [`docs/okx/DEPLOY.md`](docs/okx/DEPLOY.md) and [`docs/okx/STATUS.md`](docs/okx/STATUS.md). Resume only after a clean redeploy.

### Bybit Historical

Implemented scope today: Spot and Contract (`bizType=contract`) **orderbook**
and **trade** → S3 parquet with resume manifests;
`scripts/bybit/sync-s3.sh` defaults to Spot `DATA_TYPES=orderbook,trade` and
accepts `BYBIT_MARKET=contract`.

Remaining work:

- [ ] Run a bounded smoke window on a real deploy host and retain evidence.
- [ ] Reconcile symbol/date coverage, object counts, and failed keys for the declared window.
- [ ] Decide whether cron/incremental jobs are in scope; if yes, add and validate them on the deploy host.
- [ ] Options / klines / funding portal products (still unverified).

Explicitly out of scope for the current Spot/Contract lane:

- ClickHouse ingest.
- Rust local parquet mirror.
- Options (not yet verified).

Use [`docs/bybit/DEPLOY.md`](docs/bybit/DEPLOY.md) and [`docs/bybit/STATUS.md`](docs/bybit/STATUS.md).

## 2. Deploy and validate on a real host

- [ ] Provision a dedicated ingest host.
- [ ] Deploy from a clean checkout using the documented instructions.
- [ ] Provide approved credentials and verify least-privilege bucket/database access.
- [ ] Build release binaries from a recorded repository revision.
- [ ] Verify the Binance MergeTree schema and intended destination tables.
- [ ] Run a bounded smoke window before any full backfill.
- [ ] Force an interruption after partial progress.
- [ ] Resume the exact same scope and verify no duplicate or missing output.
- [ ] Run an identical second resume and verify idempotency.
- [ ] Capture logs, manifests, status output, object inventory, and reconciliation results.

The consolidated operating procedures are in [`docs/PRODUCTION.md`](docs/PRODUCTION.md).

## 3. Add production operations

Host-level controls still required:

- [ ] Service supervisor such as systemd or an equivalent.
- [ ] Single-writer and locking policy.
- [ ] Persistent log collection and rotation.
- [ ] Process-exit alerts.
- [ ] Stale-progress alerts.
- [ ] Disk and inode capacity alerts.
- [ ] ClickHouse free-space alerts where applicable.
- [ ] S3/object-store quota and headroom alerts.
- [ ] Credential expiry and rotation process.
- [ ] Manifest and destination backup procedure.
- [ ] Restore drill for manifests and destination data.
- [ ] Named incident owner and escalation contacts.
- [ ] Upstream outage and format-change procedure.

The repository provides resume helpers and operational scripts, but those scripts are not a complete supervisor or monitoring system by themselves.

## 4. Complete live validation

The local suite includes ignored live network tests for Binance, Kraken, and OKX (Bybit has no ignored live smoke yet). Before broad launch:

- [ ] Run the live Binance Vision smoke test.
- [ ] Run the live Kraken Drive discovery smoke test.
- [ ] Run the live OKX CDN download smoke test.
- [ ] Add or run a Bybit portal smoke path if Bybit is in the launch contract.
- [ ] Record current upstream URLs, statuses, archive sizes, and timestamps.
- [ ] Validate live writes on the clean deployment host.
- [ ] Retain reconciliation evidence for each declared scope.

Use [`docs/PRODUCTION.md`](docs/PRODUCTION.md) for exact release gates and commands. Passing local Rust tests alone is not sufficient evidence of upstream or dataset completeness.

## 5. Complete security and release controls

- [ ] Run and review dependency advisory results.
- [ ] Establish dependency license policy.
- [ ] Run secret scanning.
- [ ] Produce versioned, reproducible release artifacts.
- [ ] Record artifact provenance.
- [ ] Use explicit release tags.
- [ ] Review logs, process lists, shell history, and evidence directories for credential exposure.
- [ ] Resolve or explicitly approve any security findings before release.

CI already covers formatting, Clippy, workspace tests, dependency auditing, and ShellCheck. The remaining work is to review results and add the host/release controls above.

## Release decision

### Safe for now

- Local development.
- Parser and parquet export validation.
- Controlled staging.
- A manually supervised pilot with explicit exchange, market, data-type, symbol, and date-window exclusions.

### Not safe yet

- Claiming the complete four-exchange historical-data product is finished.
- Enabling unattended cron across unfinished datasets.
- Making broad completeness guarantees.
- Operating without monitoring, alerting, backup/restore, and recovery evidence.

A broad launch is ready only when the declared data scope is complete, the clean-host deployment and resume tests pass, reconciliation is complete, and the operational controls are owned and tested.
