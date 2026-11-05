# Rebuildable folders on a Mac

Research for hoardmap's Rebuildable tab (`src/rebuildable.rs`,
`docs/specs/true-cost-rebuildable.md`), 2026-10-30. The question: which folders on a
Mac does their owning tool fill again on its own (caches, build output, downloaded
toolchains, models and images), how can hoardmap recognise each one from names
already in the scanned tree, and which of them hold data that does not come back.

hoardmap never deletes anything. "Rebuildable" here is a description for the user,
backed by the owner's own documentation, not an instruction. Proposed `why` lines
below follow the rule-text test: no "safe", "delete" or "remove".

## Method

- **Primary sources only for claims.** Each row was checked against the owner's
  documentation page or source code on 2026-10-30, fetched directly (curl or a page
  fetch) and searched for the quoted words. Where docs were silent, the owner's
  source on GitHub (`raw.githubusercontent.com`, default branch) was read instead,
  and the row cites the file.
- **CACHEDIR.TAG writers.** GitHub code search for the signature string
  `8a477f597d28d172789f06886806bc55` (about 100 hits across all repositories), then
  per-repository searches for `CACHEDIR.TAG` and `CACHEDIR` in Cargo, ccache, sccache,
  Bazel, pip, Go, Gradle, restic, borg, mypy, pytest, uv, ruff, npm cacache, Deno,
  Bun, Yarn, pnpm, Julia Pkg, SCons, direnv and nox. Each positive was confirmed by
  reading the writing function. Mesa was checked on gitlab.freedesktop.org.
- **Not used.** Blog posts, Stack Overflow and Reddit. Where only community sources
  exist (Xcode iOS DeviceSupport, VS Code CachedData), the row is marked unverified.
- "Verified" in the table means the cited page or source file says what the row
  claims about path and how the folder comes back. "Partly" names what is missing.

## CACHEDIR.TAG

**Spec.** Bryan Ford, *Cache Directory Tagging Specification*, version 0.6
(2019-11-04, no substantive change since 0.5 in 2004). Current home:
<https://bford.info/cachedir/> (the old `brynosaurus.com/cachedir/` URL still appears
in tags written by older tools).

**What it says.**

- The file is named `CACHEDIR.TAG`, must be a regular file (not a symlink), and its
  first 43 bytes must be exactly:

  ```
  Signature: 8a477f597d28d172789f06886806bc55
  ```

  Case matters, nothing may come before the `S`, exactly one space follows the colon,
  and no line ending is required after the hash. The hash is the MD5 of
  `.IsCacheDirectory`. The rest of the file is UTF-8 text; lines starting with `#` are
  comments.
- Meaning: "the entire contents of that directory, including any and all
  subdirectories underneath it, consists of cached information that can be
  re-generated if necessary from appropriate source material located elsewhere."
- One tag goes in the topmost cache directory. Tools "strongly encouraged to verify
  ... that any file it finds named CACHEDIR.TAG is a regular file and actually
  contains the 43-byte header ... before concluding that the file is indeed a cache
  directory tag." The signature exists "to avoid the chance of an unrelated file
  named CACHEDIR.TAG being mistakenly interpreted as a cache directory tag."
- Security section: tags "should only be treated as hints, never as 'the final
  word'", and software should tell the user which directories it is treating as
  caches. The spec itself suggests that a `du`-like tool "might provide a way to
  locate, total, and report the space usage of all the cache directories", which is
  what the Rebuildable tab does.

**Who writes it** (confirmed in source unless noted):

