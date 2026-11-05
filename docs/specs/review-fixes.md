# Review fixes and a collapsible Rebuildable tab

Source: the static review of 0.5.0 (5 reviewers, 24 findings), and the request to
collapse the Rebuildable tab's groups.

## Problem

A static review of the whole app found two high findings and twenty-two medium and
low ones:

- **Server.** A request line or header is read whole before its size is checked, so a
  local process can make the app run out of memory before the token is ever checked.
  Each connection holds a thread for as long as the client likes. `start_scan` touches
  the file system while holding the shared lock. A stopped scan that finishes late
  overwrites the disk totals of the scan that replaced it. The event stream can send
  an older status after a newer snapshot. The command panics on a non-UTF-8 argument.
- **Scan.** The walk opens folders by path and follows a folder that was swapped for a
  symlink mid-scan, possibly onto another disk. Names that are not UTF-8 are converted
  lossily and then joined back into paths, so their subtrees are missed or the wrong
  folder is read. Stop is checked only between folders, so one very large folder
  ignores it.
- **Rebuildable rules.** Gradle's `build` beside `build.gradle` carries no caution,
  though `build` is a generic name a project can use for its own files. The JavaScript
  `build` rule wins over the CMake one, hiding the CMake caution. A folder named
  `CACHEDIR.TAG` counts as the tag file. The Ollama caution leaves out models made
  with `ollama create`.
- **Disks.** A drive with any root entry ending in `.previous` or `.inprogress` is
  hidden from the Disks menu as a Time Machine disk.
- **Page.** Three races let an older response overwrite newer state (old treemap,
  selection moved back, folders collapsed). Children past the first 200, the extension
  rows and the Largest files rows cannot be reached by keyboard. The path field strips
  trailing spaces from folder names.
- **Native app.** Relaunch exits the app even when the restart command failed. Two
  folders opened in quick succession race, and the earlier can win.
- **Release.** A failed `tools/release.sh` leaves `/Volumes/Hoardmap` mounted. Two
  tags released close together can leave the older version in the Homebrew tap.

And the Rebuildable tab lists every group's folders inline, up to 500 per group, so a
home-folder scan buries the group totals under thousands of rows.

## Solution

Fix each finding at its root, in the module that owns it, and collapse the
Rebuildable tab's groups by default.

1. **Server input is bounded before it is buffered.** Reading a request stops at the
   16 KiB limit (per line and in total) and answers 431 or 400, never growing a buffer
   past it. Connections get a read timeout and a cap on live connections; a refused
   connection is closed, and the accept loop never panics. `start_scan` checks the
   folder before taking the lock. A finishing scan updates disk totals only while it
   is still the current scan. The event stream sends its snapshot and later events in
   order. `main` reads arguments with `args_os`.
2. **The walk reads the folder it listed.** Both scanners open each child folder by
   its full path with `O_NOFOLLOW | O_DIRECTORY` and check it against the
   `(dev, ino)` its parent's listing reported, so a folder swapped for a symlink or
   another folder is skipped and recorded as an error. A folder's descriptor closes
   once it is listed, and at most 32 listings run at once. Paths longer than macOS's
   `PATH_MAX` (1024 bytes) fail as they did before. Names keep their
   raw bytes for traversal and for `Tree::path`; the lossy name is display only. Stop
   is checked between batches of entries inside a folder.
3. **Rules say what they cannot know.** Gradle `build` gets the generic-name caution.
   A folder that matches several sibling rules takes the most specific one (CMake
   before JavaScript). Sentinels count only when they are files (or the kind each
   rule names). The Ollama caution covers models made with `ollama create`.
4. **Time Machine disks are recognised by Time Machine's own markers**, not by any
   root name ending in `.previous` or `.inprogress`.
5. **The page ignores stale answers.** Every async load that can be overtaken carries
   a request counter checked before it writes state, including the treemap after a
   tree reset, `reveal()` and the live-refresh catch handlers. The "more" row, the
   extension rows and the Largest files rows are reachable and usable by keyboard. The
   path field keeps trailing spaces.
6. **Rebuildable groups collapse.** Each group shows one row (disclosure control,
   name, owner, size, percent, count) and, when it has one, its caution line, which
   stays visible when collapsed. Expanding shows the why line, the docs address and
   the largest 50 folders, then "Show all N" up to the 500 the API returns. Groups
   start collapsed. The page remembers which groups are open and how many folders each
   shows, across live refreshes and re-renders, for the current root. Clicking the
   group row still highlights its folders in the treemap; the disclosure control is a
   separate button.
7. **The native app keeps its promises.** Relaunch exits only after the restart
   command was started successfully, and reports the failure otherwise. Folder-open
   events are handled in order and the last one wins.
8. **Release tooling cleans up and serialises.** `tools/release.sh` detaches the image
   in its cleanup trap. The tap job runs in one concurrency group and refuses to write
   a version older than the tap's current one. The release workflow's actions move to
   their Node 24 major versions.

## User stories

1. As a user, no other program on my Mac can make Hoardmap use unbounded memory or
   tie up its server by opening connections and sending nothing.
2. As a user scanning a slow or unresponsive drive, Stop and the status line still
   answer.
