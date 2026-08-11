# Bybit spot/trade hard-fail evidence (Wave 0)

Evidence gate for the 2026-08-05 spot/`trade` mirror that exited with
`failed=221615`. Sources: `/var/log/bybit-historical-s3.log` and S3 JSONL
`historical/manifest/bybit/trade/spot/daily/manifest.jsonl` on bucket
`mercat-bybit` (compatibility snapshot last-modified `2026-08-05T22:52:31Z`).

No code changes in this lane. One-symbol local repro was not required: the
manifest already names every failure class.

## Run summary (2026-08-05)

| Field | Value |
| --- | --- |
| Dataset | `spot` / `trade` / `daily` |
| Window | `2025-04-29` → `2026-08-04` |
| Symbols | 557 (`--all-symbols`) |
| Parallel | 8 |
| Planning | `planned=226768`, `planning_gaps=31123`, `planning_failures=0` |
| Resume keys | 2681 (S3 data-prefix listing) |
| Final counters | `total=257891` `done=2474` `skipped=2679` `failed=221615` `pending=31123` |
| CLI | hard-exit: `Bybit mirror completed with 221615 failed archives` |
| Timeline | planning finished `22:32:35Z`; segment writes started immediately; snapshot at `22:52:31Z` (~20 min download phase) |

Arithmetic check: `2474 + 2679 + 221615 = 226768` (all planned archives
accounted). `pending=31123` equals `planning_gaps` (portal date absences,
counted in-process only, not persisted as failed).

Log note: this run emitted **no** `WARN` lines during downloads. Per-archive
errors are written only to the manifest (`upload_archive` → `failed(...)`),
so ops logs alone cannot explain the mass fail.

## Manifest taxonomy (full JSONL, 226770 lines)

Status histogram:

| Status | Count | Share of lines |
| --- | --- | --- |
| `failed` | **221615** | 97.7% |
| `skipped` | 2681 | 1.2% |
| `ok` | 2474 | 1.1% |

### Error classes among `failed` (exact)

| Class | Count | % of failed | Notes |
| --- | --- | --- | --- |
| **Download HTTP 403** (`public.bybit.com`) | **221615** | **100%** | `shared infrastructure error: HTTP 403 for https://public.bybit.com/spot/...` |
| Convert / parse | 0 | 0% | — |
| S3 upload | 0 | 0% | — |
| DNS / connect | 0 | 0% | — |
| Empty archive URL | 0 | 0% | — |
| Portal listing failed | 0 | 0% | `planning_failures=0` |
| HTTP 429 / 5xx | 0 | 0% | — |

Every failed entry matches:

```text
shared infrastructure error: HTTP 403 for https://public.bybit.com/spot/<SYMBOL>/<SYMBOL>_<DATE>.csv.gz
```

Host is exclusively `public.bybit.com` (CloudFront in front of S3). No other
error string appears in this manifest.

### Success cliff (rate-limit / WAF pattern)

Only **15** symbols have any `ok` rows; they cluster at the start of the
catalog (early `0*`/`A*` names). Example cutover:

- Full or near-full `ok`: `ACHUSDT`, `ACSUSDT`, `ADAEUR`, `AEROUSDT`, `AAVEUSDT`, …
- Mid-symbol cutover: `ADAUSDT` → `ok=122`, `failed=341`
- Thereafter: hundreds of symbols with `ok=0`, `failed≈463` each

With `parallel=8`, workers interleave symbols, so the cliff is not a perfect
alphabetical wall—but the shape is “first ~2.5k downloads succeed, then
nearly everything else is 403.”

`skipped` aligns with resume keys already on S3 from earlier partial success
(early symbols largely skip; later symbols never got objects).

## Category verdict

| Category | This run? |
| --- | --- |
| **Download** | **Yes — dominant (100% of failures)** |
| Convert | No |
| S3 upload / manifest flush | No (snapshot wrote successfully at end) |
| DNS | No (older runs on 2026-07-28 / 2026-08-02 showed DNS / connection-reset on **manifest flush**, not this run) |
| Empty URL | No |
| Portal gap | Counted as `pending` / `planning_gaps`, not `failed` |

## Live check (2026-08-06, post-run)

The same URLs that failed with 403 now return **HTTP 200** via `curl` from
this host (multiple User-Agents). Example:

`https://public.bybit.com/spot/ADAUSDT/ADAUSDT_2025-08-26.csv.gz` → 200.

So the archives exist; the overnight failure was a **transient edge block**
(CloudFront/WAF-style 403 under burst), not missing data or a bad portal URL.

## Why retries did not help

`BybitHttpClient::download_to_file` uses
`historical_core::http::get_to_file_with_retry`, which retries **5xx** and
optionally **429** only. **HTTP 403 is not retried.** Once the edge started
403ing, each archive failed in one shot—hence ~221k failures in ~20 minutes
with almost no log noise.

## Recommendation for Wave 1 (retry + histogram)

1. **Dominant class to handle:** treat sustained `HTTP 403` from
   `public.bybit.com` as a **transient download / edge throttle** class, not a
   permanent “archive forbidden” outcome.
2. **Retry policy:** extend download retries to include 403 (and keep 429/5xx),
   with **long exponential backoff** (seconds→minutes) and a **circuit breaker**
   / global pause when 403 rate spikes (e.g. N consecutive or M% in a window).
   Blind tight retries will burn the remaining window and re-ban the IP.
3. **Throughput:** consider lowering default download concurrency and/or adding
   a simple token-bucket for `public.bybit.com` so overnight full-catalog runs
   do not trip the edge after the first few thousand objects.
4. **Observability (required):** before CLI exit, print a **histogram of
   `entry.error` prefixes** (at least top reasons + counts). This run’s ops log
   alone could not distinguish 403-ban from convert/S3/DNS.
5. **Exit policy:** keep `failed > 0` → non-zero exit, but after retries so a
   cool-down can clear a WAF ban mid-run.
6. **Do not** change portal-gap semantics (`planning_gaps` stay non-failed /
   pending).
7. **Ops follow-up after Wave 1:** resume the same spot/trade window; expect
   prior `failed` rows to be retried because resume skips only existing data
   keys, not failed manifest lines.

## Out of scope here

- Implementing retries / histogram (Wave 1 lanes)
- Full or multi-symbol backfill
- Binance lock/cron work
