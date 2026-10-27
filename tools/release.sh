#!/usr/bin/env bash
# Builds the release downloads into target/dist:
#   Hoardmap.dmg             the app in a window that says to drag it to Applications
#   Hoardmap.zip             the same app, zipped (the Homebrew cask installs this)
#   SHA256SUMS
# The hoardmap command is inside the app, in Contents/MacOS.
# Everything is universal (Apple silicon and Intel) and signed ad hoc. The names carry no
# version, so releases/latest/download/Hoardmap.dmg always fetches the newest one.
# Laying out the DMG window drives Finder, which macOS may ask you to allow once.
set -euo pipefail
root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root"
dist=target/dist
volume=Hoardmap
rm -rf "$dist"
mkdir -p "$dist"
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

if [ -d "/Volumes/$volume" ]; then
  echo "eject /Volumes/$volume first" >&2
  exit 1
fi

tools/bundle-app.sh --universal

ditto -c -k --keepParent target/Hoardmap.app "$dist/Hoardmap.zip"

# The DMG: the app, a link to /Applications, a background with an arrow between them.
stage="$work/stage"
mkdir -p "$stage/.background"
cp -R target/Hoardmap.app "$stage/"
ln -s /Applications "$stage/Applications"
cp assets/dmg-background.tiff "$stage/.background/background.tiff"
hdiutil create -quiet -volname "$volume" -srcfolder "$stage" -fs HFS+ -format UDRW \
  -size 64m "$work/rw.dmg"
device=$(hdiutil attach -readwrite -noverify -noautoopen "$work/rw.dmg" | awk '/Apple_HFS/ {print $1}')
osascript <<APPLESCRIPT
tell application "Finder"
  tell disk "$volume"
    open
    set current view of container window to icon view
    set toolbar visible of container window to false
    set statusbar visible of container window to false
    set the bounds of container window to {200, 120, 860, 548}
    set viewOptions to the icon view options of container window
    set arrangement of viewOptions to not arranged
    set icon size of viewOptions to 112
    set text size of viewOptions to 13
    set background picture of viewOptions to file ".background:background.tiff"
    set position of item "Hoardmap.app" of container window to {165, 196}
    set position of item "Applications" of container window to {495, 196}
    close
    open
    update without registering applications
    delay 2
    close
  end tell
end tell
APPLESCRIPT
# After Finder is done with the window: Finder drops a volume icon added before it.
cp assets/hoardmap.icns "/Volumes/$volume/.VolumeIcon.icns"
xattr -wx com.apple.FinderInfo \
  "0000000000000000040000000000000000000000000000000000000000000000" "/Volumes/$volume"
sync
hdiutil detach -quiet "$device"
hdiutil convert -quiet "$work/rw.dmg" -format UDZO -imagekey zlib-level=9 -o "$dist/Hoardmap.dmg"

(cd "$dist" && shasum -a 256 -- * >SHA256SUMS)
ls -l "$dist"
