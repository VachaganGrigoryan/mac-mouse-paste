#!/usr/bin/env bash
set -euo pipefail

# -------- configurable --------
APP_NAME="MacMousePaste"
BUNDLE_ID="com.vachagan.macmousepaste"
BIN_NAME="mac-mouse-paste"   # <-- your cargo binary name (target/release/<BIN_NAME>)
VERSION="1.0"
# ------------------------------

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

echo "[1/3] Building release…"
cargo build --release

BIN_PATH="$ROOT_DIR/target/release/$BIN_NAME"
if [[ ! -f "$BIN_PATH" ]]; then
  echo "ERROR: Binary not found: $BIN_PATH"
  echo "Check BIN_NAME in this script."
  exit 1
fi

OUT_APP="$ROOT_DIR/target/release/${APP_NAME}.app"
OUT_CONTENTS="$OUT_APP/Contents"
OUT_MACOS="$OUT_CONTENTS/MacOS"

echo "[2/3] Creating app bundle: $OUT_APP"
rm -rf "$OUT_APP"
mkdir -p "$OUT_MACOS"

# Copy and rename executable to match CFBundleExecutable
cp "$BIN_PATH" "$OUT_MACOS/$APP_NAME"
chmod +x "$OUT_MACOS/$APP_NAME"

# Write Info.plist
cat > "$OUT_CONTENTS/Info.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
  <dict>
    <key>CFBundleName</key>
    <string>${APP_NAME}</string>

    <key>CFBundleDisplayName</key>
    <string>${APP_NAME}</string>

    <key>CFBundleIdentifier</key>
    <string>${BUNDLE_ID}</string>

    <key>CFBundleExecutable</key>
    <string>${APP_NAME}</string>

    <key>CFBundlePackageType</key>
    <string>APPL</string>

    <key>CFBundleVersion</key>
    <string>${VERSION}</string>

    <key>CFBundleShortVersionString</key>
    <string>${VERSION}</string>

    <!-- menu-bar / background (no Dock icon) -->
    <key>LSUIElement</key>
    <true/>

    <key>NSHighResolutionCapable</key>
    <true/>
  </dict>
</plist>
EOF

echo "[3/3] Done."
echo "Built: $OUT_APP"
echo "Try:  open \"$OUT_APP\""
