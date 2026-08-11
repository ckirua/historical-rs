# historical-rs production readiness, operations, and release guide

**Project:** `historical-rs`  
**Assessment date:** 2026-07-23 (base assessment); docs refreshed 2026-07-26 for four-exchange workspace accuracy  
**Assessment branch:** `docs/production-readiness-assessment` (historical); current code on `main`

This document consolidates the production-readiness assessment, production operations runbook, and release-validation plan. It is an operating and evidence guide, not a production approval. Status claims below preserve the documented **resume-later** posture; a command, a passing local test, or a quiet process does not make an unfinished product complete.

Practical how-to-use guidance for developers/operators: [USAGE.md](USAGE.md). The workspace now includes **Bybit** (Spot orderbook + trade → S3) in addition to Binance, Kraken, and OKX; older “three exchange” wording in historical assessment prose should be read in that light.

## 1. Executive readiness verdict and current status

### Executive verdict

> **Not ready for a full production launch with a “launch and never look back” expectation.**

The repository is a credible release candidate for controlled staging or a narrowly scoped pilot. The codebase has a healthy local validation baseline, but the documented primary data products are incomplete and operational deployment has not been demonstrated end to end on a live target host.

Production readiness is **conditional and scope-limited**, not a blanket approval for all exchanges, markets, data types, and ingestion paths.

### Assessment checks recorded

The following checks were run from the repository root during the assessment:

| Check | Recorded result |
|---|---:|
| `cargo fmt --all -- --check` | Pass |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | Pass |
| `cargo test --workspace --all-features` | **79 passed, 3 ignored** |
| `cargo run -p binance-vision-cli -- --help` | Pass |
| `cargo run -p kraken-historical-cli -- --help` | Pass |
| `cargo run -p okx-historical-cli -- --help` | Pass |
| `cargo run -p bybit-historical-cli -- --help` | Not in original assessment; required for current workspace |
| `bash -n scripts/binance/*.sh scripts/kraken/*.sh scripts/okx/*.sh scripts/bybit/*.sh` | Bybit scripts added after assessment |

These are historical assessment results, not a current release sign-off. Ignored live network smoke tests cover Binance, Kraken, and OKX; Bybit does not yet ship an ignored live smoke test. The passing suite validates local behavior and fixtures, but does not establish current upstream availability or end-to-end production correctness.

### Current product status

The exchange-specific goal, status, and deployment documents remain authoritative:

- [Binance goal](binance/GOAL.md), [status](binance/STATUS.md), and [deployment](binance/DEPLOY.md)
- [Kraken status](kraken/STATUS.md) and [deployment](kraken/DEPLOY.md)
- [OKX status](okx/STATUS.md) and [deployment](okx/DEPLOY.md)
- [Bybit status](bybit/STATUS.md) and [deployment](bybit/DEPLOY.md)
- [environment template](../.env.example)
- [how to use](USAGE.md)

#### Binance Vision

The declared product requires all four `binance_vision` tables and corresponding S3 archive coverage:

- `trades`
- `klines`
- `book_depth`
- `bvol`

Current status records:

- UM trades: complete for the recorded run.
- Klines: interrupted and resume-later.
- Book depth: not started after the interruption.
- BVOL: not started after the interruption.
- S3 product archive: interrupted and resume-later.

Shipping only the completed trades path would violate the documented product goal. The deployment documentation explicitly warns against legacy Binance `aggTrades` paths; those paths remain forbidden.

#### Kraken Historical

The primary scope is S3-backed historical data. Current status records:

- Trade quarterly backfill: evidence of 4,016 parquet objects.
- OHLCVT and complete ZIP coverage: incomplete and resume-later.
- ClickHouse ingest: explicitly out of scope for v1.
- REST gap-fill for the live quarter: deferred.

This can support a deliberately limited trades-only release, but not a complete historical-data launch.

#### OKX Historical

Current status records:

- Core CDN listing, parsing, parquet export, local mirror, and S3 mirror paths: implemented.
- Full backfill for all four declared types: incomplete and resume-later.
- Daily cron: deferred and not validated on a deployment host.
- Order books: deferred.
- Futures and options: deferred.
- ClickHouse ingest: out of scope or resume-later.

This is not yet a complete OKX production dataset.

#### Bybit Historical

Current status records:

- Spot orderbook and Spot trade portal → parquet → S3 mirror: implemented.
- Ops helper `scripts/bybit/sync-s3.sh` defaults to `DATA_TYPES=orderbook,trade`.
- Resume via S3 manifest + data-key listing: implemented (`--resume` defaults to true).
- Full symbol/date reconciliation: not claimed complete.
- ClickHouse ingest, Rust local parquet mirror, futures/derivatives: out of scope for the current Spot lane.

This can support a deliberately limited Spot S3 pilot, not a complete Bybit historical-data launch.

### Foundations worth preserving

- Workspace-wide formatting, linting, and tests are already enforced in CI.
- HTTP clients use TLS, request timeouts, retries, and handling for 404, server errors, and rate limits.
- S3 and ClickHouse workflows include resume-oriented mechanisms.
- Local parsing and parquet conversion have fixture-backed tests.
- Environment examples separate exchange-specific credentials and storage prefixes.
- Deployment documentation warns against legacy Binance `agg_trades` paths and records the current freeze posture.

These foundations do not replace completion evidence or production operations.

## 2. Launch blockers

### 2.1 Incomplete declared datasets — blocking

The primary product gates are not green across Binance, Kraken, OKX, and Bybit. Backfills are interrupted, some data types have not started, and some coverage is explicitly deferred.

**Required before broad launch:** complete the documented dataset gates or explicitly redefine the launch scope and market the release only under that narrower contract.

### 2.2 No demonstrated clean-host production deployment — blocking

The status and deployment documents state that the current VPS has no live tmux jobs, cron, or active S3/ClickHouse backfill. The documented procedure targets a future host and relies on manual setup.

**Required before launch:** perform a clean-host deployment, run the declared smoke path, verify resume after interruption, and retain the resulting evidence.

### 2.3 Missing operational safety net — blocking for unattended operation

The repository contains helper scripts and resume mechanisms, but does not establish a complete unattended production operating model. There is no demonstrated release artifact pipeline, service-manager configuration, monitoring/alerting setup, capacity alerting, backup/restore drill, or incident procedure.

**Required before unattended launch:** define ownership and add operational controls for failures, storage exhaustion, credential rotation, upstream changes, and partial backfills.

### 2.4 Live integration coverage is not normal validation — high risk

Exchange-facing integration tests are ignored because they require network access. CI runs formatting, Clippy, and the workspace test suite, but does not prove live exchange/CDN/Drive/S3 behavior.

**Required before launch:** execute scheduled or release-gated live smoke tests against each supported upstream and record expected object, row, and checksum/reconciliation results.

### 2.5 Dependency and shell-script security checks need ongoing review — high risk

CI now runs dependency auditing (`actions-rust-lang/audit`) and ShellCheck on `scripts/{binance,kraken,okx,bybit}/*.sh`, in addition to fmt/clippy/tests. `cargo-deny` / license policy review is still not fully established as a release gate.

**Required before launch:** review advisory and ShellCheck results on the release commit, establish dependency license policy (for example via `cargo-deny`), then resolve or explicitly accept findings.

### Readiness checklist

A broad production launch should wait until every applicable item below is evidenced. These checkboxes are intentionally uncompleted; do not mark them from the assessment table above.

