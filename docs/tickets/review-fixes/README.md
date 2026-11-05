# Review fixes and a collapsible Rebuildable tab: tickets

Source: [docs/specs/review-fixes.md](../../specs/review-fixes.md).

| # | Ticket | Blocked by | Files |
|---|---|---|---|
| 01 | Server and command hardening | none | `src/server.rs`, `src/main.rs`, `tests/api.rs`, `tests/cli.rs`, **`tests/common/mod.rs`** |
| 02 | Scan reads the folder it listed | none | `src/scan.rs`, `src/tree.rs`, `tests/scan.rs`, **`tests/common/mod.rs`** |
| 03 | Rebuildable rules and Time Machine disks | none | `src/rebuildable.rs`, `src/volumes.rs`, `tests/rebuildable.rs` |
| 04 | Page: stale answers, keyboard, collapsible Rebuildable tab | none | `web/app.js`, `web/style.css`, `web/index.html` |
| 05 | Native app and release tooling | none | `src/access.rs`, `src/bin/hoardmap-app.rs`, `tools/release.sh`, `.github/workflows/release.yml` |

**Lock file:** `tests/common/mod.rs`. 01 and 02 may only add new helper functions at
the end of the file, each in its own block, so the cherry-picks merge.

**Waves:** one wave. All five tickets run together; their files are disjoint apart
from the lock file above.

**Models:** 04 (the page) runs on Claude Opus 5.5; 01, 02, 03 and 05 run on
gpt-6.1-sol at xhigh effort. After all five land, gpt-6-astra at xhigh reviews the
whole branch.

**Close-out:** at most five tickets were asked for, so the orchestrator does the
close-out itself after the last landing: `README.md`, `CHANGELOG.md`,
`docs/specs/hoardmap.md`, the full validation and the budgets.

## Status

| # | Status |
|---|---|
| 01 | done: request head read through a 16 KiB `take` (431 past it, 400 cut short); 64 live connections, 2 s read timeout (none on an open event stream); folder checked before the state lock (invalid folder during a scan is now 400, not 409); disk totals and state written together only by the current scan; event snapshot queued under the state lock; `hoardmap` takes `OsString` args. Tests 20→25 api, 6→8 cli. |
| 02 | done: both scanners open children by full path with `O_NOFOLLOW \| O_DIRECTORY` and check the listed `(dev, ino)`; changed folders skipped as errors; non-UTF-8 names kept in `Tree::raw_names` (Node stays 96 bytes); Stop checked every 256 entries and between bulk batches (stops in 0.4 ms portable, 2.3 ms bulk in a 200,000-file folder). Home scan under `ulimit -n 256`: same 62 unreadable as unlimited. After the second review, each descriptor closes once its folder is listed, with at most 32 listings at once. Unverified: a real non-UTF-8 name (APFS rejects them). |
| 03 | done: Gradle `build` cautioned; tool-specific sentinels outrank `package.json` (CMake over JS for `build`, Python over JS for `dist`); 35 sentinels checked by kind; Ollama caution covers `ollama create`; Time Machine by `Backups.backupdb` dir or APFS `backup_manifest.plist`. `groups` 17.7 ms on 1,001,401 fixture entries. Unverified live: a real APFS Time Machine disk. |
| 04 | done: `layoutRequest` bumped on tree reset, new `selectRequest` for `reveal()`, `generation` checked in live-refresh, `expand()` and `setRoot()` catch paths; extension, Largest files and Rebuildable panels keyboard-operable; path sent as typed; Rebuildable groups collapsed, caution shown, 50 then Show all, open state per root. Checked in headless Chrome with screenshots. |
| 05 | done: relaunch waits on `open -n` and the new instance waits up to 10 s for the old pid; one worker opens folders in order; release cleanup detaches; tap job serialised with a version guard; checkout@v5. Unverified live: LaunchServices relaunch, DMG detach, tap concurrency. |
| close-out | done: README, CHANGELOG and `docs/specs/hoardmap.md` updated. Full suite on the merged tree: 139 tests pass, 1 timing test ignored, plus the app's ordering test; fmt and both clippy runs clean; idle-CPU test passes alone. `scan_bench ~ 3` against main (machine busy, load 17-59 from another project): bulk median 14.4 s branch vs 15.9 s and 12.5 s main, within noise; portable median 30.9 s branch (best 19.0 s) vs 25.8 s and 25.6 s main (best 24.0 s), too noisy to call. Both scanners still agree on 62 unreadable folders. |

## Departures from the plan

- 05: the orchestrator capped the relaunch wait at 10 s, so a reused pid cannot hang the new instance.
- 03 put unit tests that delete their temp fixtures in `src/volumes.rs`, which the no-delete guard in `tests/api.rs` rejects; the orchestrator made `volumes::is_backup` public and moved them to `tests/volumes.rs`.
- 02 needs private access for its race tests in `src/scan.rs`, which rename and remove fixtures; the orchestrator narrowed the guard to skip the `#[cfg(test)] mod tests` block at the end of each source file. App code is still checked. 02 and 03 also each added one fixture line to a file outside their list (`src/icicle.rs`, `src/rebuildable.rs`, `tests/treemap.rs`) for the new `Tree` field.
- 04 kept the list's "more" row out of the arrow-key order: Down past the last shown child loads the next 1,000 and selects the first of them. It also put the caution line above the why line in an open group, so the caution does not move when the group opens.
- The scan budget (no more than 5% slower) could not be measured on a quiet machine: another project's builds kept the load between 17 and 59. Rerun `scan_bench` on a quiet machine before release.

## Second and third review

The second review (gpt-6-astra at xhigh) found 14 issues on the merged branch. What
landed for them:

- Server: a whole-request deadline (2 s) and a drain deadline (1 s), 8 slots past the
  64 kept for requests with the launch token, the last folder request wins, and
  `POST /api/scan {}` rescans the server's own path.
- Scan: descriptors close after each listing, with at most 32 listings at once; Stop
  keeps the entries already listed; previews keep raw names.
- Page: only a request's own pending entry is cleared, the Rebuildable folder cursor
  resets with the tree, opened folders reopen after a scan finishes, Relaunch recovers
  from a failure, and Rescan and the deferred first scan send `{}`.
- Release: the tap takes stable versions only. The no-delete guard needs the test
  module to be the last item, and the spec's HTTP section matches the `std::net`
  server.

The third review (Claude Opus 5.5) found 10 issues, being fixed now: among them the
path field drops leading white space again, a folder closed while opened folders are
being reopened stays closed, and the spec's HTTP and scan descriptions match the code.
#10 stays a known limit: the folder picker and the path field pass names as text, so
a folder whose name is not valid UTF-8 can only be rescanned, not chosen or typed.
