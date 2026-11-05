# What Mac users want from a disk-space tool

Research for hoardmap, 2026-10-30. The question: what do Mac users want from a
disk-space tool, and which of their pains does no popular app solve? Every feature
proposed here keeps hoardmap's rule: it never deletes, moves or renames anything. It
helps people find and understand what uses space, then hands off to Finder or to the
owning app's own documented cleanup.

## Method

Sources, by weight:

- **Primary documentation.** Apple Support articles 102154 (Time Machine local
  snapshots), 102624 (Free up storage space), 108809 (iPhone backups). Apple Developer
  Forums threads 796659 and 812408 (both with Apple DTS answers). The FSEvents
  Programming Guide. Local man pages and SDK headers for `getattrlist(2)`,
  `getattrlistbulk(2)`, `tmutil(8)`, `diskutil(8)` and `sys/attr.h`/`sys/stat.h`.
  Vendor docs: DaisyDisk (home page, hidden space, APFS guides), GrandPerspective home
  page, WinDirStat home page, ncdu man page, gdu README, Docker Desktop Mac FAQ,
  `docker system df`, Homebrew manpage, npm, pnpm, pip, uv, Cargo cache blog post,
  Hugging Face cache guide.
- **Issue trackers.** GitHub search across bootandy/dust, Byron/dua-cli, dundee/gdu
  and windirstat/windirstat for clones, APFS, macOS, snapshots and scan comparison:
  about 70 issue and PR titles scanned, 6 read in full with comments (dust #590,
  dust #83, dua #371, WinDirStat #104, WinDirStat #486, plus titles of dust #546,
  gdu #622, dua #399, WinDirStat #448 and #735).
- **Users' own words.** 9 Hacker News threads (about 70 comments filtered by keyword),
  4 Apple Community threads read in full, and 1,094 recent US App Store reviews
  (500 DaisyDisk, 94 GrandPerspective, 500 CleanMyMac) pulled from Apple's public
  review feed and filtered by keyword.
- **Eclectic Light Company** (Howard Oakley): 3 articles on System Data, Finder's
  "on disk" size and APFS free space.
- **Two experiments on this Mac** (macOS 27, APFS), described where used.

**Not reachable: Reddit.** reddit.com refused the search tool and direct fetches, so no
r/mac, r/MacOS or r/applehelp thread was read. Every claim below about "Reddit
users" is therefore absent; the user voice comes from Hacker News, Apple Community,
Apple Developer Forums and App Store reviews instead. Search summaries mention many
Reddit posts about "System Data" of 100 GB or more ([greggant blog](https://blog.greggant.com/posts/2022/04/10/reclaiming-space-from-system-data-in-macos.html)),
but I could not read them.

App Store review quotes cite the app's review page; the individual reviews came from
Apple's feed, for example
`https://itunes.apple.com/us/rss/customerreviews/page=1/id=411643860/sortby=mostrecent/json`
(DaisyDisk; GrandPerspective is id 1111570163, CleanMyMac id 1339170533).

## What hoardmap does today (baseline)

From `docs/specs/hoardmap.md`, `CHANGELOG.md` and the source:

- Directory list, squarified treemap, icicle view, extension list, largest files;
  linked selection; Open in Finder; live preview while scanning; Disks menu; disk bar
  (scanned folder, everything else, free).
- Logical and Physical toggle. Physical is `ATTR_FILE_ALLOCSIZE` from
  `getattrlistbulk` (`src/scan.rs` lines 699 to 701 and 817). Hard links count once.
- Never downloads cloud files; a dataless folder is counted but not opened.
- Without Full Disk Access, folders that would raise a macOS prompt are listed as
  "Not scanned".
- Free space in the disk bar is `statvfs` `f_bavail` (`src/server.rs` line 398 on),
  which leaves out purgeable space, so it can differ from Finder's "Available".

**Finding:** the 0.1.0 changelog and spec user story 9 say the Physical toggle makes
"sparse files and clones show what they really cost on disk". That holds for sparse
files but not for clones. `ATTR_FILE_ALLOCSIZE` reports the full allocation of every
clone (see experiment 1 below), so a folder of clones is overcounted, as in `du`.

### Experiment 1: clones and `du`

One 500 MB random file, then two APFS clones of it with `/bin/cp -c`. The Data
volume's used space rose by 8 KB. `du -sk` reported 1,536,000 KB (three times the
real cost), and `ls -ls` showed 1,024,000 blocks for each copy. Any tool that sums
`st_blocks` or `ATTR_FILE_ALLOCSIZE` overcounts in the same way.

### Experiment 2: what APFS will tell us per file

A small C program called `getattrlist` with `FSOPT_ATTR_CMN_EXTENDED` and asked for
`ATTR_CMNEXT_PRIVATESIZE`, `ATTR_CMNEXT_CLONEID`, `ATTR_CMNEXT_EXT_FLAGS` and
`ATTR_CMNEXT_CLONE_REFCNT` on 64 MB test files:

| File | How made | alloc | private | clone id | may share | shares all | refcnt |
|---|---|---|---|---|---|---|---|
| a | original | 64 MB | 8 MB | 47332903 | yes | no | 1 |
| b | clone of a, then 8 MB overwritten | 64 MB | 8 MB | 47332906 | yes | no | 1 |
| c | plain copy | 64 MB | 0 | 47332905 | yes | yes | 3 |
| d | clone of c | 64 MB | 0 | 47332905 | yes | yes | 3 |
| e | clone of c | 64 MB | 0 | 47332905 | yes | yes | 3 |
| f | plain copy, never cloned | 64 MB | 64 MB | 47332909 | no | no | 1 |

So: full clones share one clone id and a reference count, and can be counted once.
Partial clones (a and b) get different ids; their private size is exact, but the
56 MB they still share cannot be tied to a group per file. The man page defines
private size as "the number of bytes that are not trapped inside a clone or snapshot,
and which would be freed immediately if the file were deleted"
([getattrlist(2)](https://keith.github.io/xcode-man-pages/getattrlist.2.html)).
`getattrlistbulk(2)` accepts `FSOPT_ATTR_CMN_EXTENDED`, so the scanner can read these
in the bulk call it already makes.

## Pain points

### A. The numbers do not add up

**A1. Clones are counted as if they were copies, so totals exceed the disk.**

- Apple DTS engineer Kevin Elliott: "The nature of file cloning on APFS VASTLY
  complicates storage usage calculations" and `du` "does not account for file clone
  double counting, so it will overestimate the size of a directory hierarchy full of
  clones." He has a test tree with "a logical size of ~200 TB ... I only have a 1 TB
  drive" ([Apple Developer Forums 796659](https://developer.apple.com/forums/thread/796659)).
- dust issue: "`uv` (Python) and `bun` (JS) both materialise their global package
  caches into per-project `.venv` / `node_modules` using `clonefile` on macOS. I
  measured a second project's install costing 0 bytes of real disk while `du`/`dust`
  billed it at full size" ([dust #590](https://github.com/bootandy/dust/issues/590),
  open since 2026-08; first reported 2020 in
  [dust #83](https://github.com/bootandy/dust/issues/83)). The maintainer: "if we
  did this its definitely behind a separate flag".
- Show HN "Du, honest": the author tried "baobab, daisydisk, du" and "They all think
  I used 1T+ of my SSD that is only 500GB, because they don't know about APFS clones
  or hardlinks" ([HN 48958921](https://news.ycombinator.com/item?id=48958921)).
  Note this contradicts DaisyDisk's own claim below; I could not test DaisyDisk.
- Finder's Get Info "on disk" size "isn't reduced to allow for storage blocks
  currently allocated to the data of more than one file"; in one test Finder was off
  by 10.48 GB ([Eclectic Light, 2026-09-05](https://eclecticlight.co/2026/09/05/when-the-finders-size-on-disk-is-misleading/)).
- Windows users ask for the same fairness for hard links: "divide their physical
  size by the number of hardlinks" ([WinDirStat #448](https://github.com/windirstat/windirstat/issues/448)).

**A2. Deleting files does not give the space back.**

- "trashing large files doesn't free up extra space": after deleting 142 GB, 5 GB
  came back. Cause: about 50 GB of Carbon Copy Cloner snapshots that DaisyDisk could
  not purge ([Apple Community 250490791](https://discussions.apple.com/thread/250490791)).
- "hourly Time Machine local snapshots holding on to very large deleted video files";
  the thread closed unresolved ([Apple Community 255987479](https://discussions.apple.com/thread/255987479)).
- DaisyDisk review (5 stars, 2022-05-23): "I deleted 16GB here and 15GB there, but my
  used space does not decline at all. Not sure why"
  ([DaisyDisk reviews](https://apps.apple.com/us/app/daisydisk/id411643860?see-all=reviews)).
- Apple: Time Machine keeps hourly snapshots for 24 hours, and "Your Mac counts the
  space used by snapshots as available storage"
  ([Apple 102154](https://support.apple.com/en-us/102154)). So "available" and "free"
  disagree by design.
- Snapshots have no single size: "Deleting one snapshot's space depends on whether
  others are deleted too", and large deletions are freed later by APFS's Reaper
  ([Eclectic Light, 2022-12-30](https://eclecticlight.co/2022/12/30/free-space-on-an-apfs-volume-is-an-illusion/)).

**A3. "System Data" is a black box.**

- Apple: System Data is "a general category that measures the storage space used by
  all Apple and third-party files" ([Apple 102624](https://support.apple.com/en-us/102624)).
  An Apple Community answer quoting Apple says its contents can't be managed directly
  ([Apple Community 255740160](https://discussions.apple.com/thread/255740160), where
  System Data used "over 800GB" of a 1 TB drive; unresolved).
- "an etcetera, a ragbag", "little more than a guesstimate"; readers call Storage
  settings "next to useless" because it groups by type, not location
  ([Eclectic Light, 2025-02-11](https://eclecticlight.co/2025/02/11/what-is-system-data-in-storage-settings/)).
- Ask HN: "in Storage under System Information, it shows that 'Other' takes up 350GB
  of data" ([HN 31974984](https://news.ycombinator.com/item?id=31974984)).
- App Store: "I had 330 GB of memory in 'system data' and chatgpt, google, nor other
  apps I tried helped" (DaisyDisk, 2026-01-06); "my settings tab was showing 216gb but
  wouldn't tell me what was there" (DaisyDisk, 2025-05-20); "99% 'Other' !?!"
  (GrandPerspective, 2016-07-26)
  ([DaisyDisk reviews](https://apps.apple.com/us/app/daisydisk/id411643860?see-all=reviews),
  [GrandPerspective reviews](https://apps.apple.com/us/app/grandperspective/id1111570163?see-all=reviews)).
- What fills it, from users: Podcasts app "100GB" (DaisyDisk, 2026-05-25), iCloud
  "CloudDocs session data ... 200 GB" (DaisyDisk, 2024-07-07), ON1 Photo RAW cache
  "250GB" in `/Library` (DaisyDisk, 2025-08-13), CloudKit cache of deleted videos
  (Apple Community 255987479), a corrupt icon store visible only as root (Apple Dev
  Forums 796659).
- Classification depends on permissions: "data can shift into other categories simply
  because that user has limited access to that data" (Apple DTS,
  [796659](https://developer.apple.com/forums/thread/796659)).

**A4. Logical size, on-disk size, cloud and sparse files confuse.**

- Docker: the Advanced settings show "the actual space the disk image is consuming.
  Note that other tools might display space usage of the file in terms of the maximum
  file size" ([Docker Desktop Mac FAQ](https://docs.docker.com/desktop/troubleshoot-and-support/faqs/macfaqs/)).
- CLI tools had to learn not to download cloud placeholders: "fix: don't materialize
  dataless files when scanning on macOS" ([gdu #622](https://github.com/dundee/gdu/pull/622)).
- Optimize Storage keeps files "available on demand"
  ([Apple guide](https://support.apple.com/guide/mac-help/optimize-storage-space-sysp4ee93ca4/mac),
  page summary only; the page returned 429 on a direct fetch).

### B. "What is this, and can it go?"

**B1. No explanation of what a folder is or whether it is safe to clear.**

- "You can DELETE things fast... but I wanted the program to show me what wasn't
  needed" (DaisyDisk, 2 stars, 2025-11-08).
- "asked AI (Copilot) if it was save to delete those files" (DaisyDisk, 2026-06-12).
- "Only catch you kinda have to figure out what to keep or not" (DaisyDisk,
  2022-01-23).
- "There are a lot of deep scans that I do not understand. I would like explainations
  as to what should and should not be kept" (CleanMyMac, 4 stars, 2026-05-12)
  ([CleanMyMac reviews](https://apps.apple.com/us/app/cleanmymac/id1339170533?see-all=reviews)).
- "I'd like to understand which of these folders are safe to clean up, and what the
  potential side effects might be" about 90 GB under `~/Library/Developer`; Apple DTS
  answers folder by folder, and for CoreSimulator says not to remove it by hand but to
  use Xcode or `simctl` ([Apple Developer Forums 812408](https://developer.apple.com/forums/thread/812408)).
- An HN user who freed 40 GB had to know folder names like
  `/private/var/db/systemstats` and `com.apple.coresymbolicationd`
  ([HN 33894358](https://news.ycombinator.com/item?id=33894358)).

**B2. Leftovers from apps that are gone.**

- "~200 GB of space on my Mac from orphaned files that were left over after
  uninstalling apps and other development environments" (GrandPerspective, 2025-07-05).
- "90 gb of data from apps that didn't uninstall correctly" (DaisyDisk, 2026-03-21).
- "21 GB of files associated with Outlook in there after deleting Outlook" (DaisyDisk,
  2026-06-12).

**B3. Developer and AI caches are scattered.**

- "Model files and caches (Ollama, Hugging Face, Whisper, etc.) grow silently and are
  spread across different locations" ([Show HN Room Service, 47523979](https://news.ycombinator.com/item?id=47523979)).
- "frustrated by the lack of visibility and mysterious nature of the Storage overview
  especially when it comes to Developer and System Data"
  ([Show HN Ballast, 50271417](https://news.ycombinator.com/item?id=50271417)).
- "40GB worth of old iOS simulator cache files" (DaisyDisk, 2026-06-06); "30 plus gB
  of unnecessary xcode data" (CleanMyMac, 2026-09-11).
- macOS's own Developer category: "I click 'Delete' and then nothing happens"
  ([Apple Community 253810545](https://discussions.apple.com/thread/253810545)).

**B4. Opaque containers named by hashes and UUIDs.** iPhone backups live under
`~/Library/Application Support/MobileSync/Backup/`
([Apple 108809](https://support.apple.com/en-us/108809)) in hash-named folders;
simulators live in UUID-named folders under CoreSimulator; Docker keeps everything in
one `Docker.raw`. A user "thought it was being eaten up by sample libraries, where in
fact it was being eatn up by old backups of huge projects. I never would have found
them" (DaisyDisk, 2023-01-19).

### C. Over time

**C1. "Where did my space go since last week?"**

- "I often find myself wondering where exactly some free space I remember I had a few
  days ago suddenly went ... it would be great if WinDirStat is able to ... show ...
  differences/changes relative to a past scan" ([WinDirStat #104](https://github.com/windirstat/windirstat/issues/104),
  open since 2024). A PR adding a Changes view was closed unmerged
  ([WinDirStat #486](https://github.com/windirstat/windirstat/pull/486)).
- "Would love to open another instance of DaisyDisk to compare sizes" (DaisyDisk,
  2022-12-04).
- ncdu users value export and later import ([HN 27949848](https://news.ycombinator.com/item?id=27949848)),
  but no listed tool diffs two scans.

**C2. "What haven't I touched in years?"** "It will not sort duplicate files or
analyze old unnecessary files" (DaisyDisk, 2 stars, 2025-11-10). GrandPerspective
answers part of this by colouring by creation, modification or access time
([App Store listing](https://apps.apple.com/us/app/grandperspective/id1111570163)).

### D. Trust and safety

- CleanMyMac, 1 star, 2026-07-26: "Ran it yesterday ... NOT A SINGLE PHOTO on my
  laptop!!" (cause not established).
- Deleting in OmniDiskSweeper is permanent: "There's no going back"
  ([MacUpdate listing](https://omnidisksweeper.macupdate.com)).
- dust triggers macOS privacy prompts "even when filtering out the /Users folder"
  ([dust #546](https://github.com/bootandy/dust/issues/546), title only). The Mac
  App Store DaisyDisk cannot scan as administrator (DaisyDisk review, 2023-11-16).

These support hoardmap's no-delete stance and its Full Disk Access handling.

## What each tool does for each pain

Legend: **Yes** solves it, **Part** partly, **No** does not, **?** could not verify.
A1 clones, A2 deleted but not freed, A3 hidden or System Data, A4 sparse and cloud,
B1 explain folder, B2 app leftovers, B3 developer caches, B4 opaque containers,
C1 growth, C2 age.

| Tool | A1 | A2 | A3 | A4 | B1 | B2 | B3 | B4 | C1 | C2 |
|---|---|---|---|---|---|---|---|---|---|---|
| macOS Storage settings | Part (1) | No (2) | Part (3) | ? | Part (4) | No | Part (5) | ? | No | ? |
| DaisyDisk | Yes? (6) | Part (7) | Part (8) | ? | No (9) | No | No | No | No | No |
| GrandPerspective | ? | No | No | ? | No | No | No | No | Part (10) | Yes (11) |
| OmniDiskSweeper | ? | No | No (12) | ? | No | No | No | No | No | No |
| Disk Inventory X | ? | No | No | ? | No | No | No | No | No | No |
| CleanMyMac | ? | Part (13) | ? | ? | Part (14) | ? (15) | Part (16) | ? | No | ? |
| ncdu | No | No | No | Part (17) | No | No | No | No | No (18) | Part |
| dust | No (19) | No | No | Part | No | No | No | No | No | Part |
| dua | Part (20) | No | No | Part | No | No | Part (21) | No | No | No |
| gdu | ? | No | No | Part (22) | No | No | No | No | No (18) | Part |
| WinDirStat | n/a | n/a | Part (23) | Part | No | No | No | No | No (24) | Part |
| WizTree | n/a | n/a | ? | ? | No | No | No | No | ? (25) | Part |
| hoardmap 0.4 | No (26) | No | Part (27) | Part (28) | No | No | No | No | No | Part (29) |

Notes:

1. Apple's calculator "tries to account for these, but I wouldn't assume it's
   perfect" ([796659](https://developer.apple.com/forums/thread/796659)).
2. Counts snapshots as available ([102154](https://support.apple.com/en-us/102154)).
3. Shows the category; contents can't be managed ([255740160](https://discussions.apple.com/thread/255740160)).
4. Explains categories, not folders.
5. A Developer category for Xcode; Delete sometimes does nothing ([253810545](https://discussions.apple.com/thread/253810545)).
6. DaisyDisk says "only the first occurrence is counted, and the rest of its clones
   are assigned a virtual size of 0 bytes" ([DaisyDisk APFS guide](https://daisydiskapp.com/guide/apfs)).
   Partial clones are not addressed, and a Show HN author says DaisyDisk overcounted
   (A1). Not tested.
7. Shows purgeable space and snapshots, and deletes snapshots outside the App Store
   build ([DaisyDisk snapshots guide](https://daisydiskapp.com/guide/snapshots)); no
   check that a deletion freed space.
8. "Hidden space" split into purgeable, other volumes and "still hidden", the last
   "the ultimate remainder of unaccounted space" ([DaisyDisk hidden space](https://daisydiskapp.com/guide/hidden-space)).
9. Names only; the home page mentions a guide for what is safe to delete
   ([daisydiskapp.com](https://daisydiskapp.com/)).
10. Rescan to compare before and after, in separate views ([GrandPerspective](https://grandperspectiv.sourceforge.net/)).
11. Colour by creation, modification or last access time (App Store listing).
12. "cannot open certain categories, such as 'System' and 'Other'" (third-party
    listing summary, [alternativeto](https://alternativeto.net/software/omnidisksweeper/about); not verified with Omni).
13. Clears purgeable space through its Performance module ([MacPaw support](https://macpaw.com/support/cleanmymac/knowledgebase/space-lens-results)).
14. Decides for the user and deletes; no reasoning shown per item.
15. CleanMyMac's uninstaller is widely said to remove leftovers; not verified.
16. A review credits it with removing Xcode data.
17. Disk usage vs apparent size; hard links shown with shared and unique columns
    ([ncdu man page](https://www.mankier.com/1/ncdu)).
18. Export and import to JSON only ([ncdu](https://www.mankier.com/1/ncdu), [gdu README](https://github.com/dundee/gdu)).
19. [dust #590](https://github.com/bootandy/dust/issues/590) open.
20. Full clones counted once, merged 2026-08 ([dua #371](https://github.com/Byron/dua-cli/pull/371)).
21. "Git-aware clean mode", which deletes ([dua #399](https://github.com/Byron/dua-cli/pull/399), title only).
22. Does not download dataless files ([gdu #622](https://github.com/dundee/gdu/pull/622)).
23. Logical and physical sizes ([windirstat.net](https://windirstat.net/)); classic
    WinDirStat also shows `<Unknown>` and free space items (from memory, not re-verified).
24. [#104](https://github.com/windirstat/windirstat/issues/104) open, [#486](https://github.com/windirstat/windirstat/pull/486) closed.
25. A release note summary says CSV can be re-imported "for comparisons or tracking"
    ([Neowin](https://www.neowin.net/software/wiztree-432/)); no diff view confirmed.
26. Hard links yes, clones no (experiment 1).
27. Disk bar and "Not scanned" list; no breakdown of the rest.
28. Logical and Physical toggle; dataless folders not opened.
29. Last change column and sorting; no colour or filter by age.

## The 10 candidate features

Ordered by value for effort. Each keeps the current look: the top bar, list, treemap
and icicle, and the bottom tabs (Extensions, Largest files). None adds a delete,
move or rename action; where an owner has a cleanup command, hoardmap shows the text
and a link to the owner's docs and never runs it.

### 1. True cost: clone-aware sizes

- **Pain:** A1 and part of A2. Totals exceed the disk; deleting a "big" clone frees
  nothing. hoardmap itself overcounts today (experiment 1), and its changelog
  promises otherwise.
- **Who and how often:** every developer using pnpm, uv, bun or git worktrees
  ([dust #590](https://github.com/bootandy/dust/issues/590),
  [HN 48958921](https://news.ycombinator.com/item?id=48958921)), VM and photo users
  who duplicate large files, and anyone scanning a Time Machine or clone-heavy folder.
  It shows on every scan of such folders, not once.
- **Why others fail:** `st_blocks` cannot tell a clone from a copy (dust #83 thread).
  dust has no fix; dua counts full clones only; DaisyDisk claims first-occurrence
  counting but says nothing about partial clones; Finder ignores sharing.
- **In hoardmap:** the size toggle gains a third state: Logical, On disk, **Unique**
  ("freed if removed"). The list shows a small "clone" marker like the hard-link one,
  with a tooltip: "64 MB on disk, 0 B unique: shares all its data with 2 other files".
  The status line says how much shared data was counted once. The treemap sizes by
  the chosen mode.
- **How to compute:** in the existing `getattrlistbulk` call, add
  `FSOPT_ATTR_CMN_EXTENDED` and request `ATTR_CMNEXT_PRIVATESIZE`,
  `ATTR_CMNEXT_CLONEID`, `ATTR_CMNEXT_EXT_FLAGS` and `ATTR_CMNEXT_CLONE_REFCNT`
  ([getattrlist(2)](https://keith.github.io/xcode-man-pages/getattrlist.2.html),
  [getattrlistbulk(2)](https://keith.github.io/xcode-man-pages/getattrlistbulk.2.html)).
  Unique = private size. On disk: for files with `EF_SHARES_ALL_BLOCKS`, key by
  `(dev, clone id)` like hard links and count the allocation once; for partial clones
  (`EF_MAY_SHARE_BLOCKS` without shares-all) count private size plus a "shared,
  not attributed" remainder of `alloc - private`, shown as its own figure. Check
  `VOL_CAP_FMT_CLONE_MAPPING` per volume and fall back to today's figures elsewhere
  (non-APFS drives, the portable scanner). Also correct the changelog and spec claim.
- **Cost:** M.
- **Risks:** speed: dua's maintainer measured the extra attributes slowing a scan
  from 98 ms to 171 ms ([dua #371](https://github.com/Byron/dua-cli/pull/371));
  hoardmap must measure with `examples/scan_bench` and may make Unique opt-in.
  Accuracy: private size also excludes bytes "trapped inside a ... snapshot", so
  Unique can read low on a Mac with snapshots; the tooltip must say so. The tree
  needs one more `u64` per node. Both scanners must still build the same tree, so the
  portable scanner reports Unique as unknown.

### 2. Where's the rest of the disk: a full ledger

- **Pain:** A3. System Data and "Other" are a black box and the numbers in Storage
  settings, Finder and Disk Utility disagree.
- **Who and how often:** most Mac users who hit "disk almost full"; the dominant
  theme in the App Store reviews read (330 GB, 216 GB, 200 GB, 100 GB of "System
  Data") and in Apple Community threads.
- **Why others fail:** Storage settings shows a category it calls unmanageable.
  DaisyDisk's "hidden space" ends in a "still hidden" remainder. GrandPerspective,
  OmniDiskSweeper and the CLI tools show only what they scanned.
- **In hoardmap:** the existing disk bar gets more segments, each clickable to an
  explanation in the status area: this scan; rest of this volume not scanned;
  folders hoardmap could not read (the existing error list); other volumes in the
  same APFS container (VM swap, Preboot, Recovery, the sealed System); local
  snapshots; purgeable; free. Hover gives exact bytes and the source of each number.
- **How to compute:** `diskutil apfs list -plist` gives each volume's role and
  "Capacity Consumed" without root ([diskutil(8)](https://keith.github.io/xcode-man-pages/diskutil.8.html));
  `tmutil listlocalsnapshots /` and `diskutil apfs listSnapshots -plist` list
  snapshots ([tmutil(8)](https://keith.github.io/xcode-man-pages/tmutil.8.html));
  purgeable is roughly `volumeAvailableCapacityForImportantUsage` minus `f_bavail`
  ([Apple docs](https://developer.apple.com/documentation/foundation/urlresourcekey/volumeavailablecapacityforimportantusagekey));
  "not scanned" is the Data volume's consumed space minus this scan's unique size
  (feature 1). Also show Finder's "available" next to the true free figure, since
  hoardmap uses `f_bavail` today.
- **Cost:** M.
- **Risks:** purgeable is computed by macOS in the background and lags (DaisyDisk
  notes "a few seconds to a few minutes"); the remainder is an estimate and must be
  labelled one; `diskutil` is a subprocess, so run it once per scan, off the request
  thread, to keep idle CPU at zero. The plist keys should be checked on macOS 11
  (the oldest supported release); not verified.

### 3. Deleted but not freed: explain the gap after a rescan

- **Pain:** A2. "I deleted 16GB ... my used space does not decline at all."
- **Who and how often:** anyone with Time Machine, Carbon Copy Cloner or another
  snapshot tool, right after a cleanup: exactly when they open a disk tool.
- **Why others fail:** DaisyDisk shows snapshots (and deletes them); no tool
  compares what the user removed with what the volume got back.
- **In hoardmap:** after Rescan, if the scanned size fell by more than free space
  rose, a line under the top bar: "Files here shrank by 16.0 GB; free space rose
  by 2.1 GB. Likely holding the rest: 3 local snapshots (Time Machine, oldest
  yesterday), Trash." Clicking lists the snapshots with date and owner, worked out
  from the snapshot name (`com.apple.TimeMachine.*`, `com.bombich.ccc.*`,
  `com.apple.os.update-*`), with a link to the owner's settings or documentation and
  the owner's command shown as text (for example `tmutil thinlocalsnapshots`). It
  also notes that APFS frees large files in the background.
- **How to compute:** keep the previous scan's totals and the volume's free and
  available figures in memory (and in the file of feature 5); read snapshots as in
  feature 2. Apple says snapshots are kept 24 hours and count as available
  ([102154](https://support.apple.com/en-us/102154)).
- **Cost:** M (S once features 2 and 5 exist).
- **Risks:** neither `tmutil` nor `diskutil` reports a size per snapshot (checked on
  this Mac), and snapshots have no single size (Eclectic Light), so hoardmap names
  suspects, not amounts. Reading `~/.Trash` may need Full Disk Access (not verified).

### 4. What is this? An explainer for known folders

- **Pain:** B1. Users ask an AI or a forum whether a folder can go.
- **Who and how often:** most people who drill into `~/Library`; every developer
  (Apple Developer Forums 812408). Asked on almost every cleanup.
- **Why others fail:** the Mac tools show names only. CleanMyMac decides and deletes
  without reasons. Storage settings explains categories, not folders. None links to
  the owner's own cleanup.
- **In hoardmap:** a small "i" marker on recognised rows in the list and a short card
  in the status area for the selection: what it is, which app owns it, whether it
  comes back on its own, and how the owner says to clear it, with a link. Examples:
  DerivedData ("rebuilt on next build", per the accepted answer in 812408),
  CoreSimulator ("manage through Xcode or `simctl`, not by hand", Apple DTS),
  MobileSync backups ("Finder or Apple Devices > Manage Backups", [Apple 108809](https://support.apple.com/en-us/108809)),
  Docker.raw ("Docker Desktop > Settings > Resources", [Docker FAQ](https://docs.docker.com/desktop/troubleshoot-and-support/faqs/macfaqs/)),
  Homebrew cache (`brew cleanup`, [manpage](https://docs.brew.sh/Manpage)),
  npm (`npm cache verify`, [docs](https://docs.npmjs.com/cli/v10/commands/npm-cache)),
  pnpm (`pnpm store prune`, [docs](https://pnpm.io/cli/store)),
  pip (`pip cache purge`, [docs](https://pip.pypa.io/en/stable/cli/pip_cache/)),
  uv (`uv cache prune`, [docs](https://docs.astral.sh/uv/concepts/cache/)),
  Cargo (automatic cache cleaning, [Rust blog](https://blog.rust-lang.org/2023/12/11/cargo-cache-cleaning/)),
  Hugging Face ([cache guide](https://huggingface.co/docs/huggingface_hub/how-to-cache)).
  The card never says "safe to delete"; it says what the owner documents.
- **How to compute:** a catalog compiled into the binary: path patterns relative to
  the home folder or the volume, plus name, owner, regrows yes or no, doc URL and
  command text. Match on the finished tree (no extra file system access). Serve the
  match with `GET /api/node/{id}`.
- **Cost:** M to build, ongoing to maintain.
- **Risks:** stale or wrong advice; every entry needs a source link and a date.
  Paths that differ by version (uv's macOS cache moved, per its docs; pnpm's macOS
  store path is not documented) must be matched loosely or resolved by the user's
  own `uv cache dir` style command, shown, not run.

### 5. What grew: compare with the last scan

- **Pain:** C1.
- **Who and how often:** anyone whose free space drops without a clear cause; an
  open request on WinDirStat since 2024.
- **Why others fail:** ncdu, gdu and WizTree export and import, GrandPerspective can
  rescan into a second view, but none shows a per-folder change. WinDirStat's attempt
  was not merged.
- **In hoardmap:** when an earlier scan of the same folder exists, the list gains a
  "Change" column (+4.2 GB, -300 MB) sortable like the others, the status line says
  "since Tuesday 14:02", and the treemap gets a "Growth" colour mode (grew warm,
  shrank cool, new outlined). A "Changes" tab beside Largest files lists the top
  growers and new large files.
- **How to compute:** when a scan finishes, write a compact summary to
  `~/Library/Application Support/Hoardmap/`: per directory path, unique and on-disk
  bytes and file count, for directories over a size floor; keep a few per root. Later,
  FSEvents can list the folders changed since the stored event ID, using
  `FSEventStreamCreateRelativeToDevice` with `sinceWhen` and the volume UUID
  ([FSEvents guide](https://developer.apple.com/library/archive/documentation/Darwin/Conceptual/FSEvents_ProgGuide/UsingtheFSEventsFramework/UsingtheFSEventsFramework.html)),
  which also opens a path to faster rescans.
- **Cost:** M (L with FSEvents rescans).
- **Risks:** this changes the spec, which lists "Saving or loading scans" as out of
  scope. The saved file holds folder names, so it must stay local, be easy to clear,
  and never leave the Mac. Renames show as one removal and one addition. FSEvents
  history is "advisory", is discarded when another OS modifies the disk, and IDs
  break on restore (FSEvents guide), so the diff must stand on its own without it.

### 6. Rebuildable: caches across all your projects

- **Pain:** B3.
- **Who and how often:** developers, and now anyone running local AI models; new
  tools appear every few months to fill this gap (Room Service, Ballast, "Du,
  honest" on HN in 2026).
- **Why others fail:** Storage settings covers Xcode only; general analysers show
  each `node_modules` separately among thousands of folders; dedicated cleaners
  (dua's clean mode and others) delete.
- **In hoardmap:** a "Rebuildable" tab next to Extensions and Largest files, grouped
  like the extension list: node_modules, Rust `target`, Python `.venv`, DerivedData,
  simulators, Gradle and CocoaPods caches, model caches; each group with its total,
  count and its folders largest first. Clicking a group highlights its folders in the
  treemap and dims the rest, as the extension list does today.
- **How to compute:** walk the finished tree once, matching directory names with a
  sentinel among their children (already in the tree, so no extra I/O):
  `node_modules` beside `package.json`, `target` containing `CACHEDIR.TAG`, `.venv`
  containing `pyvenv.cfg`. Sizes should use feature 1, since pnpm, uv and bun share
  data through clones and hard links.
- **Cost:** M.
- **Risks:** false matches (a folder called `target` with real work). Showing a
  sentinel-backed reason per group keeps it honest. Must not imply "delete these".

### 7. Leftovers: data of apps no longer installed

- **Pain:** B2.
- **Who and how often:** long-lived Macs migrated across machines; users report
  21 GB, 90 GB and 200 GB found this way.
- **Why others fail:** analysers do not know which app owns a folder; uninstallers
  only start from an app that is still present. CleanMyMac's leftover handling not
  verified.
- **In hoardmap:** a "By app" view (a tab or a list mode) that groups
  `~/Library/Containers`, `Group Containers`, `Application Support`, `Caches` and
  `Logs` by owning app, with a "not installed" badge and total. Selection opens the
  folder in Finder.
- **How to compute:** bundle IDs from container folder names; check presence with
  `LSCopyApplicationURLsForBundleIdentifier`
  ([Apple docs](https://developer.apple.com/documentation/coreservices/1449290-lscopyapplicationurlsforbundleid))
  or `NSWorkspace.urlForApplication(withBundleIdentifier:)`. Application Support
  folder names are not bundle IDs, so match them only loosely and say so.
- **Cost:** M.
- **Risks:** false "not installed" for apps on an external drive or command-line
  tools; reading other apps' containers needs Full Disk Access (hoardmap already skips
  these without it); the Launch Services call needs the app build, so the debug test
  server would need a stub.

### 8. Look inside opaque containers

- **Pain:** B4.
- **Who and how often:** iPhone owners with old local backups, iOS developers with
  dozens of simulators, Docker Desktop users. Infrequent but large (tens of GB).
- **Why others fail:** every analyser shows the hash or UUID folder name or a single
  64 GB file.
- **In hoardmap:** the explainer card (feature 4) adds detail for the selection:
  "iPhone backup: Alex's iPhone, last backup 2 Apr 2023, encrypted"; "Simulator:
  iPhone 15, iOS 17.2, runtime missing"; "Docker disk: 22 GB used of 64 GB; images
  9 GB, build cache 11 GB reclaimable (from `docker system df`)". The list can show
  the friendly name beside the folder name.
- **How to compute:** read the backup folder's `Info.plist` (keys not verified with
  Apple), the simulator's `device.plist` or `xcrun simctl list devices -j`, and
  `docker system df` ([docs](https://docs.docker.com/reference/cli/docker/system/df/)),
  all read-only, run on demand for the selected item only, with a timeout.
- **Cost:** M.
- **Risks:** external commands may be missing, slow or start a VM; never run them
  during a scan or at idle. The backup folder likely needs Full Disk Access (not
  verified). Plist formats can change between releases.

### 9. Cloud, sparse and compressed: say why sizes differ

- **Pain:** A4.
- **Who and how often:** iCloud Drive users with Optimize Mac Storage, Docker and VM
  users. Common, low-grade confusion.
- **Why others fail:** CLI tools offer an apparent-size flag without explanation;
  Finder shows two numbers with no reason.
- **In hoardmap:** row markers and tooltips: "sparse: 64 GB logical, 2.3 GB on disk";
  "in iCloud only"; "compressed by macOS"; and a status line total such as "iCloud
  Drive: 120 GB in the cloud only, not using this disk".
- **How to compute:** `EF_IS_SPARSE` from the extended flags (feature 1's call),
  `UF_COMPRESSED` and `SF_DATALESS` from `ATTR_CMN_FLAGS` (already read). For a
  dataless folder hoardmap does not open, report "contents in the cloud" without a
  size; File Provider APIs for remote sizes not verified.
- **Cost:** S.
- **Risks:** low. Must not trigger downloads (hoardmap already disables
  materialisation).

### 10. Age lens: old, untouched data

- **Pain:** C2.
- **Who and how often:** people with years of projects and media; moderate.
- **Why others fail:** GrandPerspective colours by date but has no total of "old"
  data; DaisyDisk has nothing; hoardmap has a Last change column only.
- **In hoardmap:** a treemap colour mode "Age" from the latest change (already rolled
  up per folder), and a status line "38 GB not changed in over 2 years" for the
  current root; optional "last opened" in the Largest files tab.
- **How to compute:** mtime is in the tree. "Last opened" from Spotlight's
  `kMDItemLastUsedDate` for the shown files only (not verified how complete it is).
- **Cost:** S for mtime colouring, M with Spotlight.
- **Risks:** the spec puts "filtering by ... age" and "Spotlight metadata" out of
  scope; last-change is not last-use; access times on APFS are not reliable (not
  verified).

## Suggested order

1, 2 and 9 share one scanner change and fix a correctness gap in hoardmap's own
claims; do them first. 4 and 6 need no new file system access. 3 builds on 2 and 5.
5 needs a spec change about saving scans. 7, 8 and 10 come after.

## Could not verify

- Reddit: no thread read; the search tool and fetches were blocked.
- Apple's "Change Storage settings" and "Optimize storage" guide pages (404 and 429);
  the Developer category is described from Apple Community threads.
- Whether DaisyDisk's clone counting handles partial clones, and the Show HN claim
  that it overcounts; DaisyDisk was not tested.
- CleanMyMac's leftover detection; OmniDiskSweeper details come from third-party
  listings; classic WinDirStat's `<Unknown>` item is from memory.
- Per-snapshot sizes: no `tmutil` or `diskutil` command on this Mac reports one; how
  DaisyDisk estimates them is not documented.
- Keys in iPhone backup `Info.plist` and simulator `device.plist`; whether
  `~/.Trash` and MobileSync need Full Disk Access.
- `diskutil apfs list -plist` key names on macOS 11; uv's and pnpm's exact macOS
  default paths; completeness of `kMDItemLastUsedDate`.
- The cost of the extended attributes in hoardmap's scanner (dua's figure is for dua).
