# Goal lock — historical-rs (kraken)

Read this at the start of every iteration. If the work does not advance an unfinished PRIMARY item, stop.

```
PRIMARY DELIVERABLE — S3 archive on mercat-kraken (NO ClickHouse in v1):

  1. trades   ← Drive complete + quarterly ZIPs → per-pair parquet.zstd
  2. ohlcvt   ← Drive complete + quarterly ZIPs → per-pair/interval parquet.zstd

PATH: Google Drive → stream ZIP to disk → parse CSV → parquet.zstd → local/S3
      resume via JSONL manifest (local + S3)

LAYOUT:
  historical/data/trades/{complete|quarterly/YYYYQn}/{PAIR}.parquet.zstd
  historical/data/ohlcvt/{complete|quarterly/YYYYQn}/{PAIR}/{interval}.parquet.zstd
  historical/manifest/kraken-historical/manifest.jsonl

CADENCE: quarterly (monthly cron checks for new quarter ZIPs)

FORBIDDEN until both products are mirrored with resume:
  - Storing raw ZIPs in S3
  - Loading complete multi-GB archives entirely into RAM
  - ClickHouse / REST gap-fill as a substitute for Drive dumps
```

## Per-iteration checklist

1. Quote this PRIMARY block
2. Which product/source this iteration advanced
3. CLI or log evidence
4. Mark STATUS boxes only with evidence
