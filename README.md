# macdirtree

macdirtree is WinDirStat for the Mac. It scans a folder and shows where the bytes are
in three linked views: a directory list, a treemap and an extension list. The page is
served to your browser from a small Rust binary that listens on 127.0.0.1 only. From
the page you can open any folder or file in Finder. That is the only thing it can do
to your files; it never deletes, moves or renames anything.

## Install

You need macOS and Rust 1.85 or newer. Clone the repository and run:

```sh
cargo install --path .
```

This builds the `macdirtree` binary and puts it in `~/.cargo/bin`.

## Usage

```sh
macdirtree
```

Scans your home folder and opens the page in your default browser. The scan starts at
once and the page shows progress while it runs.

```sh
macdirtree /some/folder
```

Scans that folder instead. You can also type another path in the page and press Scan.

Options:

```
Usage: macdirtree [PATH] [--port N] [--no-open] [--linger SECS] [--help]

Scans PATH (default: your home folder) and serves the result on 127.0.0.1.

  --port N        listen on port N (default: one the OS picks)
  --no-open       do not launch the browser
  --linger SECS   exit this many seconds after the last tab closes (default 10)
  -h, --help      show this help
```

With `--no-open` the first line printed to stdout is the URL to visit, in the form
`Listening on http://127.0.0.1:PORT/`. `--linger` accepts fractions of a second.

## The views

The top bar holds the path field with Scan and Rescan, an Open in Finder button, the
Logical and Physical size toggle, the Zoom and Root buttons for the treemap, and a
status line with the scanned path, folder and file counts, total bytes and scan time.
Press Rescan after clearing files in Finder to see the new figures.

### Directory list

The left pane is an expandable tree, sorted largest first. The columns are Name, Size,
%, Items, Files, Subdirs and Last change. The % column shows the share of the parent
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

### Logical and Physical

Logical is the number of bytes a file contains. Physical is the number of bytes
allocated for it on disk. The two differ for sparse files and for APFS clones. A Docker
disk image such as `Docker.raw` can report 64 GB logical while only a few GB are
allocated. Switch to Physical to see what the file costs on your disk. Every view
follows the toggle.

Hard links are counted once. A file reached again through a second link shows 0 bytes
and is marked as a hard link, so totals match what the disk says. The scan stays on the
volume of the folder you scanned, so scanning `/` does not count `/Volumes` or count
the Data volume twice through firmlinks. Symlinks are not followed.

### Unreadable folders

Folders the scan may not read do not stop it. They are counted, and the status line
shows a button such as `3 unreadable`. Click it for the list of paths and errors. The
totals do not include what is behind those folders.

### Full Disk Access

On a real home folder every unreadable entry reads `Operation not permitted`. That is
macOS privacy protection (TCC), not file permissions. It covers app sandboxes under
`Library/Containers` and `Library/Group Containers`, the Photos Library, Safari, Mail
data and the Trash. macdirtree inherits its access from the app that launched it, and
nothing in the code can bypass that.

To scan those folders, give the terminal that launches macdirtree Full Disk Access:

1. Open System Settings > Privacy & Security > Full Disk Access.
2. Add Terminal, or the terminal you use (iTerm, Warp, the VS Code terminal), and turn
   it on.
3. Quit and reopen the terminal.
4. Run macdirtree again, or press Rescan.

The unreadable button in the page lists exactly which folders were skipped, so you can
check the result.

## How it opens things

Double-clicking, pressing Enter, or pressing Open in Finder sends the selected node to
the server. For a file or symlink the server runs `open -R <path>`, which shows the
file selected in a Finder window. For a folder it runs `open <path>`, which opens that
folder in Finder. The path comes from the server's own scanned tree, never from the
request. That is the only action the server can take.

## What it never does

macdirtree does not delete, move, rename or trash files, and it has no button, key or
API endpoint that could. The test `source_never_deletes_moves_or_renames` in
`tests/api.rs` reads every source file and fails if a call such as `remove_file`,
`remove_dir`, `rename` or `trash` appears. Clearing space is done in Finder, by hand,
after looking.

## Privacy and safety

- The server binds to 127.0.0.1 only. Nothing on your network can reach it.
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

## Exit rule

The server exits by itself. It counts connected browser tabs through an event stream.
When the last tab closes, a timer of `--linger` seconds starts, 10 by default. If no
tab reconnects before it fires, the process exits with status 0. A reload within the
linger period keeps it alive. Before the first tab ever connects the server waits
indefinitely, so a slow browser launch does not lose it.

While you look at the page the server does nothing. Its threads block until a request
or a scan event arrives. A test in `tests/resources.rs` measures this: an idle server
with a tab connected must use under 0.1 s of CPU in 2 s.

## Development

Build and check, each from the repository root:

```sh
cargo build
cargo fmt --check
cargo clippy --all-targets -- -D warnings
timeout 120 cargo test                        # unit and integration tests
timeout 120 cargo test --test resources       # idle CPU and exit checks, one file
```

The page files under `web/` are embedded with `include_str!`, so a change to the page
needs a rebuild. There is no frontend build step and no JavaScript dependency.

Integration tests start the debug binary with `--no-open --linger 1` and put a stub
`open` on `PATH`, so no test opens your browser or Finder.

- `tests/resources.rs`: the listening line, static files and their content types, the
  idle CPU budget, exit after the last tab closes, reload within linger, waiting for the
  first tab, and that `--no-open` runs nothing.
- `tests/scan.rs`: the scanner on a fixture tree with known sizes, a hard-linked pair, a
  symlink, nested empty folders and an unreadable folder: totals, order, hard-link
  accounting, physical size, mtime roll-up, error count and progress.
- `tests/treemap.rs`: rectangles never overlap, stay inside the viewport and inside
  their parent, sub-pixel nodes are dropped or collapsed, colours rank the top fifteen
  extensions, extension and largest-file statistics, and a 200,000 file layout is fast.
- `tests/api.rs`: every endpoint end to end against the fixture, the 400, 404, 409, 413
  and 415 paths, the `Host` check, that `/api/open` runs `open -R` for a file and bare
  `open` for a folder, and the no-delete source check.

The JSON endpoints are `GET /api/status`, `GET /api/events` (server-sent events),
`GET /api/node/{id}`, `GET /api/treemap`, `GET /api/extensions`, `GET /api/largest`,
`GET /api/errors`, `POST /api/scan` and `POST /api/open`. Their shapes are in the spec.

The spec is `docs/specs/macdirtree.md`. The build tickets, with the departures from the
plan, are in `docs/tickets/macdirtree/`. `AGENTS.md` is the short map of the code.

## Credits

- The idle-CPU and exit-after-close checks are ported from the resource tests of the
  turtle-lsp project.
- The treemap layout is the squarified treemap of Bruls, Huizing and van Wijk.
- The three linked views, the extension legend and the logical and physical sizes
  follow WinDirStat.
