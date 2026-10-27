# Scan speed: tickets

Source: [docs/specs/scan-speed.md](../../specs/scan-speed.md).

| # | Ticket | Blocked by | Files |
|---|---|---|---|
| 01 | [Thread cap and one sort](01-threads-and-sort.md) (done) | none | `src/scan.rs`, `tests/scan.rs`, `examples/scan_bench.rs` |
| 02 | [Close-out](02-close-out.md) (done) | 01 | `README.md`, `CHANGELOG.md`, `AGENTS.md`, this folder |

## Waves

1. 01 alone.
2. 02 alone, after the benchmark against `main`.

## Results

Apple M6 (2 Super, 4 Performance, 6 Efficiency cores), home folder of about a million
items, warm cache, `scan_bench ~ 5`, `main` and the branch alternating, three rounds:

| Round | `main` (12 threads) | Branch (6 threads) |
|---|---|---|
| 1 | 1.25 s | 1.09 s |
| 2 | 1.33 s | 1.20 s |
| 3 | 1.28 s | 1.11 s |

Median 1.28 s to 1.11 s, 13% faster (budget: 10%). System CPU time over a whole
benchmark run (10 scans plus 2 warm-up) fell from about 200 s to 105 s.

Same tree: a small program built against each version dumped every field of every
node for `/System/Library` (459,440 nodes), `/usr` (23,269) and `/Applications`
(292,164); the dumps are byte-identical.

Tests: 87 pass (3 new unit tests for the thread count, 2 new in `tests/scan.rs`).
`a_hard_link_can_reorder_folders` fails when the second sort is skipped, as it
should. Idle CPU suite run alone: green.

## Departures from the plan

- The plan said to parallelise the flatten if it took 0.1 to 0.2 s. It takes 0.04 s,
  so 01 only removes the redundant second sort.
- Both tickets touch few files with no parallel work, so they are built in the
  integration branch directly instead of in slice worktrees.
