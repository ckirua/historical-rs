#!/usr/bin/env bash
# Apply ClickHouse schema optimizations (safe to re-run).
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

CH_HOST="${CLICKHOUSE_HOST:-127.0.0.1}"
CH_PORT="${CLICKHOUSE_PORT:-9000}"
CH_USER="${CLICKHOUSE_USER:-default}"
CH_DB="${CLICKHOUSE_BINANCE_DATABASE:-binance}"

if ! command -v clickhouse-client >/dev/null 2>&1; then
  echo "clickhouse-client not found; run: binance-vision clickhouse ingest --s3 --limit 0"
  exit 1
fi

run_sql() {
  local file="$1"
  echo "==> applying $(basename "$file")"
  clickhouse-client \
    --host "$CH_HOST" \
    --port "$CH_PORT" \
    --user "$CH_USER" \
    ${CLICKHOUSE_PASSWORD:+--password "$CLICKHOUSE_PASSWORD"} \
    --multiquery < "$file"
}

run_sql sql/clickhouse/001_schema.sql
run_sql sql/clickhouse/002_optimize.sql
run_sql sql/clickhouse/003_views.sql

echo "==> table stats"
clickhouse-client \
  --host "$CH_HOST" \
  --port "$CH_PORT" \
  --user "$CH_USER" \
  ${CLICKHOUSE_PASSWORD:+--password "$CLICKHOUSE_PASSWORD"} \
  -q "
    SELECT
      table,
      formatReadableQuantity(sum(rows)) AS rows,
      formatReadableSize(sum(bytes_on_disk)) AS size
    FROM system.parts
    WHERE database = '${CH_DB}' AND active
    GROUP BY table
    ORDER BY table
  "
