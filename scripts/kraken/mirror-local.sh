#!/usr/bin/env bash
# Mirror Kraken Drive → local parquet.zstd.
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

PARALLEL="${KRAKEN_HISTORICAL_PARALLEL:-4}"
DATA_TYPES="${DATA_TYPES:-trades,ohlcvt}"
SOURCE_MODE="${SOURCE_MODE:-quarterly}"
FROM_QUARTER="${FROM_QUARTER:-}"
TO_QUARTER="${TO_QUARTER:-}"

if [[ -x "$ROOT/target/release/kraken-historical" ]]; then
  BIN="$ROOT/target/release/kraken-historical"
elif command -v kraken-historical >/dev/null 2>&1; then
  BIN="kraken-historical"
else
  BIN="cargo run --quiet -p kraken-historical-cli --"
fi

PAIR_ARGS=()
if [[ -n "${PAIRS:-}" ]]; then
  PAIR_ARGS=(--pairs "$PAIRS")
fi

QUARTER_ARGS=()
if [[ -n "$FROM_QUARTER" ]]; then
  QUARTER_ARGS+=(--from-quarter "$FROM_QUARTER")
fi
if [[ -n "$TO_QUARTER" ]]; then
  QUARTER_ARGS+=(--to-quarter "$TO_QUARTER")
fi

IFS=',' read -ra TYPES <<< "$DATA_TYPES"
for dtype in "${TYPES[@]}"; do
  dtype="$(echo "$dtype" | xargs)"
  case "$dtype" in
    trades|ohlcvt)
      echo "==> mirror local data-type=$dtype source=$SOURCE_MODE"
      $BIN mirror run \
        --data-type "$dtype" --source "$SOURCE_MODE" \
        "${QUARTER_ARGS[@]}" "${PAIR_ARGS[@]}" \
        --resume --parallel "$PARALLEL" "$@"
      ;;
    *)
      echo "unknown data type: $dtype" >&2
      exit 1
      ;;
  esac
done
