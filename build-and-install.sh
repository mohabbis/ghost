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

echo "=== 6. Resetting TCC permissions database ==="
tccutil reset Accessibility com.muhammadrafiq.ghost || true
tccutil reset ListenEvent com.muhammadrafiq.ghost || true
tccutil reset PostEvent com.muhammadrafiq.ghost || true

echo "=== 7. Relaunching Ghost ==="
open /Applications/Ghost.app

echo "=== Done! Please grant Accessibility and Input Monitoring permissions in System Settings when prompted, then restart the app. ==="
