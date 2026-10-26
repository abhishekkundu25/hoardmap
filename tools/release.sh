#!/usr/bin/env bash
# Builds the release downloads into target/dist:
#   Hoardmap-VERSION-macos.dmg        the app, drag to Applications
#   Hoardmap-VERSION-macos.zip        the same app, zipped
#   hoardmap-VERSION-macos.tar.gz     the hoardmap command
#   SHA256SUMS
# Everything is universal (Apple silicon and Intel) and signed ad hoc.
set -euo pipefail
root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root"
version=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
dist=target/dist
rm -rf "$dist"
mkdir -p "$dist"

tools/bundle-app.sh --universal

for target in aarch64-apple-darwin x86_64-apple-darwin; do
  cargo build --release --locked --bin hoardmap --target "$target"
done
cli=$(mktemp -d)
trap 'rm -rf "$cli"' EXIT
lipo -create -output "$cli/hoardmap" \
  target/aarch64-apple-darwin/release/hoardmap \
  target/x86_64-apple-darwin/release/hoardmap
codesign --force --sign - "$cli/hoardmap"
cp README.md "$cli/"
tar -czf "$dist/hoardmap-$version-macos.tar.gz" -C "$cli" hoardmap README.md

ditto -c -k --keepParent target/Hoardmap.app "$dist/Hoardmap-$version-macos.zip"

stage=$(mktemp -d)
cp -R target/Hoardmap.app "$stage/"
ln -s /Applications "$stage/Applications"
hdiutil create -quiet -volname "Hoardmap $version" -srcfolder "$stage" -format UDZO \
  "$dist/Hoardmap-$version-macos.dmg"
rm -rf "$stage"

(cd "$dist" && shasum -a 256 -- * >SHA256SUMS)
ls -l "$dist"
