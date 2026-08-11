#!/usr/bin/env bash
# Install deribit-historical CLI (build + optional global install).
#
# Usage:
#   ./scripts/deribit/install.sh
#   ./scripts/deribit/install.sh --dir /path/to/historical-rs
#   ./scripts/deribit/install.sh --global
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
DIR="$ROOT"
GLOBAL=false

usage() {
  cat <<EOF
Install deribit-historical

Usage:
  ./scripts/deribit/install.sh [options]

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
cargo build --release -p deribit-historical-cli

BIN="$DIR/target/release/deribit-historical"
echo "Built: $BIN"
"$BIN" --help >/dev/null || true

if $GLOBAL; then
  echo "==> cargo install --path crates/deribit-historical-cli --force"
  cargo install --path crates/deribit-historical-cli --force
  echo "Installed: $(command -v deribit-historical || echo '~/.cargo/bin/deribit-historical')"
fi

LOCAL_BIN="${HOME}/.local/bin"
if [[ -d "$LOCAL_BIN" ]]; then
  ln -sfn "$BIN" "$LOCAL_BIN/deribit-historical"
  echo "Symlinked: $LOCAL_BIN/deribit-historical -> $BIN"
fi

echo "==> done"
echo "  CLI:  $BIN"
echo "  Sync: $DIR/scripts/deribit/sync-s3.sh"
