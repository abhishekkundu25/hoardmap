#!/usr/bin/env bash
# Renders assets/hoardmap.svg at every macOS icon size with headless Chrome and packs
# them into assets/hoardmap.icns. Run it after changing the SVG.
set -euo pipefail
root=$(cd "$(dirname "$0")/.." && pwd)
chrome="/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
mkdir "$work/hoardmap.iconset"

render() { # size output
  printf '<html><body style="margin:0;background:transparent"><img src="file://%s/assets/hoardmap.svg" width="%s" height="%s" style="display:block"></body></html>' \
    "$root" "$1" "$1" >"$work/page.html"
  "$chrome" --headless=new --disable-gpu --hide-scrollbars --default-background-color=00000000 \
    --force-device-scale-factor=1 --window-size="$1,$1" --screenshot="$2" "file://$work/page.html" >/dev/null 2>&1
}
for size in 16 32 128 256 512; do
  render "$size" "$work/hoardmap.iconset/icon_${size}x${size}.png"
  render $((size * 2)) "$work/hoardmap.iconset/icon_${size}x${size}@2x.png"
done
iconutil -c icns -o "$root/assets/hoardmap.icns" "$work/hoardmap.iconset"
echo "wrote $root/assets/hoardmap.icns"
