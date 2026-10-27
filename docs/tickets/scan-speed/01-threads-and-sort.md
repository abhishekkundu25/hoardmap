# 01: Thread cap and one sort

**What to build:** the walk runs in a rayon pool of `default_threads()` threads, and
the flatten re-sorts a folder's children only when a hard link changed a size. See the
spec's Solution and Implementation decisions.

**Blocked by:** None.

**Status:** open

## Acceptance

- [ ] `pick_threads` unit tests: M-series layouts, one level, no sysctl, clamped to
      the logical count and to at least 1.
- [ ] `HOARDMAP_THREADS` overrides the count; `scan_bench` prints the count it used.
- [ ] `tests/scan.rs`: `one_thread_and_many_build_the_same_tree`.
- [ ] `tests/scan.rs`: `a_hard_link_can_reorder_folders` (`x, y, z`).
- [ ] `cargo fmt --check`, both clippy runs, `timeout 120 cargo test` green.
