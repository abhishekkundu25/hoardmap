## Download

- **Hoardmap-…-macos.dmg**: open it and drag Hoardmap to Applications. Works on Apple silicon and Intel Macs, macOS 11 or later.
- **hoardmap-…-macos.tar.gz**: the `hoardmap` terminal command, which opens the same page in your browser.

## First launch

This build is not notarized by Apple, so macOS blocks it the first time:

1. Open Hoardmap. macOS says it cannot verify it. Press **Done**.
2. Open **System Settings → Privacy & Security**, scroll down, and press **Open Anyway** next to Hoardmap.

Or, in Terminal: `xattr -dr com.apple.quarantine /Applications/Hoardmap.app`

Hoardmap only reads sizes. It never deletes, moves or renames anything; it opens Finder at what you pick.