#### Data completeness

- [ ] Binance `trades`, `klines`, `book_depth`, and `bvol` gates are complete for the promised markets and intervals.
- [ ] Kraken promised trades and OHLCVT coverage is complete, or the public contract excludes unfinished coverage.
- [ ] OKX promised data types, markets, and time windows are complete.
- [ ] Bybit promised Spot orderbook + trade coverage is complete, or the public contract excludes unfinished coverage.
- [ ] Row counts, date bounds, symbol coverage, object counts, and failed-key counts are reconciled.
- [ ] No forbidden or legacy write path is active.

#### Reliability and recovery

- [ ] Clean-host deployment succeeds from documented instructions.
- [ ] A forced interruption followed by `--resume` produces no duplicate or missing output.
- [ ] Retry, rate-limit, 404, malformed archive, and partial-write behavior are exercised.
- [ ] Disk usage and object-store capacity thresholds are enforced.
- [ ] Credentials are supplied through a managed secret mechanism and rotation is tested.

#### Operations

- [ ] Release artifacts are versioned and reproducible.
- [ ] Jobs run under a service manager or equivalent supervisor.
- [ ] Logs, metrics, alerts, and ownership are defined.
- [ ] Backup and restore procedures are tested for manifests and destination data.
- [ ] An incident and upstream-change runbook exists.

#### Security and supply chain

- [ ] Dependency advisories are checked in CI.
- [ ] Dependency licenses and policy are checked.
- [ ] Shell scripts pass ShellCheck.
- [ ] Production credentials and endpoints are confirmed absent from tracked files and logs.

### Scope decision

**Safe now:** controlled staging; a trades-only or otherwise explicitly bounded pilot with manual supervision; local parser/export validation and development use.

**Not safe now:** advertising the repository as a complete historical-data product; enabling unattended cron across all documented datasets; treating the current status as complete merely because the Rust test suite passes; promising durable operation without monitoring, alerting, and recovery evidence.

The next milestone is completing the declared data gates and proving a recoverable, observable deployment on a clean host.

## 3. Production operations runbook

This runbook is for a **future, dedicated production host**. It is a preflight and operating procedure, not a production approval. The current posture is **resume-later**: interrupted backfills are not running on this checkout/VPS. Do not restart them here, do not enable cron examples here, and do not represent any unfinished dataset as complete.

### 3.1 Non-negotiable scope and stop conditions

Before any job starts, read the applicable `STATUS.md` and record the exact market, data type, and date window being resumed. At the time this document was written:

- **Binance Vision:** `trades` has recorded completion for one run; `klines` and the S3 archive are interrupted/resume-later; `book_depth` and `bvol` have not started after the interruption. The declared product is all four `binance_vision` tables and corresponding S3 coverage.
- **Kraken:** trades have evidence of a quarterly backfill, but OHLCVT and complete ZIP coverage are incomplete/resume-later. ClickHouse ingest is not a v1 product path.
- **OKX:** mirror and export paths exist, but the declared four-type backfill is incomplete/resume-later. Daily cron, order books, futures, and options are deferred or not validated on a deployment host.

These facts are not changed by a passing local test suite or a successful one-day smoke run.

Stop immediately if any of these are true:

- The host is this cleaned VPS rather than the authorized future ingest host.
- The operator cannot identify the owner, approved scope, revision, destination bucket/prefix, and rollback/restore source.
- Required secrets are missing, exposed in shell history/logs, or supplied from a tracked file.
- The job would write Binance legacy `aggTrades` data to `binance.agg_trades`.
- The job would use `scripts/binance/clickhouse-direct.sh` or `scripts/binance/clickhouse-ingest-s3.sh`. Those are explicitly legacy paths and must not be enabled.
- A declared data gate is unfinished but the request is for broad production or unattended execution.
- Disk/object-store headroom, destination backups, or alert ownership is not established.
- A job is already running and its ownership is unknown. Never start a second copy to “catch up.”
- A manifest or destination restore is being attempted without first stopping writers and preserving current state.

Commands below that start a job are **authorized-host examples only**. They do not override these stop conditions.

### 3.2 Roles, ownership, and handoff

The host owner must fill these in before unattended operation; names and contacts are intentionally not invented here.

| Role | Named owner/contact | Responsibility |
|---|---|---|
| Primary operator | `TODO: name/contact` | Preflight, start/stop, daily checks, incident coordination |
| Data/product owner | `TODO: name/contact` | Defines promised markets/types/windows and accepts reconciliation evidence |
| Platform/host owner | `TODO: name/contact` | Host access, supervisor, disk, object storage, backups, and patching |
| Security/credential owner | `TODO: name/contact` | Secret issuance, rotation, revocation, and exposure response |
| Upstream escalation | `TODO: contact/link` | Binance Vision, Kraken Drive, or OKX CDN availability/change escalation |

For every run, record UTC start/end, operator, repository revision, command and environment overrides (never secret values), source/upstream, destination bucket/database, date window, counts (`done`, `skipped`, `failed`), alerts, and follow-up owner. A handoff is incomplete until the next operator can identify the active process, log path, last successful checkpoint, and next safe resume command.

### 3.3 Clean-host prerequisites

Complete this section on the future host only.

#### Host and repository

- [ ] Dedicated host access and an OS/user policy approved by the platform owner.
- [ ] Rust 1.85 or newer, `cargo`, `git`, Bash, `tmux` only if explicitly chosen for supervised operation, `clickhouse-client` for the Binance ClickHouse path, and a provider-approved S3 client or console for destination administration.
- [ ] Time is synchronized to UTC. Scripts calculate `yesterday` using the host clock.
- [ ] Checkout is from the approved repository revision. Record it before building:

```bash
cd /path/to/historical-rs
git rev-parse HEAD
git status --short
```

A dirty checkout is a stop condition unless the change is reviewed and recorded. There is no repository release-artifact pipeline documented today; a source build is not evidence of a complete production release.

#### Destination prerequisites

- [ ] Binance S3 credentials and bucket/prefix are provisioned if the S3 mirror is in scope. The example bucket is `binance`; keys follow the Vision-shaped product layout. ClickHouse credentials and the `binance_vision` database are required for direct ClickHouse jobs.
- [ ] Kraken S3 bucket `mercat-kraken` and `historical/` prefix are provisioned. A Google Drive API key is optional; the public folder scrape is the default source path.
- [ ] OKX S3 bucket `mercat-okx` and `historical/` prefix are provisioned.
- [ ] Destination write/read/list/delete permissions are tested by the platform owner without placing production data in an unapproved location.
- [ ] Backup retention, restore location, and object-store capacity/retention policy are written down. The repository has no backup/restore controller or capacity alert configuration.

#### Build and schema

Use the existing install helpers; they build release binaries and do not configure a host supervisor:

```bash
cd /path/to/historical-rs
./scripts/binance/install.sh --dir .
./scripts/kraken/install.sh --dir .
./scripts/okx/install.sh --dir .
```

For Binance ClickHouse, apply the MergeTree schema from the deployment guide:

```bash
clickhouse-client --password "$CLICKHOUSE_PASSWORD" --multiquery \
  < sql/clickhouse/010_binance_vision_merge_tree.sql
```

