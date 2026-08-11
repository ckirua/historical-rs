#!/usr/bin/env bash
# Vision daily klines → binance_vision.klines (loops intervals).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"
ENV_FILE="${ENV_FILE:-$HOME/.env}"
if [[ -f "$ENV_FILE" ]]; then set -a; source <(sed 's/\r$//' "$ENV_FILE"); set +a; fi
export CLICKHOUSE_VISION_DATABASE="${CLICKHOUSE_VISION_DATABASE:-binance_vision}"
PARALLEL="${CLICKHOUSE_FETCH_PARALLEL:-8}"
SINCE_DAYS="${CLICKHOUSE_FETCH_SINCE_DAYS:-548}"
FROM="${FROM:-$(date -u -d "${SINCE_DAYS} days ago" +%F)}"
TO="${TO:-$(date -u -d 'yesterday' +%F)}"
INTERVALS="${KLINE_INTERVALS:-1m,1h,1d}"
BIN="$ROOT/target/release/binance-vision"
[[ -x "$BIN" ]] || BIN="cargo run --quiet -p binance-vision-cli --"
SYMBOL_ARGS=(--all-symbols)
[[ -n "${SYMBOLS:-}" ]] && SYMBOL_ARGS=(--symbols "$SYMBOLS")
IFS=',' read -ra IVS <<< "$INTERVALS"
for iv in "${IVS[@]}"; do
  echo "==> Vision klines interval=$iv → binance_vision.klines  ${FROM}→${TO}"
  $BIN clickhouse fetch \
    --market "${MARKET:-um}" --frequency daily --data-type klines --interval "$iv" \
    "${SYMBOL_ARGS[@]}" --from "$FROM" --to "$TO" --resume --parallel "$PARALLEL" "$@"
done
