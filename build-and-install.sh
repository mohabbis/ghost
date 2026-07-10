#!/bin/bash
set -e

echo "=== 1. Building Ghost Release package ==="
cargo tauri build --no-bundle

echo "=== 2. Ad-hoc signing bundle with persistent designated requirement ==="
APP_PATH="src-tauri/target/release/bundle/macos/Ghost.app"
codesign --force --deep --sign - -r="designated => identifier com.muhammadrafiq.ghost" "$APP_PATH"

echo "=== 3. Validating signature ==="
codesign -vvv --deep --strict "$APP_PATH"

echo "=== 4. Terminating existing Ghost processes ==="
pkill -i ghost || true
sleep 1

echo "=== 5. Copying cleanly to /Applications/ ==="
rm -rf /Applications/Ghost.app
cp -a "$APP_PATH" /Applications/

# NOTE: We intentionally do NOT reset TCC permissions here.
# The designated requirement is pinned to the bundle identifier
# (com.muhammadrafiq.ghost), not the binary hash. This means
# macOS will honour any existing Accessibility / Input Monitoring
# grants across rebuilds without requiring the user to re-grant.
#
# If you need to force a fresh permission prompt (rare), run:
#   tccutil reset Accessibility com.muhammadrafiq.ghost
#   tccutil reset ListenEvent com.muhammadrafiq.ghost

echo "=== 6. Relaunching Ghost ==="
open /Applications/Ghost.app

echo "=== Done! If this is a fresh install, grant Accessibility and Input Monitoring in System Settings → Privacy & Security when prompted. ==="