Verify that the target database contains the intended product tables (`trades`, `klines`, `book_depth`, `bvol`, and `ingest_log`) and that the engines are MergeTree. Do not apply or revive the legacy `001_schema.sql`/`binance.agg_trades` product path.

### 3.4 Secret handling

The committed [`.env.example`](../.env.example) documents variable names and non-secret defaults. It is not a secret store and must never be copied into the repository with real values.

On the authorized host, use a host-approved secret manager if one exists. If the current manual procedure is used for a supervised pilot, keep the file outside the checkout and restrict it:

```bash
install -m 600 /dev/null "$HOME/.env"
# Populate ~/.env from the approved secret source; do not paste it into chat or tickets.
chmod 600 "$HOME/.env"
```

The file may contain:

- `S3_BINANCE_VISION_BUCKET`, `S3_BINANCE_VISION_ENDPOINT`, `S3_BINANCE_VISION_REGION`, `S3_BINANCE_VISION_ACCESS_KEY`, and `S3_BINANCE_VISION_SECRET_KEY`;
- `CLICKHOUSE_HOST`, `CLICKHOUSE_PORT`, `CLICKHOUSE_USER`, `CLICKHOUSE_PASSWORD`, and `CLICKHOUSE_VISION_DATABASE`;
- `KRAKEN_GOOGLE_DRIVE_API_KEY` if the optional fallback is approved;
- `S3_KRAKEN_HISTORICAL_*` and `S3_OKX_HISTORICAL_*` destination settings;
- documented parallelism/data-directory settings, after capacity review.

Load it only for the command session, using the line-ending-safe pattern already used by the scripts:

```bash
set -a
source <(sed 's/\r$//' "$HOME/.env")
set +a
```

Do not run `set -x`, print the environment, put secrets in command-line arguments, or include environment contents in logs. Before handoff, unset variables or close the shell. Confirm that repository status, log files, process arguments, shell history, crash output, and support tickets contain no credentials. A managed secret mechanism, access review, and tested rotation are **required host-level controls**, not currently implemented repository features.

### 3.5 Preflight checklist

The primary operator must complete and retain this checklist for each new host or resumed scope.

- [ ] Host is the authorized future host; no jobs are enabled on the cleaned VPS.
- [ ] Owner, scope, revision, source, destination, and rollback/backup references are recorded.
- [ ] `~/.env` exists with mode `0600`, is outside the checkout, and contains only approved current credentials.
- [ ] `git rev-parse HEAD` and `git status --short` were recorded.
- [ ] Release binaries were built with the existing exchange install scripts.
- [ ] Required S3 bucket/prefix and ClickHouse database are reachable with least-privilege credentials.
- [ ] For Binance, the MergeTree schema is applied and no legacy `agg_trades` target is selected.
- [ ] Disk, inode, ClickHouse free space, and object-store quota/headroom checks pass the host owner’s written thresholds.
- [ ] A manifest/destination backup exists and its checksum/retention location is recorded.
- [ ] The supervisor/lock policy is known. There is only one writer per exchange/data path.
- [ ] Log retention and alert delivery have been tested by the platform owner.
- [ ] A small, explicitly bounded smoke window is approved before any wider resume.
- [ ] The data owner has confirmed that the selected scope is allowed despite the overall resume-later status.

**Preflight stop:** if any checkbox is false, do not enable cron, a background wrapper, or a full backfill.

### 3.6 Start, supervise, and stop jobs

The scripts select release binaries when available, load `ENV_FILE` (default `$HOME/.env`), pass `--resume`, expose parallelism/date/type overrides, and print progress. `mirror status --s3` reads the S3 manifest. Kraken and OKX `sync-s3-bg.sh` helpers use `nohup` plus `/tmp/*.pid` files; deployment docs also show `tmux` and optional cron. These helpers are useful for a supervised pilot, but are not a complete production supervisor, lock service, alerting system, or release manager.

Before unattended use, the platform owner must provide and test an approved supervisor or equivalent with one-instance locking, explicit environment-file permissions, restart/backoff limits, clean stop behavior, persistent log collection/rotation, process exit alerts, disk alerts, and operator-visible status. No systemd unit, container definition, scheduler policy, metrics integration, or alert routing is present in this repository; do not claim one exists unless the host owner has deployed and tested it.

#### Status snapshots

Run existing helpers from the repository root on the authorized host:

```bash
./scripts/binance/loop-status.sh
./scripts/kraken/loop-status.sh
./scripts/okx/loop-status.sh
```

They show documented status, process/cron state, recent logs, and (where configured) S3 or ClickHouse status. A status helper reporting “stopped” is not a failure by itself on a resume-later host; it is a reason to check authorization and scope before starting anything.

#### Authorized start examples

Start only the approved bounded scope first. The scripts below are existing wrappers and all include `--resume`:

```bash
# Binance S3 product types; never add aggTrades.
ENV_FILE="$HOME/.env" SYMBOLS=BTCUSDT \
  FROM=2026-07-10 TO=2026-07-12 \
  DATA_TYPES=trades,klines,bookDepth,BVOLIndex \
  ./scripts/binance/sync-s3.sh

# Binance direct ClickHouse product path (one type at a time).
ENV_FILE="$HOME/.env" SYMBOLS=BTCUSDT \
  FROM=2026-07-10 TO=2026-07-12 \
  ./scripts/binance/clickhouse-direct-trades.sh

# Kraken, one quarter and one pair for a smoke run.
ENV_FILE="$HOME/.env" SOURCE_MODE=quarterly \
  FROM_QUARTER=2024Q1 TO_QUARTER=2024Q1 PAIRS=XBTUSD \
  DATA_TYPES=trades ./scripts/kraken/sync-s3.sh

# OKX, one day and one symbol for a smoke run.
ENV_FILE="$HOME/.env" FROM=2024-01-01 TO=2024-01-01 \
  SYMBOLS=BTC-USDT DATA_TYPES=trades ./scripts/okx/sync-s3.sh
```

Broad/full commands are documented in [Binance DEPLOY](binance/DEPLOY.md), [Kraken DEPLOY](kraken/DEPLOY.md), and [OKX DEPLOY](okx/DEPLOY.md). Do not use those broad commands until the data owner has approved the scope and all gates in this document are green.

For a supervised process, capture stdout and stderr in a host-managed log. Deployment docs show `tmux` names and `/tmp` log paths; `/tmp` is not durable retention. If `tmux` is used, record the session name and make sure an external owner checks it. If a host supervisor starts a process, do not also start the same path in `tmux`, `nohup`, or cron.

To stop a job, use the supervisor’s graceful stop operation. If no supervisor has been approved, identify the exact process with the exchange status helper and stop that process only; retain its log and note the UTC interruption time. Never kill an unknown process by a broad pattern. After stopping, confirm the process is gone and no stale pidfile causes a false “running” result.

### 3.7 Storage and capacity checks

Run capacity checks before start, during long runs, and after any alert. Thresholds must be set by the platform owner based on object-store and ClickHouse recovery needs; this repository does not define safe numeric thresholds.

Local disk and inode checks:

```bash
df -h /var/lib/clickhouse "$HOME"
df -ih /var/lib/clickhouse "$HOME"
# If a custom data directory is configured, inspect it as well:
du -sh "${BINANCE_VISION_DATA_DIR:-$HOME/.data/binance-vision}" \
       "${KRAKEN_HISTORICAL_DATA_DIR:-$HOME/.data/kraken-historical}" \
       "${OKX_HISTORICAL_DATA_DIR:-$HOME/.data/okx-historical}" \
       "${BYBIT_DATA_DIR:-$HOME/.data/bybit-historical}" 2>/dev/null || true
```

