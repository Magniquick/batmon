#!/usr/bin/env bash
set -euo pipefail
SCRIPT_DIR="$(CDPATH='' cd -- "$(dirname "$0")" && pwd -P)"
. "$SCRIPT_DIR/common.sh"
PERCENT="${1:-0}"
STATE="${2:-unknown}"
THRESHOLD=15
CURRENT="${PERCENT%.*}"
if (( CURRENT <= THRESHOLD )); then
  PROFILE="power-saver"
  MESSAGE="Power saver maintained (${STATE}, ${PERCENT}%)."
else
  PROFILE="balanced"
  MESSAGE="Balanced power mode restored (${STATE}, ${PERCENT}%)."
fi
set_ppd "$PROFILE"
batwatch_notify "$PERCENT" "$STATE" "$MESSAGE" "low"
