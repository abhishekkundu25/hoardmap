# Scan speed: tickets

Source: [docs/specs/scan-speed.md](../../specs/scan-speed.md).

| # | Ticket | Blocked by | Files |
|---|---|---|---|
| 01 | [Thread cap and one sort](01-threads-and-sort.md) | none | `src/scan.rs`, `tests/scan.rs`, `examples/scan_bench.rs` |
| 02 | [Close-out](02-close-out.md) | 01 | `README.md`, `CHANGELOG.md`, `AGENTS.md`, this folder |

## Waves

1. 01 alone.
2. 02 alone, after the benchmark against `main`.

## Departures from the plan

- The plan said to parallelise the flatten if it took 0.1 to 0.2 s. It takes 0.04 s,
  so 01 only removes the redundant second sort.
- Both tickets touch few files with no parallel work, so they are built in the
  integration branch directly instead of in slice worktrees.