For Binance ClickHouse, use bounded system-part queries rather than an unbounded full-table count:

```bash
CH=(clickhouse-client --host "${CLICKHOUSE_HOST:-127.0.0.1}" \
  --port "${CLICKHOUSE_PORT:-9000}" --user "${CLICKHOUSE_USER:-default}" \
  --password "$CLICKHOUSE_PASSWORD")
"${CH[@]}" -q "
SELECT database, table,
       formatReadableSize(sum(bytes_on_disk)) AS size,
       sum(rows) AS rows
FROM system.parts
WHERE active AND database='${CLICKHOUSE_VISION_DATABASE:-binance_vision}'
GROUP BY database, table ORDER BY sum(bytes_on_disk) DESC;
"
"${CH[@]}" -q "SELECT formatReadableSize(free_space) FROM system.disks;"
```

If ClickHouse returns `Code: 243 NOT_ENOUGH_SPACE`, stop ClickHouse ingest, preserve logs, free or expand capacity through the platform owner, re-run the check, and resume only after free-space thresholds pass. Do not delete `ingest_log` to make space. For S3, check provider quota, object count, lifecycle/retention, and error responses using the provider’s approved administrative tool; no S3 quota monitor is implemented in this repository.

### 3.8 Logs, alerts, and expected evidence

Deployment docs use these paths:

- Binance: `/tmp/binance-vision-s3-product.log`, `/tmp/binance-vision-trades-ch.log`, `/tmp/binance-vision-chain.log`, and related per-type logs.
- Kraken: `/tmp/kraken-historical-s3.log`.
- OKX: `/tmp/okx-historical-s3.log`.
- Optional cron examples write under `/var/log/` on the future host.

Treat `/tmp` logs as transient diagnostics. A production host must forward or rotate logs outside the repository’s helpers before unattended execution; no durable logging or alert backend is included here.

Progress lines use `done=… skipped=… failed=…`. `skipped` can mean an existing resume record or a source 404; it is not proof of a complete product. For every run, capture command scope and UTC window; process exit status and last log timestamp; `done`, `skipped`, and `failed` totals; S3 `mirror status --s3` output; for Binance ClickHouse, table sizes/rows and `ingest_log` status counts; source 404, rate-limit, 5xx, malformed archive, checksum, permission, and destination errors; reconciliation result; and the owner for any discrepancy.

The host alerting system must notify the named primary operator (and escalate when not acknowledged) for process exit or repeated restart, stale/no progress, any nonzero failed count, sustained upstream errors or rate limits, destination authentication/permission failures, ClickHouse `NOT_ENOUGH_SPACE`, local disk/inode thresholds, object-store quota/headroom thresholds, missing/stale logs, backup failure, and credential expiry/rotation deadline. Alert thresholds and routes are host-level configuration and must be recorded; none are currently implemented by the repository.

An alert is not cleared by restarting blindly. Acknowledge it, preserve evidence, classify the cause, and use the recovery section below.

### 3.9 Enabling incremental schedules

Cron installers exist for Kraken and OKX, and the Binance deployment document provides cron examples. They are **future-host-only** conveniences, not proof of safe unattended operation. Do not run `--cron` or install any cron block until:

- all applicable data-completeness gates are green or the public contract explicitly excludes unfinished coverage;
- clean-host deployment, forced interruption, resume, and reconciliation evidence is retained;
- a tested supervisor/lock, durable logs, capacity alerts, backup/restore drill, credential rotation, and incident ownership exist;
- the data and platform owners approve the exact schedule and scope.

When approved, use the repository installer for the exchange (`scripts/kraken/install-cron.sh` or `scripts/okx/install-cron.sh`) rather than hand-editing its marked block. Verify the resulting crontab, command revision, environment source, log destination, and lock/supervisor behavior. For Binance, do not cron `scripts/binance/clickhouse-direct.sh` or `scripts/binance/clickhouse-ingest-s3.sh`; these remain forbidden legacy `aggTrades` paths.

## 4. Release validation gates

This is a gate plan, not a record of completed gates. Every result must be backed by command output, a log, a manifest, or a query result captured in the release evidence directory. Do not mark a gate passed from an expected result or a previous run. Passing local Rust tests alone is insufficient: live upstream paths, clean-host deployment, recovery behavior, operational controls, and declared data products also need evidence.

### 4.1 Freeze scope and evidence location

Before running a gate:

1. Write down the exact release identifier, commit, exchanges, markets, data types, symbols, intervals, and UTC date/time window being released.
2. Distinguish **declared scope** (what the release promises) from **validation scope** (what this run exercises). A validation subset cannot be presented as complete declared coverage.
3. Use a clean checkout and a dedicated evidence directory. Keep secrets outside the repository and redact them from captured output.

Example setup (replace values with the release candidate and approved evidence location):

```bash
export RELEASE_ID="<tag-or-commit>"
export EVIDENCE_DIR="$HOME/release-evidence/$RELEASE_ID"
mkdir -p "$EVIDENCE_DIR"/{local,live,deploy,reconcile,security,signoff}
cd /path/to/historical-rs
git rev-parse HEAD | tee "$EVIDENCE_DIR/commit.txt"
rustc --version | tee "$EVIDENCE_DIR/local/rustc-version.txt"
cargo --version | tee "$EVIDENCE_DIR/local/cargo-version.txt"
```

Do not put `~/.env`, access keys, signed URLs, cookies, or unredacted logs in evidence. Record the secret-manager reference and credential version/rotation date instead.

The declared product contract and current gaps are exchange-specific: [Binance goal](binance/GOAL.md) and [status](binance/STATUS.md) define four `binance_vision` products and forbid legacy `binance.agg_trades`; [Kraken status](kraken/STATUS.md) defines the current S3-only product and incomplete OHLCVT/complete-ZIP coverage; [OKX status](okx/STATUS.md) identifies incomplete full backfill and deferred order-book, futures, and options coverage. The [Binance deployment guide](binance/DEPLOY.md), [Kraken deployment guide](kraken/DEPLOY.md), and [OKX deployment guide](okx/DEPLOY.md) target a **future/dedicated host**; do not start documented jobs on a cleaned workstation or assessment VPS.

### 4.2 Gate classes and launch mode

#### Mandatory broad-launch gates

A broad launch is blocked unless every applicable item has current evidence for the complete declared scope:

- Local CI gates and CLI help smoke checks pass on the release commit.
- The ignored live smoke test for every supported exchange passes, with upstream URL, date, and response/archive evidence retained.
- A clean-host deployment succeeds using the exchange deployment guide.
- A forced interruption followed by resume completes without duplicate or missing output.
- Expected data keys, destination objects, manifest entries, parquet rows, and (where applicable) ClickHouse rows reconcile for each declared type, symbol, interval, and time window.
- Malformed input, HTTP 404, server-error retry, rate-limit behavior, checksum handling, and partial-write behavior are exercised and produce documented safe outcomes.
- Dependency advisory/license checks, shell-script checks, secret scanning, and artifact provenance checks pass; unresolved findings have an explicit owner and release decision.
- Operational ownership, logs, alerts, capacity thresholds, credential handling/rotation, backup/restore, and incident procedures are available for the launch mode. Unattended cron is not justified by a green test suite.

