#!/usr/bin/env bash
set -euo pipefail

APP_NAME="MacMousePaste"
LABEL="com.vachagan.macmousepaste"

DEST_APP="$HOME/Applications/${APP_NAME}.app"
PLIST_PATH="$HOME/Library/LaunchAgents/${LABEL}.plist"
U_ID="$(id -u)"
DOMAIN="gui/${U_ID}"

echo "Stopping LaunchAgent…"
launchctl bootout "$DOMAIN" "$PLIST_PATH" 2>/dev/null || true

echo "Removing LaunchAgent plist…"
rm -f "$PLIST_PATH" || true

echo "Removing app…"
rm -rf "$DEST_APP" || true

echo "Done."
