# How to use historical-rs

Practical guide for developers and operators. Exchange-specific product status and deploy recipes live under `docs/{binance,kraken,okx,bybit,deribit}/`.

## Prerequisites

- Rust **1.88** (pinned in `rust-toolchain.toml`; Clippy and rustfmt components recommended)
- Disk for local parquet when using local mirrors
- Outbound HTTPS to exchange sources (Vision CDN, Google Drive, OKX CDN, Bybit portal, Deribit history API)
- For S3 mirrors: S3-compatible credentials (see [`.env.example`](../.env.example))
- For Binance ClickHouse paths: a reachable ClickHouse and schema from `sql/clickhouse/`

## Configure

```bash
cp .env.example ~/.env
# Edit only the exchange sections you need. Never commit ~/.env.
set -a && source <(sed 's/\r$//' ~/.env) && set +a
```

Env prefixes stay separate on purpose:

| Exchange | Primary S3 vars | Parallel / data dir |
|----------|-----------------|---------------------|
| Binance | `S3_BINANCE_VISION_*` | `BINANCE_VISION_PARALLEL`, `BINANCE_VISION_DATA_DIR` |
| Kraken | `S3_KRAKEN_HISTORICAL_*` | `KRAKEN_HISTORICAL_PARALLEL`, `KRAKEN_HISTORICAL_DATA_DIR` |
| OKX | `S3_OKX_HISTORICAL_*` | `OKX_HISTORICAL_PARALLEL`, `OKX_HISTORICAL_DATA_DIR` |
| Bybit | `S3_BYBIT_HISTORICAL_*` | `BYBIT_HISTORICAL_PARALLEL` (scripts) |
| Deribit | `S3_DERIBIT_HISTORICAL_*` | `DERIBIT_HISTORICAL_PARALLEL`, `DERIBIT_CLIENT_ID`/`SECRET` |

Generic fallbacks `S3_URL`, `S3_ACCESS_KEY`, `S3_SECRET_KEY` are accepted when exchange-specific keys are unset. Exchange-specific values win.

## Build, test, lint

```bash
cargo build --release --workspace

cargo test --workspace --all-features
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

Release binaries land in `target/release/`:

- `binance-vision`
- `kraken-historical`
- `okx-historical`
- `bybit-historical`
- `deribit-historical`

Install helpers (build + optional PATH symlink / cargo install):

```bash
./scripts/binance/install.sh
./scripts/kraken/install.sh
./scripts/okx/install.sh
./scripts/bybit/install.sh
```

## Common workflows

### Local parquet mirror (Binance / Kraken / OKX)

Scripts default to a bounded window and `--resume`:

```bash
FROM=2024-01-01 TO=2024-01-07 SYMBOLS=BTCUSDT ./scripts/binance/mirror-local.sh
DATA_TYPES=trades SOURCE_MODE=quarterly ./scripts/kraken/mirror-local.sh
FROM=2024-01-01 TO=2024-01-07 SYMBOLS=BTC-USDT ./scripts/okx/mirror-local.sh
```

Equivalent CLI shapes:

```bash
./target/release/binance-vision mirror run --market um --data-type trades \
  --symbols BTCUSDT --from 2024-01-01 --to 2024-01-07

./target/release/kraken-historical mirror run --data-type trades --source quarterly

./target/release/okx-historical mirror run --market spot --data-type trades \
  --symbols BTC-USDT --from 2024-01-01 --to 2024-01-07
```

### S3 mirror (all four)

```bash
./scripts/binance/sync-s3.sh
./scripts/kraken/sync-s3.sh
./scripts/okx/sync-s3.sh
# Both Spot datasets (default DATA_TYPES=orderbook,trade).
SYMBOLS=BTCUSDT,ETHUSDT FROM=2024-01-01 TO=2024-01-07 ./scripts/bybit/sync-s3.sh
DATA_TYPES=trade SYMBOLS=BTCUSDT FROM=2024-01-01 TO=2024-01-07 ./scripts/bybit/sync-s3.sh
BYBIT_MARKET=contract DATA_TYPES=trade SYMBOLS=BTCUSDT,BTCUSD \
  FROM=2024-01-01 TO=2024-01-01 ./scripts/bybit/sync-s3.sh
# Deribit BTC/ETH/USDC futures trades (auth recommended).
INSTRUMENTS=BTC-PERPETUAL FROM=2024-01-01 TO=2024-01-01 \
  ./scripts/deribit/sync-s3.sh
```

Bybit CLI (S3 only; Spot/Contract orderbook + trade):

```bash
./target/release/bybit-historical mirror s3 \
  --data-type orderbook --symbols BTCUSDT --from 2024-01-01 --to 2024-01-07
./target/release/bybit-historical mirror s3 \
  --market contract --data-type trade --symbols BTCUSDT,BTCUSD \
  --from 2024-01-01 --to 2024-01-01
