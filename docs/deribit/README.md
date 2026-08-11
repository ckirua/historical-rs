# Deribit Historical

Public futures trades from `history.deribit.com` → daily parquet.zstd → S3.
DVOL volatility-index OHLC from `www.deribit.com` → separate S3 prefix.

## Scope (v1)

| Dimension | Values |
|-----------|--------|
| Currencies (trades) | `BTC`, `ETH`, `USDC` |
| Kind | `future` (includes perpetuals) |
| Data | Public trades + DVOL volatility-index candles |
| Options / spot / order book | Out of scope |

DVOL details (names, endpoints, S3 keys): [DVOL.md](./DVOL.md).

## API

```text
GET https://www.deribit.com/api/v2/public/get_instruments
  ?currency=BTC&kind=future&expired=false|true
  # Live catalog (history host returns a reduced set and omits DVOL futures)

GET https://history.deribit.com/api/v2/public/get_last_trades_by_instrument_and_time
  ?instrument_name=BTC-PERPETUAL&start_timestamp=...&end_timestamp=...&count=1000&sorting=asc

GET https://history.deribit.com/api/v2/public/get_last_trades_by_instrument
  ?instrument_name=BTC-PERPETUAL&start_seq=...&count=1000&sorting=asc

GET https://www.deribit.com/api/v2/public/get_volatility_index_data
  ?currency=BTC&start_timestamp=...&end_timestamp=...&resolution=3600
```

Day trade fetch bootstraps with the time endpoint, then walks `start_seq = max(page)+1`.
OAuth (when configured) uses `https://www.deribit.com/api/v2/public/auth`.
Volatility index and instrument listing are **www**; trade pages are **history**.

## Rate limits

See [EVIDENCE.md](./EVIDENCE.md) and [docs.deribit.com rate limits](https://docs.deribit.com/articles/rate-limits).

- Prefer authenticated sync (non-matching ≈ 20 req/s). Default mirror budget ≈ 12 rps.
- Unauthenticated smoke budget ≈ 2 rps (`Config::for_unauthenticated_smoke`).
- `get_instruments` capped at 1 req/s + file cache.
- Parallelism default **2** (max recommend 4).
- Code **10028** → exponential backoff + auth invalidate.

## S3 layout

```text
historical/data/trade/{currency}/{instrument}/{YYYY-MM-DD}.parquet.zstd
historical/manifest/deribit/trade/manifest.jsonl

historical/data/volatility_index/{currency}/{resolution}/{YYYY-MM-DD}.parquet.zstd
historical/manifest/deribit/volatility_index/manifest.jsonl
```

## CLI

```bash
cargo run -p deribit-historical-cli -- mirror instruments --currency BTC
cargo run -p deribit-historical-cli -- mirror s3 \
  --currency BTC --instruments BTC-PERPETUAL \
  --from 2024-01-01 --to 2024-01-01 --resume --parallel 2
cargo run -p deribit-historical-cli -- mirror volatility-index \
  --currency BTC,ETH --resolution 3600 \
  --from 2026-08-03 --to 2026-08-05 --resume --parallel 2
cargo run -p deribit-historical-cli -- mirror dvol-trades \
  --from 2026-08-03 --to 2026-08-05 --resume --parallel 1
cargo run -p deribit-historical-cli -- mirror status --s3
cargo run -p deribit-historical-cli -- mirror status --s3 --data-type volatility-index
```

Ops wrapper: `scripts/deribit/sync-s3.sh`.
