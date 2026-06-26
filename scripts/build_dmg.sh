set -euo pipefail

cd "$(dirname "$0")/.."

OUTPUT="${OUTPUT_DIR:-$(pwd)}"
APP_NAME="${APP_NAME:-AstroBox}"
DMG_NAME="${DMG_NAME:-$APP_NAME.dmg}"
MOUNT_NAME="/Volumes/$APP_NAME"
BACKGROUND="${BACKGROUND:-$(pwd)/src-tauri/modules/app/resources/dmgbg@2x.png}"
SRC="${APP_BUNDLE_PATH:-$(pwd)/src-tauri/target/release/bundle/macos/$APP_NAME.app}"
APP_BUNDLE_NAME="$(basename "$SRC")"

if [ -f "$OUTPUT/$DMG_NAME" ]; then
  echo "Existing DMG found, removing: $OUTPUT/$DMG_NAME"
  rm -f "$OUTPUT/$DMG_NAME"
fi

# Detach previously mounted DMG volume if it still exists
if [ -d "$MOUNT_NAME" ]; then
  echo "Mounted DMG volume found, detaching..."
  hdiutil detach "$MOUNT_NAME" -force || true
fi

# Restart Finder to reduce stale view/layout cache issues
killall Finder >/dev/null 2>&1 || true
sleep 1

if [ ! -d "$SRC" ]; then
  echo "App not found: $SRC"
  echo "Did you run tauri build?"
  exit 1
fi

if [ ! -f "$BACKGROUND" ]; then
  echo "Background image not found: $BACKGROUND"
  exit 1
fi

create-dmg \
  --volname "$APP_NAME" \
  --window-size 400 640 \
  --icon-size 120 \
  --text-size 14 \
  --icon "$APP_BUNDLE_NAME" 200 164 \
  --app-drop-link 200 450 \
  --background "$BACKGROUND" \
  "$OUTPUT/$DMG_NAME" \
  "$(dirname "$SRC")"
