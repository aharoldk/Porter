#!/usr/bin/env bash
# Make Porter the default browser on Linux.
# Usage: ./install-default-browser.sh /path/to/porter [icon.png]
# Revert: ./install-default-browser.sh /path/to/porter --revert
#
# macOS registration is handled in-app via LaunchServices, so this script is
# Linux-only.
set -euo pipefail

BIN="$(realpath "${1:?usage: $0 /path/to/porter [icon.png]}")"
if [[ ! -x "$BIN" ]]; then
  echo "Not an executable: $BIN" >&2
  exit 1
fi

if [[ "${2:-}" == "--revert" ]]; then
  DESKTOP="$HOME/.local/share/applications/porter.desktop"
  rm -f "$DESKTOP"
  update-desktop-database "$HOME/.local/share/applications" 2>/dev/null || true
  # Drop the packaged entry too if it was the one set as default.
  CURRENT="$(xdg-settings get default-web-browser 2>/dev/null || true)"
  if [[ "$CURRENT" == "Porter.desktop" ]]; then
    xdg-settings set default-web-browser "" 2>/dev/null || true
  fi
  echo "Porter is no longer the default browser."
  echo "To revert the default browser setting, run: xdg-settings set default-web-browser <your-previous-browser.desktop>"
  exit 0
fi

ICON_PATH="porter"
if [[ -n "${2:-}" && -f "$2" ]]; then
  ICONS_DIR="$HOME/.local/share/icons"
  mkdir -p "$ICONS_DIR"
  cp "$2" "$ICONS_DIR/porter.png"
  ICON_PATH="$ICONS_DIR/porter.png"
fi

APPS_DIR="$HOME/.local/share/applications"
DESKTOP="$APPS_DIR/porter.desktop"
mkdir -p "$APPS_DIR"

cat > "$DESKTOP" <<EOF
[Desktop Entry]
Type=Application
Name=Porter
Comment=Open links in a chosen browser profile
Exec=$BIN %u
Icon=$ICON_PATH
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
