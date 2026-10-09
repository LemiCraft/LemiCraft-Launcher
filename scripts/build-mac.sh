#!/bin/bash
set -e

TARGET="${1:-all}"
export PATH="$HOME/.cargo/bin:/opt/homebrew/bin:$PATH"

mkdir -p dist-macos/arm64 dist-macos/x86_64 dist-macos/universal

build_arm64() {
  echo "==> Building arm64 (Apple Silicon)..."
  npx tauri build --target aarch64-apple-darwin --bundles app --no-sign
  rm -rf "dist-macos/arm64/LemiCraft Launcher.app"
  if [ -d "src-tauri/target/aarch64-apple-darwin/release/bundle/macos/LemiCraft Launcher.app" ]; then
    cp -R "src-tauri/target/aarch64-apple-darwin/release/bundle/macos/LemiCraft Launcher.app" "dist-macos/arm64/"
  else
    cp -R "src-tauri/target/aarch64-apple-darwin/debug/bundle/macos/LemiCraft Launcher.app" "dist-macos/arm64/"
  fi
  codesign --force --deep --sign - "dist-macos/arm64/LemiCraft Launcher.app"
  ditto -c -k --sequesterRsrc --keepParent "dist-macos/arm64/LemiCraft Launcher.app" "dist-macos/Lemicraft-Launcher-arm64.zip"
  echo "==> arm64 ready: dist-macos/arm64/LemiCraft Launcher.app"
}

build_x86() {
  echo "==> Building x86_64 (Intel)..."
  npx tauri build --target x86_64-apple-darwin --bundles app --no-sign
  rm -rf "dist-macos/x86_64/Lemicraft Launcher.app"
  if [ -d "src-tauri/target/x86_64-apple-darwin/release/bundle/macos/Lemicraft Launcher.app" ]; then
    cp -R "src-tauri/target/x86_64-apple-darwin/release/bundle/macos/Lemicraft Launcher.app" "dist-macos/x86_64/"
  else
    cp -R "src-tauri/target/x86_64-apple-darwin/debug/bundle/macos/Lemicraft Launcher.app" "dist-macos/x86_64/"
  fi
  codesign --force --deep --sign - "dist-macos/x86_64/Lemicraft Launcher.app"
  ditto -c -k --sequesterRsrc --keepParent "dist-macos/x86_64/Lemicraft Launcher.app" "dist-macos/Lemicraft-Launcher-x86_64.zip"
  echo "==> x86_64 ready: dist-macos/x86_64/Lemicraft Launcher.app"
}

build_universal() {
  echo "==> Creating Universal bundle (Intel + Apple Silicon)..."
  rm -rf "dist-macos/universal/Lemicraft Launcher.app"
  cp -R "dist-macos/arm64/Lemicraft Launcher.app" "dist-macos/universal/"
  lipo -create \
    "dist-macos/x86_64/Lemicraft Launcher.app/Contents/MacOS/Lemicraft-Launcher" \
    "dist-macos/arm64/Lemicraft Launcher.app/Contents/MacOS/Lemicraft-Launcher" \
    -output "dist-macos/universal/Lemicraft Launcher.app/Contents/MacOS/Lemicraft-Launcher"
  codesign --force --deep --sign - "dist-macos/universal/Lemicraft Launcher.app"
  ditto -c -k --sequesterRsrc --keepParent "dist-macos/universal/Lemicraft Launcher.app" "dist-macos/Lemicraft-Launcher-universal.zip"
  echo "==> Universal ready: dist-macos/universal/Lemicraft Launcher.app"
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
