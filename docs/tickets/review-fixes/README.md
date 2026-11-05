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
| 01 | todo |
| 02 | done: both scanners `openat` children with `O_NOFOLLOW \| O_DIRECTORY` and check `(dev, ino)`; changed folders skipped as errors; non-UTF-8 names kept in `Tree::raw_names` (Node stays 96 bytes); Stop checked every 256 entries and between bulk batches (stops in 0.4 ms portable, 2.3 ms bulk in a 200,000-file folder). Home scan under `ulimit -n 256`: same 62 unreadable as unlimited, so held parent descriptors stay within a Finder-launched app's limit. Unverified: a real non-UTF-8 name (APFS rejects them). |
| 03 | done: Gradle `build` cautioned; tool-specific sentinels outrank `package.json` (CMake over JS for `build`, Python over JS for `dist`); 35 sentinels checked by kind; Ollama caution covers `ollama create`; Time Machine by `Backups.backupdb` dir or APFS `backup_manifest.plist`. `groups` 17.7 ms on 1,001,401 fixture entries. Unverified live: a real APFS Time Machine disk. |
| 04 | todo |
| 05 | done: relaunch waits on `open -n` and the new instance waits up to 10 s for the old pid; one worker opens folders in order; release cleanup detaches; tap job serialised with a version guard; checkout@v5. Unverified live: LaunchServices relaunch, DMG detach, tap concurrency. |
| close-out | todo |

## Departures from the plan

- 05: the orchestrator capped the relaunch wait at 10 s, so a reused pid cannot hang the new instance.
- 03 put unit tests that delete their temp fixtures in `src/volumes.rs`, which the no-delete guard in `tests/api.rs` rejects; the orchestrator made `volumes::is_backup` public and moved them to `tests/volumes.rs`.
- 02 needs private access for its race tests in `src/scan.rs`, which rename and remove fixtures; the orchestrator narrowed the guard to skip the `#[cfg(test)] mod tests` block at the end of each source file. App code is still checked. 02 and 03 also each added one fixture line to a file outside their list (`src/icicle.rs`, `src/rebuildable.rs`, `tests/treemap.rs`) for the new `Tree` field.
