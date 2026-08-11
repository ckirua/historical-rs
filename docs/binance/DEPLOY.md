# Deploy & operate

How to run **historical-rs** Binance Vision jobs: build, fill ClickHouse + S3 with `--resume`, check progress, and install daily **incremental** S3 cron (rolling last ~7 days ending yesterday).

**Product (read [GOAL.md](GOAL.md) first):** database `binance_vision` tables `trades`, `klines`, `book_depth`, `bvol`.  
**Do not** write to `binance.agg_trades` or treat aggTrades as the product.

**Ops posture:** Prefer daily cron via `scripts/binance/install-cron.sh` for a rolling 7-day S3 window (`FROM=7 days ago` → `TO=yesterday`, UTC calendar days via `date -u`) so delayed Vision uploads catch up. Cron **schedule times** follow the host’s local timezone (not UTC). Do **not** start a full historical backfill from cron; use explicit tmux/`FROM`/`TO` for deep catch-up.

Repo root: clone of `historical-rs` (scripts live under `scripts/binance/`).

---

## 1. One-time setup (future host)

```bash
cd /path/to/historical-rs
./scripts/binance/install.sh --dir .          # builds target/release/binance-vision
cp .env.example ~/.env                # if needed; fill secrets
# required: S3_BINANCE_VISION_* and CLICKHOUSE_PASSWORD
set -a && source <(sed 's/\r$//' ~/.env) && set +a
```

### Schema (ClickHouse)

Prefer MergeTree DDL:

```bash
clickhouse-client --password "$CLICKHOUSE_PASSWORD" --multiquery \
  < sql/clickhouse/010_binance_vision_merge_tree.sql
```

Legacy `001_schema.sql` / `binance.agg_trades` is **not** the product path.

### Env knobs (see `.env.example`)

| Variable | Default | Purpose |
|----------|---------|---------|
| `CLICKHOUSE_VISION_DATABASE` | `binance_vision` | Product DB |
| `CLICKHOUSE_FETCH_SINCE_DAYS` | `548` | ~18mo direct CH window |
| `CLICKHOUSE_FETCH_PARALLEL` | `12` | CH fetch workers |
| `BINANCE_VISION_PARALLEL` | `16` | S3/local mirror workers |
| `KLINE_INTERVALS` | `1m,1h,1d` | klines intervals |
| `DATA_TYPES` | `trades,klines,bookDepth,BVOLIndex` | S3 sync types |
| `SYMBOLS` | (all TRADING) | Optional override, e.g. `BTCUSDT` |
| `FROM` / `TO` | script defaults | Date window `YYYY-MM-DD` |

---

## 2. Is anything running?

On a host where jobs are intentionally started, check:

```bash
# optional: tmux sessions if you use them on that host
tmux ls 2>/dev/null || true

pgrep -af 'binance-vision|sync-s3|clickhouse-direct' | grep -v pgrep || true
df -h /var/lib/clickhouse
```

| Suggested tmux name | Job | Log |
|---------------------|-----|-----|
| `binance-trades` | Vision → CH `trades` | `/tmp/binance-vision-trades-ch.log` |
| `binance-s3` | Vision → S3 (trades then klines/depth/BVOL) | `/tmp/binance-vision-s3-product.log` |
| `binance-vision-chain` | After CH trades: klines → bookDepth → bvol | `/tmp/binance-vision-chain.log` |

```bash
tail -f /tmp/binance-vision-trades-ch.log
tail -f /tmp/binance-vision-s3-product.log
tail -f /tmp/binance-vision-chain.log
```

Progress lines look like: `done=… skipped=… failed=…` (`skipped` = already in `ingest_log` or 404 on Vision).

---

## 3. Check ClickHouse fill

