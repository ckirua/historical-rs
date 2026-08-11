#!/usr/bin/env bash
# Install bybit-historical CLI (build + optional global install).
#
# Usage:
#   ./scripts/bybit/install.sh
#   ./scripts/bybit/install.sh --dir /path/to/historical-rs
#   ./scripts/bybit/install.sh --global
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
DIR="$ROOT"
GLOBAL=false

usage() {
  cat <<EOF
Install bybit-historical

Usage:
  ./scripts/bybit/install.sh [options]

Options:
  --dir PATH   Repo path (default: monorepo root)
  --global     cargo install binary onto PATH
  -h, --help   Show this help
EOF
}

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
    -h|--help)
      usage
      exit 0
      ;;
    *)
      if [[ -d "$1" ]]; then
        DIR="$1"
        shift
      else
        echo "unknown option: $1 (try --help)" >&2
        exit 1
      fi
      ;;
  esac
done

DIR="$(cd "$DIR" && pwd)"
cd "$DIR"

if [[ ! -f "$DIR/Cargo.toml" ]]; then
  echo "error: not a historical-rs repo: $DIR" >&2
  exit 1
fi

echo "==> building release binary in $DIR"
cargo build --release -p bybit-historical-cli

BIN="$DIR/target/release/bybit-historical"
echo "Built: $BIN"
"$BIN" --version || true

if $GLOBAL; then
  echo "==> cargo install --path crates/bybit-historical-cli --force"
  cargo install --path crates/bybit-historical-cli --force
  echo "Installed: $(command -v bybit-historical || echo '~/.cargo/bin/bybit-historical')"
fi

LOCAL_BIN="${HOME}/.local/bin"
if [[ -d "$LOCAL_BIN" ]]; then
  ln -sfn "$BIN" "$LOCAL_BIN/bybit-historical"
  echo "Symlinked: $LOCAL_BIN/bybit-historical -> $BIN"
fi

echo "==> done"
echo "  CLI:  $BIN"
echo "  Sync: $DIR/scripts/bybit/sync-s3.sh"
echo "  Raw:  $DIR/scripts/bybit/download-orderbook.py"
