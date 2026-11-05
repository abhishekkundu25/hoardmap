# 04: Close-out

**What to build:** docs and full validation.

**Blocked by:** 03.

**Status:** done

## Acceptance

- [x] `docs/specs/hoardmap.md`: story 9 corrected, the clone
      accounting and `/api/rebuildable` documented; the scanner-agreement rule
      restated.
- [x] `AGENTS.md`: `src/rebuildable.rs`, and that the scanners agree on clone-free
      trees only.
- [x] `README.md` and `CHANGELOG.md` (Unreleased): both features, and the corrected
      Physical claim.
- [x] Full validation with nothing else running: fmt, both clippy runs,
      `timeout 120 cargo test`, the idle-CPU test alone, `tools/bundle-app.sh`.
- [x] `scan_bench ~ 5` against `main`, three alternating rounds, results and limiter
      recorded in the README of this folder; within the spec's 10% budget.