| Tool | Where the tag goes | Source |
| --- | --- | --- |
| Cargo | `target/` (and build-dir), via `exclude_from_backups`, which also sets the macOS Time Machine exclusion | `rust-lang/cargo` `crates/cargo-util/src/paths.rs` |
| ccache | Each `[0-9a-f]` subfolder of the cache, **not** the cache root (so `ccache.conf` at the root stays in backups; news.adoc) | `ccache/ccache` `src/ccache/storage/local/localstorage.cpp`, `doc/news.adoc` |
| Gradle 8.1+ | `~/.gradle/caches`, `wrapper/dists`, `daemon`, `jdks` | Gradle user guide, "Cache marking"; `CacheDirUtil.java` |
| uv | Cache root | `astral-sh/uv` `crates/uv-cache/src/lib.rs` |
| ruff | `.ruff_cache` | `astral-sh/ruff` `crates/ruff/src/cache.rs` |
| mypy | `.mypy_cache` | `python/mypy` `mypy/build.py` |
| pytest | `.pytest_cache` | `pytest-dev/pytest` `src/_pytest/cacheprovider.py` |
| nox | `.nox` | `wntrblm/nox` `nox/virtualenv.py` |
| direnv | `.direnv` layout folder | `direnv/direnv` `stdlib.sh` |
| npm (cacache) | cacache's `mkdir` helper writes it into the cache folder it creates; exact folder under `~/.npm` not traced | `npm/cacache` `lib/util/cache-dir.js` |
| huggingface_hub | The hub cache root, **and any `cache_dir` a program passes**, and `local_dir/.cache/huggingface` | `huggingface/huggingface_hub` `file_download.py`, `_local_folder.py` |
| Julia Pkg | Its cache folders | `JuliaLang/Pkg.jl` `src/Pkg.jl` |
| SCons | `CacheDir` | `SCons/scons` `SCons/CacheDir.py` |
| restic, borg | restic's own cache root; borg's cache. Both also *honour* tags (`--exclude-caches`) | `restic` `internal/backend/cache/cache.go`; `borg` `src/borg/constants.py` |
| man-db, fontconfig, libdvdcss, VLC | Their caches (code search hits; spec lists libdvdcss and VLC) | search hits, spec page |

**Who does not** (no tag in source, 2026-10-30): Go (the build cache gets a `README`
instead: "This directory holds cached build artifacts from the Go build system",
`src/cmd/go/internal/cache/default.go`), pip, sccache, Bazel, Deno, Bun, Yarn (classic
and Berry), pnpm, and Mesa (no match in `src/util/disk_cache*.c` on `main`). A negative
code search is weaker evidence than a positive one; treat these as "not found".

**Name-only false positives.** Matching on the file name alone is wrong in practice:

- Tags get committed. Code search finds `CACHEDIR.TAG` checked into repositories
  inside `target/` folders and elsewhere. One example, `RIKEN-SysSoft/PiP`
  `doc/man/CACHEDIR.TAG`, carries a valid man-db signature but sits beside hand-written
  `man1`, `man3`, `man7` sources: man-db tagged a folder that is mostly not cache.
- Tools tag folders the user chose. huggingface_hub tags whatever `cache_dir` a program
  passes (`_create_cachedir_tag(Path(cache_dir))` in `file_download.py`), so a script
  that sets `cache_dir="."` tags a project folder. `4evergr8/ASMRASR` commits
  `model/CACHEDIR.TAG` written this way.
- The signature check needs file contents (43 bytes), which `groups()` does not read
  today. Even with it, both examples above have valid signatures.

**Recommendation.** Use `CACHEDIR.TAG` as a *child sentinel inside rules that already
name the folder* (Cargo `target`, `.mypy_cache`, `.pytest_cache`, `.ruff_cache`,
`.nox`), where the name plus the tag is strong evidence. Do not add a generic "any
folder holding CACHEDIR.TAG" group as rebuildable. If one is wanted later, it needs a
new matcher (`ChildTag`, any folder name), a 43-byte signature read at scan time for
just those files, and a caution label naming the tool from the tag's comment line,
since the tag is a hint by the spec's own words.

## Table

Matcher shorthand: `Suffix[a/b/c]` is `PathSuffix(&["a","b","c"])`; `Child{x, y}` is
`ChildSentinel{name: x, child: y}`; `Sibling{x, y}` is `SiblingSentinel{name: x,
sibling: y}`. `~` is the home folder. Paths are macOS defaults.

