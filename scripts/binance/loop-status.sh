#!/usr/bin/env bash
# Snapshot for historical-rs Binance /loop ticks and manual ops checks.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"

ENV_FILE="${ENV_FILE:-$HOME/.env}"
if [[ -f "$ENV_FILE" ]]; then
  set -a
  # shellcheck disable=SC1090
  source <(sed 's/\r$//' "$ENV_FILE")
  set +a
fi

if [[ -x "$ROOT/target/release/binance-vision" ]]; then
  BIN="$ROOT/target/release/binance-vision"
elif command -v binance-vision >/dev/null 2>&1; then
  BIN="binance-vision"
else
  BIN=""
fi

echo "=== historical-rs binance loop status $(date -u +%Y-%m-%dT%H:%M:%SZ) ==="

# Phase tracking (docs/binance/STATUS.md iteration table)
if [[ -f docs/binance/STATUS.md ]]; then
  grep -E '^\| [0-9]+ ' docs/binance/STATUS.md || true
fi

echo "--- sync process ---"
if pgrep -af 'binance-vision mirror s3' >/dev/null 2>&1; then
  pgrep -af 'binance-vision mirror s3' | head -1
  echo "sync=running"
else
  echo "sync=stopped"
fi

echo "--- product logs (last 5 lines each, if present) ---"
for f in \
  /tmp/binance-vision-s3-product.log \
  /tmp/binance-vision-trades-ch.log \
  /tmp/binance-vision-chain.log \
  /tmp/binance-vision-sync.log
do
  if [[ -f "$f" ]]; then
    echo "## $f"
    tail -5 "$f"
  fi
done

if [[ -n "$BIN" ]]; then
  echo "--- mirror status (S3) ---"
  "$BIN" mirror status --s3 2>/dev/null || echo "mirror status --s3 failed (or no credentials)"
  echo "--- ClickHouse CLI status ---"
  "$BIN" clickhouse status 2>/dev/null || echo "clickhouse status failed (or CH unreachable)"
fi

CH_HOST="${CLICKHOUSE_HOST:-127.0.0.1}"
CH_PORT="${CLICKHOUSE_PORT:-9000}"
CH_USER="${CLICKHOUSE_USER:-default}"
CH_DB="${CLICKHOUSE_VISION_DATABASE:-binance_vision}"
if command -v clickhouse-client >/dev/null 2>&1; then
  echo "--- ${CH_DB} table sizes (active parts) ---"
  clickhouse-client \
    --host "$CH_HOST" --port "$CH_PORT" --user "$CH_USER" \
    ${CLICKHOUSE_PASSWORD:+--password "$CLICKHOUSE_PASSWORD"} \
    -q "
SELECT table,
       formatReadableSize(sum(bytes_on_disk)) AS size,
       sum(rows) AS rows
FROM system.parts
WHERE active AND database='${CH_DB}'
  AND table IN ('trades','klines','book_depth','bvol','ingest_log')
GROUP BY table
ORDER BY sum(bytes_on_disk) DESC
" 2>/dev/null || echo "clickhouse-client query failed (or DB absent)"
fi

echo "=== end ==="
