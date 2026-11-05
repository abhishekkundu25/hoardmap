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
| 02 | todo |
| 03 | todo |
| 04 | todo |
| 05 | todo |
| close-out | todo |

## Departures from the plan

None yet.
