#!/bin/bash
# Builds the Flatpak from a tauri-built .deb (build that .deb on the OLDEST glibc you want to support).
# Usage: build.sh <path-to.deb> [gpg-key-id]
# Output: ./repo (OSTree repo, ready to host), ./LemiCraft-Launcher.flatpak (single-file bundle)
set -e
cd "$(dirname "$0")"
DEB="${1:?path to the .deb}"
KEY="$2"
APP=ru.lemicraft.Launcher

cp -f "$DEB" lemicraft.deb
SIGN=()
[ -n "$KEY" ] && SIGN=(--gpg-sign="$KEY")

flatpak-builder --user --force-clean --disable-rofiles-fuse --default-branch=stable --repo=repo "${SIGN[@]}" build-dir "$APP.yml"
flatpak build-update-repo repo "${SIGN[@]}" --generate-static-deltas --prune
flatpak build-bundle repo LemiCraft-Launcher.flatpak "$APP" stable "${SIGN[@]}"
rm -f lemicraft.deb
echo "OK: repo/ and LemiCraft-Launcher.flatpak"
