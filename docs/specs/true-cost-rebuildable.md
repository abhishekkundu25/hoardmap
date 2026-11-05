# True cost and Rebuildable

Source: [docs/research/what-users-want.md](../research/what-users-want.md), features 1
and 6, picked from three competing proposals (see "Why these two" below).

## Problem

**Clones are counted as copies.** APFS clones (`cp -c`, Finder duplicates, pnpm, uv,
bun, git worktrees) share their data blocks. hoardmap's Physical size comes from
`ATTR_FILE_ALLOCSIZE`, which reports the full allocation for every clone, so two clones
of a 500 MB file add 1.5 GB to a folder that cost 8 KB of real space (research,
experiment 1). Totals can exceed the disk. The spec's user story 9 and the 0.1.0
changelog say the Physical toggle shows what clones really cost; it does not. No
popular tool shows what removing an item would actually free.

**Developer caches are scattered.** `node_modules`, Cargo `target`, Python `.venv`,
Xcode DerivedData, simulators, package-manager and model caches sit in hundreds of
folders across a disk. Every analyser shows each one alone among thousands of folders;
the tools that group them delete. Three tools appeared on Hacker News in 2026 for this
gap alone (research, B3).

## Solution

1. **True cost.** The scanner asks APFS, in the `getattrlistbulk` call it already
   makes, for each file's private size, clone id and extended flags.
   - **Physical** counts a fully shared clone once, keyed by `(dev, clone id)` the way
     hard links are keyed by `(dev, inode)`. The page keeps its two buttons, Logical
     and Physical; Physical is simply correct now.
   - Each node also carries its **unique** bytes (the sum of private sizes: what
     removing it would free now). It is not a size mode; it appears in the tooltip of
     rows that share data.
   - A partly shared clone keeps its full allocation in On disk; the part it shares
     cannot be tied to a group per file, so the status reports the total of those
     shared bytes as its own figure ("shared, not attributed").
   - Rows of clones get a marker beside the hard-link one.
2. **Rebuildable.** A third bottom tab, beside Extensions and Largest files, groups the
   folders that their owning tool rebuilds or downloads again: per group a label, the
   owner, one line on how it comes back (quoting the owner's documentation, with a
   link), a total, a count, and the folders largest first. Clicking a group highlights
   its folders in the treemap; clicking a folder selects it, and Open in Finder works
   as everywhere else. Nothing deletes.

The two compound: pnpm, uv and bun fill `node_modules` and `.venv` with clones of a
shared store, so the Rebuildable totals are honest only with clone-aware Physical sizes.

## Why these two

Three independent proposals (A, B, C) each chose two of the ten research features;
all three chose feature 1. For the second they chose 2 (disk ledger), 6 (Rebuildable)
and 4 (folder explainer). Two separate reviews against the same rubric both scored
1 + 6 highest (28 and 27 of 30): the second pick is fully testable with temp
fixtures, needs no subprocess or stored state, and adds nothing the main spec rules
out. The ledger (2) and the explainer (4) are the next candidates.

Grafts from the other candidates: per-volume gating and a measured speed budget for
the extra attributes (C), the "On disk" label and the visible "shared, not
attributed" figure, a fallback to today's figures where clones cannot be seen
(A), and a rule-text test that forbids "safe to delete" wording and requires a
source per rule (C).

User decision (2026-10-30): no third size mode. "Why complicate it for users?" One
correct Physical figure, with the freed-if-removed number in a tooltip only.

## User stories

1. As a developer using pnpm, uv or bun, my projects' `node_modules` and `.venv` show
   what they really cost in Physical, so I stop blaming them for space the shared store uses.
2. As a user who duplicated large files in Finder, the folder total counts the shared
   data once, so totals no longer exceed my disk.
3. As a user, the tooltip of a row that shares data says what removing it would free
   right now.
