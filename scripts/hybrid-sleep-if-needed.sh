#!/usr/bin/env bash
set -euo pipefail
TARGET_PERCENT="${TARGET_PERCENT:-10}"
DELAY="${BATWATCH_CRITICAL_DELAY:-15}"
TRIGGER_PERCENT="${1:-$TARGET_PERCENT}"
sleep "$DELAY" || true
if ! command -v upower >/dev/null 2>&1; then
  exit 0
fi
BATTERY=$(upower -e | grep -m1 BAT || true)
if [[ -z "$BATTERY" ]]; then
  exit 0
fi
INFO=$(upower -i "$BATTERY")
STATE=$(printf '%s
' "$INFO" | awk '/state:/ {print $2; exit}')
PERCENT=$(printf '%s
' "$INFO" | awk '/percentage:/ {gsub("%","",$2); print int($2); exit}')
if [[ "$STATE" == "discharging" ]] && [[ "$PERCENT" -le "$TARGET_PERCENT" ]] && [[ "$PERCENT" -le "$TRIGGER_PERCENT" ]]; then
  systemctl hybrid-sleep
fi