A failure, missing artifact, unexplained count, or unapproved waiver is a **FAIL**, not an implicit pass.

#### Narrow pilot validation

A pilot may be approved only with an explicit, bounded public/internal contract and manual supervision. Name the exchange(s), data types, symbols/markets, and UTC window it covers. At minimum validate local gates, CLI help, the relevant live smoke test, a clean-host bounded run, interruption/resume for that bounded run, reconciliation, malformed/retry behavior, and security checks. Do not enable broad unattended schedules, and do not claim unfinished products are complete.

Examples:

- Binance: one named market and symbol/date window for `trades` only; this does **not** validate `klines`, `book_depth`, or `bvol`.
- Kraken: named quarterly `trades` pairs and S3 prefix only; this does **not** validate complete ZIP/OHLCVT coverage or ClickHouse ingest.
- OKX: one named market, symbol, data type, and date; this does **not** validate all four types, order books, futures, or options.

The release sign-off must state `PILOT` rather than `BROAD` and include exact exclusions.

### 4.3 Local CI and CLI help gates

Run the repository CI commands from the repository root and capture stdout/stderr and exit status:

```bash
cargo fmt --all -- --check 2>&1 | tee "$EVIDENCE_DIR/local/cargo-fmt.txt"
cargo clippy --workspace --all-targets --all-features -- -D warnings 2>&1 | tee "$EVIDENCE_DIR/local/cargo-clippy.txt"
cargo test --workspace --all-features 2>&1 | tee "$EVIDENCE_DIR/local/cargo-test.txt"
bash -n scripts/binance/*.sh scripts/kraken/*.sh scripts/okx/*.sh scripts/bybit/*.sh \
  2>&1 | tee "$EVIDENCE_DIR/local/bash-n.txt"
```

The current CI workflow is [.github/workflows/ci.yml](../.github/workflows/ci.yml). Preserve exit status when using `tee` (for example, use `set -o pipefail` or capture command status separately). Record the exact test summary; do not rewrite an ignored test as passed.

Run the four CLI help checks exactly:

```bash
cargo run -p binance-vision-cli -- --help 2>&1 | tee "$EVIDENCE_DIR/local/binance-help.txt"
cargo run -p kraken-historical-cli -- --help 2>&1 | tee "$EVIDENCE_DIR/local/kraken-help.txt"
cargo run -p okx-historical-cli -- --help 2>&1 | tee "$EVIDENCE_DIR/local/okx-help.txt"
cargo run -p bybit-historical-cli -- --help 2>&1 | tee "$EVIDENCE_DIR/local/bybit-help.txt"
```

Pass criteria: each command exits zero on the release commit; help output identifies the expected binary and subcommands with no panic, missing configuration failure, or unexpected network request; formatting, Clippy, tests, and shell syntax have no failures; ignored live tests remain separately identified until the live gate.

### 4.4 Live integration smoke gates

These tests are ignored by default because they require network access. Run each explicitly on a host with approved egress, retain output and UTC timestamp, and do not treat a smoke result as dataset-completeness evidence.

```bash
cargo test -p binance-vision --test integration -- --ignored --nocapture \
  2>&1 | tee "$EVIDENCE_DIR/live/binance-integration.txt"
cargo test -p kraken-historical --test integration -- --ignored --nocapture \
  2>&1 | tee "$EVIDENCE_DIR/live/kraken-integration.txt"
cargo test -p okx-historical --test integration -- --ignored --nocapture \
  2>&1 | tee "$EVIDENCE_DIR/live/okx-integration.txt"
```

Current test coverage exercises a live BTCUSDT UM archive download/ZIP extraction/non-empty parse for Binance Vision, public Google Drive discovery and complete/quarterly trades artifacts for Kraken, and a live BTC-USDT spot-trades daily CDN archive with non-trivial bytes for OKX. Pass criteria are zero exit status and selected-test assertions, with resolved URL/source, HTTP status behavior, archive size, and test name recorded. A live smoke failure blocks broad launch; a pilot excluding an exchange must say so and must not imply that exchange was validated.

### 4.5 Clean-host deployment gate

Use a disposable or dedicated ingest host, not a developer machine and not the cleaned assessment VPS. Confirm documented Rust version, sufficient disk, network egress, secret-manager access, and destination bucket/database access. Capture host identity, OS/kernel, free disk, and tool versions without credentials.

#### Binance bounded deployment example

From [Binance DEPLOY](binance/DEPLOY.md), use future-host setup and schema commands. Verify MergeTree `binance_vision` schema and never use legacy `binance.agg_trades` scripts.

```bash
cd /path/to/historical-rs
./scripts/binance/install.sh --dir .
cp .env.example ~/.env                 # fill via approved secret mechanism
set -a && source <(sed 's/\r$//' ~/.env) && set +a
clickhouse-client --password "$CLICKHOUSE_PASSWORD" --multiquery \
  < sql/clickhouse/010_binance_vision_merge_tree.sql
SYMBOLS=BTCUSDT FROM=2026-07-10 TO=2026-07-12 \
  ./scripts/binance/clickhouse-direct-trades.sh
./target/release/binance-vision mirror status --s3
```

For broad release, run documented four-product paths and S3 sync with declared `DATA_TYPES`, `KLINE_INTERVALS`, market, symbols, and window. The one-symbol command is only a bounded smoke/pilot example.

#### Kraken bounded deployment example

From [Kraken DEPLOY](kraken/DEPLOY.md):

```bash
cd /path/to/historical-rs
./scripts/kraken/install.sh --dir .
./target/release/kraken-historical discover --data-type trades
FROM_QUARTER=2024Q1 TO_QUARTER=2024Q1 SOURCE_MODE=quarterly PAIRS=XBTUSD \
  ./scripts/kraken/sync-s3.sh
./target/release/kraken-historical mirror status --s3
./scripts/kraken/loop-status.sh
```

The quarterly command is a bounded smoke example. Broad scope additionally requires declared trades and OHLCVT/complete-ZIP coverage, or an explicitly narrowed contract.

#### OKX bounded deployment example

From [OKX DEPLOY](okx/DEPLOY.md):

```bash
cd /path/to/historical-rs
./scripts/okx/install.sh
./target/release/okx-historical mirror status --s3
./scripts/okx/loop-status.sh
```

For a bounded credentialed smoke:

```bash
FROM=2024-01-01 TO=2024-01-01 SYMBOLS=BTC-USDT \
  ./scripts/okx/sync-s3.sh
```

Do not install cron until backfill and reconciliation gates are green on the target host. For broad unattended operation, retain evidence from `./scripts/okx/install-cron.sh` (and corresponding [Binance](binance/DEPLOY.md) and [Kraken](kraken/DEPLOY.md) procedures), supervisor/alerting configuration, and an operator test of a failed job.

Clean-host pass criteria:

- Fresh checkout builds release binaries with documented install command.
- Secrets are injected by the approved mechanism and absent from checkout, process listings where avoidable, and captured logs.
- Bounded run writes only declared destination prefix/table and produces a status/manifest artifact.
- Disk, object-store, and (for Binance) ClickHouse capacity are checked before and after run.
- No legacy or forbidden write path is active.

### 4.6 Forced interruption and resume gate

