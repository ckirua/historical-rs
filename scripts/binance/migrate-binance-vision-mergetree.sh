#!/usr/bin/env bash
# Migrate binance_vision.* from ReplacingMergeTree → MergeTree.
# Safe to re-run: skips tables already on MergeTree.
set -euo pipefail

ENV_FILE="${ENV_FILE:-$HOME/.env}"
if [[ -f "$ENV_FILE" ]]; then
  set -a
  # shellcheck disable=SC1090
  source <(sed 's/\r$//' "$ENV_FILE")
  set +a
fi

CH_HOST="${CLICKHOUSE_HOST:-127.0.0.1}"
CH_PORT="${CLICKHOUSE_PORT:-9000}"
CH_USER="${CLICKHOUSE_USER:-default}"
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"

ch() {
  clickhouse-client \
    --host "$CH_HOST" \
    --port "$CH_PORT" \
    --user "$CH_USER" \
    ${CLICKHOUSE_PASSWORD:+--password "$CLICKHOUSE_PASSWORD"} \
    "$@"
}

echo "==> applying MergeTree DDL"
ch --multiquery < "$ROOT/sql/clickhouse/010_binance_vision_merge_tree.sql"

migrate_one() {
  local table="$1"
  local mt="${table}_mt"
  local cols="$2"
  local engine
  engine=$(ch -q "SELECT engine FROM system.tables WHERE database='binance_vision' AND name='$table'")
  if [[ "$engine" == "MergeTree" ]]; then
    echo "==> $table already MergeTree — skip"
    return 0
  fi
  echo "==> migrating $table ($engine → MergeTree)"
  local src_rows
  src_rows=$(ch -q "SELECT sum(rows) FROM system.parts WHERE database='binance_vision' AND table='$table' AND active")
  echo "    source rows≈$src_rows"
  ch -q "TRUNCATE TABLE IF EXISTS binance_vision.$mt"
  ch -q "INSERT INTO binance_vision.$mt ($cols) SELECT $cols FROM binance_vision.$table"
  local dst_rows
  dst_rows=$(ch -q "SELECT sum(rows) FROM system.parts WHERE database='binance_vision' AND table='$mt' AND active")
  echo "    dest rows≈$dst_rows"
  ch -q "RENAME TABLE binance_vision.$table TO binance_vision.${table}_replacing_old, binance_vision.$mt TO binance_vision.$table"
  echo "    done — old table kept as ${table}_replacing_old (drop later)"
}

migrate_one trades "kind, symbol, id, price, qty, quote_qty, time, is_buyer_maker"
migrate_one klines "kind, symbol, interval, open_time, close_time, open, high, low, close, volume, quote_volume, count, taker_buy_volume, taker_buy_quote_volume"
migrate_one book_depth "kind, symbol, timestamp, percentage, depth, notional"
migrate_one bvol "symbol, base_asset, quote_asset, calc_time, index_value"

echo "==> engines after migration"
ch -q "SELECT name, engine FROM system.tables WHERE database='binance_vision' AND name IN ('trades','klines','book_depth','bvol','ingest_log') ORDER BY name"
