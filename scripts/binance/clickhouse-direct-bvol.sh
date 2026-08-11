#!/usr/bin/env bash
# Vision BVOLIndex → binance_vision.bvol
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"
ENV_FILE="${ENV_FILE:-$HOME/.env}"
if [[ -f "$ENV_FILE" ]]; then set -a; source <(sed 's/\r$//' "$ENV_FILE"); set +a; fi
export CLICKHOUSE_VISION_DATABASE="${CLICKHOUSE_VISION_DATABASE:-binance_vision}"
PARALLEL="${CLICKHOUSE_FETCH_PARALLEL:-4}"
SINCE_DAYS="${CLICKHOUSE_FETCH_SINCE_DAYS:-548}"
FROM="${FROM:-$(date -u -d "${SINCE_DAYS} days ago" +%F)}"
TO="${TO:-$(date -u -d 'yesterday' +%F)}"
BIN="$ROOT/target/release/binance-vision"
[[ -x "$BIN" ]] || BIN="cargo run --quiet -p binance-vision-cli --"
# Vision option BVOLIndex symbols are BTCBVOLUSDT / ETHBVOLUSDT (not BTCUSDT).
SYMBOLS="${SYMBOLS:-BTCBVOLUSDT,ETHBVOLUSDT}"
echo "==> Vision BVOLIndex → binance_vision.bvol  ${FROM}→${TO} symbols=$SYMBOLS"
exec $BIN clickhouse fetch \
  --market option --frequency daily --data-type BVOLIndex \
  --symbols "$SYMBOLS" --from "$FROM" --to "$TO" --resume --parallel "$PARALLEL" "$@"