Perform this on a clean host against a small disposable declared window before a full backfill. Save first-run manifest/status and log. Send an intentional interrupt after at least one item completes and one remains pending. A supervisor may send `SIGINT`; this reproducible foreground method uses `timeout`:

```bash
# Example only: use the release's approved bounded scope and host env.
timeout --signal=INT 30s env \
  FROM=2024-01-01 TO=2024-01-03 SYMBOLS=BTCUSDT \
  ./scripts/binance/sync-s3.sh \
  > "$EVIDENCE_DIR/deploy/binance-interrupted.log" 2>&1 || test $? -eq 124
```

Then rerun the same command without `timeout`, using the same bucket/prefix, manifest, scope, and `--resume` behavior:

```bash
env FROM=2024-01-01 TO=2024-01-03 SYMBOLS=BTCUSDT \
  ./scripts/binance/sync-s3.sh \
  2>&1 | tee "$EVIDENCE_DIR/deploy/binance-resume.log"
```

Apply the same controlled procedure to relevant Kraken and OKX scripts:

```bash
timeout --signal=INT 30s env FROM_QUARTER=2024Q1 TO_QUARTER=2024Q1 \
  SOURCE_MODE=quarterly PAIRS=XBTUSD ./scripts/kraken/sync-s3.sh \
  > "$EVIDENCE_DIR/deploy/kraken-interrupted.log" 2>&1 || test $? -eq 124
FROM_QUARTER=2024Q1 TO_QUARTER=2024Q1 SOURCE_MODE=quarterly PAIRS=XBTUSD \
  ./scripts/kraken/sync-s3.sh 2>&1 | tee "$EVIDENCE_DIR/deploy/kraken-resume.log"

timeout --signal=INT 30s env FROM=2024-01-01 TO=2024-01-03 SYMBOLS=BTC-USDT \
  ./scripts/okx/sync-s3.sh \
  > "$EVIDENCE_DIR/deploy/okx-interrupted.log" 2>&1 || test $? -eq 124
FROM=2024-01-01 TO=2024-01-03 SYMBOLS=BTC-USDT \
  ./scripts/okx/sync-s3.sh 2>&1 | tee "$EVIDENCE_DIR/deploy/okx-resume.log"
```

If a process exits before timeout due to legitimate completion, repeat with a larger bounded scope; do not call an uninterrupted run a recovery test. Pass criteria: interrupt exits cleanly or records intentional signal with no corrupted final object or silently lost ledger entry; resume discovers completed work and skips it (`--resume`/S3 HEAD or exchange ledger), then completes pending work; reconciliation shows exactly expected object/key set, no duplicate logical keys, no missing rows, and zero unexplained failed keys; a second identical resume is idempotent and reports only expected skips.

### 4.7 Malformed input, HTTP retry, and partial-write gates

Exercise these in a deterministic staging fixture or approved local HTTP fault-injection service; never corrupt a production object to test failure handling. Existing parser and fixture tests are part of the local gate, but retain the fault scenario and observed output.

Required scenarios:

1. **Malformed archive/CSV:** truncated ZIP, ZIP with no supported CSV member, malformed header/row, invalid integer/float/bool, and malformed checksum text. The command must fail with a clear parse/checksum error, leave no promoted final parquet/object, and record the key as failed or pending for deliberate retry.
2. **404:** request a known missing upstream key. It must be classified as not found/approved skip according to the exchange path, without retry storming or a false successful object.
3. **Server errors:** return HTTP 500/502/503 for controlled attempts and then 200. The client must retry according to configured `max_retries`/backoff and eventually succeed; retain request-attempt logs or proxy counter.
4. **Rate limit:** return HTTP 429. Kraken and OKX clients configure rate-limit retry; Binance currently does not retry 429, so record actual policy and ensure resulting failure/skip is surfaced rather than treated as success. Any broad-launch policy change must be implemented and tested before relying on it.
5. **Partial transfer/write:** interrupt during download or upload. A `.partial`/temporary file must not count as a final object, and resume must safely retry it.
6. **Checksum mismatch:** provide a wrong checksum and verify that object is rejected and mismatch recorded.

Pass means observed behavior matches documented client policy, no corrupt output is promoted, retries are bounded, and failed key remains visible for operator action. A test that merely returns HTTP 200 without exercising a failure state is not a retry test.

## 5. Reconciliation and recovery

Reconciliation is a data-owner gate, not merely an operator convenience. It is against the frozen declared scope, not against whatever happened to finish. If promised coverage remains incomplete, leave the status **resume-later/incomplete** and keep unattended jobs disabled.

### 5.1 Required reconciliation evidence

For every promised market/type/window, retain:

- expected source keys or source listing snapshot, including known 404/not-published days;
- an expected-key manifest generated from release scope (exchange, type, market, symbol, interval, and UTC date/quarter), stored at `$EVIDENCE_DIR/reconcile/expected-keys.*` with its generation method;
- S3 `mirror status --s3` totals and failed entries;
- destination object inventory/count and representative key checks from the approved S3 administration tool;
- manifest entries and checksum flags where the CLI reports them;
- Binance ClickHouse `ingest_log` status counts, active-part rows/bytes, symbol coverage, and minimum/maximum timestamps;
- row counts and date bounds by type/market/window, using bounded queries appropriate to table size;
- failed-key list, retry outcome, and written explanation for every intentional skip/404;
- no legacy Binance `aggTrades` objects or `binance.agg_trades` writes.

Run the exchange status commands and capture them:

```bash
./target/release/binance-vision mirror status --s3
./target/release/kraken-historical mirror status --s3
./target/release/okx-historical mirror status --s3
./target/release/bybit-historical mirror status --s3
```

For each declared scope, reconcile:

| Quantity | Required comparison |
|---|---|
| Expected logical keys | Every expected key has one terminal `ok`/valid skipped state; no unexpected key is accepted without scope approval. |
| Manifest entries | `done + skipped + failed + pending` equals manifest total; `failed=0` and `pending=0` for a completed broad gate. |
| Destination objects | Object count and key set equal expected successful output set; a skipped key is already present and verified, not absent. |
| Object integrity | Size is non-zero; parquet opens; checksum is verified where source provides one; no `.partial` object is promoted as final. |
| Rows | Sum parquet row counts (and ClickHouse rows) by type/symbol/interval/window equals independently expected/reconciled count. Empty source files are approved skips, not silently dropped. |
| Bounds and coverage | Minimum/maximum timestamps, symbol set, intervals, and type coverage match declared scope. |

Use the provider’s object-listing command appropriate to the endpoint and retain the raw listing; do not rely on a console screenshot alone. The durable resume source is each exchange’s S3/JSONL manifest. For local JSONL inspection, use a parser or `jq` only after confirming manifest path, and redact URLs/credentials from saved copies. Do not manufacture row totals from object counts.

The CLI’s `mirror status --s3` and scripts’ status helpers are implemented. Complete source listings, object-store quota checks, cross-destination reconciliation, and product sign-off are host/data-owner controls, not one repository command. Do not claim a data gate is green without retained evidence for every declared type and interval.

### 5.2 Binance ClickHouse reconciliation

Run bounded queries, capturing schema and data evidence:

