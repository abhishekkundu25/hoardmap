# Rebuildable classes and coverage: tickets

Source: [docs/specs/rebuildable-classes.md](../../specs/rebuildable-classes.md).

| # | Ticket | Blocked by | Files |
|---|---|---|---|
| 01 | Classes, evidence, owner commands, last change and caps | none | `src/rebuildable.rs`, `src/categories.rs`, `src/api.rs` (`fn rebuildable` only), `tests/rebuildable.rs`, `web/*` |
| 02 | `PathGlob` and the first-tier rules of the coverage research | 01 | `src/rebuildable.rs` |
| 03 | Close-out | 02 | `README.md`, `CHANGELOG.md`, `AGENTS.md`, `docs/specs/*`, this folder |

One slice after another: each builds on the rule table the one before it left.

## Status

| # | Status |
|---|---|
| 01 | done. 67 rules after the bare `build` rule went: 12 `chosen`, 15 `check`. 19 owner commands, each found on the owner's page by a fetch on 2026-11-14. |
| 02 | done. 90 rules: 63 `remade`, 12 `chosen`, 15 `check`, 24 with a command. Each new rule's quoted source was fetched again on 2026-11-14 and the quoted words found, except Adobe's, which refused the fetch; that rule is left out. Zig's older `zig-cache` name is left out: it is not on Zig's page. Angular gets no command, since `ng cache clean` runs in the workspace, two folders above the match. |
| 03 | done. |

## What it did on one home folder

1.7 million entries, scanned without Full Disk Access.

| | Before | After |
|---|---|---|
| Groups | 23 | 24 |
| Listed in all | 137 GB | 145 GB |
| Made again by its tool | (not told apart) | 78 GB |
| Yours to download again | (not told apart) | 67 GB |
| Check first | (not told apart) | 10 MB |
| Rebuildable in the disk bar | 137 GB | 78 GB |
| JavaScript `dist` folders | 390 | 3 |
| `node_modules` folders | 108 | 33, with 1.9 GB of them now the npx cache |
| App caches (`Library/Caches`, app by app) | none | 7.8 GB in 139 folders |
| Chromium and Electron caches | none | 0.9 GB in 72 folders |
| `groups` | 13 ms | 12 ms |
