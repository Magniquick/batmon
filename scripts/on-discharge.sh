#!/usr/bin/env bash
set -euo pipefail
SCRIPT_DIR="$(CDPATH='' cd -- "$(dirname "$0")" && pwd -P)"
. "$SCRIPT_DIR/common.sh"
PERCENT="${1:-0}"
STATE="${2:-unknown}"
MESSAGE="Battery at ${PERCENT}% and ${STATE}. Plug in soon!"
batwatch_notify "$PERCENT" "$STATE" "$MESSAGE"
