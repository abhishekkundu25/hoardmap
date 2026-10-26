# Changelog

## Unreleased

- Sizes start in Physical, the space files take on disk, instead of Logical. The status-line total follows the toggle, also while the scan runs; `/api/status` gains `physical`.
- Renamed from macdirtree to Hoardmap. The command is `hoardmap`, the app is `Hoardmap.app` with the bundle identifier `dev.hoardmap.app`, so Full Disk Access has to be granted to it again.
- An app icon and logo, in the style of macOS icons: a treemap of glossy tiles on a squircle, the largest a gold hoard. It is the app icon, the page favicon, and sits beside the name in the toolbar and on the start screen. Source in `assets/hoardmap.svg`.
- Native Mac app (`tools/bundle-app.sh`): the same page in its own window, with a start screen that says hoardmap never changes your files and walks through Full Disk Access, which stays optional. A toolbar button reopens it at any time.
- Icicle view beside the treemap: one row per folder level, widths by bytes, names and sizes on the bars. Switch with Treemap and Icicle above the right pane.

## 0.1.0 — 2026-10-26

- One command, `hoardmap [PATH]`, scans a folder (your home folder by default) and opens the result in your browser.
- Scan progress in the page: folders, files, bytes and elapsed time while the scan runs.
- Directory list sorted largest first with Name, Size, %, Items, Files, Subdirs and Last change, a share bar, lazy expansion and keyboard navigation.
- Treemap of every file, area by size and colour by extension, relaid out on resize, with hover, click to select, double-click to open, and Zoom and Root.
- Extension list as the treemap legend with bytes, share and file count; click an extension to highlight its files.
- Largest files tab for the current treemap root.
- Logical and Physical size toggle, so sparse files and clones show what they really cost on disk.
- Hard links counted once, symlinks not followed, the scan stays on the scanned volume.
- Unreadable folders are counted and listed instead of stopping the scan.
- Open in Finder for any folder or file, by button, double-click or Enter. There is no delete, move or rename anywhere.
- Scan another path from the page, and Rescan the current one.
- Light and dark theme following the system setting; no external scripts or fonts.
- Server on 127.0.0.1 only, with a Host header check and JSON-only POSTs against cross-site requests.
- The server exits on its own 10 seconds after the last tab closes (`--linger`), and uses no CPU while idle.
- Options `--port`, `--no-open`, `--linger` and `--help`.