./target/release/bybit-historical mirror status --s3 --data-type orderbook
./target/release/bybit-historical mirror status --s3 --market contract --data-type trade
```

Deribit CLI (S3 only; BTC/ETH/USDC futures public trades):

```bash
./target/release/deribit-historical mirror s3 \
  --currency BTC --instruments BTC-PERPETUAL \
  --from 2024-01-01 --to 2024-01-01 --resume --parallel 2
./target/release/deribit-historical mirror status --s3
./target/release/deribit-historical mirror instruments --currency BTC,ETH
```

Optional raw local download (Python helper, not the Rust parquet pipeline):

```bash
BYBIT_DATA_TYPE=orderbook BYBIT_SYMBOLS=BTCUSDT \
BYBIT_FROM=2024-01-01 BYBIT_TO=2024-01-07 \
  python3 scripts/bybit/download-orderbook.py
BYBIT_MARKET=contract BYBIT_DATA_TYPE=trade BYBIT_SYMBOLS=BTCUSDT \
BYBIT_FROM=2024-01-01 BYBIT_TO=2024-01-01 \
  python3 scripts/bybit/download-orderbook.py
```

### Status / progress

```bash
./target/release/binance-vision mirror status --s3   # also supports local
./target/release/kraken-historical mirror status --s3
./target/release/okx-historical mirror status --s3
./target/release/bybit-historical mirror status --s3
./target/release/deribit-historical mirror status --s3
```

Loop helpers: `scripts/{binance,kraken,okx}/loop-status.sh`.

### Binance ClickHouse (optional)

Only Binance has a ClickHouse product path (`binance_vision.*`). See [binance/DEPLOY.md](binance/DEPLOY.md) and scripts under `scripts/binance/clickhouse-*.sh`. Do **not** write to legacy `binance.agg_trades`.

## Resume, manifests, and S3 behavior

1. **Default resume is on.** Exchange CLIs default `--resume` to `true`. Scripts also pass `--resume` explicitly where applicable.
2. **Disable when needed:** `--resume false`.
3. **Manifests** are JSONL journals. S3 stores a root object plus immutable `.segments/*.jsonl` shards; readers merge them. Stronger distributed locking is out of scope (see [ARCHITECTURE.md](ARCHITECTURE.md)).
4. **Resume index** combines successful manifest entries with a listing of existing data keys under the exchange data prefix. If listing or manifest load fails, mirrors fall back to per-object HEAD checks.
5. **Durable source of truth for production** is the S3 bucket + S3 manifest, not local `~/.data/...` files.
6. **Known S3 layout anchors:**

| Exchange | Data prefix (typical) | Manifest key |
|----------|----------------------|--------------|
| Binance | `data/` (Vision-shaped) | `manifest/binance-vision/manifest.jsonl` |
| Kraken | `historical/data/` | `historical/manifest/kraken-historical/manifest.jsonl` |
| OKX | `historical/data/` | `historical/manifest/okx-historical/manifest.jsonl` |
| Bybit | `historical/data/orderbook/` | `historical/manifest/bybit/orderbook/manifest.jsonl` |
| Deribit | `historical/data/trade/` | `historical/manifest/deribit/trade/manifest.jsonl` |

## Known limitations / sharp edges

- **Product completeness is not finished.** See [NEXT.md](../NEXT.md) and exchange `STATUS.md` files. This repo supports staging / supervised pilots; it is not an unattended full-history launch.
- **Bybit scope:** Spot and Contract `orderbook`/`trade` → S3 parquet. No ClickHouse, no Rust local parquet mirror, no options/klines/funding yet.
- **Deribit scope:** BTC/ETH/USDC futures public trades only. Auth credentials required for production backfill; no options/cron/ClickHouse in v1.
- **Kraken Drive quota:** anonymous Google Drive downloads can hit quota; use `scripts/kraken/retry-quota-s3.sh` only after quota clears.
- **OKX deferred:** L2 books, FUTURES/OPTION markets, ClickHouse ingest.
- **Live network tests are ignored** in the default suite; run with `-- --ignored` when you intentionally want upstream smoke checks.
- **Do not enable cron/tmux backfills on a cleaned assessment host** unless you own that decision; deploy docs target a future ingest host.
- **Single-writer assumption:** concurrent writers against the same S3 prefix/manifest can race; prefer one job per exchange scope.

## Where next

| Need | Doc |
|------|-----|
| Crate layout / invariants | [ARCHITECTURE.md](ARCHITECTURE.md) |
| Production readiness & release gates | [PRODUCTION.md](PRODUCTION.md) |
| Remaining work checklist | [NEXT.md](../NEXT.md) |
| Per-exchange deploy | [binance](binance/DEPLOY.md) · [kraken](kraken/DEPLOY.md) · [okx](okx/DEPLOY.md) · [bybit](bybit/DEPLOY.md) |
