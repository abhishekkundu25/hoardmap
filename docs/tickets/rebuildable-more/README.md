# More Rebuildable rules: tickets

Source: [docs/specs/rebuildable-more.md](../../specs/rebuildable-more.md).

| # | Ticket | Blocked by | Files |
|---|---|---|---|
| 01 | Rules, caution, cache tag, page and docs | none | `src/rebuildable.rs`, `src/api.rs` (`fn rebuildable` only), `tests/rebuildable.rs`, `web/*`, `README.md`, `CHANGELOG.md`, `docs/specs/hoardmap.md` |

One slice: every file belongs to this change, so nothing runs beside it.

## Status

| # | Status |
|---|---|
| 01 | done: 68 rules (54 new), 18 with a caution; `groups` takes 7.2 ms on a 1,024,588-entry home folder (budget 50 ms). Maven got a caution for `mvn install` output. Known edges: `.gradle` beside a build file in the home folder itself would match `~/.gradle`; a tagged folder above a named one takes its subtree. |
