# macdirtree: build tickets

Source: [docs/specs/macdirtree.md](../../specs/macdirtree.md). One slice per ticket for
`/parallel-slices`: slices in the same wave edit disjoint files. **Bold** files are
locks, edited by one slice at a time in the order given. The skeleton (crate, `tree.rs`,
module stubs, placeholder page) is the base commit; no slice edits `src/tree.rs` or
`src/lib.rs` without reporting it first.

Checks every slice keeps green, each under `timeout 120`: `cargo fmt --check`,
`cargo clippy --all-targets -- -D warnings`, `cargo test` for its own files.

| # | Ticket | Blocked by | Files |
|---|---|---|---|
| 01 | [Server, CLI and resource hygiene](01-server-hygiene.md) (done) | none | `src/cli.rs`, `src/server.rs`, `src/api.rs`, **`src/main.rs`**, `tests/common/mod.rs`, `tests/resources.rs` |
| 02 | [Scanner](02-scanner.md) (done) | none | `src/scan.rs`, `tests/scan.rs` |
| 03 | [Treemap layout and colours](03-treemap.md) (done) | none | `src/treemap.rs`, `tests/treemap.rs` |
| 04 | [API over the scan state](04-api.md) (done) | 01, 02, 03 | `src/api.rs`, `src/server.rs`, **`src/main.rs`**, `tests/common/mod.rs`, `tests/api.rs` |
| 05 | [The page](05-page.md) (done) | 04 | `web/index.html`, `web/style.css`, `web/app.js` |
| 06 | [Close-out](06-close-out.md) | 05 | `README.md`, `CHANGELOG.md`, `AGENTS.md`, this folder |

## Waves

1. 01, 02, 03 together (disjoint files).
2. 04 alone.
3. 05 alone.
4. 06 alone, after the full validation.

## Departures from the plan

Logged here as they happen.

- 01: HTTP is spoken over `std::net` instead of `tiny_http`. tiny_http buffers chunked
  bodies (an SSE event would sit unsent) and its private reader thread hides a closed
  socket until the next write fails, which on macOS is the write after the close: the
  exit-after-tab-closes rule would lag by up to two keepalives. `tiny_http` was removed
  from Cargo.toml. The dependency list is now serde, serde_json and rayon.
- 01: `State::Scanning` carries no progress field until 04 adds `scan::Progress` (02
  landed after 01 was written).
- 04: a `Host` header check (loopback on the bound port, else 403) was added against
  DNS rebinding. After an early error (400, 411, 413, 431) the server half-closes and
  drains the request so the client sees the status instead of a reset. `/api/errors`
  is 409 without a tree, like the other tree endpoints. The resources test that
  expected `idle` now accepts `scanning` or `done`, since the scan starts at launch.
