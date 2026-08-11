#!/usr/bin/env bash
# Fetch OKX CDN → parquet.zstd → S3 for the four CDN product types.
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

PARALLEL="${OKX_HISTORICAL_PARALLEL:-12}"
FROM="${FROM:-2024-01-01}"
TO="${TO:-$(date -u -d 'yesterday' +%F)}"
DATA_TYPES="${DATA_TYPES:-trades,candlesticks,funding,borrowrates}"

if [[ -x "$ROOT/target/release/okx-historical" ]]; then
  BIN="$ROOT/target/release/okx-historical"
elif command -v okx-historical >/dev/null 2>&1; then
  BIN="okx-historical"
else
  BIN="cargo run --quiet -p okx-historical-cli --"
fi

SYMBOL_ARGS=()
if [[ -n "${SYMBOLS:-}" ]]; then
  SYMBOL_ARGS=(--symbols "$SYMBOLS")
else
  SYMBOL_ARGS=(--all-symbols)
fi

echo "==> S3 bucket: ${S3_OKX_HISTORICAL_BUCKET:-mercat-okx} (prefix historical/)"
echo "==> Endpoint: ${S3_OKX_HISTORICAL_ENDPOINT:-unset}"
echo "==> Types: $DATA_TYPES  ${FROM}→${TO}"

IFS=',' read -ra TYPES <<< "$DATA_TYPES"
for dtype in "${TYPES[@]}"; do
  dtype="$(echo "$dtype" | xargs)"
  case "$dtype" in
    funding)
      echo "==> mirror s3 market=swap data-type=funding"
      $BIN mirror s3 \
        --market swap --frequency daily --data-type funding \
        "${SYMBOL_ARGS[@]}" --from "$FROM" --to "$TO" --resume --parallel "$PARALLEL" "$@"
      ;;
    borrowrates|borrow)
      echo "==> mirror s3 market=spot data-type=borrowrates"
      $BIN mirror s3 \
        --market spot --frequency monthly --data-type borrowrates \
        "${SYMBOL_ARGS[@]}" --from "$FROM" --to "$TO" --resume --parallel "$PARALLEL" "$@"
      ;;
    trades|candlesticks)
      for market in ${MARKETS:-spot swap}; do
        echo "==> mirror s3 market=$market data-type=$dtype"
        $BIN mirror s3 \
          --market "$market" --frequency daily --data-type "$dtype" \
          "${SYMBOL_ARGS[@]}" --from "$FROM" --to "$TO" --resume --parallel "$PARALLEL" "$@"
      done
      ;;
    *)
      echo "unknown data type: $dtype" >&2
      exit 1
      ;;
  esac
done
