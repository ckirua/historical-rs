#!/usr/bin/env bash
# Background full S3 mirror for all OKX CDN product types (all symbols).
# Log: /tmp/okx-historical-s3.log
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

LOG="${LOG:-/tmp/okx-historical-s3.log}"
PID_FILE="${PID_FILE:-/tmp/okx-historical-s3.pid}"

# Full history windows (OKX portal earliest dates; missing days → skipped/404)
export FROM="${FROM:-2021-09-01}"
export TO="${TO:-$(date -u -d 'yesterday' +%F)}"
export DATA_TYPES="${DATA_TYPES:-trades,candlesticks,funding,borrowrates}"
export MARKETS="${MARKETS:-spot swap}"
export OKX_HISTORICAL_PARALLEL="${OKX_HISTORICAL_PARALLEL:-12}"
# unset SYMBOLS → --all-symbols
unset SYMBOLS || true

if [[ ! -x "$ROOT/target/release/okx-historical" ]]; then
  echo "==> building release binary..."
  cargo build --release -p okx-historical-cli
fi

if [[ -f "$PID_FILE" ]] && kill -0 "$(cat "$PID_FILE")" 2>/dev/null; then
  echo "already running pid=$(cat "$PID_FILE") log=$LOG"
  echo "tail -f $LOG"
  exit 0
fi

echo "==> starting full S3 sync ${FROM}→${TO} types=$DATA_TYPES"
echo "==> bucket=${S3_OKX_HISTORICAL_BUCKET:-mercat-okx} prefix=historical/"
echo "==> log=$LOG"

nohup "$ROOT/scripts/okx/sync-s3.sh" >>"$LOG" 2>&1 &
echo $! >"$PID_FILE"
echo "pid=$(cat "$PID_FILE")"
echo
echo "tail -f $LOG"
