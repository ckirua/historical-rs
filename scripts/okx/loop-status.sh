#!/usr/bin/env bash
# Snapshot for historical-rs (okx) ops ticks.
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

if [[ -x "$ROOT/target/release/okx-historical" ]]; then
  BIN="$ROOT/target/release/okx-historical"
elif command -v okx-historical >/dev/null 2>&1; then
  BIN="okx-historical"
else
  BIN=""
fi

echo "=== historical-rs (okx) loop status $(date -u +%Y-%m-%dT%H:%M:%SZ) ==="

if [[ -f docs/okx/STATUS.md ]]; then
  echo "--- STATUS checklist ---"
  grep -E '^\- \[[ x]\]' docs/okx/STATUS.md || true
fi

echo "--- sync process ---"
if [[ -f /tmp/okx-historical-s3.pid ]] && kill -0 "$(cat /tmp/okx-historical-s3.pid)" 2>/dev/null; then
  echo "pid=$(cat /tmp/okx-historical-s3.pid) sync=running"
  pgrep -af 'okx-historical mirror s3|scripts/okx/sync-s3' | head -3 || true
else
  if pgrep -af 'okx-historical mirror s3|scripts/okx/sync-s3' >/dev/null 2>&1; then
    pgrep -af 'okx-historical mirror s3|scripts/okx/sync-s3' | head -3
    echo "sync=running (no pidfile)"
  else
    echo "sync=stopped"
  fi
fi

echo "--- sync log (last 8 lines) ---"
if [[ -f /tmp/okx-historical-s3.log ]]; then
  tail -8 /tmp/okx-historical-s3.log
else
  echo "(no /tmp/okx-historical-s3.log)"
fi

if [[ -n "$BIN" ]]; then
  echo "--- mirror status (S3) ---"
  "$BIN" mirror status --s3 2>/dev/null || echo "mirror status --s3 failed (or no credentials)"
fi

echo "--- cron ---"
crontab -l 2>/dev/null | sed -n '/OKX-HISTORICAL-CRON/,/OKX-HISTORICAL-CRON-END/p' || echo "(no OKX cron block)"

echo "=== end ==="
