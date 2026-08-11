#!/usr/bin/env bash
# Fetch Deribit public trades (+ optional DVOL) → parquet.zstd → S3.
# Unauthenticated is fine for daily incremental (no DERIBIT_CLIENT_* required).
# Credentials optional for higher rate limits on large backfills.
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

PARALLEL="${DERIBIT_HISTORICAL_PARALLEL:-2}"
FROM="${FROM:-2024-01-01}"
TO="${TO:-$(date -u -d 'yesterday' +%F)}"
CURRENCY="${CURRENCY:-BTC,ETH,USDC}"
INCLUDE_EXPIRED="${INCLUDE_EXPIRED:-0}"
# Daily cron sets PERPETUALS_ONLY=1 to keep the instrument set small.
PERPETUALS_ONLY="${PERPETUALS_ONLY:-0}"
SYNC_TRADES="${SYNC_TRADES:-1}"
SYNC_DVOL="${SYNC_DVOL:-1}"
VOL_CURRENCY="${VOL_CURRENCY:-BTC,ETH}"
VOL_RESOLUTION="${VOL_RESOLUTION:-3600}"

if [[ -x "$ROOT/target/release/deribit-historical" ]]; then
  BIN="$ROOT/target/release/deribit-historical"
elif command -v deribit-historical >/dev/null 2>&1; then
  BIN="deribit-historical"
else
  BIN="cargo run --quiet -p deribit-historical-cli --"
fi

if command -v stdbuf >/dev/null 2>&1; then
  RUN=(stdbuf -oL -eL "$BIN")
else
  RUN=("$BIN")
fi

cli_supports() {
  local sub="$1"
  "$BIN" mirror --help 2>/dev/null | grep -Eq "(^|[[:space:]])${sub}([[:space:]]|$)"
}

discover_perpetuals() {
  local currency="$1"
  local names
  names="$("${RUN[@]}" mirror instruments --currency "$currency" --kind future 2>/dev/null \
    | awk -F'\t' 'NF>=1 && $1 ~ /PERPETUAL$/ { print $1 }')"
  if [[ -z "$names" ]]; then
    return 1
  fi
  # shellcheck disable=SC2001
  echo "$names" | paste -sd, -
}

EXTRA_ARGS=()
if [[ -n "${INSTRUMENTS:-}" ]]; then
  EXTRA_ARGS+=(--instruments "$INSTRUMENTS")
elif [[ "$PERPETUALS_ONLY" == "1" || "$PERPETUALS_ONLY" == "true" ]]; then
  discovered=()
  IFS=',' read -r -a currencies <<<"$CURRENCY"
  for c in "${currencies[@]}"; do
    c="$(echo "$c" | tr -d '[:space:]')"
    [[ -z "$c" ]] && continue
    if names="$(discover_perpetuals "$c")"; then
      discovered+=("$names")
      echo "==> discovered ${c} perpetuals: $names"
    else
      echo "warning: no perpetuals discovered for $c" >&2
    fi
  done
  if [[ ${#discovered[@]} -eq 0 ]]; then
    echo "error: PERPETUALS_ONLY=1 but no instruments discovered" >&2
    exit 1
  fi
  # Join currency groups with commas (each group already comma-separated).
  INSTRUMENTS_JOINED="$(IFS=,; echo "${discovered[*]}")"
  EXTRA_ARGS+=(--instruments "$INSTRUMENTS_JOINED")
fi

if [[ "$INCLUDE_EXPIRED" == "1" || "$INCLUDE_EXPIRED" == "true" ]]; then
  EXTRA_ARGS+=(--include-expired)
fi

echo "==> S3 bucket: ${S3_DERIBIT_HISTORICAL_BUCKET:-${S3_DERIBIT_BUCKET:-mercat-deribit}} (prefix historical/)"
echo "==> Endpoint: ${S3_DERIBIT_HISTORICAL_ENDPOINT:-${S3_URL:-unset}}"
echo "==> currency=$CURRENCY  ${FROM}→${TO}  parallel=$PARALLEL  trades=$SYNC_TRADES  dvol=$SYNC_DVOL"
if [[ -z "${DERIBIT_CLIENT_ID:-}" || -z "${DERIBIT_CLIENT_SECRET:-}" ]]; then
  echo "info: DERIBIT_CLIENT_ID/SECRET unset — unauthenticated per-IP limits (ok for daily incremental)" >&2
fi
export RUST_LOG="${RUST_LOG:-info}"
echo "==> RUST_LOG=$RUST_LOG"

failed=0

if [[ "$SYNC_TRADES" == "1" || "$SYNC_TRADES" == "true" ]]; then
  echo "==> mirror s3 (public trades)"
  if ! "${RUN[@]}" mirror s3 \
    --currency "$CURRENCY" \
    --kind future \
    --from "$FROM" \
    --to "$TO" \
    --resume \
    --parallel "$PARALLEL" \
    "${EXTRA_ARGS[@]}"; then
    echo "error: trade mirror failed" >&2
    failed=1
  fi
fi

if [[ "$SYNC_DVOL" == "1" || "$SYNC_DVOL" == "true" ]]; then
  if cli_supports "volatility-index"; then
    echo "==> mirror volatility-index (DVOL OHLC)"
    if ! "${RUN[@]}" mirror volatility-index \
      --currency "$VOL_CURRENCY" \
      --resolution "$VOL_RESOLUTION" \
      --from "$FROM" \
      --to "$TO" \
      --resume \
      --parallel "$PARALLEL"; then
      echo "error: volatility-index mirror failed" >&2
      failed=1
    fi
  else
    echo "info: CLI lacks volatility-index — skip (rebuild deribit-historical for DVOL)" >&2
  fi

  if cli_supports "dvol-trades"; then
    echo "==> mirror dvol-trades"
    DVOL_EXTRA=()
    if [[ "$INCLUDE_EXPIRED" == "1" || "$INCLUDE_EXPIRED" == "true" ]]; then
      DVOL_EXTRA+=(--include-expired)
    fi
    if ! "${RUN[@]}" mirror dvol-trades \
      --from "$FROM" \
      --to "$TO" \
      --resume \
      --parallel "$PARALLEL" \
      "${DVOL_EXTRA[@]}"; then
      echo "error: dvol-trades mirror failed" >&2
      failed=1
    fi
  else
    echo "info: CLI lacks dvol-trades — skip (rebuild deribit-historical for DVOL)" >&2
  fi
fi

if [[ "$failed" -ne 0 ]]; then
  exit 1
fi
echo "==> deribit sync-s3 done"
