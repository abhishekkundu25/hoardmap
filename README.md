# Hoardmap

[![CI](https://github.com/abhishekkundu25/hoardmap/actions/workflows/ci.yml/badge.svg)](https://github.com/abhishekkundu25/hoardmap/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/abhishekkundu25/hoardmap)](https://github.com/abhishekkundu25/hoardmap/releases/latest)

<img src="assets/hoardmap.svg" alt="Hoardmap icon" width="128">

```sh
brew install --cask abhishekkundu25/tap/hoardmap
```

Or **[download Hoardmap.dmg](https://github.com/abhishekkundu25/hoardmap/releases/latest/download/Hoardmap.dmg)** · [Website](https://abhishekkundu25.github.io/hoardmap/)

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="site/img/treemap-dark.jpg">
  <img src="site/img/treemap-light.jpg" alt="Hoardmap showing a home folder as a treemap, with the folder list on the left and file types below">
</picture>

Hoardmap is WinDirStat for the Mac. It scans a folder and shows where the bytes are
in three linked views: a directory list, a treemap or icicle, and an extension list.
It is a Mac app; the `hoardmap` command opens it on a folder from Terminal. From the
app you can open any folder or file in Finder. That is the only thing it can do
to your files; it never deletes, moves or renames anything.

## Install

With [Homebrew](https://brew.sh), the easiest way:

```sh
brew install --cask abhishekkundu25/tap/hoardmap
```

The app goes into Applications, ready to open, with the `hoardmap` command on your
PATH, and `brew upgrade` keeps both current. The recipe lives in
[abhishekkundu25/homebrew-tap](https://github.com/abhishekkundu25/homebrew-tap); each
release writes it from `packaging/homebrew/`.

Or download from the [Releases](../../releases) page has, for Apple silicon and Intel Macs on macOS 11
or later:

- `Hoardmap.dmg`: open it and drag Hoardmap onto Applications.
- `Hoardmap.zip`: the same app, zipped.

The `hoardmap` command is inside the app, at
`Hoardmap.app/Contents/MacOS/hoardmap`; link it onto your PATH if you want it.

The newest DMG is always at
`https://github.com/abhishekkundu25/hoardmap/releases/latest/download/Hoardmap.dmg`,
which is what the website links to.

The app is not notarized by Apple, so macOS blocks the first launch. Press Done, then
open System Settings, Privacy & Security, and press Open Anyway next to Hoardmap. Or
run `xattr -dr com.apple.quarantine /Applications/Hoardmap.app` once.

## Build from source

You need macOS and Rust 1.85 or newer. Clone the repository, then build the app and
copy it to `~/Applications`:

```sh
tools/bundle-app.sh --install
```

Without `--install` the bundle is left at `target/Hoardmap.app`.

## Usage

Open Hoardmap from Applications, Launchpad or Spotlight. It scans your home folder and
shows progress while it runs. To scan another folder, pick it from the Disks menu, type
its path and press Scan, drop it on the Dock icon, or choose Open With > Hoardmap on it
in Finder. Closing the window quits the app.

From Terminal:

```sh
hoardmap ~/Developer      # open the app on a folder
hoardmap                  # just open the app
```

```
Usage: hoardmap [PATH] [--help]

Opens the Hoardmap app, scanning PATH when one is given.
```

The command runs `open -b dev.hoardmap.app PATH` and exits; the app does the rest.

## The Mac app

The app is a native window showing the page from a small server inside it, on
127.0.0.1 (see Privacy and safety).

The first time it starts, the app shows a start screen before it scans anything. The
screen says what hoardmap does and does not do, and covers Full Disk Access:

- Whether the app has access, read from a file only it can open.
- Open Settings, which goes straight to Privacy & Security, Full Disk Access.
- Show hoardmap in Finder, so you can drag the app into that list.
- Relaunch hoardmap, because macOS applies the switch to a new process.
- Continue without access, which scans everything macOS allows. Without access, macOS
  may ask once about Desktop, Documents, Downloads and other protected folders as the
  scan reaches them; the scan waits at that folder until you answer.

Access is optional. The screen comes back at each launch until access is on, unless you
tick "Don't show this at launch". The Full Disk Access button in the toolbar opens it
at any time, with a green dot when access is on and an orange one when it is off.

macOS ties Full Disk Access to the app's signature. The bundle script signs ad hoc,
which gives each build a new signature, so after a rebuild you may need to turn the
switch off and on again.

## The views

The top bar holds the Disks menu (each disk with how full it is, your home folder, and
Choose a folder… for the macOS folder picker), the path field with Scan and Rescan, an Open in Finder button, the
Logical and Physical size toggle, the Zoom and Root buttons for the treemap, and a
status line with the scanned path, folder and file counts, total bytes and scan time.
Sizes start in Physical, what the files take on disk; the status total follows the toggle.
Press Rescan after clearing files in Finder to see the new figures. While a scan runs,
Scan turns into Stop, which ends it and keeps what it has read so far; the status says
the figures are partial. If the scanned drive is disconnected, the scan ends with an
error saying so.

Under the top bar, a bar shows the whole disk: the scanned folder, everything else, and
the free space, with the sizes beside it.

While a scan runs, the list and the map fill in as folders finish, about once a
second, the way WinDirStat grows its tree. Expanded folders, the selection and the zoom
stay put across those updates. The extension and largest-file lists wait for the scan
to finish.

### Directory list

The left pane is an expandable tree, sorted largest first. Click a column heading to
sort by it instead, and click again to reverse; the page remembers the choice. The
columns are Name, Size, %, Items, Files, Subdirs and Last change. A folder lists its
200 largest items, then a row for the rest with their count and total size; click it
to show 1,000 more. The % column shows the share of the parent
with a bar behind the number. Sizes are in human units; hover a size for the exact
byte count. Symlinks and hard links are marked as such, so a 0 byte entry is not a
mystery. Click a row to select it, click the triangle to expand or collapse it, and
double-click a row to open it in Finder.

Keyboard, when the list has focus:

- Up and Down move the selection.
- Right expands a folder, or moves into it when it is already expanded.
- Left collapses a folder, or moves to the parent when it is already collapsed.
- Enter opens the selection in Finder.
- Home and End jump to the first and last row.

### Treemap

The right pane draws every file as a rectangle whose area is proportional to its size.
Colour is by extension: the fifteen extensions with the most bytes get a colour each,
every other extension is grey, and folders too small to draw their contents are a
darker grey. The layout is recomputed by the server whenever the pane is resized.

- Hover a rectangle to see its path and size.
- Click a rectangle to select that file in the directory list. The list expands the
  file's ancestors and scrolls to it.
- Double-click a rectangle to open it in Finder.
- Selecting a folder in the list outlines its region in the treemap.
- Zoom redraws the treemap with the selected folder as its root. Root goes back to the
  scanned folder. The caption above the treemap names the current root.

### Icicle

The Treemap and Icicle switch above the right pane changes the picture. The icicle
draws one row per folder level: the current root across the top, and each folder or
file under its parent with a width proportional to its bytes, largest on the left.
Bars wide enough for text show the name and size. It shows where a large file sits in
the hierarchy, which the treemap hides. Hover, click, double-click, Zoom, Root and the
extension highlight work the same as in the treemap. Rows stay at least 18 pixels tall,
so a very deep path is cut off at the bottom; zoom into a folder to see further down.
The page remembers the view you picked.

### Extension list

The Extensions tab at the bottom is the treemap legend. For each extension under the
current treemap root it shows the colour, the bytes, the share and the file count,
largest first. Files without an extension are one row. Click an extension to highlight
its rectangles in the treemap and dim all others. Click it again, or press Escape, to
clear the highlight.

### Largest files

The Largest files tab lists the biggest files under the current treemap root with their
full paths. Click a row to select the file in the directory list; double-click it to
open it in Finder.

The Rebuildable tab gathers the folders their tools fill again on their own. Project
folders: `node_modules`, Cargo `target`, Python virtual environments and
`__pycache__`, Swift `.build`, Gradle `.gradle` and `build`, Next.js `.next`,
Turborepo, Parcel, Dart `.dart_tool`, and the mypy, pytest, Ruff and nox caches.
Caches and downloads in the home folder: Xcode DerivedData, Gradle, Maven, Homebrew,
pip, Poetry, pipenv, uv, npm, pnpm, Yarn, Bun, Deno, Cargo, Go, CocoaPods, Carthage,
SwiftPM, Composer, NuGet, Playwright, Puppeteer, Electron, sccache, ccache and Bazel.
Each group names its owner, says in a line how the folder comes back, and links the
owner's documentation. A project folder is recognised by a file beside or inside it
(`package.json`, `CACHEDIR.TAG`, `pyvenv.cfg`, `Package.swift`), so a folder that
merely shares the name is left out; `dist`, `build` and `out` count only beside a
project file that explains them, such as `vite.config.js`, `pyproject.toml`,
`CMakeLists.txt` or `next.config.js`.

Some groups carry a caution, shown in red under the group's line, naming what in the
folder does not come back on its own: Docker Desktop's disk image (your containers and
volumes), Android emulators and the Android SDK, downloaded models (Hugging Face,
Ollama, LM Studio, PyTorch hub, Kaggle), Conda's package cache, JetBrains caches (Local
History), VS Code extensions, Rust toolchains, nvm and fnm Node.js versions, the Dart
pub cache, CocoaPods `Pods`, Composer `vendor`, and the generic `dist`, `build` and
`out` folders. The last group, "Marked as a cache", gathers any other folder that holds
a `CACHEDIR.TAG` file, the convention tools use to mark a cache; its caution says the
tag is only a hint. Left out on purpose: Xcode Archives (Apple asks you to keep the
archive of every build you ship), a project's `.yarn/cache` (committed under zero
installs), iOS DeviceSupport, simulators and VS Code's cached data (no documentation
from Apple or Microsoft), and whole tool folders such as `~/.m2` and `~/.gradle`,
which hold settings and credentials beside the cache.

Click a group to highlight its folders in the treemap, and a folder to select it;
double-click opens it in Finder. Hoardmap does not clear anything; that stays with you
and the owning tool.

### Logical and Physical

Logical is the number of bytes a file contains. Physical is the number of bytes
allocated for it on disk. The two differ for sparse files and for APFS clones. A Docker
disk image such as `Docker.raw` can report 64 GB logical while only a few GB are
allocated. Physical is the default because it is what the files take on disk;
switch to Logical to see file contents. Every view, and the total in the status line,
follows the toggle.

APFS clones share their data. Duplicating a file in Finder, and tools such as pnpm, uv
and bun, make clones that take no extra space until one copy changes. Physical counts
the data of clones that still share all of it once; most other disk tools, and `du`,
count every clone in full. A clone that has partly changed is counted in full.
Clones are marked in the list, and the marker's tooltip says that removing one
frees nothing now. The status line says how much shared data was counted once. Data
kept by Time Machine snapshots is not freed by removing a file either. The portable
scanner and disks other than APFS do not show clones.

Hard links are counted once. A file reached again through a second link shows 0 bytes
and is marked as a hard link, so totals match what the disk says. The scan stays on the
volume of the folder you scanned, so scanning `/` does not count `/Volumes` or count
the Data volume twice through firmlinks. Symlinks are not followed.

### Unreadable folders

Folders the scan may not read do not stop it. They are counted, and the status line
shows a button such as `3 unreadable`. Click it for the list of paths and errors. The
totals do not include what is behind those folders.

### Full Disk Access

Without Full Disk Access, unreadable entries on a home folder read `Operation not
permitted` or `Not scanned`. That is macOS privacy protection (TCC), not file
permissions. macOS blocks Safari and Mail data and the Trash outright, which gives
`Operation not permitted`. For Desktop, Documents, Downloads, iCloud Drive and other
cloud folders, other apps' data (`Library/Containers`, `Library/Group Containers`), and
the Music, TV and Photos libraries and their caches, macOS would instead stop and ask about each one. So
hoardmap does not read them and lists them as `Not scanned`. hoardmap inherits its
access from the app that launched it, and nothing in the code can bypass that.

To scan those folders, give the terminal that launches hoardmap Full Disk Access:

1. Open System Settings > Privacy & Security > Full Disk Access. This command opens
   that pane directly:

   ```sh
   open "x-apple.systempreferences:com.apple.preference.security?Privacy_AllFiles"
   ```

2. Add Terminal, or the terminal you use (iTerm, Warp, the VS Code terminal), and turn
   it on.
3. Quit and reopen the terminal.
4. Run hoardmap again, or press Rescan.

The unreadable button in the page lists exactly which folders were skipped, so you can
check the result.

## How it opens things

Double-clicking, pressing Enter, or pressing Open in Finder sends the selected node to
the server. For a file or symlink the server runs `open -R <path>`, which shows the
file selected in a Finder window. For a folder it runs `open <path>`, which opens that
folder in Finder. The path comes from the server's own scanned tree, never from the
request. That is the only action the server can take.

## What it never does

hoardmap does not delete, move, rename or trash files, and it has no button, key or
API endpoint that could. The test `source_never_deletes_moves_or_renames` in
`tests/api.rs` reads every source file and fails if a call such as `remove_file`,
`remove_dir`, `rename` or `trash` appears. Clearing space is done in Finder, by hand,
after looking.

## How it scans

On macOS each folder is listed with `getattrlistbulk`, which returns names, types,
sizes and allocated blocks for a batch of entries in one system call. Elsewhere, and
on a Mac with `HOARDMAP_SCANNER=portable`, each folder is read with `read_dir` and
each entry with `lstat`. Both build the same tree, except that only `getattrlistbulk`
sees APFS clones. Subfolders are read in parallel on
the fast cores: on Apple silicon the Performance (and Super) cores, not the Efficiency
ones, since more threads only queue for the file system. On an M6 that is 6 threads,
and a home folder of a million items scans in 1.1 s instead of 1.3 s on all 12, with
half the CPU time. `HOARDMAP_THREADS=n` sets the count.

To time both on a folder of your own and check they agree:

```sh
cargo run --release --example scan_bench -- ~ 5
```

It prints the thread count it used. On a home folder of about 900,000 files with a
warm cache, the bulk listing took a median of 2.0 s against 2.6 s; on a 180,000 file folder, 0.13 s against 0.21 s. On a
folder that apps are writing to, a few entries can differ between the two scans
because they changed in between.

## Privacy and safety

- The server binds to 127.0.0.1 only. Nothing on your network can reach it.
- Other programs on your Mac cannot use it either, and neither can a browser. Each
  launch makes a random token that only the address the app's window loads carries.
  The first load trades it for a cookie (`HttpOnly`, `SameSite=Strict`), and every
  request without that cookie is refused with 403. So an app that was not given Full
  Disk Access cannot ask Hoardmap to list protected folders for it.
- There is no browser version. The `hoardmap` command only opens the app; release
  builds contain no way to serve the page anywhere else.
- It makes no network requests of its own.
- The page loads no external scripts, fonts or styles. Everything is embedded in the
  binary and works offline.
- Every request must carry a `Host` header naming `127.0.0.1` or `localhost` on the
  bound port. Anything else is refused with 403, which defeats DNS rebinding.
- POST requests must have `Content-Type: application/json` and a JSON body. A page on
  another website cannot send such a request without a CORS preflight, and the server
  never answers a preflight with CORS headers. So another site cannot start a scan or
  open a folder.
- The path for `open` is looked up in the scanned tree by node id. A request can never
  name a path of its own.

## Idle cost

While you look at the page the server does nothing. Its threads block until a request
or a scan event arrives. A test in `tests/resources.rs` measures this: an idle server
with its page connected must use under 0.1 s of CPU in 2 s.

## Development

GitHub Actions runs the format check, both lint passes and the tests on every push to
`main` and every pull request (`.github/workflows/ci.yml`).

To release, set `version` in `Cargo.toml`, move the Unreleased notes in `CHANGELOG.md`
under that version, commit, then push a matching tag:

```sh
git tag v0.2.0 && git push origin v0.2.0
```

`.github/workflows/release.yml` checks the tag against `Cargo.toml`, runs the tests,
builds the downloads with `tools/release.sh`, publishes them as a GitHub release with
`.github/release-notes.md`, and then publishes the website. `tools/release.sh` also runs locally and leaves the
files in `target/dist`; it needs the `x86_64-apple-darwin` Rust target.
The DMG opens on a window with a background that says to drag the app onto the
Applications link beside it; the script lays it out by driving Finder, which macOS may
ask you to allow once. The background is `tools/dmg-background.html`, rendered with the
app icon by `tools/make-images.sh` (needs Google Chrome).

The website is `site/index.html`, plain HTML with its screenshots in `site/img`.
The release workflow publishes it to GitHub Pages after the downloads, through
`.github/workflows/pages.yml`, which adds the logo and the version. The site and the
downloads therefore change only with a release; edits to `site/` on `main` wait for the
next tag. To redeploy the current release by hand: `gh workflow run pages.yml --ref vX.Y.Z`.

Build and check, each from the repository root:

```sh
cargo build
cargo fmt --check
cargo clippy --all-targets -- -D warnings
timeout 120 cargo test                        # unit and integration tests
timeout 120 cargo test --test resources       # idle CPU checks, one file
```

The page files under `web/` are embedded with `include_str!`, so a change to the page
needs a rebuild. There is no frontend build step and no JavaScript dependency.

Integration tests start the debug binary's test server
(`HOARDMAP_TEST_SERVER=1 hoardmap PATH`, which serves the page the way the app's window
sees it and prints its address; release builds leave it out) and put a stub `open` on
`PATH`, so no test opens Finder or the app.

- `tests/cli.rs`: the command hands a folder to the app with `open -b`, makes it
  absolute, refuses a file, a missing folder and the old browser options, and says how
  to install the app when `open` fails.
- `tests/resources.rs`: static files and their content types, the idle CPU budget, and
  that the server neither exits nor opens anything on its own.
- `tests/scan.rs`: the scanner on a fixture tree with known sizes, a hard-linked pair, a
  symlink, nested empty folders and an unreadable folder: totals, order, hard-link
  accounting, physical size, mtime roll-up, error count and progress.
- `tests/treemap.rs`: rectangles never overlap, stay inside the viewport and inside
  their parent, sub-pixel nodes are dropped or collapsed, colours rank the top fifteen
  extensions, extension and largest-file statistics, and a 200,000 file layout is fast.
- `tests/api.rs`: every endpoint end to end against the fixture, the 400, 404, 409, 413
  and 415 paths, the icicle's rows and nesting, the `Host` check, that `/api/open` runs `open -R` for a file and bare
  `open` for a folder, and the no-delete source check.

The native app is `src/bin/hoardmap-app.rs`, built with `--features app` (tao, wry
and muda). The plain `cargo build` leaves those out.

The JSON endpoints are `GET /api/status`, `GET /api/events` (server-sent events),
`GET /api/node/{id}`, `GET /api/treemap`, `GET /api/icicle`, `GET /api/extensions`, `GET /api/largest`,
`GET /api/errors`, `GET /api/access`, `POST /api/scan`, `POST /api/open` and
`POST /api/access/settings`; the app adds `POST /api/access/reveal` and
`POST /api/relaunch`. Their shapes are in the spec.

The spec is `docs/specs/hoardmap.md`. The build tickets, with the departures from the
plan, are in `docs/tickets/hoardmap/`. `AGENTS.md` is the short map of the code.

## Credits

- The treemap layout is the squarified treemap of Bruls, Huizing and van Wijk.
- The three linked views, the extension legend and the logical and physical sizes
  follow WinDirStat.
