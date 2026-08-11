#!/usr/bin/env bash
# Snapshot for historical-rs (kraken) iteration ticks.
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

if [[ -x "$ROOT/target/release/kraken-historical" ]]; then
  BIN="$ROOT/target/release/kraken-historical"
elif command -v kraken-historical >/dev/null 2>&1; then
  BIN="kraken-historical"
else
  BIN=""
fi

echo "=== historical-rs (kraken) loop status $(date -u +%Y-%m-%dT%H:%M:%SZ) ==="

if [[ -f docs/kraken/STATUS.md ]]; then
  echo "--- STATUS checklist ---"
  grep -E '^\- \[[ x]\]' docs/kraken/STATUS.md || true
fi

echo "--- sync process ---"
if [[ -f /tmp/kraken-historical-s3.pid ]] && kill -0 "$(cat /tmp/kraken-historical-s3.pid)" 2>/dev/null; then
  echo "pid=$(cat /tmp/kraken-historical-s3.pid) sync=running"
  pgrep -af 'kraken-historical mirror s3|scripts/kraken/sync-s3' | head -3 || true
else
  if pgrep -af 'kraken-historical mirror s3|scripts/kraken/sync-s3' >/dev/null 2>&1; then
    pgrep -af 'kraken-historical mirror s3|scripts/kraken/sync-s3' | head -3
    echo "sync=running (no pidfile)"
  else
    echo "sync=stopped (expected on cleaned VPS — resume-later)"
  fi
fi

echo "--- sync log (last 8 lines) ---"
if [[ -f /tmp/kraken-historical-s3.log ]]; then
  tail -8 /tmp/kraken-historical-s3.log
else
  echo "(no /tmp/kraken-historical-s3.log)"
fi

if [[ -n "$BIN" ]]; then
  echo "--- mirror status (S3) ---"
  "$BIN" mirror status --s3 2>/dev/null || echo "mirror status --s3 failed (or offline)"
fi

echo "--- cron ---"
crontab -l 2>/dev/null | sed -n '/KRAKEN-HISTORICAL-CRON/,/KRAKEN-HISTORICAL-CRON-END/p' || echo "(no Kraken cron block — expected on cleaned VPS)"

echo "=== end ==="
