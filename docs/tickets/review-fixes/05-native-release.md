# 05: Native app and release tooling

**What to build:** spec Solutions 7 and 8. Relaunch exits only after the restart
command started, and reports a failure otherwise; folder-open events are handled in
order and the last wins; `tools/release.sh` detaches the image in its cleanup trap;
the tap job is serialised and refuses to go back a version; the release workflow's
actions move to their Node 24 major versions.

**Blocked by:** none.

**Status:** todo

## Acceptance

- [ ] Relaunch with an unwritable log folder does not leave the user with no app: it
      either restarts or stays open and says why.
- [ ] Opening A then B in quick succession shows B; a unit test or a test-server test
      covers the ordering.
- [ ] `tools/release.sh` interrupted after `hdiutil attach` leaves nothing mounted
      (read the trap; do not run a release).
- [ ] `release.yml`: the tap job has a concurrency group and a version check; actions
      use Node 24 majors; `actionlint` or a YAML parse passes if available.
- [ ] `cargo clippy --features app --all-targets -- -D warnings` and `timeout 120 cargo
      test` green.
