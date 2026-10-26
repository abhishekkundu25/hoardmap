#!/usr/bin/env bash
# Builds target/macdirtree.app from the macdirtree-app binary and signs it ad hoc.
#   tools/bundle-app.sh            build the bundle
#   tools/bundle-app.sh --install  also copy it to ~/Applications
# Full Disk Access is tied to the app's signature. An ad hoc signature changes with
# every build, so after a rebuild macOS may need the switch turned off and on again.
set -euo pipefail
root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root"
version=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)

cargo build --release --locked --features app --bin macdirtree-app

app=target/macdirtree.app
rm -rf "$app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
cp target/release/macdirtree-app "$app/Contents/MacOS/macdirtree-app"
cat >"$app/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>macdirtree</string>
  <key>CFBundleDisplayName</key><string>macdirtree</string>
  <key>CFBundleIdentifier</key><string>dev.macdirtree.app</string>
  <key>CFBundleExecutable</key><string>macdirtree-app</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleVersion</key><string>$version</string>
  <key>CFBundleShortVersionString</key><string>$version</string>
  <key>LSMinimumSystemVersion</key><string>11.0</string>
  <key>LSApplicationCategoryType</key><string>public.app-category.utilities</string>
  <key>NSHighResolutionCapable</key><true/>
  <key>NSAppTransportSecurity</key>
  <dict><key>NSAllowsLocalNetworking</key><true/></dict>
</dict>
</plist>
PLIST
codesign --force --sign - --identifier dev.macdirtree.app "$app"
echo "built $root/$app"

if [ "${1:-}" = "--install" ]; then
  mkdir -p "$HOME/Applications"
  rm -rf "$HOME/Applications/macdirtree.app"
  cp -R "$app" "$HOME/Applications/"
  echo "installed $HOME/Applications/macdirtree.app"
fi
