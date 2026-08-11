# Deribit rate limits (historical public trades)

Evidence note for the `history.deribit.com` public-trades mirror (v1).

## Official constraints

Deribit's [Rate Limits](https://docs.deribit.com/articles/rate-limits) article does
**not** publish a separate schedule for `history.deribit.com`. The same credit
model applies:

| Fact | Implication for this mirror |
|------|-----------------------------|
| Unauthenticated = opaque **per-IP** limits | Do **not** run production backfills without credentials on a host shared with live feeds |
| Authenticated non-matching default: ~500 credits/req, refill ≈ **20 req/s**, burst ≈ **100** | Sustained target **≤10–15 req/s** (default token bucket **12 rps**) |
| Credit exhaustion → `too_many_requests` / **10028**, session may disconnect | Exponential backoff + invalidate OAuth token / reopen |
| [`public/get_instruments`](https://docs.deribit.com/api-reference/market-data/public-get_instruments) custom limit **1 req/s** | Separate 1/s bucket + file/memory cache |
| [`historical: true` private fills](https://docs.deribit.com/articles/accessing-historical-trades-orders) | **Wrong path** for public tape — do not use |

## Wave 0 smoke (2026-08-06)

Unauthenticated GET against the archive host (credentials were **not** present in
`$HOME/.env` at evidence time):

```text
GET https://history.deribit.com/api/v2/public/get_last_trades_by_instrument
  ?instrument_name=BTC-PERPETUAL&count=3&sorting=asc
→ HTTP 200, result.trades[0].trade_seq=1, has_more=true

GET https://history.deribit.com/api/v2/public/get_last_trades_by_instrument_and_time
  ?instrument_name=BTC-PERPETUAL
  &start_timestamp=1704067200000&end_timestamp=1704153599000
  &count=1000&sorting=asc
→ HTTP 200, 1000 trades, has_more=true (UTC 2024-01-01)

GET https://history.deribit.com/api/v2/public/get_instruments
  ?currency=BTC&kind=future
→ HTTP 200, includes BTC-PERPETUAL and dated futures
```

OAuth `public/auth` is served on `https://www.deribit.com/api/v2` (history host
rejects auth). Production scripts should set `DERIBIT_CLIENT_ID` /
`DERIBIT_CLIENT_SECRET` so public history calls carry a Bearer token and use
the authenticated credit pool.

## Env check (Wave 0)

| Variable | Status at evidence time |
|----------|-------------------------|
| `DERIBIT_CLIENT_ID` / `DERIBIT_CLIENT_SECRET` | **unset** in `$HOME/.env` |
| Shared `S3_URL` / `S3_ACCESS_KEY` / `S3_SECRET_KEY` | present |
| Bucket `mercat-deribit` | exists on the configured endpoint |
| `S3_DERIBIT_*` overrides | unset (defaults to `mercat-deribit` + shared S3 keys) |

Unauthenticated one-day / one-instrument smokes are allowed. Multi-day backfills
must wait for credentials.
