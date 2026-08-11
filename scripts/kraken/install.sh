#!/usr/bin/env bash
# Install kraken-historical CLI (build + optional global + optional cron).
#
# Usage:
#   ./scripts/kraken/install.sh
#   ./scripts/kraken/install.sh --dir /path/to/historical-rs
#   ./scripts/kraken/install.sh --global
#   ./scripts/kraken/install.sh --cron   # future ingest host only
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
DIR="$ROOT"
GLOBAL=false
INSTALL_CRON=false

usage() {
  cat <<EOF
Install kraken-historical

Usage:
  ./scripts/kraken/install.sh [options]

Options:
  --dir PATH   Repo path (default: monorepo root)
  --global     cargo install binary onto PATH
  --cron       Install monthly S3 sync cron (future ingest host; do not enable on cleaned VPS)
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
    --cron)
      INSTALL_CRON=true
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

if [[ ! -f "$DIR/Cargo.toml" ]] || [[ ! -d "$DIR/crates/kraken-historical-cli" ]]; then
  echo "error: not historical-rs monorepo root (missing Cargo.toml or crates/kraken-historical-cli): $DIR" >&2
  exit 1
fi

echo "==> building release binary in $DIR"
cargo build --release -p kraken-historical-cli

BIN="$DIR/target/release/kraken-historical"
echo "Built: $BIN"
"$BIN" --version || true

if $GLOBAL; then
  echo "==> cargo install --path crates/kraken-historical-cli --force"
  cargo install --path crates/kraken-historical-cli --force
  echo "Installed: $(command -v kraken-historical || echo '~/.cargo/bin/kraken-historical')"
fi

LOCAL_BIN="${HOME}/.local/bin"
if [[ -d "$LOCAL_BIN" ]]; then
  ln -sfn "$BIN" "$LOCAL_BIN/kraken-historical"
  echo "Symlinked: $LOCAL_BIN/kraken-historical -> $BIN"
fi

if $INSTALL_CRON; then
  "$DIR/scripts/kraken/install-cron.sh" --dir "$DIR"
fi

echo "==> done"
echo "  CLI:    $BIN"
echo "  Sync:   $DIR/scripts/kraken/sync-s3.sh"
echo "  BG:     $DIR/scripts/kraken/sync-s3-bg.sh"
echo "  Cron:   $DIR/scripts/kraken/install-cron.sh"
echo "  Status: $DIR/scripts/kraken/loop-status.sh"
