#!/usr/bin/env bash
set -euo pipefail
SCRIPT_DIR="$(CDPATH='' cd -- "$(dirname "$0")" && pwd -P)"
. "$SCRIPT_DIR/common.sh"
PERCENT="${1:-0}"
STATE="${2:-unknown}"
set_ppd power-saver
if (( ${PERCENT%.*} <= 15 )); then
  URGENCY=normal
else
  URGENCY=low
fi
MESSAGE="Battery at ${PERCENT}% (${STATE}). Power saver enabled."
batwatch_notify "$PERCENT" "$STATE" "$MESSAGE" "$URGENCY"
