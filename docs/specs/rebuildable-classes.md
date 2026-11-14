# Rebuildable: three classes, tighter matching, and rules for every Mac

Sources: two reviews of the feature as built (one by the author of this change, one
by gpt-6-astra, 2026-11-14) and
[docs/research/rebuildable-coverage.md](../research/rebuildable-coverage.md). Builds
on [rebuildable-more.md](rebuildable-more.md).

## Problem

1. **One promise for three different things.** The tab listed build output, a 60 GB
   folder of downloaded models and Docker's disk image under one heading, "fills again
   on its own". The total, the ranking and the disk bar's Rebuildable part counted
   them alike; a red sentence was all that told them apart.
2. **Installed files counted as project output.** `dist` or `build` beside any
   `package.json` matched, so the files of installed packages and of applications
   were listed as something a build writes again. On the author's home folder 387 of
   390 "JavaScript dist folders" were of this kind.
3. **Weak evidence taken as strong.** An empty file named `CACHEDIR.TAG` made a
   cache, and in a folder called `target` made it Cargo's, with no caution. A build
   file in the home folder made all of `~/.gradle`, settings included, a project
   cache. Short paths such as `go/pkg/mod` matched anywhere.
4. **A scan of the folder itself lost it.** The root's name is its whole path, so a
   scan of a `.venv` or a `target` did not match the rule that names it.
5. **Developers only.** All 68 rules were developer tools; on a Mac without them the
   tab was empty.
6. **No next step.** The tab showed where the space was and a documentation address,
   not the owner's own command, and not how long a folder had sat unused.
7. **Lists longer than the tab shows.** "Show all" and the map's highlight stopped
   at the 500 folders served, without saying so.

## Solution

1. **Class.** Every rule has one: `Remade` (the tool makes it again as it works, or
   with its usual command from what the project keeps), `Chosen` (a collection the
   user picked, which downloads again when asked) or `Check` (it also holds things
   that do not come back, or is known by a common name alone). A `Check` rule must
   carry a caution. The tab lists the classes apart, each under a line that says what
   "comes back" means there, with its subtotal. Only `Remade` folders make up the
   Rebuildable kind in the disk bar and its filter.
2. **Evidence.**
   - `dist`, `build` and `out` need a project file that explains them; `package.json`
     alone is none. The rule for a bare `build` beside `package.json` is gone.
   - Application bundles are not looked into. Below a `node_modules` no rule took,
     the rules that go by a neighbouring project file do not apply.
   - A cache tag counts from 43 bytes, the length of the signature the convention
     requires. The scan reads no file, so the signature itself is not checked, and
     "Marked as a cache" is a `Check` group.
   - Cargo's `target` needs `.rustc_info.json` beside the tag.
   - A project's `.gradle` is not one that holds a `caches` folder: that is Gradle's
     own home, whose caches the narrower rules take.
   - CMake's build tree is a `build`, `cmake-build-debug` or `cmake-build-release`
     that holds `CMakeCache.txt`. Composer's `vendor` holds `autoload.php` and
     `composer/`. Go's module cache holds `cache/`, the pub cache `hosted/`, and fnm's
     versions are matched at fnm's three documented paths.
   - Precedence goes by evidence: something the tool keeps in the folder or a path
     only it uses, then a generic output name beside a project file, then any folder
     inside a place of caches, then the cache tag.
   - A rule goes by the last name of a folder's path, so the scan root matches too.
3. **Rules for every Mac**, from the coverage research's first tier: each app's
   folder in `Library/Caches`, a sandboxed app's and an app group's `Caches`, each
   user's cache folder under `/private/var/folders`, Chromium's cache folders inside
   `Application Support`, and Unity, Maven, sbt, .NET, SvelteKit, Nuxt, Angular,
   Elixir, Godot, Zig, Unreal, npx, Cargo's git sources, Kotlin/Native, pre-commit,
   Whisper and Keras. They need one new matcher, `PathGlob`: a path ending whose
   names may each hold one `*`. With `*` as the last name it takes every folder in a
   place and ranks below the rules that name one. Nothing under `/System` is matched.
   Apple's three shared cache rules carry one standing caution: Apple's rule binds
   the app, so what an app keeps there against it does not come back.
4. **The owner's command.** A rule may carry `clear`: the command the owner
   documents for the folder, whether it runs in the folder that holds the matched
   one, and the documentation address. Each was read on the owner's page on
   2026-11-14. The page shows it as text to copy. Hoardmap runs nothing.
5. **Last change.** Each listed folder shows the day its subtree last changed, and
   each group the newest among them.
6. **Honest caps.** "Show the largest 500", "and N smaller ones, counted above but
   not listed", and "Highlighting the largest 500 of N".

## Out of scope

- The research's second tier (Lightroom previews, Final Cut Pro generated files,
  macOS installers, Teams, Terraform, .NET `bin`, Hugging Face datasets, TensorFlow
  Datasets, rbenv and pyenv versions, Xcode's documentation cache).
- Adobe's media cache: the research quotes Adobe's page, but the page refused two
  later fetches, so the rule waits until it can be read again.
- Steam and other game launchers: no owner statement could be fetched.
- A named rule inside a folder a broader rule took (a model cache inside a tagged
  `~/.cache`): the outer folder is listed and the inner caution is not shown.
- Reading the tag's signature, or any file's content, during the scan.
- Anchoring every fixed path to a home folder: a copy of a home folder on another
  disk still matches by its path ending.
- Folding small groups into one row, and grouping thousands of `__pycache__` folders
  by project.

## Testing

- Unit tests in `src/rebuildable.rs` for each point above, with a decoy beside each
  match: installed packages and bundles, the tag's length and Cargo's record,
  Gradle's home, the scan root, short paths, Apple's cache folders with named rules
  inside, the sealed system volume, and the new project folders.
- The rule-text test also checks that a `Check` rule has a caution and that no
  command holds "safe", "delete", "remove", `rm -` or `sudo`.
- `tests/rebuildable.rs` checks `class`, `clear` and `mtime` in the answer, and that
  the Rebuildable kind of the disk bar is the `remade` groups alone.
- `groups` on a 1.7 million-entry home folder: 12 ms with 90 rules, as with 68.
