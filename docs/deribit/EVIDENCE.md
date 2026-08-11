# Deribit history API evidence (Wave 0)

Captured 2026-08-06 on this host.

## Public history host works

```text
GET https://history.deribit.com/api/v2/public/get_last_trades_by_instrument
  ?instrument_name=BTC-PERPETUAL&count=2&sorting=desc
→ trades[] + has_more=true

GET .../public/get_last_trades_by_instrument_and_time
  ?instrument_name=BTC-PERPETUAL
  &start_timestamp=1704067200000&end_timestamp=1704153600000
  &count=3&sorting=asc
→ first trade_seq=191134363 @ 2024-01-01 (deep history)

GET .../public/get_instruments?currency=BTC&kind=future
→ 3 live instruments (includes BTC-PERPETUAL)

GET .../public/get_instruments?currency=BTC&kind=future&expired=true
→ 464 expired/dated futures (required for multi-year backfill)
```

## Credentials on this host

| Variable | Status |
|----------|--------|
| `DERIBIT_CLIENT_ID` / `DERIBIT_CLIENT_SECRET` | **Missing** in `$HOME/.env` |
| `S3_DERIBIT_BUCKET` / `S3_DERIBIT_HISTORICAL_BUCKET` | **Missing** (generic `S3_*` present) |

Unauthenticated public calls succeed for small smokes. Production sync **must** use client credentials (per-IP public limits are opaque and can disconnect). Add keys before long backfills.

## Rate limits (official docs)

See [Rate Limits](https://docs.deribit.com/articles/rate-limits):

- No separate published schedule for `history.deribit.com`.
- Public unauthenticated = **per-IP**.
- Authenticated non-matching default ≈ **20 req/s** sustained (500 credits/request).
- `public/get_instruments` custom: **1 req/s**.
- `too_many_requests` / code **10028** can terminate the session — backoff and reconnect.
- Private `historical: true` is **user fills only**, not the public tape.

v1 mirror targets ≤10–15 rps with low instrument parallelism (2–4).