```bash
set -a && source <(sed 's/\r$//' ~/.env) && set +a
CH=(clickhouse-client --host "${CLICKHOUSE_HOST:-127.0.0.1}" \
  --user "${CLICKHOUSE_USER:-default}" --password "$CLICKHOUSE_PASSWORD")

# Engines should be MergeTree
"${CH[@]}" -q "
SELECT name, engine FROM system.tables
WHERE database='binance_vision'
  AND name IN ('trades','klines','book_depth','bvol','ingest_log');
"

# Disk by table (fast — use this, not full COUNT(*) on trades)
"${CH[@]}" -q "
SELECT database, table,
       formatReadableSize(sum(bytes_on_disk)) AS size,
       sum(rows) AS rows
FROM system.parts
WHERE active AND database='binance_vision'
GROUP BY database, table
ORDER BY sum(bytes_on_disk) DESC;
"

# Ingest resume ledger
"${CH[@]}" -q "
SELECT status, count()
FROM binance_vision.ingest_log
WHERE startsWith(s3_key, 'direct-trades/')
GROUP BY status ORDER BY status;
"

# Smoke-ish coverage (bounded filters — avoid full-table scans when disk is tight)
"${CH[@]}" -q "
SELECT uniqExact(symbol) AS symbols, min(time), max(time)
FROM binance_vision.trades
WHERE kind='UM' AND time >= today() - 7;
"
```

Free disk:

```bash
df -h /var/lib/clickhouse
"${CH[@]}" -q "SELECT formatReadableSize(free_space) FROM system.disks;"
```

If you see `Code: 243 NOT_ENOUGH_SPACE`, stop CH ingest, free space, then restart with `--resume`.

---

## 4. Check S3 archive

Objects use Vision-shaped keys, e.g.:

`data/futures/um/daily/trades/BTCUSDT/BTCUSDT-trades-2026-07-10.parquet.zstd`

```bash
tail -f /tmp/binance-vision-s3-product.log
# CLI status (if using local/S3 manifest)
./target/release/binance-vision mirror status --s3
```

Default sync types (not aggTrades): `trades`, `klines`, `bookDepth`, `BVOLIndex`.

---

## 5. Start / restart jobs (resume-safe, future host)

All scripts use `--resume` via `ingest_log` (CH) or S3 HEAD (S3). Safe to re-run after an interrupt.

### ClickHouse (direct Vision → `binance_vision.*`)

```bash
cd /path/to/historical-rs
set -a && source <(sed 's/\r$//' ~/.env) && set +a
export CLICKHOUSE_VISION_DATABASE=binance_vision

# Full UM ~18mo (or set FROM/TO / CLICKHOUSE_FETCH_SINCE_DAYS)
tmux new -s binance-trades -c "$PWD" \
  "./scripts/binance/clickhouse-direct-trades.sh 2>&1 | tee -a /tmp/binance-vision-trades-ch.log"

tmux new -s binance-klines -c "$PWD" \
  "./scripts/binance/clickhouse-direct-klines.sh 2>&1 | tee -a /tmp/binance-vision-klines-ch.log"

tmux new -s binance-depth -c "$PWD" \
  "./scripts/binance/clickhouse-direct-book-depth.sh 2>&1 | tee -a /tmp/binance-vision-bookdepth-ch.log"

tmux new -s binance-bvol -c "$PWD" \
  "./scripts/binance/clickhouse-direct-bvol.sh 2>&1 | tee -a /tmp/binance-vision-bvol-ch.log"
```

Smoke one symbol / few days:

```bash
SYMBOLS=BTCUSDT FROM=2026-07-10 TO=2026-07-12 ./scripts/binance/clickhouse-direct-trades.sh
```

Clear failed keys so resume retries them:

```bash
"${CH[@]}" -q "
ALTER TABLE binance_vision.ingest_log
DELETE WHERE startsWith(s3_key, 'direct-trades/') AND status='failed';
"
```

### S3 (parquet archive)

```bash
tmux new -s binance-s3 -c "$PWD" \
  "DATA_TYPES=trades,klines,bookDepth,BVOLIndex ./scripts/binance/sync-s3.sh 2>&1 | tee -a /tmp/binance-vision-s3-product.log"
```

