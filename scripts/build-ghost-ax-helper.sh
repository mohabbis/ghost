#!/usr/bin/env bash
# Build GhostAXHelper sidecars for Tauri externalBin (macOS only).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
SRC="$ROOT/native/macos/GhostAXHelper.swift"
OUT_DIR="$ROOT/src-tauri/bin"
MIN_MACOS="10.15"

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "GhostAXHelper build skipped (requires macOS + swiftc)" >&2
  exit 0
fi

if ! command -v swiftc >/dev/null 2>&1; then
  echo "swiftc not found — cannot build GhostAXHelper" >&2
  exit 1
fi

mkdir -p "$OUT_DIR"

ARM="$OUT_DIR/ghost-ax-helper-aarch64-apple-darwin"
X64="$OUT_DIR/ghost-ax-helper-x86_64-apple-darwin"
# Tauri resolves externalBin for a `--target universal-apple-darwin` build to
# `<name>-universal-apple-darwin` and does NOT lipo the per-arch sidecars itself,
# so the release build needs this fat binary staged in src-tauri/bin/ too.
UNIVERSAL="$OUT_DIR/ghost-ax-helper-universal-apple-darwin"

swiftc -O -target "arm64-apple-macos${MIN_MACOS}" -o "$ARM" "$SRC"
swiftc -O -target "x86_64-apple-macos${MIN_MACOS}" -o "$X64" "$SRC"
chmod +x "$ARM" "$X64"

if command -v lipo >/dev/null 2>&1; then
  lipo -create "$ARM" "$X64" -output "$UNIVERSAL"
else
  # No lipo (non-macOS host can't reach here anyway): fall back to the arm slice.
  cp "$ARM" "$UNIVERSAL"
fi
chmod +x "$UNIVERSAL"

# Dev convenience: universal fat binary for local runs without GHOST_AX_HELPER.
DEV="$ROOT/native/macos/ghost-ax-helper"
cp "$UNIVERSAL" "$DEV"
chmod +x "$DEV"

echo "Built GhostAXHelper:"
echo "  $ARM"
echo "  $X64"
echo "  $UNIVERSAL"
echo "  $DEV (dev)"
