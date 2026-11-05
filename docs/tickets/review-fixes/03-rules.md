# 03: Rebuildable rules and Time Machine disks

**What to build:** spec Solutions 3 and 4. Gradle `build` gets the generic-name
caution; a folder that matches several sibling rules takes the most specific one
(CMake before JavaScript, and the same check for every other `build`, `dist` and `out`
pair); sentinels count only as the kind of entry the rule expects; the Ollama caution
covers models made with `ollama create`; the Time Machine check uses Time Machine's own
markers.

**Blocked by:** none.

**Status:** todo

## Acceptance

- [ ] Gradle `build` carries a caution; test in `tests/rebuildable.rs`.
- [ ] `build` beside both `CMakeLists.txt` and `package.json` lands in the CMake group
      with its caution; test.
- [ ] A directory named `CACHEDIR.TAG` (or any sentinel name) does not make a match;
      test.
- [ ] The Ollama caution mentions models made with `ollama create`; the rule-text test
      still keeps "safe", "delete" and "remove" out.
- [ ] A drive with `project.previous` at its root is listed; a Time Machine disk is
      still hidden; unit test in `src/volumes.rs`.
- [ ] `groups` stays under 50 ms on the existing ignored timing test.
- [ ] `cargo fmt --check`, both clippy runs, and `timeout 120 cargo test --lib --test
      rebuildable` green.
