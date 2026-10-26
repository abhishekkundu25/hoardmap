# hoardmap

WinDirStat for macOS: scan a folder, see where the bytes are, open the folder in Finder.
Rust binary, page served to the browser on 127.0.0.1, no build step, no network.

## Problem statement

On Windows, WinDirStat shows a drive as a directory list, a treemap and an extension
legend, so the places that eat disk space are obvious in seconds. On a Mac the user has
Finder's Get Info, one folder at a time. They want the same three linked views for a
Mac folder, and a way to jump to any folder or file in Finder from the view. They do
not want a delete button: clearing space is done in Finder, by hand, after looking.

## Solution

`hoardmap [PATH]` scans PATH (the home folder by default) in parallel, opens the
browser on a local page, and streams scan progress to it. When the scan is done the
page shows:

- a **directory list**: an expandable tree sorted by size with columns Name, Size, %,
  Items, Files, Subdirs, Last change, and a bar for the percentage;
- a **treemap**: every file a rectangle, area proportional to size, colour by
  extension, laid out squarified by the server for the current viewport;
- an **extension list**: the colour legend with bytes, percentage and file count per
  extension, and a **largest files** tab beside it.

The three views are linked: selecting in one highlights in the others. Double-clicking
a node, or the Open in Finder button, opens the folder in Finder, with the file
selected when the node is a file. Nothing in the page or the server deletes, moves or
renames anything. The server exits by itself once the browser tab is gone.

## User stories

1. As a Mac user, I want to run one command and get a browser page of my home folder's
   disk usage, so that I do not install a native app.
2. As a user, I want to pass a folder path on the command line, so that I can scan one
   folder instead of my whole home.
3. As a user, I want to type another path in the page and rescan, so that I can move
   around without restarting.
4. As a user, I want a Rescan button, so that after clearing files in Finder I see the
   new numbers.
5. As a user, I want scan progress (folders, files, bytes so far, elapsed time) while
   the scan runs, so that I know it is working on a 500 GB folder.
6. As a user, I want the directory list sorted largest first with the children
   expandable, so that I can drill into the biggest folder in a few clicks.
7. As a user, I want Size, percentage of parent, Items, Files, Subdirs and Last change
   columns, so that I see the same facts WinDirStat shows.
8. As a user, I want a bar in the percentage column, so that I can compare siblings at
   a glance.
9. As a user, I want to switch between logical and physical (allocated) size, so that
   sparse and cloned files do not mislead me.
10. As a user, I want hard links counted once, so that totals match what the disk says.
11. As a user, I want the scan to stay on the root's volume, so that scanning `/` does
    not count `/Volumes` or the Data volume twice through firmlinks.
12. As a user, I want folders I may not read to be counted and listed as errors rather
    than stopping the scan, so that a protected folder does not break the page.
13. As a user, I want the treemap to show every file as a rectangle sized by bytes and
    coloured by extension, so that one large video stands out.
14. As a user, I want the treemap to re-lay out when I resize the window, so that it
    always fills its pane.
15. As a user, I want hovering a rectangle to show its path and size, so that I know
    what I am looking at.
16. As a user, I want clicking a rectangle to select that file in the directory list
    (expanding its ancestors and scrolling to it), so that the views stay linked.
17. As a user, I want selecting a folder in the list to outline its region in the
    treemap, so that I see where it lives.
18. As a user, I want to zoom the treemap to the selected folder and back to the root,
    so that I can read a small folder's contents.
19. As a user, I want the extension list to act as the treemap legend with bytes,
    percentage and file count, so that I know what each colour means.
20. As a user, I want clicking an extension to highlight its rectangles in the treemap
    and dim the rest, so that I can see where all the `.mov` files are.
21. As a user, I want a Largest files tab listing the biggest files under the current
    root with their paths, so that I can find the single biggest offenders.
22. As a user, I want double-clicking a folder, a file or a rectangle to open the
    folder in Finder, with the file selected when it is a file, so that I can act on it
    there.
23. As a user, I want an Open in Finder button for the selection, so that I do not
    have to double-click.
24. As a user, I want no delete, move or rename action anywhere, so that I cannot lose
    data from a page I was only looking at.
25. As a user, I want the page to work offline with no external scripts or fonts, so
    that it is private and loads instantly.
26. As a user, I want the page to follow my light or dark system theme, so that it
    looks native.
27. As a user, I want the server to exit by itself a few seconds after I close the tab,
    so that no process is left running.
28. As a user, I want the server to use no CPU while I just look at the page, so that
    it does not drain my battery.
29. As a user, I want the server to bind to 127.0.0.1 only, so that nobody on my
    network can see my file names or open my folders.
30. As a user, I want the page's actions (scan, open) to be refused when they come from
    another website, so that a malicious page cannot drive my server.
31. As a user, I want a `--port` option, so that I can choose the port when the default
    random one does not suit me.
32. As a user, I want `--no-open`, so that I can start the server from a script
    without the browser launching.
33. As a developer, I want `--linger SECS`, so that tests can make the server exit
    quickly after the tab disconnects.
