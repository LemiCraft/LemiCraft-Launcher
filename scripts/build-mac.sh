#!/bin/bash
set -e

TARGET="${1:-all}"
export PATH="$HOME/.cargo/bin:/opt/homebrew/bin:$PATH"

mkdir -p dist-macos/arm64 dist-macos/x86_64 dist-macos/universal

build_arm64() {
  echo "==> Building arm64 (Apple Silicon)..."
  npx tauri build --target aarch64-apple-darwin --bundles app --no-sign
  rm -rf "dist-macos/arm64/LemiCraft.app"
  if [ -d "src-tauri/target/aarch64-apple-darwin/release/bundle/macos/LemiCraft.app" ]; then
    cp -R "src-tauri/target/aarch64-apple-darwin/release/bundle/macos/LemiCraft.app" "dist-macos/arm64/"
  else
    cp -R "src-tauri/target/aarch64-apple-darwin/debug/bundle/macos/LemiCraft.app" "dist-macos/arm64/"
  fi
  codesign --force --deep --sign - "dist-macos/arm64/LemiCraft.app"
  ditto -c -k --sequesterRsrc --keepParent "dist-macos/arm64/LemiCraft.app" "dist-macos/LemiCraft-arm64.zip"
  echo "==> arm64 ready: dist-macos/arm64/LemiCraft.app"
}

build_x86() {
  echo "==> Building x86_64 (Intel)..."
  npx tauri build --target x86_64-apple-darwin --bundles app --no-sign
  rm -rf "dist-macos/x86_64/LemiCraft.app"
  if [ -d "src-tauri/target/x86_64-apple-darwin/release/bundle/macos/LemiCraft.app" ]; then
    cp -R "src-tauri/target/x86_64-apple-darwin/release/bundle/macos/LemiCraft.app" "dist-macos/x86_64/"
  else
    cp -R "src-tauri/target/x86_64-apple-darwin/debug/bundle/macos/LemiCraft.app" "dist-macos/x86_64/"
  fi
  codesign --force --deep --sign - "dist-macos/x86_64/LemiCraft.app"
  ditto -c -k --sequesterRsrc --keepParent "dist-macos/x86_64/LemiCraft.app" "dist-macos/LemiCraft-x86_64.zip"
  echo "==> x86_64 ready: dist-macos/x86_64/LemiCraft.app"
}

build_universal() {
  echo "==> Creating Universal bundle (Intel + Apple Silicon)..."
  rm -rf "dist-macos/universal/LemiCraft.app"
  cp -R "dist-macos/arm64/LemiCraft.app" "dist-macos/universal/"
  lipo -create \
    "dist-macos/x86_64/LemiCraft.app/Contents/MacOS/LemiCraft-Launcher" \
    "dist-macos/arm64/LemiCraft.app/Contents/MacOS/LemiCraft-Launcher" \
    -output "dist-macos/universal/LemiCraft.app/Contents/MacOS/LemiCraft-Launcher"
  codesign --force --deep --sign - "dist-macos/universal/LemiCraft.app"
  ditto -c -k --sequesterRsrc --keepParent "dist-macos/universal/LemiCraft.app" "dist-macos/LemiCraft-universal.zip"
  echo "==> Universal ready: dist-macos/universal/LemiCraft.app"
}

case "$TARGET" in
  arm64)
    build_arm64
    ;;
  x86|x86_64)
    build_x86
    ;;
  universal)
    build_arm64
    build_x86
    build_universal
    ;;
  all)
    build_arm64
    build_x86
    build_universal
    ;;
  *)
    echo "Usage: $0 [arm64|x86|universal|all]"
    exit 1
    ;;
esac