3. As a user who stops a scan and starts another, the disk bar shows the new disk.
4. As a user watching a scan finish, the status never goes back to "Scanning".
5. As a user with a folder whose name is not UTF-8, the command opens it, the scan
   reads its contents, and Open in Finder opens it.
6. As a user scanning a folder that changes while it is read, the scan never follows
   a swapped-in symlink onto another disk.
7. As a user scanning a folder with millions of files, Stop takes effect within a
   second.
8. As a Gradle user who keeps files in `build`, the group says the name is generic.
9. As a C++ developer with a JavaScript front end, my CMake `build` shows CMake's
   caution about cache settings.
10. As a user, a folder named `CACHEDIR.TAG` never marks its parent as a cache.
11. As an Ollama user, the caution says models I made myself may not come back.
12. As a user with an external drive holding `project.previous`, the drive is in the
    Disks menu.
13. As a user switching scans quickly, the treemap never shows the old scan's map.
14. As a user clicking around while folders load, my last click is the selection.
15. As a user watching a scan finish, folders I opened stay open.
16. As a keyboard user, I can reach every child of a large folder, highlight an
    extension and select a Largest files entry.
17. As a user typing a path that ends in a space, that folder is scanned.
18. As a user scanning my home folder, the Rebuildable tab shows every group on one
    screen, with cautions visible, and I open only the group I care about.
19. As a user, an open group stays open while the scan refreshes the tab.
20. As a keyboard user, I can open and close a group and show all its folders.
21. As a user clicking Relaunch, the app either comes back or tells me it could not.
22. As a user opening folder A then B from Finder, the window shows B.
23. As the maintainer, a failed release run leaves nothing mounted, and the tap never
    goes back a version.

## Implementation decisions

- **Seams.** Server: `read_request` and the accept loop in `src/server.rs`; scan state
  updates in `start_scan` and the scan-finished path. Scan: the bulk lister's open
  call and the portable lister's recursion in `src/scan.rs`; `Node` in `src/tree.rs`
  gains the raw name only where it differs from the display name, so memory does not
  grow for ordinary trees. Rules: `Rule.caution`, `Matcher::matches` and the rule
  order in `src/rebuildable.rs`. Disks: the Time Machine check in `src/volumes.rs`.
  Page: the request counters and the Rebuildable renderer in `web/app.js`. Native:
  the relaunch helper in `src/access.rs` and the open-folder handler in
  `src/bin/hoardmap-app.rs`. Release: `tools/release.sh` and
  `.github/workflows/release.yml`.
- **No API change.** `/api/rebuildable` keeps its shape and its 500-folder cap; the
  collapse and the 50-folder step are page-only.
- **Limits.** The connection cap and the read timeout are constants in `server.rs`,
  set so the page's own polling and event stream never hit them.
- **Errors.** A folder skipped because it changed under the scan is listed in
  `/api/errors` with a plain reason.

## Budgets (pass/fail)

- The idle-CPU test in `tests/resources.rs` stays under 0.1 s of CPU in 2 s.
- `scan_bench` on the home folder does not get slower by more than 5% for either
  scanner, and both scanners still agree.
- `groups` stays under 50 ms on a home folder of about a million entries.
- The tree's memory for a home-folder scan does not grow by more than 1%.

## Testing decisions

- Follow the existing integration tests: `tests/api.rs` drives the debug binary over
  HTTP, `tests/scan.rs` builds fixtures in the temp folder and compares both scanners,
  `tests/rebuildable.rs` checks groups on fixture trees, `tests/cli.rs` drives the
  command with the stub `open`.
- Server: a test sends a 1 MiB line without a newline and gets an error while the
  app's memory stays bounded; a test opens idle connections past the cap and the page
  still answers; a test stops a scan, starts another on a different folder, and checks
  the disk totals.
- Scan: a test with a non-UTF-8 folder name (on file systems that allow one; APFS
  does not, so the test skips there and runs on a disk image or tmpfs where possible);
  a test that Stop ends a scan of a folder with many files promptly.
- Rules: fixture trees for Gradle `build`, CMake plus `package.json`, a `CACHEDIR.TAG`
  folder, and the Ollama caution text.
- Page: no browser test harness exists; the UI slice checks its work in the debug app
  with screenshots of the collapsed and expanded tab, and keyboard-only walkthroughs.

## Principles

- **fix-root-causes:** every finding is fixed where the defect lives (bounded reads
  in `read_request`, request counters at each async write, raw names in the tree), not
  with guards at the symptom.
- **boundary-discipline:** HTTP input and command arguments are validated at the
  server and CLI boundary; internal code trusts the parsed request and path.
- **separate-before-serializing-shared-state:** scan results and disk totals are
  written only by the current scan; the tap job is serialised by a concurrency group.
- **experience-first:** the collapsed tab shows every group and every caution on one
  screen; opening a group is one click or key.

## Out of scope

- Any change to `/api/rebuildable`'s shape or the 500-folder cap.
- New Rebuildable rules beyond the caution and precedence fixes.
- A browser test harness for the page.
- Rewriting the HTTP server on a framework.
- Deletion, moving or renaming of anything (never in scope).
