# Deploy — historical-rs (Bybit)

Ops guide for the Bybit Spot/Contract catalog → S3 mirror on the ops host
(`Europe/Berlin` cron TZ). Repo root example: `/root/apps/historical-rs`.

## Verified catalog and current symbols

Historical archive planning uses Bybit's verified download listing endpoint:

```text
GET https://api2.bybit.com/quote/public/support/download/list-files
  bizType=spot|contract productId=orderbook|trade symbols=<SYMBOL>
  interval=daily startDay=<YYYY-MM-DD> endDay=<YYYY-MM-DD>
```

Supported historical dimensions are `market=spot|contract` with
`data_type=orderbook` (ZIP archives) or `data_type=trade` (`.csv.gz`
archives), both at `interval=daily`. Do not configure `linear`/`inverse` as
`--market`; those are V5 instrument categories under portal `contract`.
Options and other product identifiers remain unsupported.

`mirror symbols` discovers current Trading symbols independently of the
archive portal by calling:

```text
GET https://api.bybit.com/v5/market/instruments-info
  category=spot|linear|inverse limit=1000
```

`--market spot` uses `category=spot`. `--market contract` unions linear and
inverse Trading symbols. The client keeps only instruments with status
`Trading`. This is a live refresh-on-demand catalog, not a historical
universe: a currently Trading symbol may have no portal archive for a
requested date.

## Prerequisites

- Rust 1.88 (see repo `rust-toolchain.toml`)
- Hetzner/S3 credentials in `~/.env` (`S3_BYBIT_HISTORICAL_*`)
- Bucket (default name in template: `mercat-bybit`); objects under
  `historical/`
- Outbound HTTPS to `api.bybit.com`, `api2.bybit.com`,
  `quote-saver.bycsi.com`, and `public.bybit.com`

## Install

```bash
cd /root/apps/historical-rs   # or your checkout
./scripts/bybit/install.sh
./scripts/bybit/install.sh --global   # optional: cargo install onto PATH
./scripts/bybit/install-cron.sh       # daily incremental (host-local 18:00)
cp .env.example ~/.env                # fill Bybit S3 section if needed
set -a && source <(sed 's/\r$//' ~/.env) && set +a
```

## Daily incremental cron

Managed by `scripts/bybit/install-cron.sh` (do not hand-edit the marked block).

**Schedule times** follow the host’s local timezone (not UTC). On this host
(`Europe/Berlin`) that is CEST/CET wall clock. **FROM/TO** use `date -u` (UTC
calendar days). Staggered after Binance Vision 06/10/14 and before Deribit
22:00; away from OKX 02:00.

```cron
# Daily 18:00 local — yesterday UTC spot trade+orderbook → S3 (all Trading symbols)
0 18 * * * cd /root/apps/historical-rs && ENV_FILE=$HOME/.env FROM=$(date -u -d yesterday +%F) TO=$(date -u -d yesterday +%F) ALL_SYMBOLS=1 BYBIT_MARKET=spot DATA_TYPES=trade,orderbook ./scripts/bybit/sync-s3.sh >> /var/log/bybit-historical-s3-daily.log 2>&1
```

```bash
./scripts/bybit/install-cron.sh
./scripts/bybit/install-cron.sh --uninstall
crontab -l | sed -n '/BYBIT-HISTORICAL-CRON/,/BYBIT-HISTORICAL-CRON-END/p'
```

**Log:** `/var/log/bybit-historical-s3-daily.log`

## Environment

| Variable | Purpose |
|----------|---------|
| `S3_BYBIT_HISTORICAL_BUCKET` | Target bucket (template: `mercat-bybit`) |
| `S3_BYBIT_HISTORICAL_ENDPOINT` | Object storage endpoint |
| `S3_BYBIT_HISTORICAL_REGION` | e.g. `fsn1` |
| `S3_BYBIT_HISTORICAL_ACCESS_KEY` / `SECRET_KEY` | Credentials |
| `BYBIT_HISTORICAL_PARALLEL` | Mirror workers for `sync-s3.sh` (default 8) |
| `BYBIT_MARKET` | `spot` (default) or `contract` |
| `DATA_TYPES` / `BYBIT_DATA_TYPE` | S3 helper: comma-separated `orderbook,trade` (default both); single-type alias via `BYBIT_DATA_TYPE` |
| `BYBIT_INTERVAL` | Must be `daily` (default) |
| `SYMBOLS` | Explicit comma-separated symbols (default `BTCUSDT,ETHUSDT`) |
| `ALL_SYMBOLS` | When set, use the market-appropriate Trading catalog instead of `SYMBOLS` |

