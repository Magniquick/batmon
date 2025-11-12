#!/usr/bin/env bash
set -euo pipefail
SCRIPT_DIR="$(CDPATH='' cd -- "$(dirname "$0")" && pwd -P)"
. "$SCRIPT_DIR/common.sh"
PERCENT="${1:-0}"
STATE="${2:-unknown}"
set_ppd power-saver
MESSAGE="Battery critical at ${PERCENT}% (${STATE}). Hybrid sleep in 15s if still discharging."
batwatch_notify "$PERCENT" "$STATE" "$MESSAGE" "critical"
"$SCRIPT_DIR/hybrid-sleep-if-needed.sh" "$PERCENT" &
