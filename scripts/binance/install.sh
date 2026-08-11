#!/usr/bin/env bash
# Build binance-vision CLI from the historical-rs monorepo.
set -euo pipefail

DIR="${1:-}"
GLOBAL=false

while [[ $# -gt 0 ]]; do
  case "$1" in
    --dir)
      DIR="$2"
      shift 2
      ;;
    --global)
      GLOBAL=true
      shift
      ;;
    *)
      shift
      ;;
  esac
done

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
DIR="${DIR:-$ROOT}"

cd "$DIR"
cargo build --release -p binance-vision-cli

echo "Built: $DIR/target/release/binance-vision"
if $GLOBAL; then
  cargo install --path crates/binance-vision-cli --force
fi
