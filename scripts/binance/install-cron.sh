#!/usr/bin/env bash
# Install or remove Binance Vision daily incremental cron (S3 only — no ClickHouse).
#
# Usage:
#   ./scripts/binance/install-cron.sh
#   ./scripts/binance/install-cron.sh --dir /root/apps/historical-rs
#   ./scripts/binance/install-cron.sh --uninstall
#
# Window: FROM=7 days ago → TO=yesterday (UTC calendar days via date -u), inclusive rolling catch-up.
# Schedule times below are host-local (cron daemon TZ), not UTC — staggered to reduce overlap.
# sync-s3.sh passes --resume (S3 HEAD skips existing objects).
# Markets/types (no spot bookDepth):
#   UM     trades,klines,bookDepth,BVOLIndex  → /var/log/binance-vision-s3.log
#   spot   trades,klines                      → /var/log/binance-vision-s3-spot.log
#   CM     trades,klines,bookDepth            → /var/log/binance-vision-s3-cm.log
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
REPO="${BINANCE_VISION_INSTALL_DIR:-$ROOT}"
UNINSTALL=0
MARKER_BEGIN="# BINANCE-VISION-CRON-BEGIN (managed by scripts/binance/install-cron.sh)"
MARKER_END="# BINANCE-VISION-CRON-END"
LOG_DIR="/var/log"
if [[ ! -w "$LOG_DIR" ]]; then
  LOG_DIR="/tmp"
fi

usage() {
  cat <<EOF
Install Binance Vision daily incremental cron jobs (S3 only)

Usage:
  ./scripts/binance/install-cron.sh [options]

Options:
  --dir PATH     Repo path for cron cd (default: monorepo root)
  --uninstall    Remove Binance Vision cron block only
  -h, --help     Show this help

Schedule (host-local cron time), rolling last ~7 days ending yesterday (UTC dates), ENV_FILE=\$HOME/.env:
  FROM=\$(date -u -d '7 days ago' +%F) TO=\$(date -u -d yesterday +%F)
  06:00  UM S3     trades,klines,bookDepth,BVOLIndex
  10:00  spot S3   trades,klines
  14:00  CM S3     trades,klines,bookDepth

Cron times follow the host timezone (see timedatectl / /etc/localtime).
FROM/TO use date -u (UTC calendar days), independent of schedule TZ.

No ClickHouse cron — run clickhouse-direct-*.sh manually if needed.
Requires ~/.env with S3_BINANCE_VISION_* credentials.
Release binary: \$REPO/target/release/binance-vision
Docs: docs/binance/DEPLOY.md
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

# Shared env + rolling 7-day window ending yesterday (UTC calendar via date -u).
# Cron treats bare '%' as newline — escape as \% in the installed crontab.
# shellcheck disable=SC2016
cron_env_prefix() {
  printf "cd %s && ENV_FILE=\$HOME/.env FROM=\$(date -u -d '7 days ago' +\\%%F) TO=\$(date -u -d yesterday +\\%%F)" "$REPO"
}

cron_block() {
  local p
  p="$(cron_env_prefix)"
  cat <<EOF
$MARKER_BEGIN
SHELL=/bin/bash
PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin:\$HOME/.cargo/bin
# Schedule times = host-local cron TZ (not UTC). FROM/TO = UTC calendar days (date -u).
# Daily 06:00 local — UM Vision → S3 (last 7d→yesterday; trades/klines/bookDepth/BVOL)
0 6 * * * $p MARKET=um DATA_TYPES=trades,klines,bookDepth,BVOLIndex ./scripts/binance/sync-s3.sh >> $LOG_DIR/binance-vision-s3.log 2>&1
# Daily 10:00 local — spot Vision → S3 (last 7d→yesterday; trades/klines only — no bookDepth)
0 10 * * * $p MARKET=spot DATA_TYPES=trades,klines ./scripts/binance/sync-s3.sh >> $LOG_DIR/binance-vision-s3-spot.log 2>&1
# Daily 14:00 local — CM Vision → S3 (last 7d→yesterday; trades/klines/bookDepth)
0 14 * * * $p MARKET=cm DATA_TYPES=trades,klines,bookDepth ./scripts/binance/sync-s3.sh >> $LOG_DIR/binance-vision-s3-cm.log 2>&1
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
  if [[ ! -x "$REPO/target/release/binance-vision" ]]; then
    log "Release binary missing — building first"
    (cd "$REPO" && cargo build --release -p binance-vision-cli)
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
    log "Installed/updated Binance Vision cron for $REPO"
  else
    log "Installed Binance Vision cron for $REPO"
  fi

  log "  06:00 local UM S3    → $LOG_DIR/binance-vision-s3.log"
  log "  10:00 local spot S3  → $LOG_DIR/binance-vision-s3-spot.log"
  log "  14:00 local CM S3    → $LOG_DIR/binance-vision-s3-cm.log"
  log "  (schedule = host-local cron TZ; FROM/TO = UTC via date -u)"
  log "  (no ClickHouse cron)"
  log "Verify: crontab -l | sed -n '/BINANCE-VISION-CRON/,/BINANCE-VISION-CRON-END/p'"
}

uninstall_cron() {
  if ! have_crontab; then
    log "No crontab — nothing to remove"
    return 0
  fi

  local current filtered
  current="$(read_crontab)"
  if ! printf '%s\n' "$current" | grep -Fq "$MARKER_BEGIN"; then
    log "No Binance Vision cron block found"
    return 0
  fi

  filtered="$(printf '%s\n' "$current" | strip_managed_block | sed '/^[[:space:]]*$/d' || true)"
  if [[ -n "$filtered" ]]; then
    printf '%s\n' "$filtered" | crontab -
  else
    crontab -r 2>/dev/null || true
  fi
  log "Removed Binance Vision cron block"
}

if [[ "$UNINSTALL" -eq 1 ]]; then
  uninstall_cron
else
  install_cron
fi
