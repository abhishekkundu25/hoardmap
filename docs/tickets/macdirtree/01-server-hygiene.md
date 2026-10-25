# 01: Server, CLI and resource hygiene

**What to build:** `macdirtree --no-open --linger 1` starts an HTTP server on
127.0.0.1, prints `Listening on http://127.0.0.1:PORT/` as its first stdout line, serves
the embedded page files, streams `/api/events`, answers `/api/status` with
`{"state":"idle"}`, uses no CPU while a tab is connected and idle, and exits 0 one
second after the last tab disconnects. Without `--no-open` it runs `open <url>`.
This is the port of turtle-lsp's resource checks (`test/procs.mjs`,
`test/resources.test.mjs` in `../turtle-lsp`), the first thing the project gets.

Spec sections: Implementation decisions (HTTP, Presence and exit, Browser launch,
Cross-site protection), Testing decisions (Resource hygiene).

Shape:
- `cli::Options { path, port, open, linger }` parsed from `std::env::args` by hand
  (`--port N`, `--no-open`, `--linger SECS`, `--help`; the positional path defaults to
  `$HOME`). Unknown options exit 2 with usage on stderr.
- `server::App` holds the scan state behind a mutex (only `Idle` is produced in this
  ticket; the enum has the four variants from the spec so 04 fills them in), the
  presence counter and the event broadcaster. `server::run(options)` never returns:
  it binds, prints the line, launches the browser, and loops on `recv`, one thread
  per request.
- `api::handle(app, request)` answers `/api/status` and `/api/events`; everything
  else under `/api/` is 404. The static files are served by `server` from
  `include_str!("../web/...")`.
- SSE: `Content-Type: text/event-stream`, chunked (`Response::new` with `None`
  length), the reader blocks on a `crossbeam`-free `std::sync::mpsc` receiver with a
  15 s timeout for the keepalive comment; its `Drop` decrements presence and arms the
  linger timer. The timer is one thread that sleeps `linger` and exits the process if
  presence is still 0 and the generation has not changed (a reconnect bumps it).
- `tests/common/mod.rs`: `cpu_seconds(pid)` (ps on macOS, `/proc` on Linux), `Server`
  helper that spawns `CARGO_BIN_EXE_macdirtree` with args, reads the port line, offers
  `get(path) -> (status, headers, body)` over a raw `TcpStream`, `open_events() ->
  TcpStream` (keeps the SSE socket), `wait_exit(timeout) -> Option<ExitStatus>`, and
  kills the child on drop.

**Blocked by:** None (can start immediately).

**Status:** done

- [ ] `tests/resources.rs`: first stdout line matches `Listening on http://127.0.0.1:\d+/`
- [ ] `GET /` is 200 `text/html` and contains `macdirtree`; `/app.js` and `/style.css`
      have their content types; `/nope` is 404
- [ ] `GET /api/status` is 200 JSON with `state: "idle"`
- [ ] with one events socket open, 2 s of idleness uses under 0.1 s of CPU (sample
      twice as in turtle-lsp's `assertIdle`), and the stream has sent `event: status`
- [ ] closing the only events socket: process exits 0 within `linger + 3` s
- [ ] closing and reopening within linger: process still running 3 s later
- [ ] before any tab connects, the process is still running after 3 s with `--linger 1`
- [ ] `--no-open` given: no `open` is run (stub `open` on `PATH` writes a marker file,
      which must not exist); without it the stub is run with the URL
- [ ] `fmt`, `clippy -D warnings`, `cargo test` green
