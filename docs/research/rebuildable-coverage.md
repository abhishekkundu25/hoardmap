# Rebuildable coverage beyond developers

Research for hoardmap's Rebuildable tab (`src/rebuildable.rs`), 2026-11-14. The
question: the tab has 68 rules, almost all for developer tools. Which rules would let
it find the rebuildable space of 99% of the people who install a Mac disk-space app?

This builds on [rebuildable-folders.md](rebuildable-folders.md) (2026-10-30) and
[../specs/rebuildable-more.md](../specs/rebuildable-more.md). It does not repeat
their findings. hoardmap never deletes anything; "rebuildable" describes a folder,
backed by its owner's documentation. Proposed `why` and `caution` lines follow the
rule-text test: no "safe", "delete" or "remove", and a caution is one sentence.

## Summary

99% of rebuildable bytes is not reachable. 99% of users seeing their main rebuildable
folders is reachable, and four rules built on Apple's own cache folders do most of it.

- On this Mac (a developer's), the 68 rules already find about 93% of the rebuildable
  space I could identify under the home folder (198 of 213 GiB). On a Mac with no
  developer tools they find nothing, because every rule names a developer tool.
- Apple defines `Library/Caches` as "cached data that can be regenerated as needed"
  and tells apps they "should never rely on the existence of cache files". One rule
  for the children of `Library/Caches`, one for a sandboxed app's
  `Data/Library/Caches`, one for group containers and one for the per-user cache
  folder under `/private/var/folders` cover every browser and most apps on every Mac.
- Electron apps (Slack, Discord, VS Code, Notion, Figma and others) keep Chromium's
  cache inside `Application Support`, which the Apple rules miss. Chromium's own
  folder names (`Code Cache`, `GPUCache`, `Cache` holding `Cache_Data`) match them
  with the existing matcher kinds.
- The largest things a non-developer can get back are not caches and do not belong
  in a plain rule: downloaded podcasts, music and TV, macOS installers, sound
  libraries, game libraries. Each needs a caution or stays out, and for several the
  owner documents no path.
- One new matcher kind is enough: `PathGlob`, a `PathSuffix` whose names may hold
  one `*`. Section "A generic rule for Apple's cache folders" says what it must do.

Tier 1 shortlist (add now). Numbers refer to the Candidates section.

| # | Rule | Match | Needs new matcher |
| --- | --- | --- | --- |
| 1 | App caches (`Library/Caches/<app>`) | `Glob[Library/Caches/*]`, lowest priority | yes |
| 2 | Sandboxed app caches | `Suffix[Data/Library/Caches]` | no |
| 3 | Group container caches | `Glob[Group Containers/*/Library/Caches]` | yes |
| 4 | Per-user cache folder | `Glob[var/folders/*/*/C]` | yes |
| 5 | Chromium and Electron caches in Application Support | `Child{Cache, Cache_Data}`, `Suffix[Code Cache]`, `Suffix[GPUCache]`, Dawn names | no |
| 6 | Adobe media cache | `Suffix[Adobe/Common/Media Cache Files]`, `Suffix[Adobe/Common/Media Cache]` | no |
| 7 | Unity `Library` | `Child{Library, ArtifactDB}` | no |
| 8 | Maven `target` | `Sibling{target, pom.xml}` | no |
| 9 | .NET `obj` | `Child{obj, project.assets.json}` | no |
| 10 | npx cache | `Suffix[.npm/_npx]` | no |
| 11 | Cargo git sources | `Suffix[.cargo/git]` | no |
| 12 | Kotlin/Native `.konan` | `Suffix[.konan]` | no |
| 13 | SvelteKit, Nuxt, Angular caches | `Sibling{.svelte-kit, package.json}`, `Sibling{.nuxt, package.json}`, `Suffix[.angular/cache]` | no |
| 14 | Elixir `_build` and `deps` | `Sibling{_build, mix.exs}`, `Sibling{deps, mix.lock}` | no |
| 15 | sbt `target` | `Sibling{target, build.sbt}` | no |
| 16 | Godot `.godot` | `Sibling{.godot, project.godot}` | no |
| 17 | Zig `.zig-cache` | `Sibling{.zig-cache, build.zig}`, `Sibling{zig-cache, build.zig}` | no |
| 18 | Unreal `DerivedDataCache` | `Suffix[DerivedDataCache]` | no |
| 19 | pre-commit, Whisper and Keras downloads | `Suffix[.cache/pre-commit]`, `Suffix[.cache/whisper]`, `Suffix[.keras/datasets]` | no |

## Method

- **Primary sources.** Each claim about what a folder holds and how it comes back
  cites the owner's documentation, support article or source code. I fetched each
  page with curl, stripped the HTML and searched for the quoted words. Every quote
  below was read on 2026-11-14.
- **Local man page.** `confstr(3)` on this Mac (macOS 27.0, build 26A428) for the
  per-user cache and temp folders.
- **This Mac, read only.** `ls`, `du -sk` and `find` with a depth limit. Nothing was
  opened, changed or cleared, and nothing was installed. The shell has no Full Disk
  Access, so `~/Library/Containers/*/Data`, `~/Library/Mail`, `~/Library/Messages`,
  Chrome's profile folder and eight children of `~/Library/Caches` (Safari, CloudKit,
  HomeKit and others) returned "Operation not permitted".
- **Apps not on this Mac.** Xcode, Steam, Slack, Discord, Teams, Spotify, any Adobe
  app, Final Cut Pro, Logic Pro, Unity and Unreal are not installed. For those, paths
  come from the owner's page only, and the entry says so where the page gives no path.
- **Prevalence.** Secondary sources, named with their year: the Stack Overflow
  Developer Survey 2025 and StatCounter Global Stats for October 2026.
- **Pages I could not fetch.** Mozilla's support site (bot wall), Steam's help site
  (script-rendered), Discord and Signal support (HTTP 403), bundler.io and tox.wiki
  (script-rendered). Claims that depended on them are in "Could not verify".
- **Sizes.** "This Mac" means measured here with `du`. "Review" means an App Store
  review quoted in [what-users-want.md](what-users-want.md). Where neither exists the
  entry says "no figure". I did not take sizes from blog posts.

Matcher shorthand follows the earlier research: `Suffix[a/b]` is `PathSuffix`,
`Child{x, y}` is `ChildSentinel`, `Sibling{x, y}` is `SiblingSentinel`. `Glob[a/*/b]`
is the proposed `PathGlob`.

## Who the users are

No source gives the split of people who install a Mac disk-space app. The evidence
that exists:

- **What they complain about.** In the 1,094 App Store reviews read for
  [what-users-want.md](what-users-want.md), the dominant theme was "System Data"
  (330 GB, 216 GB, 200 GB, 100 GB), followed by app leftovers, Podcasts downloads
  (100 GB), an ON1 Photo RAW cache (250 GB), iCloud session data, simulators (40 GB)
  and Xcode data (30 GB). Most of those reviewers are not developers.
