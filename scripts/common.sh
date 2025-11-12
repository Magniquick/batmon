#!/usr/bin/env bash
set -euo pipefail
SCRIPT_DIR="$(CDPATH='' cd -- "$(dirname "$0")" && pwd -P)"
DEFAULT_ICON_NAME="batwatch-symbolic"
DEFAULT_ICON_FALLBACK="$SCRIPT_DIR/../assets/bat_white.svg"
DEFAULT_NOTIFY_ID=5307972

set_ppd() {
  local profile="$1"
  if command -v powerprofilesctl >/dev/null 2>&1; then
    powerprofilesctl set "$profile" >/dev/null 2>&1 || true
  fi
}

resolve_icon() {
  local candidate="${BATWATCH_ICON:-$DEFAULT_ICON_NAME}"
  if [[ -f "$candidate" || "$candidate" = /* ]]; then
    printf '%s' "$candidate"
    return
  fi
  if icon_in_theme "$candidate"; then
    printf '%s' "$candidate"
    return
  fi
  printf '%s' "$DEFAULT_ICON_FALLBACK"
}

icon_in_theme() {
  local name="$1"
  local dirs=()
  if [[ -n "${XDG_DATA_HOME:-}" ]]; then
    dirs+=("$XDG_DATA_HOME/icons")
  fi
  dirs+=("$HOME/.local/share/icons")
  IFS=: read -ra data_dirs <<< "${XDG_DATA_DIRS:-/usr/local/share:/usr/share}"
  for base in "${data_dirs[@]}"; do
    dirs+=("$base/icons")
  done
  for dir in "${dirs[@]}"; do
    if [[ -f "$dir/hicolor/scalable/apps/$name.svg" ]]; then
      return 0
    fi
  done
  return 1
}

batwatch_notify() {
  local percent="$1"
  local state="$2"
  local message="$3"
  local urgency="${4:-normal}"
  local icon="$(resolve_icon)"
  notify-send --icon "$icon" --replace-id "$DEFAULT_NOTIFY_ID" --urgency "$urgency" "BatWatch" "$message" 2>/dev/null || echo "$message"
}