```bash
set -a && source <(sed 's/\r$//' ~/.env) && set +a
CH=(clickhouse-client --host "${CLICKHOUSE_HOST:-127.0.0.1}" \
  --user "${CLICKHOUSE_USER:-default}" --password "$CLICKHOUSE_PASSWORD")
"${CH[@]}" -q "
SELECT name, engine FROM system.tables
WHERE database='binance_vision'
  AND name IN ('trades','klines','book_depth','bvol','ingest_log');
"
"${CH[@]}" -q "
SELECT database, table, formatReadableSize(sum(bytes_on_disk)) AS size,
       sum(rows) AS rows
FROM system.parts
WHERE active AND database='binance_vision'
GROUP BY database, table ORDER BY sum(bytes_on_disk) DESC;
"
"${CH[@]}" -q "
SELECT status, count() FROM binance_vision.ingest_log
WHERE startsWith(s3_key, 'direct-trades/')
GROUP BY status ORDER BY status;
"
"${CH[@]}" -q "
SELECT uniqExact(symbol) AS symbols, min(time), max(time)
FROM binance_vision.trades WHERE kind='UM' AND time >= today() - 7;
"
df -h /var/lib/clickhouse
"${CH[@]}" -q "SELECT formatReadableSize(free_space) FROM system.disks;"
```

For a quick ledger check:

```bash
"${CH[@]}" -q "
SELECT status, count()
FROM binance_vision.ingest_log
GROUP BY status ORDER BY status;
"
```

For a bounded recent coverage smoke check:

```bash
"${CH[@]}" -q "
SELECT uniqExact(symbol) AS symbols, min(time), max(time)
FROM binance_vision.trades
WHERE kind='UM' AND time >= today() - 7;
"
```

For broad Binance scope, all four product tables and corresponding S3 archive must be covered. Confirm zero writes to `binance.agg_trades`, and retain query/schema evidence proving `binance_vision` MergeTree plus `ingest_log` dedupe.

### 5.3 Interruption and resume/recovery procedure

Mirrors are designed to resume: local/S3 mirrors use manifests or S3 HEAD checks, and Binance ClickHouse work uses `ingest_log`. Resume safety reduces duplicate work but does not prove no gaps. Every interruption requires reconciliation:

1. **Stop and identify.** Confirm whether process exited, was killed, or lost connectivity. Record last log line, UTC time, command scope, revision, and pidfile/session.
2. **Prevent overlap.** Confirm no second copy is running. Do not start a broad retry while a supervisor is still restarting the old process.
3. **Classify cause.** Check disk/inodes, ClickHouse free space, destination permissions, upstream status/rate limits, malformed/checksum errors, and host restarts.
4. **Preserve evidence.** Copy relevant log and current local manifest before repair. Do not truncate or rewrite a manifest by hand.
5. **Repair cause.** Free space, fix credentials/permissions, wait for upstream recovery, or route a Drive quota issue to the owner. Do not mark failed keys complete.
6. **Resume the same bounded scope.** Re-run the exact wrapper with `--resume`, retaining `FROM`, `TO`, symbols/pairs, markets, data types, and destination. Changed scope is a new run and must be recorded.
7. **Retry failed Binance direct keys only after review.** The documented operation is:

```bash
"${CH[@]}" -q "
ALTER TABLE binance_vision.ingest_log
DELETE WHERE startsWith(s3_key, 'direct-trades/') AND status='failed';
"
```

This is destructive ledger maintenance: back up evidence first, limit prefix/type, and use it only when the owner confirms failures are retryable. Do not run it for unrelated types or to hide unresolved failures.

8. **Reconcile before declaring recovery.** Compare manifest/ledger counts, object/destination inventory, row counts, date bounds, symbols, and failed keys. A clean process exit is not completion evidence.

### 5.4 Backup and restore: manifests and destinations

A resume state is split across local manifests, S3 manifests, destination objects, and (for Binance ClickHouse) `ingest_log`. Known manifest keys are:

- Binance S3: `manifest/binance-vision/manifest.jsonl`
- Kraken S3: `historical/manifest/kraken-historical/manifest.jsonl`
- OKX S3: `historical/manifest/okx-historical/manifest.jsonl`
- Bybit S3: `historical/manifest/bybit/orderbook/manifest.jsonl` and `historical/manifest/bybit/trade/spot/daily/manifest.jsonl`

Local manifests (where local mirrors exist) default to `~/.data/binance-vision/manifest.jsonl`, `~/.data/kraken-historical/manifest.jsonl`, and `~/.data/okx-historical/manifest.jsonl`. Bybit’s Rust mirror is S3-manifest only. Optional `*_DATA_DIR` variables in [`.env.example`](../.env.example) can change local roots.

Before a full backfill, credential rotation, upgrade, or restore test:

- [ ] Stop all writers and confirm no supervisor/cron restart is pending.
- [ ] Copy relevant local manifest and record a SHA-256 checksum.
- [ ] Capture S3 manifest object and destination inventory using provider-approved backup/versioning/snapshot mechanism.
- [ ] Back up ClickHouse schema metadata and `binance_vision.ingest_log`/destination data according to ClickHouse host policy.
- [ ] Record bucket, endpoint, region, prefix, database, revision, and backup timestamp without secret values.
- [ ] Verify backup is readable from designated restore location.

A simple local manifest capture (for an existing manifest) is:

```bash
BACKUP_DIR=/approved/backup/path/$(date -u +%Y%m%dT%H%M%SZ)
install -d -m 700 "$BACKUP_DIR"
for pair in \
  "binance:${BINANCE_VISION_DATA_DIR:-$HOME/.data/binance-vision}" \
  "kraken:${KRAKEN_HISTORICAL_DATA_DIR:-$HOME/.data/kraken-historical}" \
  "okx:${OKX_HISTORICAL_DATA_DIR:-$HOME/.data/okx-historical}"; do
  name=${pair%%:*}; root=${pair#*:}
  test -f "$root/manifest.jsonl" || { echo "missing $name manifest" >&2; exit 1; }
  cp --preserve=mode,timestamps "$root/manifest.jsonl" "$BACKUP_DIR/$name-manifest.jsonl"
done
sha256sum "$BACKUP_DIR"/*-manifest.jsonl | tee "$BACKUP_DIR/SHA256SUMS"
```

`/approved/backup/path` is a placeholder for a real, access-controlled backup destination chosen by the platform owner; it is not an infrastructure claim. The repository does not implement backup scheduling, object versioning, ClickHouse snapshots, or restore drills.

Restore procedure:

1. Declare an incident/change and assign an owner. Stop all writers and disable any restart/cron trigger.
2. Preserve current manifest, destination inventory, and logs before changing anything.
3. Restore destination objects/database and matching manifest/ledger snapshot from the same backup point. Restoring a manifest without its destination can cause `--resume` to skip missing data.
4. Restore only to an approved destination and verify permissions, schema, object counts, checksums, and manifest readability.
5. Start a bounded smoke scope with `--resume`; reconcile against pre-incident evidence.
6. Resume approved window only after data owner accepts result. Record what was restored and intentionally re-downloaded keys.

A backup never restored in a non-production location is not evidence of recoverability. Schedule and retain a host-level restore drill before unattended launch.

### 5.5 Credential rotation

Credential rotation is a controlled change, not a reason to delete manifests or restart the entire backfill.

