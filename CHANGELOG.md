# Changelog

## Unreleased

- The bar under the top bar now shows what the disk holds, like the Storage bar in System Settings. The scanned folder is split by kind of thing: Applications, Rebuildable, Photos, Music & Movies, Mail & Messages, iCloud Drive, App data, Trash, Documents and Other files. The rest of the disk, which used to be one "Everything else" figure, is split into Other folders, macOS, Swap and other volumes. Point at a part to see its size, its share of the disk and what it is. Swap is memory macOS has moved to disk, so a large Swap part points at an app using too much memory.
- Click a kind in that bar to see it alone: the list, the map and the tabs show only that kind, with every folder's size counting it alone. A band in the kind's colour above the views says "Showing only App data", the kind stays lit in the bar and its name becomes a pill with an ×. Click it again, press Show everything or press Esc to go back.
- The Rebuildable tab is in three sections, since "comes back" does not mean one thing: Made again by its tool (caches, build output, installed packages), Yours to download again (models, toolchains, the Android SDK, extensions) and Check first (folders that also hold things that do not come back, such as Docker's disk image, or that are known by a common name alone). Only the first counts as Rebuildable in the disk bar, so a folder of models you chose no longer does.
- Rebuildable now finds something on every Mac, not only a developer's: each app's cache folder in `Library/Caches`, sandboxed apps' and app groups' caches, each user's system cache folder, and the caches Chromium browsers and Electron apps keep in `Application Support`. For developers it adds Maven and sbt `target`, .NET `obj`, Unity `Library`, SvelteKit, Nuxt, Angular, Elixir, Godot, Zig, Unreal, the npx cache, Cargo's git sources, Kotlin/Native, pre-commit, Whisper and Keras. 90 kinds of folder, up from 68.
- Where the owner documents its own command for a folder, the group shows it as text to copy: `cargo clean`, `brew cleanup --prune=all`, `uv cache clean`, `npm cache clean --force` and 20 more. Hoardmap still runs nothing and clears nothing. Each folder shows the day it last changed, so the ones nothing has touched for months stand out.
- Fixed: the files of installed packages and applications were listed as build output. A `dist` or `build` folder counted beside any `package.json`, and `node_modules` counted inside application bundles. `dist`, `build` and `out` now need a project file that explains them, and application bundles are not looked into.
- Fixed: an empty `CACHEDIR.TAG` marked a cache, and one in any folder called `target` made it Cargo's. A build file in the home folder made all of `~/.gradle`, settings included, a project cache. A scan started on a `.venv` or `target` itself did not recognise it.
- A group of more than 500 folders says so: "Show the largest 500", and the map's highlight says how many of the folders it marks.
- Fixed: scanning the startup disk (Macintosh HD in the Disks menu, or `/`) showed only the system itself, with `/Users`, `/Applications` and `/Library` at 0 bytes and listed as unreadable. Since 0.6.0 the scan skipped them because they are links to the data volume. They are read again.

## 0.6.0 — 2026-11-06

- The Rebuildable tab opens with every group collapsed to one line, so a home-folder scan fits on a screen. Cautions stay visible; open a group with its arrow (or the Right key) to see why it comes back, its documentation and its largest 50 folders, then Show all. Open groups stay open while a scan refreshes the tab.
- Gradle `build` folders carry the caution that `build` is a common name. A `build` folder beside both `CMakeLists.txt` and `package.json` is grouped as CMake output, with CMake's caution. A folder named like a marker file (`CACHEDIR.TAG`, `package.json`) no longer counts as one. The Ollama caution says models made with `ollama create` do not download again.
- Drives with a folder whose name ends in `.previous` or `.inprogress` are no longer hidden from the Disks menu; only Time Machine's own markers hide a drive.
- The scan reads exactly the folder it listed: a folder swapped for a symlink while the scan runs is skipped and listed as unreadable, never followed onto another disk. Folder names that are not UTF-8 are read and opened correctly. Stop takes effect inside a folder with millions of files.
- Keyboard: every child of a large folder, the extension rows, the Largest files rows and the Rebuildable groups can be reached and used without a mouse.
- Fixed: an older answer could redraw the previous scan's treemap, move the selection back, or close folders you had opened. A path ending in a space is scanned as typed, and spaces before a pasted path are dropped. Opening two folders quickly from Finder now shows the last one. Relaunch after granting Full Disk Access no longer closes the app when it cannot start the new copy.
- Hoardmap's local server limits each request's size before reading it, closes connections that send nothing, and caps how many it holds, so no other program on the Mac can make it use unbounded memory or tie it up. The `hoardmap` command accepts folder names that are not UTF-8.

## 0.5.0 — 2026-11-06

- Physical size counts APFS clones once. Files duplicated in Finder or installed by pnpm, uv or bun share their data, and Hoardmap used to count every copy in full, as `du` does, so folders could add up to more than the disk. Clones that still share all their data are counted once and marked in the list; a clone that has partly changed is counted in full, as before. This corrects the 0.1.0 note that Physical already showed what clones cost.
- A Rebuildable tab gathers the folders their tools fill again: `node_modules`, Cargo `target`, Python virtual environments, Xcode DerivedData and the package and model caches of Homebrew, pip, npm, pnpm, Cargo, Go, uv, Gradle and Hugging Face, each with its owner and a link to the owner's documentation. Click a group to see its folders in the treemap.
- The Rebuildable tab knows 68 kinds of folder, up from 14: Yarn, Bun, Maven, Poetry, `__pycache__`, `.next`, Docker's disk image, downloaded models, Android emulators and more. Groups whose folders also hold something that does not come back, such as Docker's containers or an emulator's apps, carry a caution in red, and a last group, "Marked as a cache", gathers any other folder a tool tagged with `CACHEDIR.TAG`.

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
