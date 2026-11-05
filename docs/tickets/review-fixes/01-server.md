# 01: Server and command hardening

**What to build:** spec Solution 1. Bound `read_request` before buffering (per line
and in total, 16 KiB), a read timeout and a live-connection cap in the accept loop
that never panics, `start_scan` checks the folder before taking the lock, a finishing
scan writes disk totals only while it is the current scan, the event stream sends its
snapshot and later events in order, and `main` reads arguments with `args_os`.

**Blocked by:** none.

**Status:** done

## Acceptance

- [ ] A request line or header longer than the limit, with or without a newline, is
      answered 431 or 400 without the server reading past the limit; test in
      `tests/api.rs` sends 1 MiB without a newline.
- [ ] Idle connections past the cap do not stop the page's requests from being
      answered, and the accept loop survives; test in `tests/api.rs`.
- [ ] A connection that sends nothing is closed after the read timeout.
- [ ] `start_scan` does no file-system call while holding the state lock.
- [ ] Stop scan A, start scan B on another folder: when A finishes late, the disk
      totals stay B's; test in `tests/api.rs`.
- [ ] The event stream never sends a status older than one it already sent.
- [ ] `hoardmap` with a non-UTF-8 argument does not panic; test in `tests/cli.rs`
      (skip on file systems that reject such names, saying so).
- [ ] The idle-CPU test in `tests/resources.rs` still passes.
- [ ] `cargo fmt --check`, both clippy runs, and `timeout 120 cargo test --test api
      --test cli --test resources` green.
