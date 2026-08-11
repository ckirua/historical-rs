#!/usr/bin/env bash
# Ingest mirrored parquet.zstd datasets into ClickHouse.
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

if command -v clickhouse-client >/dev/null 2>&1; then
  EXISTS=$(clickhouse-client \
    --host "$CH_HOST" \
    --port "$CH_PORT" \
    --user "$CH_USER" \
    ${CLICKHOUSE_PASSWORD:+--password "$CLICKHOUSE_PASSWORD"} \
    -q "EXISTS TABLE ${CH_DB}.agg_trades" 2>/dev/null || echo 0)
  if [[ "$EXISTS" != "1" ]]; then
    echo "==> Applying ClickHouse schema (database: ${CH_DB})"
    clickhouse-client \
      --host "$CH_HOST" \
      --port "$CH_PORT" \
      --user "$CH_USER" \
      ${CLICKHOUSE_PASSWORD:+--password "$CLICKHOUSE_PASSWORD"} \
      --multiquery < sql/clickhouse/001_schema.sql
  fi
fi

if [[ -x "$ROOT/target/release/binance-vision" ]]; then
  BIN="$ROOT/target/release/binance-vision"
elif command -v binance-vision >/dev/null 2>&1; then
  BIN="binance-vision"
else
  BIN="cargo run --quiet -p binance-vision-cli --"
fi

exec $BIN clickhouse ingest --resume --parallel "${CLICKHOUSE_INGEST_PARALLEL:-4}"