| Tool | Path (env override) | Match | Comes back by | User data? | Source URL | Verified |
| --- | --- | --- | --- | --- | --- | --- |
| Go build cache | `~/Library/Caches/go-build` (`GOCACHE`) | `Suffix[Library/Caches/go-build]` | "The go command caches build outputs for reuse in future builds"; refilled by builds | No | https://pkg.go.dev/cmd/go#hdr-Build_and_test_caching | yes (path from `os.UserCacheDir` in `default.go`) |
| Yarn classic cache | `~/Library/Caches/Yarn` (`YARN_CACHE_FOLDER`, `cache-folder`) | `Suffix[Library/Caches/Yarn]` | "populated again the next time yarn or yarn install is run" | No | https://classic.yarnpkg.com/lang/en/docs/cli/cache/ | yes (macOS path from `src/util/user-dirs.js`) |
| Yarn Berry global cache | `~/.yarn/berry/cache` (`globalFolder`, `cacheFolder`) | `Suffix[.yarn/berry/cache]` | Packages "retrieved from the npm registry" on install | No | https://yarnpkg.com/configuration/yarnrc | yes (`globalFolder` default `${HOME}/.yarn/berry`; `/cache` appended in `Configuration.ts`) |
| Yarn Berry project cache | `<project>/.yarn/cache` when `enableGlobalCache: false` | `Sibling{.yarn, .yarnrc.yml}` would find the parent only | With zero-installs it is committed so "every given commit is thus guaranteed to be installable" | **Yes in effect**: committed, part of the repo | https://yarnpkg.com/features/caching | yes |
| Bun | `~/.bun/install/cache` (`BUN_INSTALL_CACHE_DIR`) | `Suffix[.bun/install/cache]` | "Bun downloads the package from the registry, then installs it" on a miss | No | https://bun.sh/docs/install/cache | yes |
| Maven | `~/.m2/repository` (`localRepository` in settings) | `Suffix[.m2/repository]` | "caches remote downloads"; "erase it completely if you are willing to download everything again" | Holds `mvn install` output of your own unreleased projects ("temporary build artifacts that you have not yet released"); `~/.m2/settings.xml` is user config, outside the match | https://maven.apache.org/guides/introduction/introduction-to-repositories.html, https://maven.apache.org/settings.html | yes |
| Conda / mamba / miniforge | `<root>/pkgs`, e.g. `~/miniforge3/pkgs` (`pkgs_dirs`, `CONDA_PKGS_DIRS`) | `Sibling{pkgs, conda-meta}` (the root prefix has `conda-meta`) | Package cache; `conda clean -p` targets "unused packages from writable package caches" | Environments hard-link (or soft-link) into `pkgs`, so its Physical size overlaps them and soft-linked envs need it | https://docs.conda.io/projects/conda/en/latest/user-guide/configuration/settings.html, https://docs.conda.io/projects/conda/en/stable/commands/clean.html | partly (default `root_dir/pkgs` yes; hard-link detail from the `conda clean` warning only) |
| Xcode Archives | `~/Library/Developer/Xcode/Archives` | `Suffix[Library/Developer/Xcode/Archives]` | Does not come back: each archive is the signed build you shipped | **Yes**: "You must retain the Xcode archive for each build of your app you distribute. Without this archive, you might not be able to diagnose an issue from crash reports." | https://developer.apple.com/documentation/xcode/building-your-app-to-include-debugging-information | yes |
| Xcode iOS DeviceSupport | `~/Library/Developer/Xcode/iOS DeviceSupport` | `Suffix[Library/Developer/Xcode/iOS DeviceSupport]` | Xcode copies symbols from a connected device ("Processing symbol files") | Symbols for an OS version can be hard to get back without a device on that version | none from Apple found | **no** |
| Android SDK | `~/Library/Android/sdk` (`ANDROID_HOME`) | `Suffix[Library/Android/sdk]`; narrower `Suffix[Library/Android/sdk/system-images]` | SDK Manager; "You can also download system images later when creating Android Virtual Devices" | No, but projects need specific platform versions | https://developer.android.com/tools/variables, https://developer.android.com/studio/intro/update | partly (path is the docs' example, not a stated default) |
| Android emulators (AVD) | `~/.android/avd` (`ANDROID_AVD_HOME`) | `Suffix[.android/avd]` | Not rebuilt: an AVD "stores the device user data, such as installed apps and settings, as well as an emulated SD card" | **Yes** | https://developer.android.com/studio/run/managing-avds, https://developer.android.com/tools/variables | yes |
| Docker Desktop disk image | `~/Library/Containers/com.docker.docker/Data/vms/0/data/Docker.raw` (Settings, disk image location) | `Suffix[Library/Containers/com.docker.docker/Data/vms]` (the match is a folder; `Docker.raw` is a file inside) | Images re-pull; containers do not come back. Docker "stores Linux containers and images in a single, large disk image file" | **Yes**: containers, and named volumes (the engine keeps them under `/var/lib/docker/volumes`, which on Desktop is inside this disk) | https://docs.docker.com/desktop/troubleshoot-and-support/faqs/macfaqs/, https://docs.docker.com/engine/storage/volumes/ | partly (FAQ names containers and images; volumes inferred from the engine doc) |
| Ollama models | `~/.ollama/models` (`OLLAMA_MODELS`) | `Suffix[.ollama/models]` | `ollama pull` downloads again | Models built locally with `ollama create` from your own files are not on a registry | https://docs.ollama.com/faq | yes (path, env var); create caveat from general knowledge, not that page |
| Next.js | `<app>/.next` (`distDir`) | `Sibling{.next, package.json}` | `next build` writes it; "Next.js takes ownership of distDir and will sometimes delete its contents" | No | https://nextjs.org/docs/app/api-reference/config/next-config-js/distDir | yes |
| Python bytecode | `__pycache__` beside each module | `Suffix[__pycache__]` (name only; the name is Python's own) | "Python checks the modification date of the source against the compiled version ... This is a completely automatic process" | No | https://docs.python.org/3/tutorial/modules.html#compiled-python-files | yes |
| Turborepo | `<repo>/.turbo` (`.turbo/cache` inside) | `Sibling{.turbo, package.json}` | "Turborepo stores the results of tasks in the .turbo/cache directory"; refilled when tasks run | No | https://turborepo.dev/docs/crafting-your-repository/caching | partly (refill implied, not stated) |
| Parcel | `<project>/.parcel-cache` | `Sibling{.parcel-cache, package.json}` | "the cache is stored in the .parcel-cache folder inside your project" | No | https://parceljs.org/features/development/ | yes |
| Gradle project cache | `<project>/.gradle` | `Sibling{.gradle, settings.gradle}`, `Sibling{.gradle, settings.gradle.kts}`, same with `build.gradle(.kts)` | "Version-specific build state for incremental build" and configuration cache | No. Never match `~/.gradle` whole: it holds `gradle.properties` and `init.d` (user config) | https://docs.gradle.org/current/userguide/directory_layout.html | yes |
| Gradle build output | `<project>/build` | `Sibling{build, build.gradle}`, `Sibling{build, build.gradle.kts}` | "transient files Gradle uses to support features like incremental builds" | No | same | yes |
| Gradle wrapper and JDKs | `~/.gradle/wrapper/dists`, `~/.gradle/jdks` (`GRADLE_USER_HOME`) | `Suffix[.gradle/wrapper/dists]`, `Suffix[.gradle/jdks]` (both also carry CACHEDIR.TAG) | "Distributions downloaded by the Gradle Wrapper"; "JDKs downloaded by the toolchain support" | No | same | yes |
| Deno | `~/Library/Caches/deno` (`DENO_DIR`) | `Suffix[Library/Caches/deno]` | "Downloaded dependencies and compiled artefacts are stored in Deno's cache directory" | No | https://docs.deno.com/runtime/getting_started/installation/ | yes |
| Poetry | `~/Library/Caches/pypoetry` (`POETRY_CACHE_DIR`); virtualenvs in `{cache-dir}/virtualenvs` | `Suffix[Library/Caches/pypoetry]` | Cache and environments recreated by `poetry install` | No | https://python-poetry.org/docs/configuration/ | yes (paths); refill wording not on that page |
| pipenv virtualenvs | `~/.local/share/virtualenvs` (`WORKON_HOME`) | `Suffix[.local/share/virtualenvs]` | "Recreate the virtual environment (Pipenv will create a fresh one for the new path)"; `pipenv install` reads Pipfile and Pipfile.lock | No | https://pipenv.pypa.io/en/latest/virtualenv.html | yes |
| CocoaPods cache | `~/Library/Caches/CocoaPods` (`CP_CACHE_DIR`) | `Suffix[Library/Caches/CocoaPods]` | `pod cache clean` documented; pods fetched again on install | No | https://guides.cocoapods.org/terminal/commands.html#pod_cache_clean | yes (path from `lib/cocoapods/config.rb`) |
| CocoaPods `Pods` | `<project>/Pods` | `Sibling{Pods, Podfile.lock}` | "CocoaPods is generally able to recreate the same installation" | **Often committed**: "We recommend that you keep the Pods directory under source control" | https://guides.cocoapods.org/using/using-cocoapods.html | yes |
| Carthage | `~/Library/Caches/org.carthage.CarthageKit`; `<project>/Carthage/Build`, `Carthage/Checkouts` | `Suffix[Library/Caches/org.carthage.CarthageKit]`; `Suffix[Carthage/Build]` | Cache: "automatically repopulated the next time carthage checkout is run"; Build: `carthage build` | `Checkouts` may hold submodule edits; both may be committed | https://github.com/Carthage/Carthage/blob/master/Documentation/Artifacts.md | yes |
| Dart / Flutter pub cache | `~/.pub-cache` (`PUB_CACHE`) | `Suffix[.pub-cache]` | "When pub gets a remote package, it downloads it into a single pub system cache directory" | Also holds globally activated packages (`dart pub global activate`), which are user choices | https://dart.dev/tools/pub/glossary | yes (cache); global packages from general knowledge |
| Dart `.dart_tool` | `<package>/.dart_tool` | `Sibling{.dart_tool, pubspec.yaml}` | `dart pub get` writes `.dart_tool/package_config.json` | No | https://dart.dev/tools/pub/glossary | partly (only `package_config.json` named) |
| Flutter build | `<app>/build` | `Sibling{build, pubspec.yaml}` | `flutter build` | No | none fetched | **no** |
| Composer | cache `~/.composer/cache` (`COMPOSER_CACHE_DIR`); `<project>/vendor` | `Suffix[.composer/cache]`; `Sibling{vendor, composer.json}` | `install` downloads "the dependencies' files into the vendor directory" | `~/.composer` holds `config.json` (user config): match only `cache`. Some projects commit `vendor` | https://getcomposer.org/doc/03-cli.md, https://getcomposer.org/doc/01-basic-usage.md | yes |
| NuGet | `~/.nuget/packages` (`NUGET_PACKAGES`); `~/.local/share/NuGet/v3-cache` (`NUGET_HTTP_CACHE_PATH`) | `Suffix[.nuget/packages]`; `Suffix[.local/share/NuGet/v3-cache]` | "After clearing the global-packages directory, you will need to restore your projects again to redownload all required packages" | No. `~/.nuget/NuGet/NuGet.Config` is config: never match `~/.nuget` whole | https://learn.microsoft.com/en-us/nuget/consume-packages/managing-the-global-packages-and-cache-folders | yes |
| Playwright | `~/Library/Caches/ms-playwright` (`PLAYWRIGHT_BROWSERS_PATH`) | `Suffix[Library/Caches/ms-playwright]` | `npx playwright install` downloads the browsers | No | https://playwright.dev/docs/browsers | yes |
| Puppeteer | `~/.cache/puppeteer` (`cacheDirectory` in config) | `Suffix[.cache/puppeteer]` | `npx puppeteer browsers install` | No | https://pptr.dev/guides/configuration | yes |
| JetBrains IDEs | `~/Library/Caches/JetBrains/<product><version>` | `Suffix[Library/Caches/JetBrains]` | Caches and indexes rebuilt by the IDE | **Yes**: "system directory contains caches and local history files"; Local History does not come back | https://www.jetbrains.com/help/idea/directories-used-by-the-ide-to-store-settings-caches-plugins-and-logs.html | yes |
| VS Code extensions | `~/.vscode/extensions` (`VSCODE_EXTENSIONS`, `--extensions-dir`) | `Suffix[.vscode/extensions]` | Reinstalled from the Marketplace | User-chosen set; side-loaded VSIX and delisted extensions do not come back | https://code.visualstudio.com/docs/configure/extensions/extension-marketplace | yes (path) |
| VS Code cached data | `~/Library/Application Support/Code/CachedData`, `Cache`, `CachedExtensionVSIXs` | `Suffix[Library/Application Support/Code/CachedData]` | Not documented | Unknown | none found | **no** |
| sccache | `~/Library/Caches/Mozilla.sccache` (`SCCACHE_DIR`) | `Suffix[Library/Caches/Mozilla.sccache]` | Compiler output cache, refilled by builds (default size 10 GB) | No | https://github.com/mozilla/sccache/blob/main/docs/Local.md | yes |
| ccache | `~/Library/Caches/ccache`, or `~/.ccache` if it exists (`CCACHE_DIR`) | `Suffix[Library/Caches/ccache]` | `ccache -C` "removing all cached files, but keeping the configuration file"; refilled by builds | The cache root can hold `ccache.conf` (legacy `~/.ccache`, or when `CCACHE_DIR` is set). The default macOS config is `~/Library/Preferences/ccache/ccache.conf`, outside the match | https://ccache.dev/manual/latest.html | yes |
| Bazel | Bazel 9+: `~/Library/Caches/bazel`; Bazel 8 and earlier: `/private/var/tmp/_bazel_$USER` (`--output_user_root`) | `Suffix[Library/Caches/bazel]`; the old path has a user-specific name and needs a prefix matcher | `bazel clean --expunge` "will clean the entire outputBase"; rebuilt by the next build | No. Convenience symlinks (`bazel-out`, `bazel-bin`) in the workspace are symlinks and count nothing | https://bazel.build/remote/output-directories | yes |
| Swift Package Manager | `~/Library/Caches/org.swift.swiftpm` | `Suffix[Library/Caches/org.swift.swiftpm]` | Repository and manifest caches, refetched on resolve | No | https://github.com/swiftlang/swift-package-manager/blob/main/Sources/Basics/FileSystem/FileSystem%2BExtensions.swift | partly (source only; no docs page) |
| PyTorch hub | `~/.cache/torch/hub` (`TORCH_HOME`, `XDG_CACHE_HOME`, `hub.set_dir`) | `Suffix[.cache/torch/hub]` | Hub "directory used for storing downloaded models & weights"; downloaded when missing | Downloaded models (large, user-chosen) | https://docs.pytorch.org/docs/main/hub.html | yes |
| Kaggle (kagglehub) | `~/.cache/kagglehub` (`KAGGLEHUB_CACHE`) | `Suffix[.cache/kagglehub]` | "By default, kagglehub downloads files to your home folder at ~/.cache/kagglehub/" | Downloaded models and datasets | https://github.com/Kaggle/kagglehub | yes |
| LM Studio | `~/.lmstudio/models` (changed in My Models) | `Suffix[.lmstudio/models]` | Downloaded again from the in-app catalogue | **Yes in part**: the same folder takes models imported from local files ("Import models"), which do not come back | https://lmstudio.ai/docs/app/advanced/import-model | yes |
| Electron downloads | `~/Library/Caches/electron` | `Suffix[Library/Caches/electron]` | "caches it so that future requests for that asset can be returned instantly" | No | https://github.com/electron/get | yes |
| rustup toolchains | `~/.rustup/toolchains` (`RUSTUP_HOME`) | `Suffix[.rustup/toolchains]` | `rustup toolchain install` | No, but which toolchains are installed is a user choice | https://rust-lang.github.io/rustup/environment-variables.html | yes (path) |
| nvm | `~/.nvm/versions/node` (`NVM_DIR`) | `Suffix[.nvm/versions]` | `nvm install` | Each version holds its own global npm packages ("NVM_BIN: where node, npm, and global packages for the active version of node are installed") | https://github.com/nvm-sh/nvm | yes (`versions/node` from `nvm.sh`) |
| fnm | `~/.local/share/fnm/node-versions`, legacy `~/Library/Application Support/fnm/node-versions` or `~/.fnm` (`FNM_DIR`) | `Suffix[fnm/node-versions]` | `fnm install` | Global npm packages per version, as with nvm | https://github.com/Schniz/fnm/blob/master/src/directories.rs | partly (source only) |
| Volta | `~/.volta/tools/image` (`VOLTA_HOME`) | `Suffix[.volta/tools/image]` | `volta install` / `volta fetch` | Global tools are user choices | https://docs.volta.sh/advanced/installers | **no** (`~/.volta` yes; `tools/image` not documented) |
| Python test and lint caches | `.mypy_cache`, `.pytest_cache`, `.ruff_cache`, `.nox` | `Child{.mypy_cache, CACHEDIR.TAG}` and likewise | Rewritten on the next run | No | source files listed in the CACHEDIR.TAG section | yes |
| Android Studio, Xcode simulators | `~/Library/Developer/CoreSimulator/Devices` | `Suffix[Library/Developer/CoreSimulator/Devices]` | Recreated by Xcode, but each device holds installed apps and their data | **Yes** | none fetched | **no** |

## Recommended rules for hoardmap

Proposed `why` text is in quotes, phrased from the owner's documentation.

### Add as rebuildable

1. **Go build cache**, `Suffix[Library/Caches/go-build]`: "The go command caches
   build outputs here and fills the cache again as it builds."
2. **Yarn cache (classic)**, `Suffix[Library/Caches/Yarn]`: "Yarn's global cache; it
   is populated again the next time yarn or yarn install is run."
3. **Yarn cache (Berry)**, `Suffix[.yarn/berry/cache]`: "Yarn's global package cache;
   packages are retrieved from the registry again on install."
4. **Bun cache**, `Suffix[.bun/install/cache]`: "Bun downloads a package from the
   registry again when it is not in the cache."
5. **Maven local repository**, `Suffix[.m2/repository]`: "Maven caches remote
   downloads here and downloads them again when a build needs them." Mention in the
   UI text that `mvn install` output of unreleased projects also lives here.
6. **Deno cache**, `Suffix[Library/Caches/deno]`: "Deno's cache of downloaded
   dependencies and compiled artefacts; Deno fills it again as it runs."
7. **Poetry cache**, `Suffix[Library/Caches/pypoetry]`: "Poetry's cache and virtual
   environments; poetry install creates them again."
8. **pipenv virtualenvs**, `Suffix[.local/share/virtualenvs]`: "pipenv install
   builds the environment again from Pipfile and Pipfile.lock."
9. **CocoaPods cache**, `Suffix[Library/Caches/CocoaPods]`.
10. **Carthage cache and build**, `Suffix[Library/Caches/org.carthage.CarthageKit]`,
    `Suffix[Carthage/Build]`: "carthage checkout repopulates the cache; carthage
    build writes the frameworks again."
11. **Dart `.dart_tool`**, `Sibling{.dart_tool, pubspec.yaml}`.
12. **Composer cache**, `Suffix[.composer/cache]`.
13. **NuGet packages and HTTP cache**, `Suffix[.nuget/packages]`,
    `Suffix[.local/share/NuGet/v3-cache]`: "NuGet downloads the packages again when
    you restore your projects."
14. **Playwright browsers**, `Suffix[Library/Caches/ms-playwright]`: "npx playwright
    install downloads the browsers again."
15. **Puppeteer browsers**, `Suffix[.cache/puppeteer]`.
16. **sccache**, `Suffix[Library/Caches/Mozilla.sccache]`, and **ccache**,
    `Suffix[Library/Caches/ccache]`: "compiler output cache, filled again by builds."
17. **Bazel output root (Bazel 9+)**, `Suffix[Library/Caches/bazel]`.
18. **Gradle wrapper, JDKs, project `.gradle`, project `build`**, matchers as in the
    table.
19. **Swift Package Manager cache**, `Suffix[Library/Caches/org.swift.swiftpm]`
    (source-only citation; acceptable under the spec if the source URL counts).
20. **Electron downloads**, `Suffix[Library/Caches/electron]`.
21. **Next.js, Turborepo, Parcel**, `Sibling{.next|.turbo|.parcel-cache,
    package.json}`.
22. **`__pycache__`**, `Suffix[__pycache__]`.
23. **Python tool caches**, `Child{.mypy_cache|.pytest_cache|.ruff_cache|.nox,
    CACHEDIR.TAG}`.

### Add with a caution label

- **Docker Desktop disk image**: images come back with a pull, but the same file holds
  your containers and volumes. Label: "Holds images, containers and volumes; only
  the images download again." hoardmap cannot split it without Docker, which the
  spec keeps out of scope; consider this the one-line pointer to `docker system df`.
- **Android emulators (`~/.android/avd`)**: each AVD keeps installed apps, settings and
  an SD card.
- **Android SDK**: re-downloadable through SDK Manager, but projects depend on its
  exact platform versions. `system-images` alone could be plain rebuildable.
- **Downloaded models**: Ollama, LM Studio, PyTorch hub, kagglehub (and the existing
  Hugging Face rule). Large, user-chosen, may be withdrawn upstream; Ollama `create`
  and LM Studio imports are local-only.
- **Conda `pkgs`**: environments link into it, so its Physical figure overlaps them.
- **JetBrains caches**: Local History lives in the same folder.
- **VS Code extensions**, **rustup toolchains**, **nvm / fnm node versions**: they come
  back, but they are a user's chosen set, and nvm/fnm versions carry global npm
  packages.
- **Dart pub cache**: includes globally activated packages.
- **CocoaPods `Pods`**, **Composer `vendor`**: rebuildable from lock files, but the
  owners' docs allow or recommend committing them.
- **Generic build folders** (`dist`, `build`, `out`), only with the sentinels below.

### Do not add

- **Xcode Archives**: Apple says "You must retain the Xcode archive for each build of
  your app you distribute."
- **Yarn project `.yarn/cache`**: with zero-installs it is committed so each commit is
  installable; it is part of the repository.
- **Xcode iOS DeviceSupport, CoreSimulator Devices, VS Code CachedData**: no primary
  source found; simulators also hold app data. Revisit when Apple or Microsoft
  documents them.
- **Bazel 8 output root** (`/private/var/tmp/_bazel_$USER`) and a **generic
  CACHEDIR.TAG group**: not matchable with the three kinds (user-specific name, any
  folder name), and the tag is only a hint.
- **Whole tool homes** (`~/.gradle`, `~/.m2`, `~/.composer`, `~/.nuget`, `~/.ccache`,
  `~/.cargo`): each holds user config or credentials beside the cache. Match only the
  cache subfolder.
- **Volta tools** until a documented path is found.

## False-positive risks for generic names

`dist`, `build` and `out` are common names for folders people make by hand (a `build`
folder of design files, an `out` folder of exports). Name alone is not enough. Use
the build tool's manifest as a sibling sentinel, and keep these in the caution group,
because a published library may commit `dist`, and hoardmap cannot see git status.

| Folder | Sibling sentinel | Owner | Remaining risk |
| --- | --- | --- | --- |
| `dist` | `vite.config.js`/`.ts`/`.mjs` (Vite's `build.outDir` "Default: dist", https://vite.dev/config/build-options) | Vite | Low |
| `dist` | `package.json` | npm packages generally | Libraries that commit `dist`; any repo with a hand-made `dist` |
| `dist` | `pyproject.toml` or `setup.py` | Python packaging | It holds the built wheels and sdists you may be about to upload |
| `build` | `build.gradle`, `build.gradle.kts` | Gradle (documented above) | Low |
| `build` | `pubspec.yaml` | Flutter | Low; unverified |
| `build` | `package.json` | Create React App and others | Medium |
| `build` | `CMakeLists.txt` | CMake in-source build folders | Medium: the folder name is the user's choice and it may hold configured caches they tuned |
| `out` | `next.config.js`/`.mjs`/`.ts` | Next.js static export | Low; not verified this session |
| `out` | none reliable | IntelliJ, Rust `cargo --out-dir`, Go tools | Do not match |
| `target` | `Cargo.toml` (in addition to the current `Child{target, CACHEDIR.TAG}`) | Cargo | Maven also uses `target` beside `pom.xml`; worth a separate Maven rule |
| `vendor` | `composer.json` / `go.mod` | Composer / Go | Often committed |

Two more risks apply to every sibling rule: a sentinel can match a folder in an
unrelated tree (a `build` beside a stray `package.json` in a downloads folder), and a
rule only sees names, so a folder that was committed to git looks the same as one
the tool wrote. Neither is fixable without reading contents or running git, which the
spec rules out; the caution label and the source link are the mitigation.

## Unverified or partly verified

- Xcode iOS DeviceSupport, CoreSimulator Devices, VS Code CachedData: no Apple or
  Microsoft page found.
- Volta's `tools/image` path; Flutter's `build`; Next.js `out` for static export.
- Docker volumes inside `Docker.raw`: inferred from the engine's
  `/var/lib/docker/volumes` and the Desktop FAQ's "containers and images", not stated
  together on one page.
- Turborepo's refill on a cache miss is implied, not stated.
- Conda's hard-linking into `pkgs` is taken from the `conda clean` warning, not a page
  that describes it.
- Ollama `create` and pub global packages being local-only come from general
  knowledge, not the cited pages.
- Negative CACHEDIR.TAG findings (pip, sccache, Bazel, Deno, Bun, Yarn, pnpm, Mesa) rest
  on code search and a few source files; a writer in an unsearched file is possible.
- Where npm's cacache writes its tag under `~/.npm` was not traced.
