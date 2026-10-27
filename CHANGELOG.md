# Changelog

## Unreleased

## 0.4.0 — 2026-10-28

- Hoardmap is a Mac app only; there is no browser version any more. The `hoardmap` command now opens the app on a folder (`hoardmap ~/Developer`) instead of serving the page to a browser, and its `--port`, `--no-open` and `--linger` options are gone. Homebrew installs the command with the app (`brew install --cask abhishekkundu25/tap/hoardmap`); the separate `hoardmap` formula and the `hoardmap-macos.tar.gz` download are retired.
- Open a folder in Hoardmap from Finder (Open With → Hoardmap) or by dropping it on the Dock icon. Hoardmap never becomes the default app for folders.
- Only Hoardmap's own page can use its local server. Each launch makes a secret that the page receives when it opens; requests without it, from other programs on the Mac, are refused. Before, an app without Full Disk Access could ask a running Hoardmap to list protected folders.

## 0.3.3 — 2026-10-28

- Scans are about 13% faster and use half the CPU: folders are read on the fast cores only, since more threads just queue for the file system. `HOARDMAP_THREADS` sets the count.

## 0.3.2 — 2026-10-27

- Install with Homebrew: `brew install --cask abhishekkundu25/tap/hoardmap` for the Mac app, `brew install abhishekkundu25/tap/hoardmap` for the command. The app opens straight away, and `brew upgrade` keeps it current.

## 0.3.1 — 2026-10-27

- No more Apple Music, TV or Photos prompts without Full Disk Access: their libraries, caches and system copies (`~/Movies/TV`, `~/Library/Photos` and others) are skipped like the other protected folders.
- Relaunching after turning on Full Disk Access is more reliable: a helper waits for the old window to close, then opens the app again with its settings.
- The start screen says when Hoardmap runs from Downloads or its installer disk, where Full Disk Access does not stick, and asks to move it to Applications.
- The Mac app notes each launch step in `~/Library/Logs/Hoardmap/launch.log`, so a launch that never shows a window can be traced.

## 0.3.0 — 2026-10-27

- Live updates: the list and the map fill in while the scan runs, refreshed about once a second, and keep the open folders and the selection.
- The list sorts by any column: click Name, Size, Items, Files, Subdirs or Last change. Size, largest first, is the default.
- A bar under the top bar shows the whole disk: the scanned folder, everything else, and the free space.
- A Disks menu next to the path field: the startup disk and connected drives with how full they are, your home folder, and Choose a folder… for the macOS folder picker.
- Scan turns into Stop while a scan runs, keeping what it has read so far.
- A drive disconnected mid-scan ends the scan with a clear error instead of waiting.
- Folders with many thousands of items open at once: the list shows the 200 largest and a row for the rest.
- Fixed a rare case where the page could keep showing "Scanning" after a scan had finished.

## 0.2.1 — 2026-10-26

- No more run of macOS prompts on the first scan. Without Full Disk Access, Hoardmap leaves out the folders macOS would ask about one by one (Desktop, Documents, Downloads, iCloud Drive and other cloud folders, other apps' data, Music and Photos) and lists them as "Not scanned". Turning on Full Disk Access includes them all.
- Scans no longer stall on iCloud Drive, Google Drive, OneDrive or Dropbox folders kept online only. Hoardmap never makes macOS download anything: a folder whose contents are only in the cloud is counted but not opened, since it takes no space on the Mac.

## 0.2.0 — 2026-10-26

- Faster scans on macOS: folders are listed with `getattrlistbulk`, which returns a batch of entries with their sizes in one call instead of one `lstat` per file. About 20 to 40 percent faster on a warm cache. Other systems keep the portable scanner, and `HOARDMAP_SCANNER=portable` selects it on a Mac.
- The README shows the app, with CI and release badges and a download link.

## 0.1.0 — 2026-10-26

The first release.

- **Hoardmap.app**, a native Mac app: the disk map in its own window, with the usual menus. A start screen says Hoardmap never changes your files and walks through Full Disk Access, which stays optional; a toolbar button reopens it at any time.
- **Downloads**: `Hoardmap.dmg` opens on a window that says to drag the app onto Applications. A zipped app and the `hoardmap` command come alongside. All are universal (Apple silicon and Intel), macOS 11 or later.
- **Website** at https://abhishekkundu25.github.io/hoardmap/, with the download as its main button. It is published with each release.
- An app icon and logo in the style of macOS icons: a treemap of glossy tiles on a squircle, the largest a gold hoard.
- Icicle view beside the treemap: one row per folder level, widths by bytes, names and sizes on the bars.
- Sizes start in Physical, the space files take on disk; the status-line total follows the size toggle, also while the scan runs.
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
