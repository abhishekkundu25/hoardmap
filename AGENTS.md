# AGENTS.md

macdirtree is a Rust binary that scans a folder and serves a disk-usage page
(directory list, treemap, extension list) to the browser on 127.0.0.1. It never
deletes, moves or renames anything; do not add such an action.

## Where things live

- `src/tree.rs`: the arena tree every module reads. Root at index 0, children
  largest first, directory figures are subtree totals.
- `src/scan.rs`: file system to tree, in parallel, with progress atomics.
- `src/treemap.rs`: squarified layout, extension colours, extension and largest-file
  statistics.
- `src/server.rs`: HTTP, static files, browser presence, exit when the last tab is gone.
- `src/api.rs`: the JSON endpoints, documented in `docs/specs/macdirtree.md`.
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
```

- Integration test files: `tests/resources.rs`, `tests/scan.rs`, `tests/treemap.rs`,
  `tests/api.rs`, with shared helpers in `tests/common/mod.rs`.
- Integration tests spawn the debug binary (`CARGO_BIN_EXE_macdirtree`) with
  `--no-open --linger 1` and put a stub `open` on `PATH`; nothing opens the user's
  browser or Finder from a test.
- Tests build their fixtures in the system temp folder and remove them.
- The idle-CPU budget is a pass/fail line (under 0.1 s of CPU in 2 s). Run that test
  alone when the machine is busy before blaming a change.
