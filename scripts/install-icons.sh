#!/usr/bin/env bash
set -euo pipefail
SCRIPT_DIR="$(CDPATH='' cd -- "$(dirname "$0")" && pwd -P)"
ROOT="${SCRIPT_DIR}/.."
ICON_SRC_COLOR="$ROOT/assets/bat.svg"
ICON_SRC_SYMBOLIC="$ROOT/assets/bat_white.svg"
TARGET_BASE="${XDG_DATA_HOME:-$HOME/.local/share}/icons/hicolor/scalable/apps"
mkdir -p "$TARGET_BASE"
install -m 0644 "$ICON_SRC_COLOR" "$TARGET_BASE/batwatch.svg"
install -m 0644 "$ICON_SRC_SYMBOLIC" "$TARGET_BASE/batwatch-symbolic.svg"
if command -v gtk-update-icon-cache >/dev/null 2>&1; then
  gtk-update-icon-cache -f "${XDG_DATA_HOME:-$HOME/.local/share}/icons" >/dev/null 2>&1 || true
fi
echo "Installed batwatch.svg and batwatch-symbolic.svg into $TARGET_BASE"
echo "You may need to restart your session or run 'gtk-update-icon-cache' manually for the icon theme to pick them up."
