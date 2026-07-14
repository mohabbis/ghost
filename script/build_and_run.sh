#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP_NAME="Ghost"
SWIFT_PRODUCT="GhostNative"
BUNDLE_ID="com.muhammadrafiq.ghost.native"
MIN_MACOS="26.0"
DIST_DIR="$ROOT/dist"
APP_BUNDLE="$DIST_DIR/$APP_NAME.app"
CONTENTS="$APP_BUNDLE/Contents"
MACOS_DIR="$CONTENTS/MacOS"
RESOURCES_DIR="$CONTENTS/Resources"
MODE="run"

usage() {
  cat <<USAGE
Usage: ./script/build_and_run.sh [--debug|--logs|--telemetry|--verify|--build-only]

Builds the shared Rust bridge and native SwiftUI app, stages dist/Ghost.app,
then launches the fresh bundle. The existing Tauri application is untouched.
USAGE
}

for arg in "$@"; do
  case "$arg" in
    --debug) MODE="debug" ;;
    --logs) MODE="logs" ;;
    --telemetry) MODE="telemetry" ;;
    --verify) MODE="verify" ;;
    --build-only) MODE="build-only" ;;
    -h|--help) usage; exit 0 ;;
    *) echo "Unknown option: $arg" >&2; usage >&2; exit 2 ;;
  esac
done

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "The native Ghost app requires macOS. This host reports $(uname -s)." >&2
  exit 1
fi

command -v cargo >/dev/null 2>&1 || {
  echo "cargo is required to build the trusted Rust core." >&2
  exit 1
}
command -v swift >/dev/null 2>&1 || {
  echo "swift is required to build the native macOS shell." >&2
  exit 1
}

VERSION="$(sed -n 's/^version = "\([^"]*\)"/\1/p' "$ROOT/src-tauri/Cargo.toml" | head -n 1)"
VERSION="${VERSION:-0.0.0}"

pkill -x "$APP_NAME" >/dev/null 2>&1 || true
pkill -x ghost_core_bridge >/dev/null 2>&1 || true

printf 'Building Rust bridge...\n'
cargo build \
  --manifest-path "$ROOT/src-tauri/Cargo.toml" \
  --bin ghost_core_bridge

printf 'Building SwiftUI application...\n'
swift build \
  --package-path "$ROOT/apps/macos" \
  --configuration debug \
  --product "$SWIFT_PRODUCT"

SWIFT_BIN_DIR="$(swift build --package-path "$ROOT/apps/macos" --configuration debug --show-bin-path)"
SWIFT_BINARY="$SWIFT_BIN_DIR/$SWIFT_PRODUCT"
RUST_BINARY="$ROOT/src-tauri/target/debug/ghost_core_bridge"

[[ -x "$SWIFT_BINARY" ]] || {
  echo "Swift product was not produced at $SWIFT_BINARY" >&2
  exit 1
}
[[ -x "$RUST_BINARY" ]] || {
  echo "Rust bridge was not produced at $RUST_BINARY" >&2
  exit 1
}

rm -rf "$APP_BUNDLE"
mkdir -p "$MACOS_DIR" "$RESOURCES_DIR"
cp "$SWIFT_BINARY" "$MACOS_DIR/$APP_NAME"
cp "$RUST_BINARY" "$MACOS_DIR/ghost_core_bridge"
chmod 755 "$MACOS_DIR/$APP_NAME" "$MACOS_DIR/ghost_core_bridge"

ICON_KEY=""
if [[ -f "$ROOT/src-tauri/icons/icon.icns" ]]; then
  cp "$ROOT/src-tauri/icons/icon.icns" "$RESOURCES_DIR/AppIcon.icns"
  ICON_KEY='<key>CFBundleIconFile</key><string>AppIcon</string>'
fi

cat > "$CONTENTS/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleDevelopmentRegion</key><string>en</string>
  <key>CFBundleDisplayName</key><string>$APP_NAME</string>
  <key>CFBundleExecutable</key><string>$APP_NAME</string>
  <key>CFBundleIdentifier</key><string>$BUNDLE_ID</string>
  <key>CFBundleInfoDictionaryVersion</key><string>6.0</string>
  <key>CFBundleName</key><string>$APP_NAME</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>$VERSION</string>
  <key>CFBundleVersion</key><string>$VERSION</string>
  $ICON_KEY
  <key>LSMinimumSystemVersion</key><string>$MIN_MACOS</string>
  <key>NSHighResolutionCapable</key><true/>
  <key>NSPrincipalClass</key><string>NSApplication</string>
</dict>
</plist>
PLIST

SIGNING_IDENTITY="${GHOST_CODESIGN_IDENTITY:--}"
/usr/bin/codesign \
  --force \
  --deep \
  --options runtime \
  --sign "$SIGNING_IDENTITY" \
  "$APP_BUNDLE"

printf 'Staged %s\n' "$APP_BUNDLE"

if [[ "$MODE" == "build-only" ]]; then
  exit 0
fi

if [[ "$MODE" == "debug" ]]; then
  exec /usr/bin/lldb -- "$MACOS_DIR/$APP_NAME"
fi

/usr/bin/open -n "$APP_BUNDLE"

case "$MODE" in
  logs)
    exec /usr/bin/log stream --info --style compact \
      --predicate 'process == "Ghost" OR process == "ghost_core_bridge"'
    ;;
  telemetry)
    exec /usr/bin/log stream --info --style compact \
      --predicate 'subsystem == "com.muhammadrafiq.ghost.native" OR process == "ghost_core_bridge"'
    ;;
  verify)
    sleep 2
    pgrep -x "$APP_NAME" >/dev/null || {
      echo "$APP_NAME did not remain running." >&2
      exit 1
    }
    pgrep -x ghost_core_bridge >/dev/null || {
      echo "ghost_core_bridge did not remain running." >&2
      exit 1
    }
    printf '%s and ghost_core_bridge are running.\n' "$APP_NAME"
    ;;
  run) ;;
esac
