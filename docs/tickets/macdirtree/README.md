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
| 01 | [Server, CLI and resource hygiene](01-server-hygiene.md) | none | `src/cli.rs`, `src/server.rs`, `src/api.rs`, **`src/main.rs`**, `tests/common/mod.rs`, `tests/resources.rs` |
| 02 | [Scanner](02-scanner.md) | none | `src/scan.rs`, `tests/scan.rs` |
| 03 | [Treemap layout and colours](03-treemap.md) | none | `src/treemap.rs`, `tests/treemap.rs` |
| 04 | [API over the scan state](04-api.md) | 01, 02, 03 | `src/api.rs`, `src/server.rs`, **`src/main.rs`**, `tests/common/mod.rs`, `tests/api.rs` |
| 05 | [The page](05-page.md) | 04 | `web/index.html`, `web/style.css`, `web/app.js` |
| 06 | [Close-out](06-close-out.md) | 05 | `README.md`, `CHANGELOG.md`, `AGENTS.md`, this folder |

## Waves

1. 01, 02, 03 together (disjoint files).
2. 04 alone.
3. 05 alone.
4. 06 alone, after the full validation.

## Departures from the plan

Logged here as they happen.
