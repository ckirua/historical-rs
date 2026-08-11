#!/usr/bin/env bash
# Install or remove Kraken historical monthly cron (S3 check for new quarters).
#
# Usage:
#   ./scripts/kraken/install-cron.sh
#   ./scripts/kraken/install-cron.sh --dir /path/to/historical-rs
#   ./scripts/kraken/install-cron.sh --uninstall
#
# Intended for a future ingest host only — do not enable cron on the cleaned VPS.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
REPO="${KRAKEN_HISTORICAL_INSTALL_DIR:-$ROOT}"
UNINSTALL=0
MARKER_BEGIN="# KRAKEN-HISTORICAL-CRON-BEGIN (managed by scripts/kraken/install-cron.sh)"
MARKER_END="# KRAKEN-HISTORICAL-CRON-END"
LOG_DIR="/var/log"
if [[ ! -w "$LOG_DIR" ]]; then
  LOG_DIR="/tmp"
fi
CRON_LOG="${KRAKEN_HISTORICAL_CRON_LOG:-$LOG_DIR/kraken-historical-s3-monthly.log}"

usage() {
  cat <<EOF
Install Kraken historical monthly cron jobs

Usage:
  ./scripts/kraken/install-cron.sh [options]

Options:
  --dir PATH     Repo path for cron cd (default: script parent dir)
  --uninstall    Remove Kraken historical cron block only
  -h, --help     Show this help

Schedule (UTC):
  Monthly 1st 08:00  sync-s3.sh (complete + all quarters, resume)

Requires ~/.env with S3_KRAKEN_HISTORICAL_* and preferably KRAKEN_GOOGLE_DRIVE_API_KEY.
Log: $CRON_LOG
Docs: docs/kraken/DEPLOY.md
EOF
}

log() { printf '==> %s\n' "$*"; }
die() { printf 'error: %s\n' "$*" >&2; exit 1; }

while [[ $# -gt 0 ]]; do
  case "$1" in
    --dir) REPO="${2:?}"; shift 2 ;;
    --uninstall) UNINSTALL=1; shift ;;
    -h|--help) usage; exit 0 ;;
    *) die "unknown option: $1 (try --help)" ;;
  esac
done

REPO="$(cd "$REPO" && pwd)"

have_crontab() {
  crontab -l >/dev/null 2>&1
}

read_crontab() {
  if have_crontab; then
    crontab -l
  fi
}

strip_managed_block() {
  awk -v begin="$MARKER_BEGIN" -v end="$MARKER_END" '
    $0 == begin { skip=1; next }
    $0 == end { skip=0; next }
    !skip { print }
  '
}

cron_block() {
  cat <<EOF
$MARKER_BEGIN
SHELL=/bin/bash
PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin:\$HOME/.cargo/bin
# Monthly 1st 08:00 UTC — Kraken Drive → S3 (resume; pick up new quarters)
0 8 1 * * cd $REPO && ENV_FILE=\$HOME/.env SOURCE_MODE=both ./scripts/kraken/sync-s3.sh >> $CRON_LOG 2>&1
$MARKER_END
EOF
}

already_installed() {
  local current
  current="$(read_crontab || true)"
  [[ -n "$current" ]] || return 1
  printf '%s\n' "$current" | grep -Fq "$MARKER_BEGIN" || return 1
  printf '%s\n' "$current" | grep -Fq "cd $REPO && ENV_FILE" || return 1
}

install_cron() {
  if [[ ! -x "$REPO/target/release/kraken-historical" ]]; then
    log "Release binary missing — building first"
    (cd "$REPO" && cargo build --release -p kraken-historical-cli)
  fi

  local filtered new_crontab
  filtered="$(read_crontab | strip_managed_block | sed '/^[[:space:]]*$/d' || true)"
  if [[ -n "$filtered" ]]; then
    new_crontab="$(printf '%s\n\n' "$filtered"; cron_block)"
  else
    new_crontab="$(cron_block)"
  fi
  printf '%s\n' "$new_crontab" | crontab -

  if already_installed; then
    log "Installed/updated Kraken historical cron for $REPO"
  else
    log "Installed Kraken historical cron for $REPO"
  fi

  log "  Monthly 1st 08:00 UTC — ./scripts/kraken/sync-s3.sh (SOURCE_MODE=both)"
  log "  Log: $CRON_LOG"
  log "Verify: crontab -l | sed -n '/KRAKEN-HISTORICAL-CRON/,/KRAKEN-HISTORICAL-CRON-END/p'"
}

uninstall_cron() {
  if ! have_crontab; then
    log "No crontab — nothing to remove"
    return 0
  fi

  local current filtered
  current="$(read_crontab)"
  if ! printf '%s\n' "$current" | grep -Fq "$MARKER_BEGIN"; then
    log "No Kraken historical cron block found"
    return 0
  fi

  filtered="$(printf '%s\n' "$current" | strip_managed_block | sed '/^[[:space:]]*$/d' || true)"
  if [[ -n "$filtered" ]]; then
    printf '%s\n' "$filtered" | crontab -
  else
    crontab -r 2>/dev/null || true
  fi
  log "Removed Kraken historical cron block"
}

if [[ "$UNINSTALL" -eq 1 ]]; then
  uninstall_cron
else
  install_cron
fi
