#!/usr/bin/env bash
# Ingest aggTrades parquet from S3 into ClickHouse (recent window only).
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
  BIN="cargo run --quiet -p binance-vision-cli --"
fi

PREFIX="${S3_INGEST_PREFIX:-data/futures/um/daily/aggTrades/}"
# ClickHouse hot tier: default last 3 years (S3 keeps full history).
SINCE_DAYS="${CLICKHOUSE_INGEST_SINCE_DAYS:-1095}"
SINCE=$(date -u -d "${SINCE_DAYS} days ago" +%F)

echo "==> ClickHouse ingest since ${SINCE} (${SINCE_DAYS}d window)"

exec $BIN clickhouse ingest \
  --s3 \
  --s3-prefix "$PREFIX" \
  --since "$SINCE" \
  --resume \
  --parallel "${CLICKHOUSE_INGEST_PARALLEL:-4}" \
  "$@"
