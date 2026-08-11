# historical-rs documentation

This tree documents five exchange-specific historical-data pipelines and their shared architecture. Start with [how to use](USAGE.md), the shared [architecture](ARCHITECTURE.md), and the [production assessment and operating guide](PRODUCTION.md), then open the exchange landing page for scope, evidence, and deployment entrypoints.

## Shared guides

| Doc | Purpose |
|---|---|
| [USAGE.md](USAGE.md) | Build, configure, mirror, resume |
| [ARCHITECTURE.md](ARCHITECTURE.md) | Crates, invariants, resume flow |
| [PRODUCTION.md](PRODUCTION.md) | Readiness assessment, ops, release gates |
| [../NEXT.md](../NEXT.md) | Remaining work before broad production readiness |
| [../.env.example](../.env.example) | Environment template |

## Exchange entrypoints

| Exchange | Scope and current status | Deployment |
|---|---|---|
| [Binance Vision](binance/README.md) | `binance_vision` ClickHouse tables plus Vision-shaped S3 archive; [status](binance/STATUS.md) | [Binance deployment guide](binance/DEPLOY.md) |
| [Kraken Historical](kraken/README.md) | Drive-to-S3 trades and OHLCVT archive; [status](kraken/STATUS.md) | [Kraken deployment guide](kraken/DEPLOY.md) |
| [OKX Historical](okx/README.md) | CDN-to-S3 historical archive; [status](okx/STATUS.md) | [OKX deployment guide](okx/DEPLOY.md) |
| [Bybit Historical](bybit/README.md) | Portal Spot orderbook + trade → S3 parquet; [status](bybit/STATUS.md) | [Bybit deployment guide](bybit/DEPLOY.md) |
| [Deribit Historical](deribit/README.md) | history.deribit.com BTC/ETH futures trades → S3 parquet; [status](deribit/STATUS.md) | [Deribit deployment guide](deribit/DEPLOY.md) |

The status files are the evidence-backed source of truth for completion and resume-later state. Deployment instructions target a future ingest host where the source documents say so; a passing local test suite is not by itself production-completion evidence.
