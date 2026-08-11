# Deribit DVOL / volatility index

## Naming (not BTCUSD / ETHUSD)

| Concept | Official id | Notes |
|---------|-------------|--------|
| Volatility index API currency | `BTC`, `ETH` | Params to `public/get_volatility_index_data` — **not** `BTCUSD`/`ETHUSD` |
| Index price name | `btcdvol_usdc`, `ethdvol_usdc` | From `public/get_index_price_names` / `get_index_price` |
| Tradeable DVOL future | `BTCDVOL_USDC-{expiry}` | Listed under `currency=USDC`, `kind=future` |
| ETH DVOL future | *(none active as of 2026-08-06)* | Index `ethdvol_usdc` still exists; no `ETHDVOL_USDC-*` instrument |

The volatility index is **OHLC candle data**, not a trade tape. Public trades for the
index itself do not exist. Related **trades** are only for listed DVOL futures
(currently BTC only).

## Endpoints

```text
# Index candles — live www host only (history.deribit.com returns HTTP 400)
GET https://www.deribit.com/api/v2/public/get_volatility_index_data
  ?currency=BTC|ETH
  &start_timestamp=<ms>
  &end_timestamp=<ms>
  &resolution=1|60|3600|43200|1D

# Response: result.data = [[ts, open, high, low, close], ...]
# Values are annualized volatility in percent (e.g. 35.08).
# result.continuation → next end_timestamp when truncated (null when done).

# Spot index price
GET https://www.deribit.com/api/v2/public/get_index_price?index_name=btcdvol_usdc

# DVOL futures trades — history archive host (same as other futures)
GET https://history.deribit.com/api/v2/public/get_last_trades_by_instrument_and_time
  ?instrument_name=BTCDVOL_USDC-26AUG26&...

# Instrument discovery — www (history omits DVOL from get_instruments)
GET https://www.deribit.com/api/v2/public/get_instruments?currency=USDC&kind=future
```

Docs: [public/get_volatility_index_data](https://docs.deribit.com/api-reference/market-data/public-get_volatility_index_data).

## S3 layout

```text
historical/data/volatility_index/{currency}/{resolution}/{YYYY-MM-DD}.parquet.zstd
historical/manifest/deribit/volatility_index/manifest.jsonl

historical/data/trade/USDC/BTCDVOL_USDC-{expiry}/{YYYY-MM-DD}.parquet.zstd
historical/manifest/deribit/trade/manifest.jsonl
```

Parquet columns (index): `currency`, `index_name`, `resolution`, `timestamp_ms`,
`open`, `high`, `low`, `close`.

## CLI

```bash
# BTC+ETH hourly DVOL index for a few days (unauthenticated ok for smoke)
deribit-historical mirror volatility-index \
  --currency BTC,ETH --resolution 3600 \
  --from 2026-08-03 --to 2026-08-05 --resume --parallel 2

# Discover + mirror DVOL futures public trades
deribit-historical mirror instruments --currency USDC --dvol-only
deribit-historical mirror dvol-trades \
  --from 2026-08-03 --to 2026-08-05 --resume --parallel 1

deribit-historical mirror status --s3 --data-type volatility-index
deribit-historical mirror status --s3 --data-type trade
```

Auth: prefer no `DERIBIT_CLIENT_*` for tiny smokes (per-IP limits). Production
backfill should authenticate; see [RATE-LIMITS.md](./RATE-LIMITS.md).
