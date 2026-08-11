#!/usr/bin/env bash
# Background full S3 mirror for Kraken historical (complete + all quarters).
# Log: /tmp/kraken-historical-s3.log
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

LOG="${LOG:-/tmp/kraken-historical-s3.log}"
PID_FILE="${PID_FILE:-/tmp/kraken-historical-s3.pid}"

export DATA_TYPES="${DATA_TYPES:-trades,ohlcvt}"
export SOURCE_MODE="${SOURCE_MODE:-both}"
export KRAKEN_HISTORICAL_PARALLEL="${KRAKEN_HISTORICAL_PARALLEL:-4}"
unset PAIRS || true
unset FROM_QUARTER || true
unset TO_QUARTER || true

if [[ ! -x "$ROOT/target/release/kraken-historical" ]]; then
  echo "==> building release binary..."
  cargo build --release -p kraken-historical-cli
fi

if [[ -f "$PID_FILE" ]] && kill -0 "$(cat "$PID_FILE")" 2>/dev/null; then
  echo "already running pid=$(cat "$PID_FILE") log=$LOG"
  echo "tail -f $LOG"
  exit 0
fi

echo "==> starting full S3 sync types=$DATA_TYPES source=$SOURCE_MODE"
echo "==> bucket=${S3_KRAKEN_HISTORICAL_BUCKET:-mercat-kraken} prefix=historical/"
echo "==> log=$LOG"

nohup "$ROOT/scripts/kraken/sync-s3.sh" >>"$LOG" 2>&1 &
echo $! >"$PID_FILE"
echo "pid=$(cat "$PID_FILE")"
echo
echo "tail -f $LOG"
