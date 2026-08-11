# historical-rs

Cargo workspace for historical crypto market-data mirrors:

| Binary | Crate | Source | Env prefix |
|--------|-------|--------|------------|
| `binance-vision` | `binance-vision-cli` | Binance Vision | `S3_BINANCE_VISION_*` |
| `kraken-historical` | `kraken-historical-cli` | Kraken Google Drive | `S3_KRAKEN_HISTORICAL_*` |
| `okx-historical` | `okx-historical-cli` | OKX CDN | `S3_OKX_HISTORICAL_*` |
| `bybit-historical` | `bybit-historical-cli` | Bybit portal | `S3_BYBIT_HISTORICAL_*` |
| `deribit-historical` | `deribit-historical-cli` | Deribit history API | `S3_DERIBIT_HISTORICAL_*` |

Each exchange keeps its own env prefix, S3 layout, and JSONL manifest so interrupted mirrors can resume from object storage (or local JSONL where supported) without unifying configs.

Shared plumbing lives in `historical-core` (progress, S3 config/manifest helpers, tracing, HTTP retry).

## Capabilities

| Exchange | Local parquet | S3 parquet | ClickHouse | Data types |
|----------|:-------------:|:----------:|:----------:|------------|
| Binance Vision | yes | yes | yes (`binance_vision.*`) | `trades`, `klines`, `bookDepth`, `BVOLIndex` |
| Kraken | yes | yes | — | `trades`, `ohlcvt` |
| OKX | yes | yes | — | `trades`, `candlesticks`, `funding`, `borrowrates` |
| Bybit | helper script only | yes | — | Spot `orderbook` / Spot+Contract `trade` |
| Deribit | — | yes | — | BTC/ETH futures public `trade` |

Bybit’s Rust CLI mirrors directly to S3. For a raw local download of portal archives, use `scripts/bybit/download-orderbook.py`.
Deribit mirrors public trades from `history.deribit.com` (auth recommended for production rate limits).

## Quick start

**Prerequisites:** Rust **1.88** (see `rust-toolchain.toml`), network egress for upstream sources, and (for S3) credentials in `~/.env`.

```bash
cd /path/to/historical-rs
cp .env.example ~/.env   # fill exchange sections you need

cargo build --release --workspace
cargo test --workspace --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings

# Local mirrors (Binance / Kraken / OKX)
./scripts/binance/mirror-local.sh
./scripts/kraken/mirror-local.sh
./scripts/okx/mirror-local.sh

# S3 mirrors (requires S3_* credentials)
./scripts/binance/sync-s3.sh
./scripts/kraken/sync-s3.sh
./scripts/okx/sync-s3.sh
./scripts/bybit/sync-s3.sh
./scripts/deribit/sync-s3.sh
```

Or invoke CLIs directly:

```bash
./target/release/binance-vision mirror run --market um --data-type trades \
  --symbols BTCUSDT --from 2024-01-01 --to 2024-01-02

./target/release/kraken-historical mirror run --data-type trades --source quarterly

./target/release/okx-historical mirror run --market spot --data-type trades \
  --symbols BTC-USDT --from 2024-01-01 --to 2024-01-02

./target/release/bybit-historical mirror s3 --symbols BTCUSDT \
  --from 2024-01-01 --to 2024-01-02

./target/release/deribit-historical mirror s3 --currency BTC \
  --instruments BTC-PERPETUAL --from 2024-01-01 --to 2024-01-01
```

Practical operator walkthrough: [docs/USAGE.md](docs/USAGE.md). Configuration template: [.env.example](.env.example).

## Resume from S3

`--resume` defaults to **true** on `mirror run` / `mirror s3` for exchange CLIs (pass `--resume false` to force re-upload checks via HEAD only / ignore completed keys as appropriate per exchange).

After a host wipe or redeploy, point the same `S3_*` credentials at the existing bucket and re-run the exchange sync script (or CLI). Completed objects are skipped using:

1. The per-exchange JSONL manifest (root + `.segments/*.jsonl`)
2. A listing of existing data keys under the exchange data prefix

Local progress under `~/.data/{binance-vision,kraken-historical,okx-historical}/` is optional; **S3 manifests are the durable resume source for production**. Bybit is S3-manifest only for the Rust mirror (`historical/manifest/bybit/orderbook/manifest.jsonl`). Deribit resumes from `historical/data/trade/` listings + `historical/manifest/deribit/trade/manifest.jsonl`.

## Layout

```
historical-rs/
├── crates/
│   ├── historical-core/
│   ├── binance-vision{,-cli}/
│   ├── kraken-historical{,-cli}/
│   ├── okx-historical{,-cli}/
│   ├── bybit-historical{,-cli}/
│   └── deribit-historical{,-cli}/
├── sql/clickhouse/                 # Binance ClickHouse DDL
├── scripts/{binance,kraken,okx,bybit,deribit}/
├── docs/
│   ├── README.md                   # Documentation index
│   ├── USAGE.md                    # How to build, configure, and run
│   ├── ARCHITECTURE.md             # Shared workspace architecture
│   ├── PRODUCTION.md               # Production readiness / ops / release
│   └── {binance,kraken,okx,bybit,deribit}/ # Exchange README / status / deploy
```

## Docs

| Path | Contents |
|------|----------|
| [docs/USAGE.md](docs/USAGE.md) | How to use: build, configure, mirror, resume |
| [docs/README.md](docs/README.md) | Documentation index |
| [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) | Shared workspace architecture |
| [docs/PRODUCTION.md](docs/PRODUCTION.md) | Production readiness, operations, release |
| [Binance](docs/binance/README.md) · [Kraken](docs/kraken/README.md) · [OKX](docs/okx/README.md) · [Bybit](docs/bybit/README.md) · [Deribit](docs/deribit/README.md) | Exchange entrypoints |
| [NEXT.md](NEXT.md) | Remaining work before broad production readiness |

Exchange ops notes under `docs/{binance,kraken,okx,bybit,deribit}/` target a **future** ingest host — not a cleaned assessment VPS.

## License

MIT — ckirua.
