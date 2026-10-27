## Install

The easiest way is [Homebrew](https://brew.sh); the app opens straight away and `brew upgrade` keeps it current:

```sh
brew install --cask abhishekkundu25/tap/hoardmap
```

Or download:

- **Hoardmap.dmg**: open it and drag Hoardmap onto Applications. Works on Apple silicon and Intel Macs, macOS 11 or later.
- **Hoardmap.zip**: the same app, zipped.

## First launch of a downloaded copy

macOS asks you to confirm it once:

1. Open Hoardmap. If macOS stops it, press **Done**.
2. Open **System Settings → Privacy & Security**, scroll down, and press **Open Anyway** next to Hoardmap.

Or, in Terminal: `xattr -dr com.apple.quarantine /Applications/Hoardmap.app`

Hoardmap only reads sizes. It never deletes, moves or renames anything; it opens Finder at what you pick.
