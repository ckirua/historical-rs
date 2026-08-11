#!/usr/bin/env bash
# Fetch verified Bybit Spot/Contract catalogs → parquet.zstd → S3.
# Supported datasets: {spot|contract}/{orderbook|trade}/daily.
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

PARALLEL="${BYBIT_HISTORICAL_PARALLEL:-8}"
FROM="${FROM:-2024-01-01}"
TO="${TO:-$(date -u -d 'yesterday' +%F)}"
MARKET="${BYBIT_MARKET:-spot}"
INTERVAL="${BYBIT_INTERVAL:-daily}"
# Prefer DATA_TYPES (comma-separated). BYBIT_DATA_TYPE remains a single-type alias.
DATA_TYPES="${DATA_TYPES:-${BYBIT_DATA_TYPE:-orderbook,trade}}"

if [[ "$MARKET" != "spot" && "$MARKET" != "contract" ]]; then
  cat >&2 <<EOF
unsupported Bybit S3 market: ${MARKET@Q}
verified markets are 'spot' and 'contract' (portal bizType; not linear/inverse)
EOF
  exit 2
fi

if [[ "$INTERVAL" != "daily" ]]; then
  cat >&2 <<EOF
unsupported Bybit S3 interval: ${INTERVAL@Q}
verified interval is 'daily' only
EOF
  exit 2
fi

if [[ -x "$ROOT/target/release/bybit-historical" ]]; then
  BIN="$ROOT/target/release/bybit-historical"
elif command -v bybit-historical >/dev/null 2>&1; then
  BIN="bybit-historical"
else
  BIN="cargo run --quiet -p bybit-historical-cli --"
fi

SYMBOL_ARGS=()
if [[ -n "${ALL_SYMBOLS:-}" ]]; then
  SYMBOL_ARGS=(--all-symbols)
elif [[ -n "${SYMBOLS:-}" ]]; then
  SYMBOL_ARGS=(--symbols "$SYMBOLS")
else
  SYMBOL_ARGS=(--symbols "BTCUSDT,ETHUSDT")
fi

echo "==> S3 bucket: ${S3_BYBIT_HISTORICAL_BUCKET:-mercat-bybit} (prefix historical/)"
echo "==> Endpoint: ${S3_BYBIT_HISTORICAL_ENDPOINT:-unset}"
echo "==> Types: $DATA_TYPES  market=$MARKET interval=$INTERVAL  ${FROM}→${TO}"
export RUST_LOG="${RUST_LOG:-info}"
echo "==> RUST_LOG=$RUST_LOG"

if command -v stdbuf >/dev/null 2>&1; then
  RUN=(stdbuf -oL -eL "$BIN")
else
  RUN=("$BIN")
fi

IFS=',' read -ra TYPES <<< "$DATA_TYPES"
FAILED=0
for dtype in "${TYPES[@]}"; do
  dtype="$(echo "$dtype" | xargs)"
  case "$dtype" in
    orderbook|trade)
      echo "==> mirror s3 market=$MARKET data-type=$dtype interval=$INTERVAL"
      set +e
      "${RUN[@]}" mirror s3 \
        --market "$MARKET" \
        --data-type "$dtype" \
        --interval "$INTERVAL" \
        "${SYMBOL_ARGS[@]}" \
        --from "$FROM" \
        --to "$TO" \
        --resume \
        --parallel "$PARALLEL" \
        "$@"
      rc=$?
      set -e
      if [[ "$rc" -ne 0 ]]; then
        echo "==> ERROR: market=$MARKET data-type=$dtype failed (exit $rc); continuing with remaining types" >&2
        FAILED=1
      fi
      ;;
    *)
      cat >&2 <<EOF
unknown Bybit data type: ${dtype@Q}
verified values are 'orderbook' and 'trade' (comma-separated via DATA_TYPES)
EOF
      exit 1
      ;;
  esac
done

if [[ "$FAILED" -ne 0 ]]; then
  echo "==> one or more data types failed for market=$MARKET" >&2
  exit 1
fi
