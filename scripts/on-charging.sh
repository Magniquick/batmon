#!/usr/bin/env bash
set -euo pipefail
SCRIPT_DIR="$(CDPATH='' cd -- "$(dirname "$0")" && pwd -P)"
. "$SCRIPT_DIR/common.sh"
PERCENT="${1:-0}"
STATE="${2:-unknown}"
MESSAGE="$(hostname) is at ${PERCENT}% and ${STATE}."
batwatch_notify "$PERCENT" "$STATE" "$MESSAGE"
