# True cost and Rebuildable: tickets

Source: [docs/specs/true-cost-rebuildable.md](../../specs/true-cost-rebuildable.md).

| # | Ticket | Blocked by | Files |
|---|---|---|---|
| 01 | [True cost: scanner, model and API](01-true-cost.md) | none | `src/scan.rs`, `src/tree.rs`, `src/treemap.rs`, `src/icicle.rs`, **`src/api.rs`**, `src/server.rs`, `examples/scan_bench.rs`, `tests/scan.rs`, `tests/treemap.rs`, `tests/api.rs` |
| 02 | [Rebuildable: rules and API](02-rebuildable.md) | none | `src/rebuildable.rs` (new), `src/lib.rs`, **`src/api.rs`**, `tests/rebuildable.rs` (new) |
| 03 | [Page: clone marker and Rebuildable tab](03-page.md) | 01, 02 | `web/index.html`, `web/app.js`, `web/style.css` |
| 04 | [Close-out](04-close-out.md) | 03 | `README.md`, `CHANGELOG.md`, `AGENTS.md`, `docs/specs/hoardmap.md`, this folder |

**Lock file `src/api.rs`.** 01 edits `Query::mode`, `node_json`, `extensions`,
`largest`, `src/api.rs`'s own unit tests and nothing else. 02 adds one match arm,
`("GET", "/api/rebuildable")`, directly after the `/api/errors` arm, appends
`| "/api/rebuildable"` to the 405 list after `"/api/errors"`, and adds one function,
`rebuildable`, directly after `fn errors`. 02 calls `query.mode()` and
`treemap::Mode` only through their public names, so 01's new variant needs no edit
from 02.

## Waves

1. 01 and 02 together.
2. 03, after both land.
3. 04, after the full validation and the benchmark against `main`.

## Status and departures

| # | Status |
|---|---|
| 01 | landed (2c19abe). Departure: every member of a full clone group is `Shared::Clone`, the first one keeping the data, so the row whose number looks big is marked too. `shared_unattributed` counts the shared part once per partial clone, so it is an upper bound. |
| 02 | landed (685247f). 14 rules, each URL fetched; CoreSimulator left out (Apple says manage it through Xcode or `simctl`), CocoaPods left out (no documented cache path). DerivedData cites Apple's archived Xcode 4 release notes, the only Apple source found. |
| 03 | landed (0f7702b). "Docs" is shown as selectable text: the app window only loads its own page, so a link would do nothing. Clone figures show in Physical only. |
| 04 | done. |

## Departure: no private size

`scan_bench ~ 5` (1,021,727 entries), three alternating rounds against `main` with
private size, clone id and flags requested: `main` medians 1.41, 1.41, 1.28 s; branch
4.50, 3.84, 4.48 s, about three times slower against a 10% budget. One attribute at a
time, three runs each:

| Extended attributes | Bulk median |
|---|---|
| none | 1.39 s |
| clone id | 1.19 s |
| flags | 1.35 s |
| clone id and flags | 1.35 s |
| private size | 4.20 s |
| all three | 4.12 s |

Private size is the whole cost: the kernel works out per file how many of its blocks
no other file or snapshot shares. Clone id and flags are enough to count fully shared
clones once, 2.66 GB on this home folder; partly changed clones came to 155 KB. So, as
the spec's budget line allows, private size is gone, and with it `unique`,
`PartialClone` and `shared_unattributed`. Partly changed clones are counted in full,
as before. A clone's note still says what removing it frees: nothing, while the other
copies exist.

## Results

After the departure, `scan_bench ~ 5` (about 1,024,000 entries, warm cache), `main`
and the branch alternating, three rounds, nothing else building:

| Round | `main` | Branch |
|---|---|---|
| 1 | 1.39 s | 1.28 s |
| 2 | 1.33 s | 1.29 s |
| 3 | 1.29 s | 1.40 s |

Medians 1.33 s and 1.29 s: no measurable change (budget: at most 10% slower). The
spread between rounds (0.1 s) is larger than any difference, so the limit is still the
file system listing itself, as in `docs/tickets/scan-speed/`; asking for clone id and
flags adds no system time a run can see. The home folder had 2.65 GB of clones counted
once (On disk 139.7 GB against 142.3 GB before). The two scanners differ on 0 of
292,152 entries in `/Applications` and on 6 of 1,024,452 in the home folder, where
files change between the two scans.

Tests: lib 39, api 21, cli 7, rebuildable 5, resources 6 (idle CPU, also alone),
scan 18, treemap 13; fmt, both clippy runs and `tools/bundle-app.sh` green.
