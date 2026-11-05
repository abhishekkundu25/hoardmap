# 03: Page: clone marker and Rebuildable tab

**What to build:** the page side of both features in the existing look. No new size
mode (user decision). See the
spec's "Page".

**Blocked by:** 01, 02.

**Status:** done

## Acceptance

- [x] `#mode` unchanged (Logical, Physical, Physical default); Physical's title says
      shared clone data is counted once.
- [x] Clone marker beside the hard-link marker, reading `shared`; its tooltip says
      what removing the item frees now (`unique`) and that snapshot data is not counted.
- [x] Status line shows the clone figures when above 0.
- [x] Rebuildable tab: group rows with owner, size, percent, count, `why` and source
      link; folder rows; group click highlights in the treemap and Escape clears;
      folder rows select, Enter and double-click open in Finder; follows root and
      mode; fills in during a scan.
- [x] Light and dark, keyboard reachable; no new colours outside the existing tokens.
- [x] `cargo build` and `timeout 120 cargo test --test api --test resources` green;
      screenshots of the app at rest, with a clone fixture, and on the Rebuildable tab.
