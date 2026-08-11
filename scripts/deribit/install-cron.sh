#!/usr/bin/env bash
# Install or remove Deribit historical daily incremental cron (S3 only).
#
# Usage:
#   ./scripts/deribit/install-cron.sh
#   ./scripts/deribit/install-cron.sh --dir /root/apps/historical-rs
#   ./scripts/deribit/install-cron.sh --uninstall
#
# Window: FROM=3 days ago → TO=yesterday (UTC calendar days via date -u), resume-safe.
# Schedule: host-local cron TZ (Europe/Berlin on ops host) — late evening to avoid
# Binance Vision stampede at 06/10/14 local.
# Syncs: BTC/ETH/USDC perpetuals public trades + DVOL volatility-index + DVOL futures trades.
# Does NOT require DERIBIT_CLIENT_* (unauthenticated preferred for daily incremental).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
REPO="${DERIBIT_HISTORICAL_INSTALL_DIR:-$ROOT}"
UNINSTALL=0
MARKER_BEGIN="# DERIBIT-HISTORICAL-CRON-BEGIN (managed by scripts/deribit/install-cron.sh)"
MARKER_END="# DERIBIT-HISTORICAL-CRON-END"
LOG_DIR="/var/log"
if [[ ! -w "$LOG_DIR" ]]; then
  LOG_DIR="${HOME}/logs/deribit"
  mkdir -p "$LOG_DIR"
fi
CRON_LOG="${DERIBIT_HISTORICAL_CRON_LOG:-$LOG_DIR/deribit-historical-s3.log}"

usage() {
  cat <<EOF
Install Deribit historical daily incremental cron (S3 only)

Usage:
  ./scripts/deribit/install-cron.sh [options]

Options:
  --dir PATH     Repo path for cron cd (default: monorepo root)
  --uninstall    Remove Deribit historical cron block only
  -h, --help     Show this help

Schedule (host-local cron time), rolling last ~3 days ending yesterday (UTC dates),
ENV_FILE=\$HOME/.env — no DERIBIT_CLIENT_* required:
  FROM=\$(date -u -d '3 days ago' +%F) TO=\$(date -u -d yesterday +%F)
  22:00  Deribit → S3  BTC/ETH/USDC perpetuals trades + DVOL vol-index + DVOL trades

Cron times follow the host timezone (see timedatectl / /etc/localtime).
FROM/TO use date -u (UTC calendar days), independent of schedule TZ.

Requires ~/.env with S3_* / S3_DERIBIT_* credentials (bucket mercat-deribit).
Release binary: \$REPO/target/release/deribit-historical
Log: $CRON_LOG
Docs: docs/deribit/DEPLOY.md
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

# Shared env + rolling 3-day window ending yesterday (UTC calendar via date -u).
# Cron treats bare '%' as newline — escape as \% in the installed crontab.
# shellcheck disable=SC2016
cron_env_prefix() {
  printf "cd %s && ENV_FILE=\$HOME/.env FROM=\$(date -u -d '3 days ago' +\\%%F) TO=\$(date -u -d yesterday +\\%%F) CURRENCY=BTC,ETH,USDC PERPETUALS_ONLY=1 SYNC_TRADES=1 SYNC_DVOL=1 DERIBIT_HISTORICAL_PARALLEL=2" "$REPO"
}

cron_block() {
  local p
  p="$(cron_env_prefix)"
  cat <<EOF
$MARKER_BEGIN
SHELL=/bin/bash
PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin:\$HOME/.cargo/bin
# Schedule times = host-local cron TZ (not UTC). FROM/TO = UTC calendar days (date -u).
# Daily 22:00 local — Deribit → S3 (last 3d→yesterday; BTC/ETH/USDC perps + DVOL)
# Staggered after Binance Vision 06/10/14. Unauthenticated OK (no DERIBIT_CLIENT_*).
0 22 * * * $p ./scripts/deribit/sync-s3.sh >> $CRON_LOG 2>&1
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
  if [[ ! -x "$REPO/target/release/deribit-historical" ]]; then
    log "Release binary missing — building first"
    (cd "$REPO" && cargo build --release -p deribit-historical-cli)
  elif ! "$REPO/target/release/deribit-historical" mirror --help 2>/dev/null | grep -Fq "volatility-index"; then
    log "Release binary lacks DVOL commands — rebuilding"
    (cd "$REPO" && cargo build --release -p deribit-historical-cli)
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
    log "Installed/updated Deribit historical cron for $REPO"
  else
    log "Installed Deribit historical cron for $REPO"
  fi

  log "  22:00 local Deribit S3 → $CRON_LOG"
  log "  (BTC/ETH/USDC perpetuals + DVOL vol-index + DVOL trades; last 3d→yesterday UTC)"
  log "  (schedule = host-local cron TZ; FROM/TO = UTC via date -u)"
  log "  (no DERIBIT_CLIENT_* required)"
  log "Verify: crontab -l | sed -n '/DERIBIT-HISTORICAL-CRON/,/DERIBIT-HISTORICAL-CRON-END/p'"
}

uninstall_cron() {
  if ! have_crontab; then
    log "No crontab — nothing to remove"
    return 0
  fi

  local current filtered
  current="$(read_crontab)"
  if ! printf '%s\n' "$current" | grep -Fq "$MARKER_BEGIN"; then
    log "No Deribit historical cron block found"
    return 0
  fi

  filtered="$(printf '%s\n' "$current" | strip_managed_block | sed '/^[[:space:]]*$/d' || true)"
  if [[ -n "$filtered" ]]; then
    printf '%s\n' "$filtered" | crontab -
  else
    crontab -r 2>/dev/null || true
  fi
  log "Removed Deribit historical cron block"
}

if [[ "$UNINSTALL" -eq 1 ]]; then
  uninstall_cron
else
  install_cron
fi