BVOL symbols default to `BTCBVOLUSDT,ETHBVOLUSDT` (not `BTCUSDT`).

---

## 6. Daily cron (S3 incremental — rolling last 7 days)

Once historical S3 coverage is green, install managed daily **S3-only** cron (preserves other crontab blocks such as Eurostat). No ClickHouse cron — run `clickhouse-direct-*.sh` manually if needed.

```bash
cd /path/to/historical-rs
./scripts/binance/install-cron.sh
# ./scripts/binance/install-cron.sh --uninstall
```

Managed markers: `# BINANCE-VISION-CRON-BEGIN` … `# BINANCE-VISION-CRON-END`.  
Uses `ENV_FILE=$HOME/.env`, release binary under `$REPO/target/release/binance-vision`, and `--resume` inside `sync-s3.sh`.

**Window (UTC calendar):** `FROM=$(date -u -d '7 days ago' +%F)` → `TO=$(date -u -d yesterday +%F)` — inclusive ~7 calendar days ending yesterday (covers delayed Vision uploads). Independent of the host timezone used for schedule times.

**Schedule:** host-local cron time (staggered 4h apart to reduce concurrent market overlap). On this host (`Europe/Berlin`), that is CEST/CET wall clock — not UTC.

| Local | Job | Types | Log |
|-------|-----|-------|-----|
| 06:00 | UM → S3 | trades, klines, bookDepth, BVOLIndex | `/var/log/binance-vision-s3.log` |
| 10:00 | spot → S3 | trades, klines (**no** bookDepth) | `/var/log/binance-vision-s3-spot.log` |
| 14:00 | CM → S3 | trades, klines, bookDepth | `/var/log/binance-vision-s3-cm.log` |

Do **not** cron ClickHouse (`clickhouse-direct-*.sh`) or legacy aggTrades scripts. Do **not** invent spot `bookDepth`.

---

## 7. Scripts map

| Script | Writes to |
|--------|-----------|
| `scripts/binance/install-cron.sh` | User crontab managed block (daily S3 incremental only) |
| `scripts/binance/clickhouse-direct-trades.sh` | `binance_vision.trades` |
| `scripts/binance/clickhouse-direct-klines.sh` | `binance_vision.klines` (`KLINE_INTERVALS`) |
| `scripts/binance/clickhouse-direct-book-depth.sh` | `binance_vision.book_depth` |
| `scripts/binance/clickhouse-direct-bvol.sh` | `binance_vision.bvol` |
| `scripts/binance/sync-s3.sh` | S3 parquet for product types |
| `scripts/binance/mirror-local.sh` | Local parquet for product types |
| `scripts/binance/clickhouse-direct.sh` | **legacy** aggTrades — avoid |
| `scripts/binance/clickhouse-ingest-s3.sh` | **legacy** S3→`binance.agg_trades` — avoid |

---

## 8. Docs index

| Doc | Use when |
|-----|----------|
| [GOAL.md](GOAL.md) | What “done” means (all four tables) |
| [STATUS.md](STATUS.md) | Current iteration / evidence |
| [DEPLOY.md](DEPLOY.md) | This file — ops on a future host |
| [ARCHITECTURE.md](ARCHITECTURE.md) | Code layout |
| [ROADMAP.md](ROADMAP.md) | Historical phase list |
| [LOOP.md](LOOP.md) | Optional Cursor `/loop` ticks (if present) |

Quick status helper:

```bash
./scripts/binance/loop-status.sh
```

---

## 9. Validation checklist

```bash
cargo test -p binance-vision --all-features
cargo test -p binance-vision-cli --all-features
SYMBOLS=BTCUSDT FROM=$(date -u -d '3 days ago' +%F) TO=$(date -u -d 'yesterday' +%F) \
  ./scripts/binance/clickhouse-direct-trades.sh
# Expect done>0 failed=0; rows in binance_vision.trades for that symbol
```
