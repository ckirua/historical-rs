#!/usr/bin/env bash
# Install or remove OKX historical daily cron (S3 incremental, no DB).
#
# Usage:
#   ./scripts/okx/install-cron.sh
#   ./scripts/okx/install-cron.sh --dir /root/apps/historical-rs
#   ./scripts/okx/install-cron.sh --uninstall
#
# Window: FROM=TO=yesterday (UTC calendar days via date -u), all CDN types.
# Schedule: host-local cron TZ (Europe/Berlin on ops host) — 02:00 to avoid
# Binance Vision 06/10/14 and Deribit 22:00 local.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
REPO="${OKX_HISTORICAL_INSTALL_DIR:-$ROOT}"
UNINSTALL=0
MARKER_BEGIN="# OKX-HISTORICAL-CRON-BEGIN (managed by scripts/okx/install-cron.sh)"
MARKER_END="# OKX-HISTORICAL-CRON-END"
LOG_DIR="/var/log"
# Fall back to /tmp if /var/log is not writable
if [[ ! -w "$LOG_DIR" ]]; then
  LOG_DIR="/tmp"
fi
CRON_LOG="${OKX_HISTORICAL_CRON_LOG:-$LOG_DIR/okx-historical-s3-daily.log}"

usage() {
  cat <<EOF
Install OKX historical daily cron jobs

Usage:
  ./scripts/okx/install-cron.sh [options]

Options:
  --dir PATH     Repo path for cron cd (default: monorepo root)
  --uninstall    Remove OKX historical cron block only
  -h, --help     Show this help

Schedule (host-local cron time), yesterday only (UTC dates), ENV_FILE=\$HOME/.env:
  FROM=\$(date -u -d yesterday +%F) TO=\$(date -u -d yesterday +%F)
  02:00  sync-s3.sh (all product types, all symbols)
  Sunday 04:00  symbols refresh

Cron times follow the host timezone (see timedatectl / /etc/localtime).
FROM/TO use date -u (UTC calendar days), independent of schedule TZ.
Staggered before Binance Vision 06/10/14 and Deribit 22:00.

Requires ~/.env with S3_OKX_HISTORICAL_* (or S3_OKX_BUCKET / shared S3_* aliases).
Log: $CRON_LOG
Docs: docs/okx/DEPLOY.md
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
# Schedule times = host-local cron TZ (not UTC). FROM/TO = UTC calendar days (date -u).
# Daily 02:00 local — OKX CDN → S3 (yesterday only, all types)
# Staggered before Binance Vision 06/10/14 and Deribit 22:00.
# Cron treats bare '%' as newline — escape as \%.
0 2 * * * cd $REPO && ENV_FILE=\$HOME/.env FROM=\$(date -u -d yesterday +\%F) TO=\$(date -u -d yesterday +\%F) ./scripts/okx/sync-s3.sh >> $CRON_LOG 2>&1
# Weekly Sunday 04:00 local — refresh instrument cache
0 4 * * 0 cd $REPO && ENV_FILE=\$HOME/.env ./target/release/okx-historical symbols refresh >> $CRON_LOG 2>&1
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
  if [[ ! -x "$REPO/target/release/okx-historical" ]]; then
    log "Release binary missing — building first"
    (cd "$REPO" && cargo build --release -p okx-historical-cli)
  fi

  if already_installed; then
    # Refresh block (schedule may have changed)
    local filtered new_crontab
    filtered="$(read_crontab | strip_managed_block | sed '/^[[:space:]]*$/d' || true)"
    if [[ -n "$filtered" ]]; then
      new_crontab="$(printf '%s\n\n' "$filtered"; cron_block)"
    else
      new_crontab="$(cron_block)"
    fi
    printf '%s\n' "$new_crontab" | crontab -
    log "Updated OKX historical cron for $REPO"
  else
    local filtered new_crontab
    filtered="$(read_crontab | strip_managed_block | sed '/^[[:space:]]*$/d' || true)"
    if [[ -n "$filtered" ]]; then
      new_crontab="$(printf '%s\n\n' "$filtered"; cron_block)"
    else
      new_crontab="$(cron_block)"
    fi
    printf '%s\n' "$new_crontab" | crontab -
    log "Installed OKX historical cron for $REPO"
  fi

  log "  Daily 02:00 local — ./scripts/okx/sync-s3.sh (FROM=TO=yesterday UTC)"
  log "  Sunday 04:00 local — symbols refresh"
  log "  Log: $CRON_LOG"
  log "Verify: crontab -l | sed -n '/OKX-HISTORICAL-CRON/,/OKX-HISTORICAL-CRON-END/p'"
}

uninstall_cron() {
  if ! have_crontab; then
    log "No crontab — nothing to remove"
    return 0
  fi

  local current filtered
  current="$(read_crontab)"
  if ! printf '%s\n' "$current" | grep -Fq "$MARKER_BEGIN"; then
    log "No OKX historical cron block found"
    return 0
  fi

  filtered="$(printf '%s\n' "$current" | strip_managed_block | sed '/^[[:space:]]*$/d' || true)"
  if [[ -n "$filtered" ]]; then
    printf '%s\n' "$filtered" | crontab -
  else
    crontab -r 2>/dev/null || true
  fi
  log "Removed OKX historical cron block"
}

if [[ "$UNINSTALL" -eq 1 ]]; then
  uninstall_cron
else
  install_cron
fi
