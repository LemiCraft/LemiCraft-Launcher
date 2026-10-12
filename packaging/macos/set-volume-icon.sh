#!/usr/bin/env bash
# Tauri takes the dmg disk icon from the same icon.icns as the app, so the real disk icon is swapped in here after the build
# Usage: set-volume-icon.sh <file.dmg> <icon.icns>
set -euo pipefail

dmg="${1:?path to the .dmg}"
icon="${2:?path to the .icns}"
[ -f "$dmg" ] || { echo "No such dmg: $dmg" >&2; exit 1; }
[ -f "$icon" ] || { echo "No such icon: $icon" >&2; exit 1; }

work="$(mktemp -d)"
mnt="$work/mnt"
mkdir "$mnt"
attached=0

detach() {
  local attempt
  for attempt in 1 2 3 4 5; do
    if hdiutil detach "$mnt" -quiet; then
      attached=0
      return 0
    fi
    sleep $((attempt * 2))
  done
  hdiutil detach "$mnt" -force -quiet
  attached=0
}

cleanup() {
  if [ "$attached" = 1 ]; then
    hdiutil detach "$mnt" -force -quiet || true
  fi
  rm -rf "$work"
}
trap cleanup EXIT

# A finished dmg is read-only and compressed: make a writable copy, change the icon, compress it back
hdiutil convert "$dmg" -format UDRW -o "$work/rw.dmg" -quiet
hdiutil attach "$work/rw.dmg" -readwrite -noverify -noautoopen -nobrowse -mountpoint "$mnt" -quiet
attached=1

cp "$icon" "$mnt/.VolumeIcon.icns"
SetFile -c icnC "$mnt/.VolumeIcon.icns"
SetFile -a C "$mnt"
sync
detach

hdiutil convert "$work/rw.dmg" -format UDZO -imagekey zlib-level=9 -ov -o "$dmg" -quiet

# Open the result once more and make sure the new icon is really inside
hdiutil attach "$dmg" -readonly -noverify -noautoopen -nobrowse -mountpoint "$mnt" -quiet
attached=1
cmp "$icon" "$mnt/.VolumeIcon.icns"
detach

echo "Disk icon set in $dmg"
