#!/usr/bin/env sh
set -eu

REPO_URL="${BATWATCH_REPO_URL:-https://github.com/Magniquick/batmon.git}"
INSTALL_REF="${BATWATCH_INSTALL_REF:-}"
BIN_DIR="${CARGO_HOME:-$HOME/.cargo}/bin"
BATWATCH_BIN="$BIN_DIR/batwatch"
CONFIG_HOME="${XDG_CONFIG_HOME:-$HOME/.config}"
SERVICE_FILE="$CONFIG_HOME/systemd/user/batwatch.service"

if ! command -v cargo >/dev/null 2>&1; then
  echo "BatWatch: cargo is required. Install Rust from https://rustup.rs/ first." >&2
  exit 1
fi

if ! command -v systemctl >/dev/null 2>&1; then
  echo "BatWatch: systemctl is required to enable the user service." >&2
  exit 1
fi

if [ -n "$INSTALL_REF" ]; then
  cargo install --git "$REPO_URL" --rev "$INSTALL_REF" --force
else
  cargo install --git "$REPO_URL" --force
fi

if [ ! -x "$BATWATCH_BIN" ]; then
  if command -v batwatch >/dev/null 2>&1; then
    BATWATCH_BIN="$(command -v batwatch)"
  else
    echo "BatWatch: installed binary was not found on PATH or at $BATWATCH_BIN." >&2
    exit 1
  fi
fi

"$BATWATCH_BIN" --init-config --force

UNIT_BIN="$BATWATCH_BIN"
case "$BATWATCH_BIN" in
  "$HOME"/*) UNIT_BIN="%h/${BATWATCH_BIN#"$HOME"/}" ;;
esac

if [ -f "$SERVICE_FILE" ]; then
  tmp_service="$(mktemp)"
  sed "s|^ExecStart=.*|ExecStart=$UNIT_BIN|" "$SERVICE_FILE" > "$tmp_service"
  cat "$tmp_service" > "$SERVICE_FILE"
  rm -f "$tmp_service"
fi

systemctl --user daemon-reload
systemctl --user enable batwatch.service
systemctl --user restart batwatch.service

echo "BatWatch: installed, configured, and started."
