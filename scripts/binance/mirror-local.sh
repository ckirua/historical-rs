#!/usr/bin/env bash
# Mirror Binance Vision → local parquet.zstd for PRIMARY types (not aggTrades).
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

PARALLEL="${BINANCE_VISION_PARALLEL:-8}"
FROM="${FROM:-2024-01-01}"
TO="${TO:-$(date -u -d 'yesterday' +%F)}"
DATA_TYPES="${DATA_TYPES:-trades,klines,bookDepth,BVOLIndex}"
KLINE_INTERVALS="${KLINE_INTERVALS:-1m,1h,1d}"
MARKET="${MARKET:-um}"

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

IFS=',' read -ra TYPES <<< "$DATA_TYPES"
for dtype in "${TYPES[@]}"; do
  dtype="$(echo "$dtype" | xargs)"
  case "$dtype" in
    klines)
      IFS=',' read -ra IVS <<< "$KLINE_INTERVALS"
      for iv in "${IVS[@]}"; do
        echo "==> mirror local market=$MARKET data-type=klines interval=$iv"
        $BIN mirror run \
          --market "$MARKET" --frequency daily --data-type klines --interval "$iv" \
          "${SYMBOL_ARGS[@]}" --from "$FROM" --to "$TO" --resume --parallel "$PARALLEL" "$@"
      done
      ;;
    BVOLIndex|bvol|BVOL)
      echo "==> mirror local market=option data-type=BVOLIndex"
      BVOL_SYMBOLS="${BVOL_SYMBOLS:-BTCBVOLUSDT,ETHBVOLUSDT}"
      $BIN mirror run \
        --market option --frequency daily --data-type BVOLIndex \
        --symbols "$BVOL_SYMBOLS" --from "$FROM" --to "$TO" --resume --parallel "$PARALLEL" "$@"
      ;;
    *)
      echo "==> mirror local market=$MARKET data-type=$dtype"
      $BIN mirror run \
        --market "$MARKET" --frequency daily --data-type "$dtype" \
        "${SYMBOL_ARGS[@]}" --from "$FROM" --to "$TO" --resume --parallel "$PARALLEL" "$@"
      ;;
  esac
done