34. As a developer, I want the resource hygiene tests (idle CPU, exit on tab close)
    written first, so that every later slice is held to them.
35. As a user, I want sizes shown in human units (KB, MB, GB with one decimal) and
    exact bytes on hover, so that the list is readable.
36. As a user, I want the scan of a one-million-file home folder to finish in seconds,
    not minutes, so that the tool is usable on a full disk.
37. As a user, I want the error count visible, with the list of unreadable paths one
    click away, so that I know what the totals miss.
38. As a user, I want the list to show symlinks and hard links as such, so that a 0
    byte entry is not a mystery.
39. As a user, I want the treemap to drop rectangles smaller than a pixel instead of
    sending them, so that the page stays fast on a big tree.
40. As a user, I want keyboard navigation in the directory list (arrows to move, left
    and right to collapse and expand, Enter to open in Finder), so that I can work
    without the mouse.

## Implementation decisions

- One Rust crate with a library and a binary. Dependencies: `tiny_http` (HTTP),
  `serde` and `serde_json` (JSON), `rayon` (parallel scan). No async runtime, no
  frontend framework, no build step: the page is three static files embedded in the
  binary with `include_str!`.
- **Tree**: an arena `Vec<Node>`, root at index 0, `NodeId = u32`. A node holds name,
  parent, kind (dir, file, symlink, other), logical size, physical size, file count,
  dir count, latest mtime, children (largest first), and a hard-link flag. Directory
  figures are subtree totals. The root's name is the scanned path. The extension is
  derived from the name (`tree::extension`), not stored.
- **Scan**: `scan::scan(path, &Progress) -> io::Result<Tree>`. Parallel over
  subdirectories with rayon, `symlink_metadata` (symlinks are not followed), stays on
  the root's device (`st_dev`). Each worker builds a local subtree; the result is
  flattened depth-first into the arena with children sorted by size. Files with
  `nlink > 1` are keyed by `(dev, ino)`: the first occurrence in depth-first order
  keeps its bytes, later ones get `size = 0`, `physical = 0`, `hard_link = true`.
  Physical size is `blocks × 512`. Unreadable directories are counted in
  `error_count`, the first 1000 kept with their error text, and the scan goes on.
  Progress is three atomics (dirs, files, bytes) the scanner increments and a reader
  samples; nothing polls when no scan runs.
- **Scan state** (held by the server behind a mutex): `Idle`, `Scanning { path,
  progress, started }`, `Done { path, tree: Arc<Tree>, duration, colours }`, `Failed {
  path, error }`. One scan at a time; a scan request during a scan is refused with 409.
  Per finished tree the server computes the **colour table** once: extensions ranked by
  bytes over the whole tree; the top 15 get the palette's colours in order, every other
  extension gets grey. Collapsed directory rectangles get a darker grey.
