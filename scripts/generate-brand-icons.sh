#!/usr/bin/env bash
# Generate Tauri + marketing icon assets from brand/ghost-app-icon.svg
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
SRC="$ROOT/brand/ghost-app-icon.svg"
MARK="$ROOT/brand/ghost-mark.svg"
FAV="$ROOT/brand/favicon.svg"
ICONS="$ROOT/src-tauri/icons"
PUBLIC="$ROOT/public"

if ! command -v rsvg-convert >/dev/null 2>&1; then
  echo "rsvg-convert required (librsvg2-bin)" >&2
  exit 1
fi

render() {
  local size="$1"
  local out="$2"
  rsvg-convert -w "$size" -h "$size" "$SRC" -o "$out"
}

mkdir -p "$ICONS"

echo "Rendering app icons from $SRC"
render 32 "$ICONS/32x32.png"
render 128 "$ICONS/128x128.png"
render 256 "$ICONS/128x128@2x.png"
render 512 "$ICONS/icon.png"
render 1024 "$ICONS/icon-source.png"

for size in 30 44 71 89 107 142 150 284 310; do
  render "$size" "$ICONS/Square${size}x${size}Logo.png"
done
cp "$ICONS/Square150x150Logo.png" "$ICONS/StoreLogo.png"

# Windows .ico from common sizes
if command -v convert >/dev/null 2>&1; then
  convert "$ICONS/32x32.png" "$ICONS/128x128.png" "$ICONS/128x128@2x.png" "$ICONS/icon.png" "$ICONS/icon.ico"
else
  cp "$ICONS/icon.png" "$ICONS/icon.ico"
fi

# macOS .icns
if command -v png2icns >/dev/null 2>&1; then
  png2icns "$ICONS/icon.icns" \
    "$ICONS/32x32.png" \
    "$ICONS/128x128.png" \
    "$ICONS/128x128@2x.png" \
    "$ICONS/icon.png" \
    "$ICONS/icon-source.png" 2>/dev/null || \
  png2icns "$ICONS/icon.icns" "$ICONS/icon.png"
else
  echo "png2icns not found — skipping icon.icns (macOS release may regenerate)"
fi

# Marketing + desktop favicons
cp "$FAV" "$PUBLIC/favicon.svg"
cp "$FAV" "$ROOT/src/favicon.svg"
cp "$MARK" "$PUBLIC/assets/ghost-mark.svg"
render 180 "$PUBLIC/apple-touch-icon.png"

echo "Done. Icons written to $ICONS and $PUBLIC"