Short aliases `S3_BYBIT_BUCKET` / `S3_BYBIT_ACCESS_KEY` /
`S3_BYBIT_SECRET_KEY` are also accepted by the crate.

## Refresh and inspect the symbol catalog

These commands make no S3 request. They fetch the current Trading instruments,
print one symbol per line, and then print `count=N`:

```bash
./target/release/bybit-historical mirror symbols --market spot
./target/release/bybit-historical mirror symbols --market contract
```

The result is live and can change between runs. Save the printed list if an
audit requires the exact symbol set used for a later mirror.

## Resume-safe S3 sync

`scripts/bybit/sync-s3.sh` mirrors both verified datasets (`orderbook` and
`trade`) by default for the selected market. Start with a bounded window:

```bash
# Both verified Spot datasets (default DATA_TYPES=orderbook,trade).
SYMBOLS=BTCUSDT,ETHUSDT FROM=2024-01-01 TO=2024-01-07 \
  ./scripts/bybit/sync-s3.sh

# Contract trade + orderbook for an explicit symbol set.
BYBIT_MARKET=contract DATA_TYPES=orderbook,trade \
SYMBOLS=BTCUSDT,BTCUSD FROM=2024-01-01 TO=2024-01-07 \
  ./scripts/bybit/sync-s3.sh

# Single dataset.
DATA_TYPES=trade SYMBOLS=BTCUSDT,ETHUSDT FROM=2024-01-01 TO=2024-01-07 \
  ./scripts/bybit/sync-s3.sh

# Current Trading catalog for both datasets.
ALL_SYMBOLS=1 FROM=2024-01-01 TO=2024-01-07 ./scripts/bybit/sync-s3.sh

./target/release/bybit-historical mirror status --s3 --market spot --data-type orderbook
./target/release/bybit-historical mirror status --s3 --market contract --data-type trade
```

For the CLI, `--from` and `--to` are required. The defaults are
`--market spot`, `--data-type orderbook`, and `--interval daily`. Select
symbols in exactly one of these ways:

```bash
# Explicit, reproducible selection.
./target/release/bybit-historical mirror s3 \
  --market spot --data-type orderbook --symbols BTCUSDT,ETHUSDT \
  --from 2024-01-01 --to 2024-01-07 --parallel 8

# Live current Spot Trading selection.
./target/release/bybit-historical mirror s3 \
  --market spot --all-symbols --from 2024-01-01 --to 2024-01-07 --parallel 8

# Bounded Contract trade smoke (linear + inverse symbols).
./target/release/bybit-historical mirror s3 \
  --market contract --data-type trade --symbols BTCUSDT,BTCUSD \
  --from 2024-01-01 --to 2024-01-01 --parallel 4
```

`--symbols` and `--all-symbols` are mutually exclusive, and omitting both is
rejected. `--all-symbols` prints the resolved Trading symbol count before
portal planning. The CLI and `sync-s3.sh` convert verified archives to S3
Parquet.

`--resume` is explicit in the helper and defaults to true in the CLI. Existing
objects and successful manifest entries are skipped. Dimension-aware catalog
keys keep orderbook/trade and spot/contract products separated (Spot orderbook
keeps the legacy key layout).

## Coverage reconciliation

The public `reconcile_coverage` helper compares a dataset, unique
case-insensitive symbols, an inclusive date range, and manifest entries as
deterministic symbol/date cells. `ok` and resume `skipped` entries are
successful objects. A cell with no manifest entry is `missing`. The planner
records a portal date that was not listed, or a portal listing failure, as a
`failed` cell with an error; download, conversion, and upload errors are also
failed. Missing/unavailable portal dates are therefore never counted as
successful objects merely because the symbol is currently Trading.

## Optional raw local catalog download

Not required for the S3 Parquet pipeline:

```bash
BYBIT_DATA_DIR=$HOME/.data/bybit-historical \
BYBIT_MARKET=spot BYBIT_DATA_TYPE=orderbook \
BYBIT_SYMBOLS=BTCUSDT \
BYBIT_FROM=2024-01-01 BYBIT_TO=2024-01-07 \
  python3 scripts/bybit/download-orderbook.py
```

For Contract trades, use `BYBIT_MARKET=contract BYBIT_DATA_TYPE=trade`; the
helper retains the product and archive dimensions in its local path and status
JSONL.

## Boundaries

- Single-writer per bucket prefix/manifest.
- Spot and Contract orderbook/trade portal products only.
- Current-symbol discovery is not a claim of complete symbol/date history.
- No options / klines / funding coverage.
