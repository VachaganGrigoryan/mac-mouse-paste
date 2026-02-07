#!/usr/bin/env bash
set -euo pipefail

APP_NAME="MacMousePaste"
BUNDLE_ID="com.vachagan.macmousepaste"
LABEL="com.vachagan.macmousepaste"

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SRC_APP="$ROOT_DIR/target/release/${APP_NAME}.app"

DEST_DIR="$HOME/Applications"
DEST_APP="$DEST_DIR/${APP_NAME}.app"

PLIST_DIR="$HOME/Library/LaunchAgents"
PLIST_PATH="$PLIST_DIR/${LABEL}.plist"

U_ID="$(id -u)"
DOMAIN="gui/${U_ID}"

if [[ ! -d "$SRC_APP" ]]; then
  echo "ERROR: Build the .app first:"
  echo "  ./scripts/build_app.sh"
  exit 1
fi

echo "[1/6] Install app to: $DEST_APP"
mkdir -p "$DEST_DIR"
/usr/bin/ditto "$SRC_APP" "$DEST_APP"

chmod +x "$DEST_APP/Contents/MacOS/$APP_NAME" || true

echo "[2/6] Remove quarantine"
xattr -dr com.apple.quarantine "$DEST_APP" 2>/dev/null || true

echo "[3/6] (Optional) Ad-hoc codesign"
if command -v codesign >/dev/null 2>&1; then
  codesign --force --deep --sign - "$DEST_APP" 2>/dev/null || true
fi

echo "[4/6] Register bundle (helps Finder)"
LSREG="/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister"
if [[ -x "$LSREG" ]]; then
  "$LSREG" -f "$DEST_APP" 2>/dev/null || true
fi

echo "[5/6] Install LaunchAgent (Start at Login = ON by default)"
mkdir -p "$PLIST_DIR"

APP_EXEC="$DEST_APP/Contents/MacOS/$APP_NAME"

cat > "$PLIST_PATH" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
  <dict>
    <key>Label</key>
    <string>${LABEL}</string>

    <key>ProgramArguments</key>
    <array>
      <string>${APP_EXEC}</string>
    </array>

    <key>RunAtLoad</key>
    <true/>

    <!-- No KeepAlive: quitting should quit -->
    <key>StandardOutPath</key>
    <string>/tmp/macmousepaste.out.log</string>
    <key>StandardErrorPath</key>
    <string>/tmp/macmousepaste.err.log</string>
  </dict>
</plist>
EOF

# Modern launchd flow
echo "  - bootout old (if any)"
launchctl bootout "$DOMAIN" "$PLIST_PATH" 2>/dev/null || true

echo "  - bootstrap + enable + kickstart"
launchctl bootstrap "$DOMAIN" "$PLIST_PATH"
launchctl enable "$DOMAIN/${LABEL}" 2>/dev/null || true
launchctl kickstart -k "$DOMAIN/${LABEL}" 2>/dev/null || true

echo "[6/6] Done."
echo "App:   $DEST_APP"
echo "Agent: $PLIST_PATH"
echo ""
echo "Permissions: System Settings → Privacy & Security → Accessibility (and Input Monitoring if needed)"
