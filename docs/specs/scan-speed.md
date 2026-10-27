# Scan speed: fewer threads, one sort

## Problem

A scan of a home folder (about a million items) on an Apple M6 (2 Super, 4
Performance and 6 Efficiency cores) takes 1.3 s with one scanner thread per logical
CPU, rayon's default. Nearly all of it is system time: threads contend for the file
system, and the scan gets slower past six threads.

| Threads | Bulk scan, median of 3 |
|---|---|
| 2 | 1.77 s |
| 4 | 1.28 s |
| 5 to 6 | 1.10 to 1.17 s |
| 8 | 1.30 s |
| 12 (default) | 1.32 s |
| 24 | 1.30 s |

The flatten after the walk takes 0.04 s of that, about 4%. It sorts each folder's
children twice: by the walk's sizes, then again after hard links lose their bytes.

## Solution

- The walk runs in its own rayon pool sized to the Mac's non-efficiency cores
  (`hw.perflevel*` except the last level), or physical cores where there is one
  level. Elsewhere, the logical CPU count, as before. `HOARDMAP_THREADS=n` overrides.
- The flatten sorts a folder's children a second time only when a hard link changed
  a child's size.
- The flatten stays sequential: at 0.04 s, parallelising it would not pay for the
  complexity (hard-link first occurrence must stay in arena order).

## User stories

1. As a user, a scan finishes sooner, so I see my disk sooner.
2. As a user on battery, a scan burns less CPU, so the fans and battery notice less.
3. As a user running other apps, the scan leaves cores free, so they stay responsive.
4. As a user, the tree is the same as before: same totals, same order, same hard-link
   accounting.
5. As a developer, I can set `HOARDMAP_THREADS` to compare thread counts.
6. As a developer, the benchmark compares the branch with `main` on the same folder.

## Implementation decisions

- `src/scan.rs` only: a pure `pick_threads(levels, physical, logical)` chooses the
  count; `default_threads()` reads the env override and `sysctl`; the walk runs in
  `rayon::ThreadPoolBuilder::new().num_threads(n)`, falling back to the global pool
  if the pool cannot be built. The pool's threads end with the scan.
- `pub fn scan_with_threads(path, progress, method, threads)` lets tests and the
  benchmark pick a count; the other entry points use `default_threads()`.

## Testing decisions

- Unit tests for `pick_threads` (several perflevel layouts, one level, none, clamps).
- `tests/scan.rs`: a fixture scanned with 1 thread and with 8 gives the same
  fingerprint (the existing `fingerprint` helper); a fixture where a hard link
  reorders two folders (`x/` and `z/` share a 100-byte inode, `y/` holds 60 bytes)
  comes out `x, y, z`.
- Existing scan, API and resource suites stay green.

## Budgets (pass/fail)

- Bulk scan of `~` with the default threads: median at least 10% faster than `main`
  on this Mac, measured back to back on a quiet machine.
- `main` and the branch produce the same tree on the same folder (differences only
  from files changing during the scans).
- Idle CPU test (`tests/resources.rs`) unchanged.

## Out of scope

- Parallel flatten, incremental rescans, memory layout of the tree.
- Tuning for Linux or Intel beyond the defaults above.