1. Open a change/incident record and identify affected exchange, destination, owner, and current run.
2. Stop affected writer gracefully and confirm no duplicate process remains. Preserve logs and manifest checksum.
3. Issue a new least-privilege key through approved secret owner. Do not print old or new values.
4. Update host secret source atomically, retaining mode `0600`; do not edit tracked template with real values.
5. Start a bounded `mirror status --s3`/destination-read check, then a one-day or one-quarter approved smoke scope.
6. Confirm writes and reads succeed, alerts are quiet, and reconciliation shows no new gaps.
7. Revoke old key, invalidate exposed shell/session material, and record rotation time and owner.
8. Resume exact prior scope with `--resume`; do not reset manifests or delete destination data merely because credentials changed.

If a secret may have appeared in logs, shell history, process arguments, or a ticket, stop affected jobs, revoke it immediately, preserve evidence for the security owner, and rotate all potentially exposed credentials.

### 5.6 Upstream failure handling

Clients include TLS, request timeouts, retries, and handling for 404, server errors, and rate limits, but these do not guarantee upstream availability. Distinguish expected absence from failure:

- A 404/not-published source day can be recorded as an intentional skip only when it matches documented coverage and the data owner accepts it.
- Repeated 429/rate limits require reducing parallelism, waiting for upstream window, and resuming the same scope; do not launch more workers.
- Repeated 5xx, timeouts, TLS, DNS, or CDN failures require checking upstream/platform status and pausing if retries continue without progress.
- Malformed archives or checksum mismatches are data-quality incidents. Preserve key/error, do not mark it complete, and escalate with source URL and checksum evidence.
- Destination 401/403, bucket, region, or endpoint errors require credential/platform-owner action; do not change prefixes or buckets ad hoc.
- Kraken Drive anonymous quota failures may be retried on ingest host with `scripts/kraken/retry-quota-s3.sh` only after quota owner confirms it cleared. This is not a bypass for quota policy.
- If an upstream format, listing, symbol set, or endpoint changes, stop affected data type, retain failing sample/log, and open an upstream-change issue. Do not patch around it by enabling a legacy Binance path.

After upstream recovery, run the smallest approved smoke window, inspect `done/skipped/failed`, and reconcile before resuming wider window. Persistent failure with no owner or escalation path is a stop condition.

### 5.7 Incident closeout

An incident is closed only when the owner records:

- timeline, affected exchange/type/market/window, revision, and root cause;
- process and alert evidence, including first and last failed keys;
- actions taken to protect capacity, credentials, manifests, and destination data;
- resume command and resulting reconciliation (objects, rows, dates, symbols, failed keys);
- whether data was restored, re-downloaded, intentionally skipped, or remains incomplete;
- follow-up owner and due date for missing host controls or upstream escalation.

If any promised coverage remains incomplete, leave status **resume-later/incomplete** and keep unattended jobs disabled. A passing command or quiet process is never, by itself, a completion claim.

## 6. Security and supply chain

The readiness assessment recorded that advisory, license-policy, and ShellCheck checks were not previously present; their absence is a broad-launch blocker, not a pass. Run security checks used by release CI. If CI has not been updated, record `BLOCKED` and do not silently substitute local success.

Where the release CI exposes these commands, capture exact versions and output:

```bash
cargo audit 2>&1 | tee "$EVIDENCE_DIR/security/cargo-audit.txt"
cargo deny check 2>&1 | tee "$EVIDENCE_DIR/security/cargo-deny.txt"
shellcheck scripts/binance/*.sh scripts/kraken/*.sh scripts/okx/*.sh \
  2>&1 | tee "$EVIDENCE_DIR/security/shellcheck.txt"
```

Also require:

- dependency lockfile and release source are the reviewed commit; advisory and license findings are resolved or explicitly accepted by owner with expiry;
- release artifacts have reproducible build record, checksum, and provenance/attestation appropriate to distribution channel;
- secret scan over tracked tree and captured release logs finds no credentials, private endpoints, or tokens; check deploy host process/environment and shell history according to host policy;
- production credentials use approved secret mechanism; rotation is exercised or has current evidence, and access is least-privilege for destination prefix/database;
- shell scripts use documented `set -euo pipefail` behavior and no unreviewed command substitution or unsafe path is introduced.

A missing tool, missing workflow job, or unreviewed finding is `FAIL`/`BLOCKED` for broad launch. Do not claim a tool passed when it was unavailable.

## 7. Launch sign-off template

Store the evidence package under `$EVIDENCE_DIR` (or an approved immutable evidence store). It must include:

- `commit.txt`, toolchain/version output, scope declaration, and launch mode (`BROAD` or `PILOT`);
- local CI and CLI help outputs with exit statuses;
- each live integration output, UTC timestamp, upstream URLs, and approved network/credential preconditions;
- clean-host setup transcript, host/tool/disk snapshot, install/build output, destination schema confirmation, and redacted logs;
- interrupt and resume logs, before/after status, and idempotent second-resume result;
- expected-key manifest, object listing, mirror manifests/status, parquet row/bounds report, ClickHouse reconciliation queries/results, and evidence-file checksums;
- malformed/retry fault scenarios and observed outcomes;
- security tool versions/results, dependency/license decisions, secret-scan result, artifact checksum/provenance, and waiver records;
- incident owner, on-call route, monitoring/alert links, capacity thresholds, backup/restore result, and rollback/stop procedure for launch mode.

Example evidence inventory command (the evidence directory must not contain secrets):

```bash
( cd "$EVIDENCE_DIR" && find . -type f -print0 | sort -z | xargs -0 sha256sum ) \
  > "$EVIDENCE_DIR/evidence-sha256.txt"
```

Copy this template into the release ticket or sign-off system; do not mark a box without linking its evidence artifact:

```text
Release ID / commit:
Validation date (UTC):
Validator:
Launch mode: BROAD | PILOT
Public/internal scope (exchanges, markets, types, symbols, intervals, UTC window):
Explicit exclusions and deferred data products:
Evidence root / immutable bundle:

Gate                                      Status (PASS/FAIL/BLOCKED/WAIVED)  Evidence
Scope frozen and completeness contract    _________________________________  __________
Local CI (fmt/Clippy/tests/shell syntax)  _________________________________  __________
CLI help smoke checks                     _________________________________  __________
Binance ignored live smoke                _________________________________  __________
Kraken ignored live smoke                 _________________________________  __________
OKX ignored live smoke                    _________________________________  __________
Clean-host deployment                     _________________________________  __________
Forced interruption and resume            _________________________________  __________
Object/manifest reconciliation             _________________________________  __________
Parquet row/bounds reconciliation          _________________________________  __________
ClickHouse reconciliation (if in scope)  _________________________________  __________
Malformed input and partial-write checks  _________________________________  __________
HTTP 404/5xx/429 retry checks             _________________________________  __________
Dependency advisory/license checks        _________________________________  __________
ShellCheck and secret scan                _________________________________  __________
Artifact provenance/reproducibility       _________________________________  __________
Operations/alerts/capacity/backup         _________________________________  __________

Open failures or accepted risks:
Waiver owner, rationale, expiry, and rollback trigger:

Decision: APPROVE | APPROVE PILOT ONLY | HOLD | REJECT
Decision scope (must match the declared contract above):
Release owner:
Operations owner:
Security owner:
Data owner:
Approver and timestamp (UTC):
```

A `BROAD` approval requires all mandatory gates to be `PASS` or an explicitly approved, time-bounded waiver that does not hide incomplete data. If data completeness does not match declared launch scope, the decision is `HOLD` regardless of local test results. A `PILOT` approval must retain narrower scope and exclusions in the release artifact and must not be reused as broad-launch evidence.
