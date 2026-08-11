#!/usr/bin/env bash
# Install okx-historical CLI (build + optional global + optional cron).
#
# Usage:
#   ./scripts/okx/install.sh
#   ./scripts/okx/install.sh --dir /root/merge-hist/historical-rs
#   ./scripts/okx/install.sh --global
#   ./scripts/okx/install.sh --cron
#   ./scripts/okx/install.sh --global --cron
#
# Note: --cron is for a future deploy host. Do not enable cron on the cleaned VPS.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
DIR="$ROOT"
GLOBAL=false
INSTALL_CRON=false

usage() {
  cat <<EOF
Install okx-historical

Usage:
  ./scripts/okx/install.sh [options]

Options:
  --dir PATH   Repo path (default: monorepo root)
  --global     cargo install binary onto PATH
  --cron       Install daily S3 incremental cron (yesterday UTC; future host)
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
      # positional dir for compatibility
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
cargo build --release -p okx-historical-cli

BIN="$DIR/target/release/okx-historical"
echo "Built: $BIN"
"$BIN" --version || true

if $GLOBAL; then
  echo "==> cargo install --path crates/okx-historical-cli --force"
  cargo install --path crates/okx-historical-cli --force
  echo "Installed: $(command -v okx-historical || echo '~/.cargo/bin/okx-historical')"
fi

# Convenience symlink into ~/.local/bin when present
LOCAL_BIN="${HOME}/.local/bin"
if [[ -d "$LOCAL_BIN" ]]; then
  ln -sfn "$BIN" "$LOCAL_BIN/okx-historical"
  echo "Symlinked: $LOCAL_BIN/okx-historical -> $BIN"
fi

if $INSTALL_CRON; then
  "$DIR/scripts/okx/install-cron.sh" --dir "$DIR"
fi

echo "==> done"
echo "  CLI:    $BIN"
echo "  Sync:   $DIR/scripts/okx/sync-s3.sh"
echo "  BG:     $DIR/scripts/okx/sync-s3-bg.sh"
echo "  Cron:   $DIR/scripts/okx/install-cron.sh"
echo "  Status: $DIR/scripts/okx/loop-status.sh"
