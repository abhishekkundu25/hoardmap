#!/usr/bin/env bash
# Builds target/Hoardmap.app from the hoardmap-app binary, with the hoardmap command
# beside it in Contents/MacOS (Homebrew links it onto PATH), and signs it ad hoc.
#   tools/bundle-app.sh              build the bundle for this Mac
#   tools/bundle-app.sh --universal  one binary for Apple silicon and Intel (releases)
#   tools/bundle-app.sh --install    also copy it to ~/Applications
# Full Disk Access is tied to the app's signature. An ad hoc signature changes with
# every build, so after a rebuild macOS may need the switch turned off and on again.
set -euo pipefail
root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root"
version=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)

install=false
universal=false
for arg in "$@"; do
  case "$arg" in
    --install) install=true ;;
    --universal) universal=true ;;
    *) echo "unknown option: $arg" >&2; exit 2 ;;
  esac
done

app=target/Hoardmap.app
rm -rf "$app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
if $universal; then
  for target in aarch64-apple-darwin x86_64-apple-darwin; do
    cargo build --release --locked --features app --bin hoardmap-app --bin hoardmap --target "$target"
  done
  for bin in hoardmap-app hoardmap; do
    lipo -create -output "$app/Contents/MacOS/$bin" \
      "target/aarch64-apple-darwin/release/$bin" \
      "target/x86_64-apple-darwin/release/$bin"
  done
else
  cargo build --release --locked --features app --bin hoardmap-app --bin hoardmap
  cp target/release/hoardmap-app target/release/hoardmap "$app/Contents/MacOS/"
fi
cp assets/hoardmap.icns "$app/Contents/Resources/hoardmap.icns"
cat >"$app/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>Hoardmap</string>
  <key>CFBundleDisplayName</key><string>Hoardmap</string>
  <key>CFBundleIconFile</key><string>hoardmap</string>
  <key>CFBundleIdentifier</key><string>dev.hoardmap.app</string>
  <key>CFBundleExecutable</key><string>hoardmap-app</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleVersion</key><string>$version</string>
  <key>CFBundleShortVersionString</key><string>$version</string>
  <key>LSMinimumSystemVersion</key><string>11.0</string>
  <key>LSApplicationCategoryType</key><string>public.app-category.utilities</string>
  <key>NSHighResolutionCapable</key><true/>
  <key>CFBundleDocumentTypes</key>
  <array>
    <dict>
      <!-- Folders, from Open With, the Dock icon or the hoardmap command. Alternate:
           never the default, so Finder keeps opening folders itself. -->
      <key>CFBundleTypeName</key><string>Folder</string>
      <key>CFBundleTypeRole</key><string>Viewer</string>
      <key>LSHandlerRank</key><string>Alternate</string>
      <key>LSItemContentTypes</key><array><string>public.folder</string></array>
    </dict>
  </array>
  <key>NSAppTransportSecurity</key>
  <dict><key>NSAllowsLocalNetworking</key><true/></dict>
</dict>
</plist>
PLIST
# The command first: a bundle's signature covers its other executables only once
# they are signed themselves.
codesign --force --sign - --identifier dev.hoardmap.cli "$app/Contents/MacOS/hoardmap"
codesign --force --sign - --identifier dev.hoardmap.app "$app"
echo "built $root/$app"

if $install; then
  mkdir -p "$HOME/Applications"
  rm -rf "$HOME/Applications/Hoardmap.app"
  cp -R "$app" "$HOME/Applications/"
  echo "installed $HOME/Applications/Hoardmap.app"
fi
