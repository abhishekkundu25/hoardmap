# AGENTS.md

hoardmap is a Mac app, written in Rust, that scans a folder and shows a disk-usage
page (directory list, treemap, extension list) in its window, served from 127.0.0.1
inside the app behind a launch token. There is no browser version; the `hoardmap`
command only opens the app on a folder. It never
deletes, moves or renames anything; do not add such an action.

## Where things live

- `src/tree.rs`: the arena tree every module reads. Root at index 0, children
  largest first, directory figures are subtree totals.
- `src/scan.rs`: file system to tree, in parallel, with progress atomics. On macOS it
  lists folders with `getattrlistbulk`; the portable `read_dir` plus `lstat` path is
  the fallback everywhere and `HOARDMAP_SCANNER=portable` forces it. Both must build
  the same tree on clone-free folders (`tests/scan.rs` checks); only the bulk listing
  sees APFS clones, so with clones its Physical is lower. The walk runs in its own pool of
  `default_threads()` (the non-efficiency cores; `HOARDMAP_THREADS` overrides), and
  any thread count must build the same tree.
- `src/rebuildable.rs`: the Rebuildable rule table (owner, how the folder comes back,
  the owner's documentation URL) and `groups`, matched on the finished tree. A test
  keeps "safe", "delete" and "remove" out of rule text.
- `src/treemap.rs`: squarified layout, extension colours, extension and largest-file
  statistics.
- `src/server.rs`: HTTP, static files, the launch token, and `App::open_folder` for
  folders handed over by Finder or the command.
- `src/main.rs`, `src/cli.rs`: the `hoardmap` command, `open -b dev.hoardmap.app PATH`.
  Debug builds add the test server (`HOARDMAP_TEST_SERVER=1`); keep it out of release
  builds (`cfg(debug_assertions)`).
- `packaging/homebrew/`: the cask and tap README the release writes into
  abhishekkundu25/homebrew-tap.
- `src/api.rs`: the JSON endpoints, documented in `docs/specs/hoardmap.md`.
- `src/volumes.rs`: the disks the Disks menu offers, and the macOS folder picker.
- `assets/`: the logo. `hoardmap.svg` is the source, served as `/logo.svg` and
  embedded with `include_str!`; `hoardmap.icns` is the app icon and
  `dmg-background.tiff` the DMG window, both made by `tools/make-images.sh` (needs
  Google Chrome).
- `site/`: the GitHub Pages website. It is published only by a release (`release.yml`
  calls `pages.yml`), never by a push to `main`.
- `web/`: the page, embedded in the binary with `include_str!`; no build step, but a
  page change needs a rebuild (`cargo build`) before the binary serves it.
- Specs in `docs/specs/`, tickets in `docs/tickets/<name>/` with a README table.

## Build and test

```sh
cargo build
cargo fmt --check
cargo clippy --all-targets -- -D warnings
timeout 120 cargo test                        # unit and integration tests
timeout 120 cargo test --test resources       # idle CPU and exit checks, one file
cargo clippy --features app --all-targets -- -D warnings   # the native app too
tools/bundle-app.sh                           # target/Hoardmap.app, signed ad hoc
tools/release.sh                              # universal dmg, zip and CLI tarball in target/dist
cargo run --release --example scan_bench -- ~ 5   # time both scanners, check they agree
```

- The native app (`src/bin/hoardmap-app.rs`) needs `--features app`. It shows the
  same page from `server::spawn` with `native: true`, never exits on its own, and
  defers the first scan until the page's start screen closes when Full Disk Access is
  off. `src/access.rs` owns the access check, the Settings pane and relaunching.

- Integration test files: `tests/resources.rs`, `tests/scan.rs`, `tests/treemap.rs`,
  `tests/api.rs`, with shared helpers in `tests/common/mod.rs`.
- Integration tests spawn the debug binary (`CARGO_BIN_EXE_hoardmap`) with
  `HOARDMAP_TEST_SERVER=1` and put a stub `open` on `PATH`; nothing opens Finder or the
  app from a test.
- Tests build their fixtures in the system temp folder and remove them.
- The idle-CPU budget is a pass/fail line (under 0.1 s of CPU in 2 s). Run that test
  alone when the machine is busy before blaming a change.
