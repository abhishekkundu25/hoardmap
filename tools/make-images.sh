#!/usr/bin/env bash
# Renders the release images with headless Chrome. Run it after changing their sources.
#   assets/hoardmap.svg         -> assets/hoardmap.icns (every macOS icon size)
#   tools/dmg-background.html   -> assets/dmg-background.tiff (1x and 2x)
set -euo pipefail
root=$(cd "$(dirname "$0")/.." && pwd)
chrome="/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
mkdir "$work/hoardmap.iconset"

shoot() { # url width height scale output
  "$chrome" --headless=new --disable-gpu --hide-scrollbars --default-background-color=00000000 \
    --force-device-scale-factor="$4" --window-size="$2,$3" --screenshot="$5" "$1" >/dev/null 2>&1
}

icon() { # size output
  printf '<html><body style="margin:0;background:transparent"><img src="file://%s/assets/hoardmap.svg" width="%s" height="%s" style="display:block"></body></html>' \
    "$root" "$1" "$1" >"$work/icon.html"
  shoot "file://$work/icon.html" "$1" "$1" 1 "$2"
}
for size in 16 32 128 256 512; do
  icon "$size" "$work/hoardmap.iconset/icon_${size}x${size}.png"
  icon $((size * 2)) "$work/hoardmap.iconset/icon_${size}x${size}@2x.png"
done
iconutil -c icns -o "$root/assets/hoardmap.icns" "$work/hoardmap.iconset"
echo "wrote $root/assets/hoardmap.icns"

shoot "file://$root/tools/dmg-background.html" 660 400 1 "$work/bg.png"
shoot "file://$root/tools/dmg-background.html" 660 400 2 "$work/bg@2x.png"
tiffutil -cathidpicheck "$work/bg.png" "$work/bg@2x.png" -out "$root/assets/dmg-background.tiff" 2>/dev/null
echo "wrote $root/assets/dmg-background.tiff"