- **Mac share of desktops.** StatCounter, October 2026: "OS X" 9.86% plus "macOS"
  5.81% of desktop page views, 15.67% together
  (<https://gs.statcounter.com/os-market-share/desktop/worldwide>).
- **Browsers.** StatCounter, October 2026, all desktops: Chrome 66.98%, Edge 13.24%,
  Safari 7.83%, Firefox 5.72%, Opera 2.65%, Brave 1.01%
  (<https://gs.statcounter.com/browser-market-share/desktop/worldwide>). Safari on a
  desktop is a Mac, so Safari is about half of Mac page views (7.83 of 15.67) and
  Chromium browsers most of the rest. This is my division of two StatCounter figures,
  not a StatCounter statistic.
- **Developers.** Stack Overflow Developer Survey 2025
  (<https://survey.stackoverflow.co/2025/technology>): macOS is the primary system
  for 32.7% (personal) and 32.9% (professional) of respondents. Tool use among all
  respondents: Docker 71.1%, npm 56.8%, pip 40.9%, Homebrew 25.7%, Vite 25.4%, Yarn
  21.1%, NuGet 18.9%, Webpack 18.4%, Terraform 17.8%, Maven 16.4%, Cargo 14.4%, Gradle
  14.4%, pnpm 13.4%, Podman 11.1%, Composer 11%, Poetry 9%. I left the cloud
  services out of that list. The existing rules have something for every tool on it
  except Terraform and Podman, and they miss the project output of Maven (`target`)
  and .NET (`bin`, `obj`).

Segments, with where each one's rebuildable space sits:

| Segment | Widespread and large | Covered today |
| --- | --- | --- |
| Ordinary users | Browser caches, app caches in `Library/Caches` and containers, Electron app caches, the per-user cache folder, streaming app caches | Nothing |
| Creative users | Adobe media cache, Final Cut Pro render and transcoded media, Lightroom previews, DaVinci Resolve cache, sound libraries, plus everything ordinary users have | Nothing |
| Web and systems developers | `node_modules`, build output, package caches, Docker | Nearly all |
| Apple platform developers | DerivedData, simulators and runtimes, device support, documentation cache | DerivedData, SwiftPM, CocoaPods, Carthage |
| Game developers | Unity `Library`, Unreal `DerivedDataCache` and `Intermediate`, Godot `.godot` | Nothing specific |
| Data and machine learning | Model and dataset caches, virtual environments, conda | Hub models, Ollama, LM Studio, PyTorch hub, venvs, conda `pkgs` |
| Gamers | Steam and other launcher libraries, shader caches | Nothing |

## Candidates

### Tier 1: add now

**1. App caches (`Library/Caches/<app>`).** Owner: Apple defines the folder, each
app fills its own subfolder.

- Paths: `~/Library/Caches/<name>`, `/Library/Caches/<name>`. This is where Chrome,
  Edge, Firefox, Brave, Opera, Spotify, Cypress, Coursier, Lima and most unsandboxed
  apps cache. Chromium documents it: "On Mac OS X and iOS, the user cache dir is
  derived from the profile dir as follows: If Library/Application Support is an
  ancestor of the profile dir, the user cache dir is Library/Caches plus the relative
  path from Application Support to the profile dir", example
  `~/Library/Caches/Google/Chrome/Default`
  (<https://chromium.googlesource.com/chromium/src/+/main/docs/user_data_dir.md>).
  Firefox's source says the same: "At least on OS X, the cache directory (under
  ~/Library/Caches/) is always the 'local' user profile directory"
  (<https://github.com/mozilla-firefox/firefox/blob/main/toolkit/xre/nsXREDirProvider.cpp>).
- Match: `Glob[Library/Caches/*]`, ranked below every named rule so that Homebrew,
  pip, JetBrains and the other 12 existing `Library/Caches/<x>` rules keep their
  folders. Details and exceptions are in the section on the generic rule.
- why: "Apple gives each app this folder for cached data that can be regenerated as needed, and tells apps never to rely on it existing."
- caution: "Apple's rule binds the app, so anything an app keeps here against that rule does not come back, as with Local History in JetBrains IDEs."
- Source: <https://developer.apple.com/library/archive/documentation/FileManagement/Conceptual/FileSystemProgrammingGuide/MacOSXDirectories/MacOSXDirectories.html>:
  "Caches: Contains cached data that can be regenerated as needed. Apps should never
  rely on the existence of cache files."
- Size: 11.0 GiB on this Mac in 156 folders. The existing rules match 3.7 GiB of it
  (Homebrew, Playwright, pip, Deno). The rest is Chrome 1.9 GiB, two Codex folders
  2.8 GiB, a VS Code update download 1.0 GiB, Edge 0.6 GiB and about 140 small folders.
- Owner's way to clear: per app. Chrome: "Cached images and files: Chrome remembers
  parts of pages to help them open faster during your next visit"
  (<https://support.google.com/chrome/answer/2392709>). Edge: "Cached images and
  files: Copies of pages, images, and other media content stored on your device"
  (<https://support.microsoft.com/en-us/microsoft-edge/view-and-delete-browser-history-in-microsoft-edge-00cf7943-a9e1-975a-a33d-ac10ce454ca4>).
  Cypress: `cypress cache clear`. Apple names no user command for the folder as a whole.

**2. Sandboxed app caches.** Owner: Apple, each sandboxed app.

- Path: `~/Library/Containers/<bundle id>/Data/Library/Caches`. Safari, Mail, new
  Teams and every Mac App Store app cache here. Apple: "macOS apps that are sandboxed
  have all their Application Support, Cache, temporary directories and other related
  documents stored within a directory located at a system-defined path"
  (<https://developer.apple.com/library/archive/documentation/FileManagement/Conceptual/FileSystemProgrammingGuide/FileSystemOverview/FileSystemOverview.html>).
- Match: `Suffix[Data/Library/Caches]`, an existing kind. It takes the whole `Caches`
  folder of each container as one row. It must come before rule 1 in the table so
  the walk stops at the container's `Caches` and does not list its children.
- why: "A sandboxed app's own Caches folder, which Apple defines as cached data that can be regenerated as needed."
- caution: the same sentence as rule 1.
- Source: as rule 1, and
  <https://developer.apple.com/documentation/foundation/filemanager/searchpathdirectory/cachesdirectory>:
  "Discardable cache files (Library/Caches)."
- Size: no figure. This Mac has 699 containers and the shell cannot read them.
  hoardmap reads them only with Full Disk Access, so without it these rows are absent.
- Owner's way to clear: Safari documents Develop > Empty Caches: "Empties the network
  cache and other Safari caches"
  (<https://support.apple.com/guide/safari-developer/dev39df999c1/mac>).

**3. Group container caches.** Owner: Apple, app groups.

- Path: `~/Library/Group Containers/<group id>/Library/Caches`. Present on this Mac
  for 15 or more groups (weather, tips, stocks, Docker and others).
- Match: `Glob[Group Containers/*/Library/Caches]`.
- why: "An app group's Caches folder, which Apple defines as cached data that can be regenerated as needed."
- caution: the same sentence as rule 1.
- Source: Apple's definition of `Library/Caches` as in rule 1. I found no Apple page
  that describes the group container's `Library/Caches` in particular.
- Size: no figure (unreadable here).

**4. Per-user cache folder.** Owner: macOS.

- Path: `/private/var/folders/<2 chars>/<hash>/C`, which is `DARWIN_USER_CACHE_DIR`.
  It holds Metal shader caches, clang module caches, icon caches and each app's
  per-user caches. A scan of the home folder never reaches it. A scan of `/` does.
- Match: `Glob[var/folders/*/*/C]`, the whole folder as one row per user.
- why: "macOS gives each user this cache folder for apps and system services, and the caches are created again as needed."
- caution: none.
- Source: `confstr(3)` on macOS 27.0: "_CS_DARWIN_USER_CACHE_DIR Provides the path to
  the user's cache directory. ... a good location for user cache data as it will not
  be automatically cleaned by the system. Files in this location will be removed
  during safe boot." Apple Support: "start up your Mac in safe mode. Your Mac then
  clears certain system caches, which are automatically created again as needed"
  (<https://support.apple.com/en-us/102624>).
- Size: 12 GiB on this Mac, of which 11.1 GiB is one app's Metal shader cache
  (`com.liuliu.draw-things`).
- Owner's way to clear: starting up in safe mode, per the support article.
- Two limits. The support article says "certain system caches" and does not name
  this folder; the man page is what ties the folder to that startup mode, so "created
  again as needed" is the two sources read together. And Apple's term for the mode
  contains a word the rule-text test rejects, so the `why` line cannot name it.

**5. Chromium and Electron caches inside Application Support.** Owner: Chromium,
through Electron.

- Paths: `~/Library/Application Support/<app>/Cache`, `Code Cache`, `GPUCache`,
  `DawnCache`, `DawnWebGPUCache`, `DawnGraphiteCache`, `GraphiteDawnCache`, and the
  same names under `Partitions/<name>/` and profile folders. Seen on this Mac in VS
  Code, Codex, T3 Code, Comfy Desktop and Docker Desktop. Slack, Discord, Notion,
  Figma, Signal and classic Teams are Electron apps and are expected to match, but
  none is installed here.
- Match, all existing kinds: `Child{Cache, Cache_Data}` with `Kind::Dir` (the name
  `Cache` alone is too common), `Suffix[Code Cache]`, `Suffix[GPUCache]`,
  `Suffix[DawnCache]`, `Suffix[DawnWebGPUCache]`, `Suffix[DawnGraphiteCache]`,
  `Suffix[GraphiteDawnCache]`. The last six are name-only, as `__pycache__` is
  today. The names are Chromium's own, and a hand-made folder with one of these
  names is the only false positive.
- why: "Chromium and Electron apps keep their disk cache, code cache and compiled GPU shaders here and fill them again as pages load."
- caution: none.
- Source: <https://www.electronjs.org/docs/latest/api/app>: "sessionData: The
  directory for storing data generated by Session, such as localStorage, cookies,
  disk cache, downloaded dictionaries, network state, DevTools files and compiled GPU
  shaders. By default this points to userData. Chromium may write very large disk
  cache here". The same page names "Chromium's own subdirectories (such as Cache,
  GPUCache, and Local Storage)". <https://www.electronjs.org/docs/latest/api/session>
  places the code cache in "Code Cache under the respective user data folder".
- Size: this Mac, T3 Code `Cache` 531 MiB, VS Code `Cache` 46 MiB. No general figure.
- Owner's way to clear: Electron's `ses.clearCache()`, "Clears the session's HTTP
  cache". Slack: "click Help ... Select Troubleshooting. Click Clear cache and
  restart" (<https://slack.com/help/articles/205138367-Troubleshoot-connection-issues>).
- Leave out `Local Storage`, `IndexedDB`, `Cookies`, `Session Storage` and
  `Service Worker/CacheStorage`. The first four are user state. For the last, see
  "Could not verify".

**6. Adobe media cache.** Owner: Adobe (Premiere Pro, After Effects, Media Encoder).

- Paths: `~/Library/Application Support/Adobe/Common/Media Cache Files` and
  `~/Library/Application Support/Adobe/Common/Media Cache`. Not checked on this Mac.
- Match: `Suffix[Adobe/Common/Media Cache Files]`, `Suffix[Adobe/Common/Media Cache]`.
  A user who moved the cache in Premiere's settings is missed.
- why: "Adobe video apps keep conformed audio, peak files and their database here, and recreate the files whenever the source media requires them."
- caution: none.
- Source: <https://helpx.adobe.com/premiere/desktop/troubleshooting/media-issues/manage-media-cache.html>:
  "These are referred to as media cache files and stored in the Media Cache Files
  folder. ... media cache database files ... are located in the Media Cache folder.
  ... Deleted cache files are recreated whenever the source media requires them."
  Location: "macOS: /Users/<username>/Library/Application Support/Adobe/Common"
  (<https://helpx.adobe.com/premiere/desktop/troubleshooting/media-issues/delete-media-cache-files-manually.html>).
- Size: no figure.
- Owner's way to clear: "Premiere > Settings (macOS) > Media Cache ... select the
  Delete button next to Remove Media Cache Files"
  (<https://helpx.adobe.com/premiere/desktop/troubleshooting/media-issues/clear-media-cache-using-preferences.html>).

**7. Unity `Library`.** Owner: Unity.

- Path: `<project>/Library`.
- Match: `Child{Library, ArtifactDB}` with `Kind::File`. Unity documents the file:
  "The Artifact database (Library\ArtifactDB)"
  (<https://docs.unity3d.com/Manual/asset-database-contents.html>). Do not use
  `Sibling{Library, ProjectSettings}`: a wrong match there would mark a whole
  `~/Library`. Projects from before the current Asset Database have no `ArtifactDB`
  and are missed.
- why: "Unity keeps its imported form of your assets here and regenerates all of it from the Assets and ProjectSettings folders the next time the project opens."
- caution: none.
- Source: <https://docs.unity3d.com/6000.2/Documentation/Manual/ImportingAssets.html>:
  "Unity stores the internal representation of your assets in the Library folder,
  which behaves like a cache folder. ... If your project isn't open, you can safely
  delete the Library folder, because Unity can regenerate all its data from the
  Assets and ProjectSettings folders the next time you launch your project."
- Size: no figure.

**8. Maven `target`.** Owner: Apache Maven.

- Match: `Sibling{target, pom.xml}`. Cargo's `target` keeps its own rule through
  `CACHEDIR.TAG`.
- why: "Maven houses all output of the build here, and the next build writes it again."
- caution: none.
- Source: <https://maven.apache.org/guides/introduction/introduction-to-the-standard-directory-layout.html>:
  "The target directory is used to house all output of the build."
- Owner's way to clear: `mvn clean`.

**9. .NET `obj`.** Owner: Microsoft.

- Match: `Child{obj, project.assets.json}` with `Kind::File`.
- why: "The intermediate output of a .NET build and the project.assets.json that dotnet restore creates; the next build writes both again."
- caution: none.
- Source: <https://learn.microsoft.com/en-us/dotnet/core/tools/dotnet-clean>: "Both
  intermediate (obj) and final output (bin) folders are cleaned."
  <https://learn.microsoft.com/en-us/dotnet/core/tools/dotnet-build>: "Building
  requires the project.assets.json file ... The file is created when dotnet restore
  is executed."
- Owner's way to clear: `dotnet clean`. For `bin`, see rule 25.

**10. npx cache.** Owner: npm.

- Path: `~/.npm/_npx`. The existing npm rule matches only `~/.npm/_cacache`.
- Match: `Suffix[.npm/_npx]`.
- why: "Packages that npx and npm exec installed into the npm cache; npm installs them again the next time the command runs."
- caution: none.
- Source: <https://docs.npmjs.com/cli/v10/commands/npm-exec>: "The requested
  packages are installed to a folder in the npm cache". The page does not name
  `_npx`. The folder name comes from this Mac.
- Size: 1.8 GiB on this Mac.

**11. Cargo git sources.** Owner: Cargo.

- Path: `~/.cargo/git` (`db` and `checkouts`). The existing rule matches only
  `~/.cargo/registry`.
- Match: `Suffix[.cargo/git]`.
- why: "Git repositories Cargo cloned for git dependencies and the commits it checked out; Cargo clones them again when a build needs them."
- caution: none.
- Source: <https://doc.rust-lang.org/cargo/guide/cargo-home.html>: "In theory, you
  can always remove any part of the cache and Cargo will do its best to restore
  sources if a crate needs them either by reextracting an archive or checking out a
  bare repo or by simply redownloading the sources from the web."
- Size: 121 MiB on this Mac.

**12. Kotlin/Native `.konan`.** Owner: JetBrains.

- Match: `Suffix[.konan]`.
- why: "Kotlin/Native downloads its compiler components and caches build results here, and creates the folder again when a build needs it."
- caution: none.
- Source: <https://kotlinlang.org/docs/native-improving-compilation-time.html>:
  "Kotlin/Native downloads the required components and caches some results of its
  work to the $USER_HOME/.konan directory. ... the compiler may have to create the
  ~/.konan directory from scratch for each build."
- Size: no figure.

**13. SvelteKit, Nuxt and Angular caches.** Three rules.

- `Sibling{.svelte-kit, package.json}`. why: "SvelteKit generates these files and regenerates them the next time you run dev or build."
  Source: <https://svelte.dev/docs/kit/project-structure>: "You can ignore its
  contents, and delete them at any time (they will be regenerated when you next dev
  or build)."
- `Sibling{.nuxt, package.json}`. why: "Nuxt generates your Vue application here in development and generates it again on the next run."
  Source: <https://nuxt.com/docs/guide/directory-structure/nuxt>: "Nuxt uses the
  .nuxt/ directory in development to generate your Vue application."
- `Suffix[.angular/cache]`. why: "The Angular CLI stores its build cache results here and fills the cache again on the next build."
  Source: <https://angular.dev/cli/cache>: "By default, .angular/cache is used as a
  base directory to store cache results." Owner's way to clear: `ng cache clean`.

**14. Elixir `_build` and `deps`.** Owner: Mix.

- Match: `Sibling{_build, mix.exs}`, `Sibling{deps, mix.lock}`.
- why (`_build`): "Mix writes the project's build artifacts here, and mix compile writes them again."
- why (`deps`): "mix deps.get fetches unavailable dependencies again from mix.lock."
- caution: none.
- Source: <https://hexdocs.pm/mix/Mix.Tasks.Clean.html>: "This command deletes all
  build artifacts for the current project." <https://hexdocs.pm/mix/Mix.Tasks.Deps.Get.html>:
  "Fetches unavailable and out of date dependencies." Default folder names from
  <https://hexdocs.pm/mix/Mix.html> (`MIX_BUILD_ROOT`, `MIX_DEPS_PATH`).

**15. sbt `target`.** Owner: sbt.

- Match: `Sibling{target, build.sbt}`.
- why: "sbt writes all generated files to the target directory, and the next compile writes them again."
- Source: <https://www.scala-sbt.org/1.x/docs/Command-Line-Reference.html>: "clean:
  Deletes all generated files (the target directory)."

**16. Godot `.godot`.** Owner: Godot.

- Match: `Sibling{.godot, project.godot}`.
- why: "Godot creates this folder of project cache data when the editor opens the project."
- Source: <https://docs.godotengine.org/en/stable/tutorials/best_practices/version_control_systems.html>:
  "There are some files and folders Godot automatically creates when opening a
  project in the editor for the first time. ... .godot/: This folder stores various
  project cache data."

**17. Zig `.zig-cache`.** Owner: Zig.

- Match: `Sibling{.zig-cache, build.zig}` and `Sibling{zig-cache, build.zig}` (the
  name before Zig 0.13, from memory, not on the page).
- why: "Zig's build cache, which makes later builds faster; zig build fills it again."
- Source: <https://ziglang.org/learn/build-system/>: ".zig-cache ... contains files
  that will make subsequent builds faster, but these files are not intended to be
  checked into source-control and this directory can be completely deleted at any
  time with no consequences."

**18. Unreal `DerivedDataCache`.** Owner: Epic Games.

- Path: `<project>/DerivedDataCache`, and the engine's own.
- Match: `Suffix[DerivedDataCache]`, name-only. The name is Unreal's.
- why: "Unreal Engine generates these derived data files when it loads content, and generates them again when they are missing."
- caution: none. The page warns of the cost, not of loss: "Not having cache files
  present for referenced content can increase load times dramatically."
- Source: <https://dev.epicgames.com/documentation/en-us/unreal-engine/unreal-engine-directory-structure>:
  "DerivedDataCache - Contains derived data files generated on-load for referenced
  content."

**19. Small sourced caches under the home folder.** Three rules.

- `Suffix[.cache/pre-commit]`. why: "pre-commit's store of hook repositories; it sets a hook up again the next time the hook runs."
  Source: <https://pre-commit.com/>: "pre-commit by default places its repository
  store in ~/.cache/pre-commit". Owner's way to clear: `pre-commit clean` ("Clean
  out cached pre-commit files"), `pre-commit gc`.
- `Suffix[.cache/whisper]`. why: "Whisper downloads its model files here and downloads them again when a model is missing."
  Source: <https://github.com/openai/whisper/blob/main/whisper/__init__.py>:
  "download_root: path to download the model files; by default, it uses
  '~/.cache/whisper'".
- `Suffix[.keras/datasets]`. why: "Keras downloads a file here when it is not already in the cache."
  Source: <https://keras.io/api/utils/python_utils/>: "Downloads a file from a URL
  if it not already in the cache. By default the file at the url origin is
  downloaded to the cache_dir ~/.keras, placed in the cache_subdir datasets". Never
  match `~/.keras` whole; it also holds `keras.json`.

### Tier 2: add with a caution

**20. Lightroom Classic previews.** Owner: Adobe.

- Path: `<catalog folder>/<catalog name> Previews.lrdata`, by default
  `~/Pictures/Lightroom/Lightroom Catalog Previews.lrdata`.
- Match: `Glob[* Previews.lrdata]`. The same pattern also matches
  `<catalog name> Smart Previews.lrdata`, which is why this is tier 2.
- why: "Lightroom Classic keeps its preview cache here and re-creates previews the next time you need them."
- caution: "Smart Previews share this ending, and Lightroom builds those again only from the original photos."
- Source: <https://helpx.adobe.com/lightroom-classic/kb/optimize-performance-lightroom.html>:
  "the [Catalog name] Previews.lrdata file can grow large ... check the size of this
  file. Delete it if it's too large", and "Every time you discard them, Lightroom
  Classic has to re-create them the next time you need them". Paths from
  <https://helpx.adobe.com/lightroom-classic/kb/preference-file-and-other-file-locations.html>:
  "Preview cache /Users/[username]/Pictures/Lightroom/Lightroom Catalog
  Previews.lrdata; Smart Preview cache ... Lightroom Catalog Smart Previews.lrdata".
  That Smart Previews need the originals is not on either page (my knowledge).
- Size: no figure.

**21. Final Cut Pro generated files.** Owner: Apple.

- Path: inside the library bundle (`<name>.fcpbundle`). Apple's pages name the files
  (render, optimized, proxy) but not the folders. From memory the folders are
  `<event>/Render Files` and `<event>/Transcoded Media`; no Mac with Final Cut Pro
  was available to check.
- Match once the names are confirmed: `Glob[*.fcpbundle/*/Render Files]` for a plain
  rule, `Glob[*.fcpbundle/*/Transcoded Media]` for the cautioned one.
- why (render files): "Final Cut Pro regenerates render files from the original media the next time you open and work on the project."
- why (transcoded media): "Optimized and proxy media that Final Cut Pro creates from the original media."
- caution (transcoded media): "Final Cut Pro creates optimized and proxy media again only when you ask it to, and only from the original media."
- Source: <https://support.apple.com/guide/final-cut-pro/ver68a8c250/mac>: "Original
  media files are not deleted, so you can always regenerate render files from the
  original media." <https://support.apple.com/en-us/119610>: "The next time you open
  and work on your project, Final Cut Pro regenerates render files. If you delete
  optimized and proxy media files, you'll have to regenerate them manually when you
  reopen the library."
- Owner's way to clear: "Choose File > Delete Generated Library Files." Apple shows
  the size under "Cache in the Library Properties window".

**22. macOS installers.** Owner: Apple.

- Path: `/Applications/Install macOS <name>.app`.
- Match: `Glob[Applications/Install macOS *.app]`.
- why: "A macOS installer; the App Store or softwareupdate --fetch-full-installer downloads it again."
- caution: "Apple offers only the versions available for your Mac model, so an older installer may not download again."
- Source: <https://support.apple.com/en-us/102662>: "softwareupdate
  --fetch-full-installer --full-installer-version [number] ... As macOS downloads to
  the Applications folder ... If Terminal says update not found, that version of
  macOS is not available for your Mac."
- Size: not stated on the page. None on this Mac.

**23. Microsoft Teams data.** Owner: Microsoft.

- Paths: `~/Library/Containers/com.microsoft.teams2`,
  `~/Library/Group Containers/UBF8T346G9.com.microsoft.teams`, and for classic Teams
  `~/Library/Application Support/Microsoft/Teams`.
- Match: `Suffix[Library/Containers/com.microsoft.teams2]`,
  `Suffix[Library/Group Containers/UBF8T346G9.com.microsoft.teams]`,
  `Suffix[Application Support/Microsoft/Teams]`.
- why: "Microsoft's steps for clearing the Teams client cache clear this whole folder, and Teams fills it again when it starts."
- caution: "It also holds the diagnostic logs Microsoft support asks for, which do not come back."
- Source: <https://learn.microsoft.com/en-us/troubleshoot/microsoftteams/teams-administration/clear-teams-cache>:
  "rm -rf ~/Library/Group Containers/UBF8T346G9.com.microsoft.teams; rm -rf
  ~/Library/Containers/com.microsoft.teams2", and "Clearing the Teams cache ...
  deletes the diagnostic logs that are needed to find the cause." The page does not
  say that Teams fills the folders again. That part of the `why` line is inferred
  from the page calling them a cache, so check it on a Mac with Teams before adding.
- Without this rule, rule 2 still takes the container's `Data/Library/Caches`.

**24. Terraform `.terraform`.** Owner: HashiCorp.

- Match: `Sibling{.terraform, .terraform.lock.hcl}`.
- why: "Terraform keeps cached provider plugins and modules here and creates the folder again during terraform init."
- caution: "It also records the selected workspace and the last backend configuration, which terraform init has to be given again."
- Source: <https://developer.hashicorp.com/terraform/cli/init>: "A hidden .terraform
  directory, which Terraform uses to manage cached provider plugins and modules,
  record which workspace is currently active, and record the last known backend
  configuration in case it needs to migrate state on the next run. This directory is
  automatically managed by Terraform, and is created during initialization."
- Prevalence: 17.8% of Stack Overflow 2025 respondents use Terraform.

**25. .NET `bin`.** Owner: Microsoft.

- Match: `Sibling{bin, obj}` with `Kind::Dir`. A hand-made `bin` of scripts beside
  an unrelated `obj` folder is the false positive.
- why: "The final output of a .NET build; dotnet build writes it again."
- caution: the existing `GENERIC_CAUTION`.
- Source: as rule 9.

**26. Hugging Face datasets.** Owner: Hugging Face.

- Path: `~/.cache/huggingface/datasets`. The existing rule matches only `hub`.
- Match: `Suffix[.cache/huggingface/datasets]`.
- why: "The Hugging Face Datasets cache; a dataset downloads again from the Hub when a program loads it."
- caution: "You chose these datasets and they are large to download again, and a dataset withdrawn upstream does not come back."
- Source: <https://huggingface.co/docs/datasets/cache>: "The default Datasets cache
  directory is ~/.cache/huggingface/datasets".
- Never match `~/.cache/huggingface` whole. It holds the login token: "HF_TOKEN_PATH
  ... Defaults to '$HF_HOME/token' (e.g. ~/.cache/huggingface/token by default)"
  (<https://huggingface.co/docs/huggingface_hub/package_reference/environment_variables>).

**27. TensorFlow Datasets.** Owner: TensorFlow.

- Match: `Suffix[tensorflow_datasets]`.
- why: "TensorFlow Datasets downloads and prepares datasets here and prepares them again when they are missing."
- caution: "Datasets you had to download by hand sit in the same folder and do not come back on their own."
- Source: <https://www.tensorflow.org/datasets/api_docs/python/tfds/load>: `data_dir`
  "falls back to '~/tensorflow_datasets'". The hand-downloaded datasets in the
  caution are from my knowledge of `downloads/manual`, not from that page.

**28. Ruby and Python versions (rbenv, pyenv).** Owners: rbenv, pyenv.

- Match: `Suffix[.rbenv/versions]`, `Suffix[.pyenv/versions]`.
- why (rbenv): "rbenv install builds a Ruby version again."
- why (pyenv): "pyenv install builds a Python version again."
- caution: "You chose which versions are installed, and each one keeps the gems or packages you installed into it, which do not come back on their own."
- Source: <https://github.com/rbenv/rbenv>: "Ruby versions you install will
  accumulate in your ~/.rbenv/versions directory", and `gem env home` shows
  "~/.rbenv/versions/<version>/lib/ruby/gems". <https://github.com/pyenv/pyenv>: "you
  will accumulate Python versions in your $(pyenv root)/versions directory. To
  remove old Python versions, use pyenv uninstall <versions>."
- asdf (`~/.asdf/installs`) and mise have the same shape. asdf's page documents
  `asdf uninstall <name> <version>` but I did not find the `installs` path stated on
  it, so they wait.

**29. Xcode documentation cache.** Owner: Xcode.

- Paths: `~/Library/Developer/Xcode/DocumentationCache` and `DocumentationIndex`.
  Not checked: Xcode is not on this Mac.
- Match: `Suffix[Library/Developer/Xcode/DocumentationCache]`,
  `Suffix[Library/Developer/Xcode/DocumentationIndex]`.
- why: "Xcode's documentation cache and its index; Xcode rebuilds them."
- caution: none in the text. Tier 2 because the source is a forum reply by an Apple
  engineer and not documentation.
- Source: Quinn, Apple DTS, <https://developer.apple.com/forums/thread/812408>:
  "DocumentationCache is pretty much what it says on the tin. You can delete it, but
  Xcode will have to rebuild it ... I recommend that you delete DocumentationIndex at
  the same time."

### Tier 3: not worth a rule now

| # | Candidate | Path | Why not now | Source |
| --- | --- | --- | --- | --- |
| 30 | Every child of `~/.cache` | `Glob[.cache/*]` | The XDG text supports it ("user-specific non-essential (cached) data"), but Hugging Face keeps its login token there, and a generic match on `~/.cache/huggingface` would take the folder before the `hub` rule sees it. Add named rules (19, 26) instead. | <https://specifications.freedesktop.org/basedir-spec/latest/> |
| 31 | Per-user temp folder | `/private/var/folders/*/*/T` | Temp files are working files of running programs, not copies of something else. macOS clears them itself: "files in this location may be cleaned (removed) by the system if they are not accessed in 3 days". 2.4 GiB here. | `confstr(3)` |
| 32 | Lima and Colima VM disks | `~/.lima/<instance>/disk`, `~/.colima` | Same nature as Docker's disk (images come back, containers and volumes do not), few users, and the path needs the glob. Lima's download cache is under `~/Library/Caches/lima` and rule 1 takes it. | <https://lima-vm.io/docs/dev/internals/> |
| 33 | Haskell `.stack-work`, `dist-newstyle` | `Sibling{.stack-work, stack.yaml}`, `Suffix[dist-newstyle]` | Small population. Stack's page names the folder; Cabal's says only that `dist-newstyle/cache` "can be safely deleted to clear the cache". | <https://docs.haskellstack.org/en/stable/topics/stack_work/>, <https://cabal.readthedocs.io/en/stable/nix-local-build.html> |
| 34 | Ivy cache | `Suffix[.ivy2/cache]` | Small population. Coursier's cache is `~/Library/Caches/Coursier/v1` and rule 1 takes it. | <https://ant.apache.org/ivy/history/latest-milestone/settings/caches.html>, <https://get-coursier.io/docs/cache> |
| 35 | Gatsby `.cache` and `public` | beside `gatsby-config.js` | `gatsby clean` documents both ("Delete the .cache and public directories"), but `public` is a common hand-made name. | <https://www.gatsbyjs.com/docs/reference/gatsby-cli/> |
| 36 | Zig `zig-out` | `Sibling{zig-out, build.zig}` | Zig calls it "an installation prefix": the user's chosen output, not a cache. | <https://ziglang.org/learn/build-system/> |
| 37 | Unreal `Intermediate` | beside `<name>.uproject` | "Contains temporary files generated during building the engine or game", but the name is generic and the sentinel is a file with a wildcard name, which no matcher can express as a sibling. | Epic page in rule 18 |
| 38 | Homebrew Cellar old versions | `/opt/homebrew/Cellar` | Homebrew cleans up on its own: it "periodically performs additional cleanup every 30 days". 1.3 GB here. | <https://docs.brew.sh/FAQ> |
| 39 | Named labels for big `Library/Caches` children | `Suffix[Library/Caches/Cypress]` and others | Rule 1 already takes them. A named rule only gives a better label and the owner's command, for example Cypress: "By default, global cache folders are: MacOS: ~/Library/Caches/Cypress". | <https://docs.cypress.io/app/references/advanced-installation> |
| 40 | Dropbox cache | `Suffix[.dropbox.cache]` | It empties itself ("automatically cleared every three days") and is not a copy of something else: "a staging area for downloading and uploading files". | <https://help.dropbox.com/delete-restore/cache-folder> |
| 41 | CocoaPods spec repos, Expo `.expo`, NLTK data, Nix store, Flutter SDK cache | various | Small, or not read to quote level in this session. | not checked |

## A generic rule for Apple's cache folders

Recommendation: add it. It is the largest single step, and Apple's text supports
the claim more directly than most tool documentation supports the existing rules.

What Apple says:

- "Caches: Contains cached data that can be regenerated as needed. Apps should never
  rely on the existence of cache files." (macOS Library Directory Details)
- "Put data cache files in the Library/Caches/ directory. ... Generally speaking, the
  application does not require cache data to operate properly ... Note that the
  system may delete the Caches/ directory to free up disk space, so your app must be
  able to re-create or download these files as needed." (File System Basics. This
  sentence sits in the iOS section. I found no Apple statement that macOS itself
  clears `~/Library/Caches` on its own.)
- "Discardable cache files (Library/Caches)." (`FileManager.SearchPathDirectory.cachesDirectory`)
- Per-user cache folder: "will not be automatically cleaned by the system. Files in
  this location will be removed during safe boot." (`confstr(3)`)
- By contrast: "Application Support: Contains all app-specific data and support
  files ... and can include files that contain user data." No generic rule fits
  `Application Support`.

Would "a `Caches` folder directly under a `Library` folder" be accurate? Yes for
what Apple promises, with these adjustments:

1. **Match the children, not `Library/Caches` itself.** The walk stops at a matched
   folder. A match on `~/Library/Caches` would hide the 15 existing rules that sit
   one level below it (Homebrew, pip, JetBrains and the rest). `Glob[Library/Caches/*]`
   matches each child, and a named rule on the same child wins by priority.
2. **New matcher: `PathGlob(&[&str])`.** It compares the last N names of the path
   with N patterns, as `PathSuffix` does. A pattern is a literal name or holds
   exactly one `*`, which matches any run of characters inside that one name. A `*`
   never spans two names. When the last pattern is a bare `*`, `priority()` must
   rank it after every named matcher (at or below `AnyWithChild`), so a named rule
   on the same folder wins. The same matcher serves rules 3, 4, 20, 21 and 22, and
   the Bazel 8 output root the earlier research had to leave out
   (`Glob[var/tmp/_bazel_*]`).
3. **Guard against `/System`.** In a scan of `/`, `/System/Library/Caches/*` matches
   too. It is on the sealed system volume and is not the user's. The rule needs a
   check that the name before `Library` is not `System`, either in the matcher or as
   a named rule that takes `Suffix[System/Library/Caches]` whole and is left out of
   the tab.
4. **A deeper named rule cannot win.** Once the generic rule takes
   `~/Library/Caches/Google`, a rule for something inside it never runs. Any
   exception has to be a named rule on the direct child.
5. **Files directly in `Library/Caches` are missed.** The matchers only see folders.
   On this Mac they are negligible.
6. **Backups match too.** A copy of a home folder on another volume has the same
   path ending. This is already true of every `PathSuffix` rule.

Known content in these folders that does not come back, or needs care:

| What | Where | Handling |
| --- | --- | --- |
| JetBrains Local History | `~/Library/Caches/JetBrains` | Already a named rule with a caution; it keeps winning. Source in the existing rule. |
| Android Studio Local History | believed to be `~/Library/Caches/Google/AndroidStudio<version>` | Android Studio is a JetBrains IDE. The path is from memory: Google's configuration page names only `idea.system.path`. If confirmed, add `Suffix[Library/Caches/Google]` as a named rule with the JetBrains caution. It then covers Chrome's cache too. |
| Poetry virtual environments | `~/Library/Caches/pypoetry/virtualenvs` | Existing named rule. |
| Update downloads in progress | `com.microsoft.VSCode.ShipIt` (1.0 GiB here), `<app>/org.sparkle-project.Sparkle`, `t3code-updater` | They come back: the updater downloads again. No carve-out. |
| Podcasts downloads | `~/Library/Group Containers/243LU875E5.groups.com.apple.podcasts/Library/Cache` | The folder is `Cache`, singular, so rule 3 does not match it. Keep it that way; see the next section. |
| CloudKit and iCloud Drive caches | `~/Library/Caches/CloudKit`, `com.apple.bird` | Unreadable here. A review in what-users-want.md mentions a CloudKit cache holding deleted videos. Whether anything in it is waiting to upload is not documented. The standing caution has to cover it. |
| Protected folders | Safari, HomeKit, CloudKit, `familycircled` and four more children of `~/Library/Caches`, and all of `~/Library/Containers` | Not scanned without Full Disk Access, so the tab shows less on such a Mac. The page should say so where it already lists "Not scanned". |
| Apps that break Apple's rule | unknown | Apple publishes no list. The standing caution in rule 1 is the mitigation. |

The caution stays on rules 1 to 3 for that last row. Rule 4 has none: `confstr(3)`
says macOS itself empties that folder.

## Not rebuildable, though it looks it

| What | Path | Why it stays out | Source |
| --- | --- | --- | --- |
| Xcode Archives | `~/Library/Developer/Xcode/Archives` | Confirmed again: "You must retain the Xcode archive for each build of your app you distribute. Without this archive, you might not be able to diagnose an issue from crash reports." | <https://developer.apple.com/documentation/xcode/building-your-app-to-include-debugging-information> |
| Simulator devices | `~/Library/Developer/CoreSimulator` | Apple DTS: "it's probably best to not monkey with that yourself but instead manage it via either Window > Devices & Simulators in Xcode or the simctl command-line tool." Devices also hold installed apps and their data. | <https://developer.apple.com/forums/thread/812408> |
| iPhone and iPad backups | `~/Library/Application Support/MobileSync/Backup` | "After you delete a backup, you can't recover it." | <https://support.apple.com/en-us/108809> |
| Messages attachments | `~/Library/Messages/Attachments` | Apple's guide treats them as content with a 30-day Recently Deleted folder and permanent removal after it. Nothing says they download again. | <https://support.apple.com/guide/messages/delete-messages-and-conversations-icht1035/mac> |
| Podcasts, Music and TV downloads | Podcasts group container `Library/Cache`; `~/Music`; `~/Movies/TV` | Apple manages them in Storage settings ("the Applications, Documents, Messages, Music Creation, Podcasts, Trash, and TV categories"). They download again only while the publisher or the subscription still offers them, and Music also holds imported files that exist nowhere else. Apple documents no path, so no rule. | <https://support.apple.com/en-us/102624> |
| Final Cut Pro and iMovie original media | inside the library bundle | Apple's regeneration promise covers "render, optimized, and proxy files only". | <https://support.apple.com/guide/final-cut-pro/ver68a8c250/mac> |
| Unreal `Saved` | `<project>/Saved` | "Contains autosaves, configuration (.ini) files, and log files." | Epic page in rule 18 |
| Hugging Face token | `~/.cache/huggingface/token` | A credential inside a cache folder. | page in rule 26 |
| Electron user state | `Local Storage`, `IndexedDB`, `Cookies`, `Session Storage` under an app's Application Support folder | Electron lists them beside the cache as session data ("localStorage, cookies"). They are sign-ins and app state. | <https://www.electronjs.org/docs/latest/api/app> |
| conda environments | `<root>/envs` | They come back only from an `environment.yml` the user exported. Not read to quote level; left out on the absence of an owner statement that they return. | not checked |
| Logs | `~/Library/Logs` (1.1 GiB here), `/private/var/log` | A log is a record of the past. Apps write new ones, but the old content does not return. Microsoft makes the same point about Teams logs in rule 23. | page in rule 23 |
| Mail store and Mail downloads | `~/Library/Mail` | Unreadable here and not researched to quote level. "On My Mac" mailboxes exist only locally (my knowledge), so no rule without a source. | not checked |
| Android emulators, Docker containers and volumes | existing cautioned rules | Unchanged from the earlier research. | rebuildable-folders.md |

## Could not verify

- **Steam.** `help.steampowered.com` renders with scripts and returned no article
  text. No rule for the game library (`steamapps/common`), the shader cache or the
  download cache without Valve's words. This is the largest unverified item for
  gamers.
- **Epic Games Launcher, Battle.net.** Not researched to quote level.
- **Firefox support articles.** Mozilla's support site returned a bot wall. The
  cache path is confirmed from Firefox's source and on this Mac
  (`~/Library/Caches/Firefox/Profiles/<profile>/cache2`); Mozilla's user-facing
  wording on what the cache holds is not quoted.
- **Discord, Signal, WhatsApp, Zoom, Notion, Figma.** Support pages returned 403 or
  held no relevant text, and none is installed here. Rule 5 is expected to match
  their Chromium caches through Electron's documented names, but that is unconfirmed
  per app. Signal and WhatsApp attachments are assumed not to come back; I could not
  quote either owner.
- **Spotify.** Spotify says it uses storage to "Store parts of music and podcasts as
  cache" and to "Store downloaded music and podcasts to play offline", with Settings
  > Storage > Clear cache on desktop (<https://support.spotify.com/us/article/storage-information/>,
  read through a page-summary tool, not curl). It gives no macOS path and does not
  say whether clearing the cache drops downloads.
- **`Service Worker/CacheStorage`** in Chromium profiles. Electron's page does not
  mention it, and Chrome clears it with site data, not with "Cached images and
  files" (my knowledge). Left out of rule 5.
- **VS Code `CachedData`, `CachedExtensionVSIXs`, `CachedProfilesData`.** Present on
  this Mac (88 MB, 6 MB, 2.5 MB). Microsoft still documents none of them.
- **Simulator runtimes.** Apple documents how to get rid of and re-download them
  ("click the information button next to the component. In the dialog that appears,
  click Delete", and `xcodebuild -downloadPlatform iOS`,
  <https://developer.apple.com/documentation/xcode/downloading-and-installing-additional-xcode-components>)
  but not where they are stored, and Xcode is not on this Mac.
- **iOS DeviceSupport, Xcode Previews.** Still no Apple source. The DeviceSupport
  advice in forum thread 812408 comes from a community member, not Apple.
- **Logic Pro and GarageBand sound libraries.** They can be installed again
  ("GarageBand > Sound Library > Reinstall Sound Library",
  <https://support.apple.com/guide/garageband/gbnd9ed77587/mac>), but no Apple page
  ties that to `/Library/Application Support/GarageBand` (908 MB here),
  `/Library/Application Support/Logic` (896 MB) or `/Library/Audio/Apple Loops`
  (473 MB). Logic Pro 12 adds a twist for hoardmap's clone accounting: "the app
  clones your sound library. The new file is located in your Music folder. The
  cloning operation requires no extra storage space"
  (<https://support.apple.com/en-us/125036>).
- **DaVinci Resolve cache and optimized media, Capture One caches, Blender caches,
  Sketch.** Blackmagic's manual is a PDF I did not read; the others were not
  researched.
- **iOS software update files** (`~/Library/iTunes/iPhone Software Updates`). Apple
  documents no path, and the folder does not exist on this Mac.
- **Photos library derivatives.** No Apple statement that Photos regenerates them.
- **Google Drive for desktop cache.** Google's page says "Streaming a file uses
  almost no computer space, but mirroring downloads a full copy", and names no cache
  folder.
- **Bundler `vendor/bundle`, tox `.tox`, Nx `.nx/cache`, Android `.cxx`, Volta,
  OrbStack, Podman machine disks.** Documentation sites were script-rendered or did
  not state the path on the pages I tried.
- **Android Studio's cache path** and its Local History, as in the exceptions table.
- **Final Cut Pro's folder names** inside the library bundle (rule 21), and whether
  Teams refills its folders (rule 23).

## Coverage estimate

These are rough. Only the developer row rests on a measurement, and that
measurement is one Mac. The others are reasoned from where each segment's
rebuildable data lives and which rules reach it. "Rebuildable space" means what an
owner documents as coming back; it leaves out media downloads, installers and game
libraries, which is where much of a non-developer's reclaimable space is.

| Segment | Found by the 68 rules | Found with tier 1 and 2 | Basis |
| --- | --- | --- | --- |
| Ordinary users | about 0% | 70% to 90% with Full Disk Access, perhaps half without | No existing rule names anything such a Mac has. Rules 1 to 5 reach every browser cache, app cache and the per-user cache folder. Safari and all sandboxed apps need Full Disk Access. The per-user cache folder (12 GiB here) needs a scan of `/`. |
| Creative users | about 0% | 40% to 70% | Adobe media cache and Lightroom previews are covered. Final Cut Pro waits on folder names. Resolve, Capture One and sound libraries are not covered. |
| Web, systems and data developers | about 93% | about 97% | This Mac. The 68 rules match 198 GiB under the home folder: 103 GiB plain (Cargo `target` 79, package caches 14, `node_modules` 5, virtual environments 3, `.next` 2) and 95 GiB cautioned (LM Studio models 59, Docker 34, toolchains 2). The new rules add about 10 GiB (the rest of `~/Library/Caches` 7.3, npx 1.8, Electron caches 0.6, Cargo git 0.1). About 5 GiB that looks rebuildable stays unmatched (`~/.cache/codex-runtimes`, an app's `download-cache`, LM Studio's runtime downloads). Project folders were counted by name with `find` to depth 7, without checking each rule's sentinel, and `du` counts clones in full, so the absolute figures run high. |
| Apple platform developers | 50% to 70% | 50% to 70% | DerivedData is covered. Simulators, runtimes and device support (reviews report 30 to 40 GB) stay out or unverified. The additions change little. |
| Game developers | under 20% | 60% to 80% | Unity `Library` and Unreal `DerivedDataCache` are the bulk. Unreal `Intermediate` is not covered. |
| Machine learning users | 80% to 90% | about 90% | Hub models, Ollama and LM Studio were already covered. Datasets, Whisper and Keras add a little. Model folders of Draw Things, ComfyUI and Stable Diffusion front ends are the user's chosen files and stay out. |
| Gamers | about 0% | under 20% | Only launcher and browser caches through rules 1 and 5. Game libraries are unverified. |

The 99% target, read as "99% of users see their main rebuildable folders", is met
by rules 1 to 5 for everyone except gamers, whose main item is the game library. Read
as "99% of rebuildable bytes", it is out of reach while Steam, simulators and the
creative apps' caches lack either an owner statement or a documented path.
