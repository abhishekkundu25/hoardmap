# More Rebuildable rules, caution labels and the cache tag

Source: [docs/research/rebuildable-folders.md](../research/rebuildable-folders.md).
Builds on [true-cost-rebuildable.md](true-cost-rebuildable.md).

## Problem

The Rebuildable tab knows 14 kinds of folder, compiled into the app. Common ones are
missing (Yarn, Bun, Maven, Conda, `.next`, `__pycache__`, Docker's disk image,
downloaded models, Android emulators), and a tool nobody listed never shows up. Some
of the missing folders come back on their own but also hold things that do not
(Docker's image holds containers and volumes; an emulator holds its apps and data),
so they cannot be presented the same way.

## Solution

1. **Rules from the research's lists.** Every "add as rebuildable" entry, and every
   "add with a caution label" entry, each with its owner's documentation URL, checked.
2. **Caution labels.** A rule is either plain rebuildable or carries a caution: one
   sentence naming what in the folder does not come back on its own. The page shows
   the caution under the group's line, marked so it reads as a warning.
3. **The cache tag.** The Cache Directory Tagging convention (`CACHEDIR.TAG`) lets a
   tool mark a folder as a cache. Named rules use it as their sentinel where the tool
   writes it. One last rule, "Marked as a cache", catches any other folder with a
   `CACHEDIR.TAG` inside, with a caution: the tag is only a hint (the spec says so),
   it can be committed to a repository, and a program can tag a folder of its user's
   choosing. Named rules win: a folder matched by one is not in this group.
4. **Left out, with reasons in the research:** Xcode Archives (Apple: keep the archive
   of every build you distribute), project `.yarn/cache` (committed under zero
   installs), iOS DeviceSupport, simulators and VS Code CachedData (no primary
   source), whole tool homes (`~/.m2`, `~/.gradle`; they hold settings and
   credentials beside the cache).

## User stories

1. As a JavaScript developer, Yarn, Bun, `.next`, `.turbo` and `.parcel-cache` appear.
2. As a Java or Android developer, Maven's repository, Gradle's wrapper downloads and
   project `build` folders appear, and the Android SDK and emulators appear with a
   caution.
3. As a Python developer, `__pycache__`, Poetry, pipenv, `.mypy_cache`,
   `.pytest_cache` and `.ruff_cache` appear, and Conda's package cache with a caution.
4. As a Docker user, the disk image appears with a caution that it also holds my
   containers and volumes.
5. As someone running local models, Ollama, LM Studio and PyTorch hub folders appear
   with a caution that I chose them and they are large to download again.
6. As a user of a tool hoardmap does not know, a folder its tool tagged with
   `CACHEDIR.TAG` appears under "Marked as a cache", with the caution.
7. As a user, generic names (`dist`, `build`, `out`) appear only beside a project file
   that explains them.
8. As a user, a cautioned group can never be mistaken for a plain one.
9. As a maintainer, each rule has an owner, a checked `https://` source and a date, and
   no rule text says "safe", "delete" or "remove" (existing test).

## Implementation decisions

- `src/rebuildable.rs`: `Rule` gains `caution: Option<&'static str>`. A new matcher,
  `AnyWithChild { child }`, matches a folder of any name containing `child`; it is
  used only by the last rule. Rule order is match priority, as today.
- `GET /api/rebuildable` groups gain `caution` (string or null).
- Page: the caution line under the `why` line, prefixed "Caution:" and styled with an
  existing warning colour token.
- `README.md`, `CHANGELOG.md` (Unreleased), `docs/specs/hoardmap.md`: the new rules
  and the caution field.

## Budgets (pass/fail)

- `cargo fmt --check`, both clippy runs, `timeout 300 cargo test` green.
- `groups` on a home folder of about a million entries takes under 50 ms (a unit timing
  print in the slice's report; the match runs once per request, not per scan).
- No delete, move or rename action; no subprocess; no file contents read.

## Testing decisions

- `src/rebuildable.rs` unit tests: the text guard now also covers `caution`; each new
  matcher kind; a named rule wins over "Marked as a cache"; `dist` without a sentinel
  is not matched.
- `tests/rebuildable.rs`: a fixture with a cautioned folder (a Docker-like path), a
  generic tagged folder, `.next` beside `package.json`, a bare `dist`, and checks of
  `caution` in the JSON.

## Out of scope

- A user rule file (JSON); reading tag contents; Xcode Archives and the other "do not
  add" entries.
