# 02: Rebuildable: rules and API

**What to build:** `src/rebuildable.rs` with the rule table and `groups`, and
`GET /api/rebuildable`. See the spec's "Rebuildable".

**Blocked by:** None.

**Status:** done

## Acceptance

- [x] Rule table with the three matcher kinds; every rule has owner, `why`, an
      `https://` source from a primary source (the owner's docs) and a checked date.
- [x] `groups(tree, root, mode)`: one walk, no descent into a match, groups and
      folders largest first, folders capped at 500 with `more`.
- [x] Unit tests: the text guard (no "safe", "delete" or "remove"; owner, source and
      date present) and each matcher kind on a hand-built tree.
- [x] `/api/rebuildable?root=&mode=` as specified, added only in the hunks the README
      names; 409 without a tree, 400 on a bad root or mode.
- [x] `tests/rebuildable.rs`: the spec's fixture, decoys absent, nested
      `node_modules` once, `root=` narrows, `mode=` changes bytes.
- [x] `cargo fmt --check`, both clippy runs, `timeout 120 cargo test --test
      rebuildable` and `cargo test --lib` green.
