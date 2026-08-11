# Bybit Historical

The Bybit download portal is queried as a data-driven catalog, not as a
hardcoded list of files. The verified request is:

```text
GET https://api2.bybit.com/quote/public/support/download/list-files
  ?bizType=spot|contract
  &productId=orderbook|trade
  &symbols=BTCUSDT
  &interval=daily
  &startDay=2024-01-01
  &endDay=2024-01-07
```

The `mirror symbols` command discovers the current Trading instrument universe
separately from the historical portal. It calls Bybit's public V5 instruments
endpoint:

```text
GET https://api.bybit.com/v5/market/instruments-info
  ?category=spot|linear|inverse
  &limit=1000
```

For `--market spot`, only `category=spot` is used. For `--market contract`,
the CLI unions `linear` and `inverse` Trading symbols (deduped, sorted). Only
instruments whose status is exactly `Trading` are returned. This is a live,
refresh-on-demand catalog; it is not a claim that every returned symbol has an
archive for every historical date.

Only these combinations are enabled:

| `market` / `bizType` | `data_type` / `productId` | Verified archive |
| --- | --- | --- |
| `spot` | `orderbook` | ZIP under `quote-saver.bycsi.com/orderbook/spot/...` |
| `spot` | `trade` | `.csv.gz` under `public.bybit.com/spot/...` |
| `contract` | `orderbook` | ZIP under `quote-saver.bycsi.com/orderbook/linear|inverse/...` |
| `contract` | `trade` | `.csv.gz` under `public.bybit.com/trading/...` |

Each catalog archive carries `market`, `data_type`, `interval`, `symbol`,
`date`, `url`, `filename`, and `size`. A missing listing for a requested
symbol/date is reported as unavailable; it is not evidence of complete
historical coverage.

Do **not** pass `linear` or `inverse` as CLI `--market`; the portal market id
is `contract`. Options, klines/funding, and unknown product identifiers remain
unsupported and fail clearly.

### Catalog and coverage semantics

For a requested dataset, symbol set, and inclusive date range, coverage
reconciliation deterministically evaluates one cell per symbol/date. A
successful object (`ok`) and a resume hit (`skipped`) are complete cells.
Cells with no matching manifest entry are **missing**. The mirror planner
records a portal date that was not listed, or a failed portal listing, as a
`failed` cell with an explanatory error; downloads, conversions, and uploads
can also fail. Missing and unavailable portal dates are never treated as
successful objects. Duplicate symbols are normalized case-insensitively, and
manifest entries for another dataset are ignored.

## Current status and deployment

- [Status](STATUS.md) — implementation and evidence boundaries for the
  data-driven Spot/Contract catalog.
- [Deploy on a future ingest host](DEPLOY.md) — build, environment, and
  resume-safe S3 sync. Do not start unattended backfills on a cleaned
  assessment VPS.

## Key commands

Run from the repository root:

```bash
./scripts/bybit/install.sh

# Both verified Spot datasets (orderbook + trade) → S3 Parquet.
SYMBOLS=BTCUSDT,ETHUSDT FROM=2024-01-01 TO=2024-01-07 \
  ./scripts/bybit/sync-s3.sh

# Contract (linear + inverse symbols) trade + orderbook.
BYBIT_MARKET=contract DATA_TYPES=orderbook,trade \
SYMBOLS=BTCUSDT,BTCUSD FROM=2024-01-01 TO=2024-01-07 \
  ./scripts/bybit/sync-s3.sh

# Or one dataset only.
DATA_TYPES=trade SYMBOLS=BTCUSDT FROM=2024-01-01 TO=2024-01-07 \
  ./scripts/bybit/sync-s3.sh

# Refresh and print current Trading symbols (no S3 access).
./target/release/bybit-historical mirror symbols --market spot
./target/release/bybit-historical mirror symbols --market contract

# Mirror an explicit symbol set.
./target/release/bybit-historical mirror s3 \
  --market spot --data-type orderbook --symbols BTCUSDT,ETHUSDT \
  --from 2024-01-01 --to 2024-01-07

# Or discover the current Trading symbols and mirror all of them.
./target/release/bybit-historical mirror s3 \
  --market spot --all-symbols --data-type orderbook \
  --from 2024-01-01 --to 2024-01-07

./target/release/bybit-historical mirror s3 \
  --market contract --all-symbols --data-type trade \
  --from 2024-01-01 --to 2024-01-07

./target/release/bybit-historical mirror status --s3 --market spot --data-type orderbook
./target/release/bybit-historical mirror status --s3 --market contract --data-type trade
```

`mirror s3` defaults to `--market spot`, `--data-type orderbook`, and
`--interval daily`; `--from` and `--to` are required. Use exactly one symbol
selection: `--symbols` for an explicit comma-separated list or `--all-symbols`
to fetch the market-appropriate Trading catalog. Supplying both, or neither, is
rejected. `--all-symbols` prints the discovered count before planning the
mirror. `--resume` is explicit in the helper and defaults to **true** in the
CLI. `scripts/bybit/sync-s3.sh` defaults to `DATA_TYPES=orderbook,trade` and
`BYBIT_MARKET=spot`.

## Optional raw local catalog download

`scripts/bybit/download-orderbook.py` downloads either verified product for
`spot` or `contract`. It writes raw files below
`$BYBIT_DATA_DIR/{data_type}/{market}/{SYMBOL}/{YYYY-MM-DD}/` and appends
descriptor/status JSONL records to the matching product directory:

```bash
BYBIT_DATA_TYPE=orderbook \
BYBIT_SYMBOLS=BTCUSDT \
BYBIT_FROM=2024-01-01 BYBIT_TO=2024-01-07 \
  python3 scripts/bybit/download-orderbook.py

BYBIT_MARKET=contract BYBIT_DATA_TYPE=trade \
BYBIT_SYMBOLS=BTCUSDT,BTCUSD \
BYBIT_FROM=2024-01-01 BYBIT_TO=2024-01-07 \
  python3 scripts/bybit/download-orderbook.py
```

The helper's optional `BYBIT_INTERVAL` must remain `daily`. `BYBIT_MARKET`
defaults to `spot` and also accepts `contract`.

## Boundaries

This operational lane covers verified Spot and Contract orderbook/trade portal
products above. Current-symbol discovery and deterministic symbol/date
reconciliation are available, but they do not prove that every current symbol
has every historical date. Options, klines, funding, ClickHouse ingest, and
unattended cron are not claimed.
