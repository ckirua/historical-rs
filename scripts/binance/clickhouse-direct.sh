#!/usr/bin/env bash
# Fetch Binance Vision aggTrades over HTTP and insert directly into ClickHouse.
# Skips S3 — faster path for a recent window. Resume via binance.ingest_log.
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

PARALLEL="${CLICKHOUSE_FETCH_PARALLEL:-12}"
# Default ~18 months (half of 3y hot tier) — override with CLICKHOUSE_FETCH_SINCE_DAYS.
SINCE_DAYS="${CLICKHOUSE_FETCH_SINCE_DAYS:-548}"
FROM="${FROM:-$(date -u -d "${SINCE_DAYS} days ago" +%F)}"
TO="${TO:-$(date -u -d 'yesterday' +%F)}"

if [[ -x "$ROOT/target/release/binance-vision" ]]; then
  BIN="$ROOT/target/release/binance-vision"
elif command -v binance-vision >/dev/null 2>&1; then
  BIN="binance-vision"
else
  BIN="cargo run --quiet -p binance-vision-cli --"
fi

SYMBOL_ARGS=()
if [[ -n "${SYMBOLS:-}" ]]; then
  SYMBOL_ARGS=(--symbols "$SYMBOLS")
else
  SYMBOL_ARGS=(--all-symbols)
fi

echo "==> direct Vision → ClickHouse"
echo "==> range: ${FROM} → ${TO} (${SINCE_DAYS}d window)"
echo "==> parallel: ${PARALLEL}"

exec $BIN clickhouse fetch \
  --market um \
  --frequency daily \
  --data-type aggTrades \
  "${SYMBOL_ARGS[@]}" \
  --from "$FROM" \
  --to "$TO" \
  --resume \
  --parallel "$PARALLEL" \
  "$@"
