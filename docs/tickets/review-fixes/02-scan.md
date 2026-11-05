# 02: Scan reads the folder it listed

**What to build:** spec Solution 2. Open each child folder relative to its parent's
descriptor with `O_NOFOLLOW | O_DIRECTORY` in the bulk scanner, and check `(dev, ino)`
against the listing in the portable scanner, so a folder swapped for a symlink is
skipped and listed as an error. Keep raw name bytes for traversal and `Tree::path`
where they differ from the display name. Check Stop between batches inside a folder.

**Blocked by:** none.

**Status:** done

## Acceptance

- [ ] A folder replaced by a symlink between listing and reading is not followed, by
      either scanner, and appears in the errors; test in `tests/scan.rs` (a hook or a
      unit test may stand in for the race).
- [ ] A non-UTF-8 folder name is read, its subtree counted, and `Tree::path` returns
      the real path; test skips with a message on file systems that reject such names.
- [ ] Stop ends a scan inside one folder of 200,000 files promptly; test in
      `tests/scan.rs`.
- [ ] Both scanners still build the same tree on clone-free folders, for any thread
      count (existing tests).
- [ ] `Node` grows only for names that need raw bytes; report the size of `Node`
      before and after.
- [ ] `cargo fmt --check`, both clippy runs, and `timeout 120 cargo test --lib --test
      scan --test treemap` green.
