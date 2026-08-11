#!/usr/bin/env bash
# Retry Kraken Drive → S3 for artifacts blocked by anonymous Drive quota.
# Downloads one quarter at a time with long cool-downs; resumes via S3 HEAD.
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
# Short cool-down between per-quarter retries; outer loops should space rounds.
SLEEP_SECS="${SLEEP_SECS:-120}"
DATA_TYPE="${DATA_TYPE:-ohlcvt}"
BIN="${BIN:-$ROOT/target/release/kraken-historical}"

if [[ ! -x "$BIN" ]]; then
  cargo build --release -p kraken-historical-cli
fi

mapfile -t QUARTERS < <("$BIN" discover --data-type "$DATA_TYPE" 2>/dev/null | awk '$2 ~ /^[0-9]{4}Q[1-4]$/ {print $2}' | sort -u)
if [[ ${#QUARTERS[@]} -eq 0 ]]; then
  echo "no quarterly artifacts discovered for $DATA_TYPE" >&2
  exit 1
fi

echo "==> retrying $DATA_TYPE quarters: ${QUARTERS[*]}" | tee -a "$LOG"
echo "==> cool-down ${SLEEP_SECS}s between failed downloads" | tee -a "$LOG"

for q in "${QUARTERS[@]}"; do
  echo "===== $(date -u -Iseconds) attempt $DATA_TYPE $q =====" | tee -a "$LOG"
  if ENV_FILE="$ENV_FILE" SOURCE_MODE=quarterly FROM_QUARTER="$q" TO_QUARTER="$q" \
    DATA_TYPES="$DATA_TYPE" "$ROOT/scripts/kraken/sync-s3.sh" >>"$LOG" 2>&1; then
    # sync-s3 always exits 0 even on artifact skip — check for failure line
    if tail -n 30 "$LOG" | rg -q "skipping artifact.*$q|quota exceeded"; then
      echo "==> $q still blocked by Drive quota; sleeping ${SLEEP_SECS}s" | tee -a "$LOG"
      sleep "$SLEEP_SECS"
      # one more try
      ENV_FILE="$ENV_FILE" SOURCE_MODE=quarterly FROM_QUARTER="$q" TO_QUARTER="$q" \
        DATA_TYPES="$DATA_TYPE" "$ROOT/scripts/kraken/sync-s3.sh" >>"$LOG" 2>&1 || true
    else
      echo "==> $q ok or resumed" | tee -a "$LOG"
    fi
  fi
done

echo "==> retry-quota pass finished $(date -u -Iseconds)" | tee -a "$LOG"
"$BIN" mirror status --s3 | tee -a "$LOG"
