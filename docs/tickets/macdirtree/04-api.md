# 04: API over the scan state

**What to build:** the server scans PATH at start and every JSON endpoint of the spec
works: status and events show progress then totals, nodes, treemap, extensions,
largest, errors, scan, open. The idle-CPU check holds after a scan.

Spec sections: Implementation decisions (Scan state, HTTP, Cross-site protection,
Open in Finder, API), Testing decisions (API end to end).

Shape:
- `server::App` state becomes the spec's four variants. `App::start_scan(path)` spawns
  one thread that runs `scan::scan`, builds `treemap::Colours`, stores `Done` or
  `Failed`, and notifies the event broadcaster. While scanning, a ticker thread sends
  the status every 250 ms and stops when the scan ends (no ticker exists otherwise).
- `api::handle` routes the endpoints. Parsing of ids and query strings by hand;
  errors are JSON `{"error": "..."}` with the status codes in the spec.
- POST bodies: refuse without `Content-Type: application/json` (415) or over 64 KiB
  (413).
- `/api/open` builds the `Command` with `open` (macOS) or `xdg-open` (else), `-R` for
  non-directories, and `spawn`s it without waiting; a spawn failure is 500.
- Extend `tests/common/mod.rs` with `post(path, body)` and `wait_for_done()`, and a
  `fixture()` builder shared with the scanner's layout (copy its shape; the scanner
  test keeps its own).

**Blocked by:** 01, 02, 03.

**Status:** ready-for-agent

- [ ] `tests/api.rs`: start on the fixture with `--no-open --linger 1`; `/api/status`
      reaches `done` within 10 s with `bytes` equal to the fixture total
- [ ] `/api/events` carried at least one `scanning` and one `done` status
- [ ] `/api/node/0` has the fixture path, children largest first with `has_children`;
      `/api/node/999999` is 404
- [ ] `/api/treemap?root=0&w=400&h=300` has rects inside the viewport with colours
      matching `/api/extensions`
- [ ] `/api/extensions` lists `bin`, `jpg`, `txt`, `""` with the right bytes and files
- [ ] `/api/largest?n=2` is `big.bin` then `x.jpg` with full paths
- [ ] `/api/errors` reports the locked directory (skipped as root)
- [ ] `POST /api/scan` with a file path is 400; during a scan a second one is 409
      (use a large real folder such as `/usr` to make the window, or skip when it ends
      too fast and say so in the test output); after `done` a rescan works
- [ ] `POST /api/open` without the JSON content type is 415; with `{"id": <file>}`
      the stub `open` on `PATH` received `-R <path>`; with a directory id it received
      the bare path; the server never deletes or writes under the fixture
- [ ] after `done`, 2 s idle with a tab connected uses under 0.1 s CPU (same check as
      `tests/resources.rs`)
- [ ] no endpoint anywhere deletes, moves or renames: `grep -rn "remove_\|rename(" src`
      finds nothing
- [ ] `fmt`, `clippy -D warnings`, `cargo test` green
