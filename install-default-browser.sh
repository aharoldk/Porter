#!/usr/bin/env bash
# Make Porter the default browser on macOS & Linux.
# Usage: ./install-default-browser.sh /path/to/porter
# Revert: ./install-default-browser.sh /path/to/porter --revert
set -euo pipefail

BIN="$(realpath "${1:?usage: $0 /path/to/porter}")"
if [[ ! -x "$BIN" ]]; then
  echo "Not an executable: $BIN" >&2
  exit 1
fi

if [[ "${2:-}" == "--revert" ]]; then
  if [[ "$OSTYPE" == "darwin"* ]]; then
    # macOS
    defaults delete com.apple.LaunchServices LSSetDefaultHandlerForURLScheme http 2>/dev/null || true
    defaults delete com.apple.LaunchServices LSSetDefaultHandlerForURLScheme https 2>/dev/null || true
    echo "Porter is no longer the default browser on macOS."
  else
    # Linux
    DESKTOP="$HOME/.local/share/applications/porter.desktop"
    rm -f "$DESKTOP"
    update-desktop-database "$HOME/.local/share/applications" 2>/dev/null || true
    echo "Porter is no longer the default browser."
    echo "To revert the default browser setting, run: xdg-settings set default-web-browser <your-previous-browser.desktop>"
  fi
  exit 0
fi

if [[ "$OSTYPE" == "darwin"* ]]; then
  # macOS
  APP_DIR="$HOME/Library/Application Support/Porter"
  mkdir -p "$APP_DIR"
  cp "$(dirname "$0")/src-tauri/icons/32x32.png" "$APP_DIR/porter.png" 2>/dev/null || true

  defaults write com.apple.LaunchServices LSSetDefaultHandlerForURLScheme http "$BIN"
  defaults write com.apple.LaunchServices LSSetDefaultHandlerForURLScheme https "$BIN"
  echo "Porter is now the default browser on macOS."
  echo "http/https links will open Porter."
else
  # Linux
  APPS_DIR="$HOME/.local/share/applications"
  ICONS_DIR="$HOME/.local/share/icons"
  DESKTOP="$APPS_DIR/porter.desktop"
  ICON="$ICONS_DIR/porter.png"

  mkdir -p "$APPS_DIR" "$ICONS_DIR"
  cp "$(dirname "$0")/src-tauri/icons/32x32.png" "$ICON"

  cat > "$DESKTOP" <<EOF
[Desktop Entry]
Type=Application
Name=Porter
Comment=Open links in a chosen browser profile
Exec=$BIN %u
Icon=$ICON
Terminal=false
Categories=Network;WebBrowser;
MimeType=x-scheme-handler/http;x-scheme-handler/https;
StartupNotify=false
EOF

  chmod +x "$DESKTOP"
  update-desktop-database "$APPS_DIR" 2>/dev/null || true
  xdg-settings set default-web-browser porter.desktop

  echo "Porter is now the default browser."
  echo "Links will open in the Porter panel. Press Enter to launch."
fi