4. As a user, a clone row carries a marker and a tooltip ("64 MB on disk, frees 0 B if removed:
   shares all its data with other files"), so I know why its numbers differ.
5. As a user, the status line says how much shared data was counted once, so I can see
   that clone handling did something.
6. As a user with partly changed clones, the status says how many bytes are shared but
   not attributed to one file, so nothing is silently hidden.
7. As a user on a non-APFS drive, or with `HOARDMAP_SCANNER=portable`, Physical
   behaves as before.
8. As a Time Machine user, the freed-if-removed tooltip says that snapshot-held bytes
   are not counted, so a low number does not surprise me.
9. As a user, the treemap, icicle, extension list and Largest files all use the
   corrected Physical figure.
10. As a developer, I open the Rebuildable tab and see every `node_modules`, Cargo
    `target`, `.venv` and Xcode DerivedData under the scanned folder, grouped, with
    totals.
11. As a developer, a nested `node_modules` inside another is counted once, inside its
    outer folder.
12. As a developer, a folder that merely shares a name (a `target` folder with my work
    in it) is not listed, because each rule needs a sentinel file or a fixed path.
13. As a user, each group says who owns it and how it comes back, with a link to the
    owner's documentation, and never tells me it is safe to delete.
14. As a user, clicking a group highlights its folders in the treemap and dims the
    rest, as clicking an extension does, and Escape clears it.
15. As a user, clicking a folder in the tab selects it in the list and the map; Enter
    or a double-click opens it in Finder.
16. As a user, the tab follows the size mode and the current root, so drilling into a
    project narrows it to that project.
17. As a user, the tab fills in during a scan like the other tabs.
18. As a user, the page looks the same: the same top bar, segmented control, tabs,
    tables and colours.
19. As a user, scans are no slower than the budget below, and the app at rest uses no
    more CPU than before.
20. As a maintainer, every rule in the Rebuildable table names its owner, a primary
    source URL and the date it was checked, and a test fails if one is missing.
21. As a maintainer, both scanners still build the same tree on clone-free folders, and
    the tests say exactly where they may differ.

## Implementation decisions

Departure after measuring (see the tickets README): private size tripled the scan
time, so the scanner reads only clone id and flags. Where this section mentions
`unique`, `PartialClone` or `shared_unattributed`, those were built and then removed.

### True cost

- **Scanner** (`src/scan.rs`, `bulk`): pass `FSOPT_ATTR_CMN_EXTENDED` as the options
  argument and request `ATTR_CMNEXT_PRIVATESIZE`, `ATTR_CMNEXT_CLONEID` and
  `ATTR_CMNEXT_EXT_FLAGS` in the `forkattr` word (the extended common attributes; see
  getattrlist(2)). Parse them in bit order after the file attributes. Missing libc
  constants (`ATTR_CMNEXT_CLONE_REFCNT` if used, `EF_MAY_SHARE_BLOCKS`,
  `EF_SHARES_ALL_BLOCKS`) are local constants with the `<sys/attr.h>` comment, like
  `ATTR_CMN_ERROR`.
- **Gate:** request the extended attributes only when the scanned volume reports clone
  support (`VOL_CAP_INT_CLONE` from `ATTR_VOL_CAPABILITIES`, read once per scan).
  Without it the tree is built as today and marked not clone-aware.
- **Model** (`src/tree.rs`): `Node` gains `unique: u64` (subtree total, like
  `physical`). `hard_link: bool` becomes `shared: Shared`, an enum `None`, `HardLink`,
  `Clone` (a later member of a fully shared group: `physical` 0, `unique` 0), and
  `PartialClone` (may share, not all: full `physical`, `unique` = private size). `Tree`
  gains `clone_aware: bool`, `clone_counted_once: u64` and `shared_unattributed: u64`.
  One enum, not a second boolean beside `hard_link`.
- **Flatten:** a second `HashSet<(u64, u64)>` of `(dev, clone id)` beside the hard-link
  set. A file with `EF_SHARES_ALL_BLOCKS` whose key was seen becomes `Shared::Clone`;
  its allocation is added to `clone_counted_once`. A first occurrence keeps its
  allocation. Files that are neither keep `unique = physical` when the scan is not
  clone-aware, and `unique = private size` when it is.
- **Modes:** unchanged, Logical and Physical; Physical reads the corrected figure.
- **API** (`src/api.rs`, `src/server.rs`): node JSON gains `unique`
  and `shared` (`"hard_link"`, `"clone"`, `"partial_clone"` or absent) and drops
  `hard_link`. Largest adds `unique`. `/api/status` adds `unique`,
  `clone_aware`, `clone_counted_once` and `shared_unattributed`.
- **Scanner agreement:** the portable scanner cannot see clones. It sets
  `unique = physical` and `clone_aware = false`. On a clone-free tree both scanners
  agree on every field except `unique`; on a tree with clones they agree on logical
  size, counts and times, and the bulk On disk total is at most the portable one.
  `examples/scan_bench` compares the fields both scanners know and prints the clone
  figures.

### Rebuildable

- **Rules** (`src/rebuildable.rs`, new): a compiled table, one entry per kind:
  key, label, owner, `why` (one line on how it comes back), source URL, date checked,
  and a matcher, an enum:
  - `ChildSentinel { name, child }`: a folder called `name` that contains `child`
    (Cargo `target` with `CACHEDIR.TAG`, `.venv` or `venv` with `pyvenv.cfg`).
  - `SiblingSentinel { name, sibling }`: a folder called `name` beside `sibling`
    (`node_modules` beside `package.json`, `.build` beside `Package.swift`).
  - `PathSuffix(&[&str])`: a folder whose path ends in these names (Xcode
    `Library/Developer/Xcode/DerivedData`, `Library/Developer/CoreSimulator/Devices`,
    `.gradle/caches`, `Library/Caches/Homebrew`, `Library/Caches/pip`,
    `Library/Caches/CocoaPods`, `Library/pnpm/store`, `.cache/huggingface/hub` and
    the others the slice can source).
  Only rules with a primary-source URL go in; the research file has most of them.
- **Matching:** `groups(tree, root, mode) -> Vec<Group>`, one depth-first walk of the
  finished tree below `root`. A matched folder is not descended, so nested matches
  count once. No file system access, no subprocess. Matches use names already in the
  tree.
- **API:** `GET /api/rebuildable?root=&mode=` returns `{total, groups: [{key, label,
  owner, why, source, bytes, count, folders: [{id, path, bytes}]}]}`, groups and
  folders largest first, folders capped at 500 per group with `more` holding the rest's
  count. Empty groups are left out. 409 without a tree, 400 on a bad root or mode, and
  served from the preview during a scan, like the other tree endpoints.
- **Text:** no rule text says "safe", "delete" or "remove"; a unit test enforces it,
  and that every rule has a non-empty owner, an `https://` source and a date.

### Page

- `#mode` is unchanged: Logical and Physical, Physical the default. Physical's title
  says shared clone data is counted once.
- A clone marker beside the hard-link marker. Its tooltip gives the sizes and what
  removing the item would free now ("Shares all its data with other files: removing
  it frees 0 B. Data held by snapshots is not counted.").
- The status line adds "N shared by clones, counted once" when `clone_counted_once` is
  above 0, and "N shared, not attributed" when `shared_unattributed` is.
- A "Rebuildable" tab after Largest files, the same table styling as Extensions: a
  group row (label, owner, size, percent, count, the `why` line and a source link) and
  its folders under it. Clicking a group highlights its folders in the treemap through
  the existing dim path; Escape clears. A folder row selects and reveals like a
  Largest files row.

## Budgets (pass/fail)

- `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo clippy
  --features app --all-targets -- -D warnings`, `timeout 120 cargo test` all green.
- Scan speed: `cargo run --release --example scan_bench -- ~ 5`, branch against `main`
  alternating, three rounds, nothing else running. The branch's median bulk scan is at
  most 10% slower than `main`'s. If it is over: first request only the clone id and
  flags (dropping private size, so the freed-if-removed tooltip falls back to Physical),
  then drop the gate's private size entirely. Record the numbers and what limits them.
- `tests/resources.rs` idle-CPU test passes, with the Rebuildable tab loaded.
- Memory: at most 9 more bytes per node (`unique` plus the enum in the old boolean's
  place).
- No delete, move or rename action anywhere; no subprocess added; nothing written to
  disk.

## Testing decisions

Follow the existing harness: unit tests beside the code, integration tests that spawn
the debug binary with `HOARDMAP_TEST_SERVER=1` and the stub `open` (`tests/common/mod.rs`),
fixtures in the system temp folder, removed afterwards.

- `tests/scan.rs`: a clone fixture (`cp -c`) with a 4 MB random file, a full clone, a
  clone with 1 MB rewritten, a plain copy and the existing hard-link pair. The bulk
  tree counts the full clone once in Physical, marks it `Clone` with `unique` 0, gives
  the partial clone a `unique` close to 1 MB (within block rounding) and
  `PartialClone`, and leaves logical sizes unchanged. The portable tree matches the
  bulk one on logical sizes and counts and reports `clone_aware` false. The existing
  `bulk_and_portable_build_the_same_tree` stays green on its clone-free fixture,
  comparing every field but `unique`.
- `tests/api.rs`: node JSON has `unique` and `shared`, status has the new fields,
  `mode=bogus` still 400.
- `tests/rebuildable.rs` (new): a fixture with `proj/package.json` and
  `proj/node_modules/a/node_modules/b/big` (nested, counted once), `rust/target/CACHEDIR.TAG`
  with a large file, `py/.venv/pyvenv.cfg`, `Library/Developer/Xcode/DerivedData/App-x`,
  and decoys: `work/target/real.dat` without a sentinel and a bare `node_modules`
  without `package.json`. `/api/rebuildable` lists exactly the four groups with the
  fixture's bytes, decoys absent, `root=` narrows, `mode=` changes the bytes, 409
  without a tree.
- `src/rebuildable.rs` unit tests: the rule-text guard and each matcher kind on a
  hand-built tree.
- Page: by hand in the app, light and dark, with a clone fixture and a real home
  folder.

## Principles

- redesign-from-first-principles: clone sharing changes what Physical means; the model, the API, the page label, the scanner-agreement rule, story 9 and the changelog all change with it, not a flag on the side.
- model-the-domain and type-system-discipline: `Shared` replaces `hard_link: bool` instead of adding a second boolean; Rebuildable matchers are an enum, not branches.
- foundational-thinking: the model and API land before the page.
- boundary-discipline: kernel attribute buffers and query strings are parsed at the edge; `groups` is a pure function of the tree.
- separate-before-serializing-shared-state: slices 01 and 02 both touch `src/api.rs`; their hunks are named and kept apart.
- explain-the-number: the scan-speed budget is reported with runs, spread and the limiter.
- encode-lessons-in-structure: "never say safe to delete" and "every rule has a source" are tests, not review notes.
- experience-first: two features finished in every mode and theme, over more features.

## Out of scope

- Any delete, move, rename or cleanup action, and running any owner's command.
- The disk ledger, the folder explainer, scan history and the other research features.
- Snapshot sizes and purgeable space.
- Attributing the shared part of partly changed clones to a group.
- Clone support for the portable scanner or non-APFS volumes.
- Rules that need file contents or a subprocess (Docker's disk image, `simctl`).
