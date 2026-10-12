#!/usr/bin/env bash
# Compiles the Icon Composer .icon into Assets.car with actool, exactly as Tauri does, and prints actool's own output
# On the GitHub macOS runner Tauri's built-in call fails with no message while this one works, so CI compiles the icon
# here and hands Tauri the finished file (see ci-icons.conf.json)
# Usage: compile-icon.sh <Icon.icon> <Assets.car>
set -uo pipefail

src="${1:?path to the .icon folder}"
dest="${2:?where to put Assets.car}"
[ -d "$src" ] || { echo "No such .icon folder: $src" >&2; exit 1; }

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
cp -R "$src" "$tmp/Icon.icon"

compile() {
  rm -rf "$tmp/out"
  mkdir "$tmp/out"
  actool "$tmp/Icon.icon" --compile "$tmp/out" --output-format human-readable-text --notices --warnings \
    --output-partial-info-plist "$tmp/out/assetcatalog_generated_info.plist" --app-icon Icon --include-all-app-icons \
    --accent-color AccentColor --enable-on-demand-resources NO --development-region en \
    --target-device mac --minimum-deployment-target 26.0 --platform macosx
  local code=$?
  if [ "$code" = 0 ] && [ -f "$tmp/out/Assets.car" ]; then
    return 0
  fi
  echo "actool exit code: $code"
  return 1
}

for attempt in 1 2 3; do
  echo "actool attempt $attempt"
  if compile; then
    cp "$tmp/out/Assets.car" "$dest"
    echo "Assets.car written to $dest"
    exit 0
  fi
  # Apple's CI reports name this as the usual cure for actool failing on a fresh machine
  if [ "$attempt" = 1 ]; then
    sudo xcodebuild -runFirstLaunch
  fi
  sleep 5
done

echo "actool failed three times" >&2
exit 1