- **Treemap layout**: `treemap::layout(tree, root, w, h, min) -> Layout` with the
  squarified algorithm (Bruls, Huizing, van Wijk): children largest first, rows laid
  along the shorter side, aspect ratio kept near 1. Recursion into a directory stops
  when its area is below `min²` pixels (default 1); that directory becomes one rect of
  kind `dir`. Files below `min²` are dropped. The output has `rects` (files and
  collapsed dirs: id, x, y, w, h, kind, ext, colour) and `dirs` (every laid-out
  directory's bounds: id, x, y, w, h) so the page can outline a selection. Sizes use
  the requested mode, logical or physical.
- **HTTP**: `tiny_http` bound to `127.0.0.1:PORT` (PORT from `--port`, else 0 and the
  OS picks). The first stdout line is `Listening on http://127.0.0.1:PORT/`. Each
  request is handled on its own thread. GET `/`, `/app.js`, `/style.css` serve the
  embedded files with the right content types and `Cache-Control: no-store`.
- **Presence and exit**: GET `/api/events` is a Server-Sent Events stream. Its open
  connections are the connected tabs. When the count falls from one to zero a timer of
  `--linger` seconds (default 10) starts; if no tab connects before it fires, the
  process exits 0. A tab that reconnects (reload) cancels the timer. Before the first
  tab ever connects the server waits indefinitely. The stream sends `event: status`
  with the status JSON on every state change, every 250 ms while a scan runs, and a
  comment line every 15 s as keepalive; between those its thread blocks on a channel
  with a timeout, so an idle server has no busy thread.
- **Browser launch**: at start the server runs `open http://127.0.0.1:PORT/` unless
  `--no-open`. The scan of PATH starts at once.
- **Cross-site protection**: POST endpoints require `Content-Type: application/json`
  (so a cross-origin request needs a preflight, which gets no CORS headers and fails)
  and a request body; anything else is 415 or 400. No CORS headers are ever sent.
- **Open in Finder**: POST `/api/open {"id"}` resolves the node's path from the tree
  (never a path from the request) and runs `open -R <path>` for a file or symlink and
  `open <path>` for a directory, through `PATH` lookup so tests can substitute `open`.
  On a non-macOS build the command is `xdg-open <dir>`.
- **API** (all JSON):
  - `GET /api/status` → `{state, path, dirs, files, bytes, physical, elapsed_ms,
    error_count, error}`; `state` is `idle | scanning | done | failed`. While scanning the counts are
    the progress so far; when done they are the tree's totals.
  - `GET /api/events` → SSE as above.
  - `POST /api/scan {"path"}` → 202 and starts a scan; 409 while one runs; 400 for a
    path that is not a directory.
  - `GET /api/node/{id}` → the node with `path` and `children`, each child with `id,
    name, kind, size, physical, files, dirs, mtime, hard_link, has_children`, largest
    first. 404 for an unknown id, 409 when no tree is loaded.
  - `GET /api/treemap?root=ID&w=PX&h=PX&min=PX&mode=logical|physical` → the Layout.
  - `GET /api/extensions?root=ID&mode=` → `{total, items: [{ext, color, bytes,
    physical, files}]}` largest first; `ext` is `""` for files without one.
  - `GET /api/largest?root=ID&n=N&mode=` → `{items: [{id, path, size, physical,
    mtime}]}` largest first, N capped at 1000.
  - `GET /api/errors` → `{count, items: [{path, error}]}`. Without Full Disk Access
    on macOS, the home folders that would each raise a macOS prompt are counted but
    not read, and listed with the error `Not scanned: needs Full Disk Access`.
  - `POST /api/open {"id"}` → 204.
- **Page**: `index.html`, `style.css`, `app.js`, vanilla JS and canvas. Layout: a top
  bar (path field, Scan, Rescan, Open in Finder, Logical/Physical toggle, status text),
  the directory list on the left, the treemap on the right, the extension list and
  largest files tabs at the bottom. The list fetches children lazily per expanded node.
  The treemap is one canvas drawn from the server's rects, redrawn on resize (debounced)
  and on selection change; hit testing is a linear scan of the rects. Selection is one
  node id shared by all views. Colours come from the server. System theme via
  `prefers-color-scheme`.
- **Paths in JSON** are strings; a non-UTF-8 name is shown lossily and `open` uses the
  tree's `PathBuf`, so the real path is still opened.

## Testing decisions

- A good test drives the binary or the public library functions and checks what the
  user would see: JSON from the endpoints, rects that tile the viewport, the process
  gone after the tab closes. No test reads private state.
- **Resource hygiene** (helpers in `tests/common/mod.rs`): CPU seconds of a process
  come from `ps -o time= -p PID` on macOS and `/proc/PID/stat` on Linux. The tests
  check that a server idle after a scan uses under 0.1 s of CPU in 2 s with a tab
  connected; that the process exits 0 within linger plus 3 s after its only tab
  disconnects; that it is still alive after 3 s when `--linger` has not passed; and
  that a reload (disconnect then reconnect within linger) keeps it alive.
- **Scanner** on a fixture tree built in a temp folder with known sizes, a hard-linked
  pair, a symlink, a dotfile, nested empty dirs and an unreadable dir (mode 000,
  skipped when running as root): sizes, counts, order, hard-link accounting, error
  count and mtime roll-up.
- **Treemap**: rects never overlap, stay inside the viewport, their total area matches
  the root's share, children of a directory lie inside its bounds, sub-pixel nodes are
  dropped or collapsed, a zero-size tree yields no rects.
- **API end to end** in `tests/api.rs`: start the binary on the fixture with `--no-open
  --linger 1`, wait for `done`, check each endpoint's shape and figures, check 409/400/
  404/415 paths, and check that `/api/open` ran a stub `open` from a temp `PATH` dir
  with `-R file` for a file and the bare dir for a directory.
- No browser automation. The page's logic that is testable without a DOM (size
  formatting, tree-state bookkeeping) stays in small pure functions.
- Every test file runs under `timeout 120`; integration tests use the debug binary via
  `CARGO_BIN_EXE_hoardmap`.
- Budgets as pass/fail lines in the resource tests: idle CPU < 0.1 s per 2 s; exit
  within linger + 3 s.

## Out of scope

- Deleting, moving, renaming, trashing or emptying anything. Not now, not later.
- Duplicate detection, file recovery, permissions inspection, cloud or storage-tier
  costs, MTP devices, network shares.
- Saving or loading scans, CSV or JSON export.
- Watching the file system for changes; the user presses Rescan.
- Sunburst and flame graph views.
- Search and filtering by name, size or age.
- Following symlinks, crossing volumes, Spotlight metadata, resource forks.
- Native app packaging, menu bar icon, Homebrew formula.
- Windows and Linux as targets (the code compiles there, but only `open` is tested).

## Further notes

- WinDirStat features researched for this spec: directory list, treemap, extension
  list with colours as legend, logical and physical sizes, hard-link reporting,
  largest files list, selection highlighting across views, open in Explorer.
- The treemap colours are a fixed 15-colour palette picked for distinctness on light
  and dark backgrounds; the mapping is per scan, so colours change between scans. That
  matches WinDirStat.
- The borrowed resource checks are the project's first commit after the skeleton, as
  asked, so every feature slice is measured against them.
