//! Rebuildable folders: the folders a tool fills again on its own, such as
//! `node_modules`, Cargo's `target` and Python's `.venv`, gathered from the finished
//! tree into one group per rule.
//!
//! [`RULES`] is the table; each rule names its owner, says how the folder comes back
//! in the owner's words, and links the owner's documentation; a rule whose folder
//! also holds something that does not come back carries a caution. [`groups`] is a pure
//! function of the tree: no file system access, no subprocess, and nothing here
//! changes a file.

use std::cmp::Reverse;

use crate::tree::{Kind, NodeId, Tree};
use crate::treemap::Mode;

/// At most this many folders per group are returned; the rest are counted in
/// [`Group::more`].
pub const MAX_FOLDERS: usize = 500;

/// How a rule recognises a folder, from names already in the tree.
#[derive(Debug)]
pub enum Matcher {
    /// A folder called `name` that contains an entry called `child`.
    ChildSentinel {
        name: &'static str,
        child: &'static str,
    },
    /// A folder called `name` whose parent also contains an entry called `sibling`.
    SiblingSentinel {
        name: &'static str,
        sibling: &'static str,
    },
    /// A folder whose path ends in these names.
    PathSuffix(&'static [&'static str]),
    /// A folder of any name that contains an entry called `child`.
    AnyWithChild { child: &'static str },
}

/// One kind of rebuildable folder.
#[derive(Debug)]
pub struct Rule {
    pub key: &'static str,
    pub label: &'static str,
    /// The tool or vendor that fills the folder.
    pub owner: &'static str,
    /// One line on how the folder comes back.
    pub why: &'static str,
    /// The owner's own documentation, `https://`.
    pub source: &'static str,
    /// The date `source` was last read against `why`, `YYYY-MM-DD`.
    pub checked: &'static str,
    /// One sentence on what in the folder does not come back on its own, or `None`
    /// when all of it does.
    pub caution: Option<&'static str>,
    /// A folder matches the rule when any of these match.
    pub matchers: &'static [Matcher],
}

const CHECKED: &str = "2026-10-30";

/// The rules in match order: where two rules match one folder, the earlier one takes
/// it. Rules for a named folder come before rules for a generic name (`build`, `dist`,
/// `out`), and the cache tag rule, which matches any name, comes last.
pub const RULES: &[Rule] = &[
    // Project folders, recognised by a file the tool keeps beside or inside them.
    Rule {
        key: "node_modules",
        label: "node_modules",
        owner: "npm",
        why: "npm install installs the packages again from package.json and its lock file.",
        source: "https://docs.npmjs.com/cli/v10/commands/npm-install",
        checked: CHECKED,
        caution: None,
        matchers: &[Matcher::SiblingSentinel {
            name: "node_modules",
            sibling: "package.json",
        }],
    },
    Rule {
        key: "cargo_target",
        label: "Cargo target",
        owner: "Cargo",
        why: "Cargo stores the output of a build here; cargo build writes it again.",
        source: "https://doc.rust-lang.org/cargo/reference/build-cache.html",
        checked: CHECKED,
        caution: None,
        matchers: &[Matcher::ChildSentinel {
            name: "target",
            child: "CACHEDIR.TAG",
        }],
    },
    Rule {
        key: "python_venv",
        label: "Python virtual environments",
        owner: "Python",
        why: "python -m venv creates it again, and pip install -r requirements.txt reinstalls its packages.",
        source: "https://docs.python.org/3/library/venv.html",
        checked: CHECKED,
        caution: None,
        matchers: &[
            Matcher::ChildSentinel {
                name: ".venv",
                child: "pyvenv.cfg",
            },
            Matcher::ChildSentinel {
                name: "venv",
                child: "pyvenv.cfg",
            },
        ],
    },
    Rule {
        key: "swiftpm_build",
        label: "Swift package builds",
        owner: "Swift Package Manager",
        why: "swift build writes the build output here again from Package.swift and the sources.",
        source: "https://www.swift.org/documentation/server/guides/building.html",
        checked: CHECKED,
        caution: None,
        matchers: &[Matcher::SiblingSentinel {
            name: ".build",
            sibling: "Package.swift",
        }],
    },
    Rule {
        key: "gradle_project",
        label: "Gradle project caches",
        owner: "Gradle",
        why: "Gradle keeps version-specific build state for incremental builds here and writes it again on the next build.",
        source: "https://docs.gradle.org/current/userguide/directory_layout.html",
        checked: CHECKED,
        caution: None,
        matchers: &[
            Matcher::SiblingSentinel {
                name: ".gradle",
                sibling: "settings.gradle",
            },
            Matcher::SiblingSentinel {
                name: ".gradle",
                sibling: "settings.gradle.kts",
            },
            Matcher::SiblingSentinel {
                name: ".gradle",
                sibling: "build.gradle",
            },
            Matcher::SiblingSentinel {
                name: ".gradle",
                sibling: "build.gradle.kts",
            },
        ],
    },
    Rule {
        key: "gradle_build",
        label: "Gradle build output",
        owner: "Gradle",
        why: "Gradle writes build output and the transient files it uses for incremental builds here; the next build writes them again.",
        source: "https://docs.gradle.org/current/userguide/directory_layout.html",
        checked: CHECKED,
        caution: None,
        matchers: &[
            Matcher::SiblingSentinel {
                name: "build",
                sibling: "build.gradle",
            },
            Matcher::SiblingSentinel {
                name: "build",
                sibling: "build.gradle.kts",
            },
        ],
    },
    Rule {
        key: "next_build",
        label: "Next.js builds",
        owner: "Next.js",
        why: "next build writes its build output here again.",
        source: "https://nextjs.org/docs/app/api-reference/config/next-config-js/distDir",
        checked: CHECKED,
        caution: None,
        matchers: &[Matcher::SiblingSentinel {
            name: ".next",
            sibling: "package.json",
        }],
    },
    Rule {
        key: "turbo_cache",
        label: "Turborepo cache",
        owner: "Turborepo",
        why: "Turborepo stores the results of tasks here and stores them again when the tasks run.",
        source: "https://turborepo.dev/docs/crafting-your-repository/caching",
        checked: CHECKED,
        caution: None,
        matchers: &[Matcher::SiblingSentinel {
            name: ".turbo",
            sibling: "package.json",
        }],
    },
    Rule {
        key: "parcel_cache",
        label: "Parcel cache",
        owner: "Parcel",
        why: "Parcel keeps its build cache here and fills it again on the next build.",
        source: "https://parceljs.org/features/development/",
        checked: CHECKED,
        caution: None,
        matchers: &[Matcher::SiblingSentinel {
            name: ".parcel-cache",
            sibling: "package.json",
        }],
    },
    Rule {
        key: "dart_tool",
        label: "Dart .dart_tool",
        owner: "Dart",
        why: "dart pub get writes the package configuration here again from pubspec.yaml.",
        source: "https://dart.dev/tools/pub/glossary",
        checked: CHECKED,
        caution: None,
        matchers: &[Matcher::SiblingSentinel {
            name: ".dart_tool",
            sibling: "pubspec.yaml",
        }],
    },
    Rule {
        key: "python_bytecode",
        label: "Python bytecode",
        owner: "Python",
        why: "Python compiles modules into __pycache__ and compiles them again when the source is newer; it is a completely automatic process.",
        source: "https://docs.python.org/3/tutorial/modules.html#compiled-python-files",
        checked: CHECKED,
        caution: None,
        matchers: &[Matcher::PathSuffix(&["__pycache__"])],
    },
    Rule {
        key: "mypy_cache",
        label: "mypy cache",
        owner: "mypy",
        why: "mypy caches type information here, tags the folder CACHEDIR.TAG, and writes it again on the next run.",
        source: "https://github.com/python/mypy/blob/master/mypy/build.py",
        checked: CHECKED,
        caution: None,
        matchers: &[Matcher::ChildSentinel {
            name: ".mypy_cache",
            child: "CACHEDIR.TAG",
        }],
    },
    Rule {
        key: "pytest_cache",
        label: "pytest cache",
        owner: "pytest",
        why: "pytest keeps state between runs here, tags the folder CACHEDIR.TAG, and writes it again on the next run.",
        source: "https://github.com/pytest-dev/pytest/blob/main/src/_pytest/cacheprovider.py",
        checked: CHECKED,
        caution: None,
        matchers: &[Matcher::ChildSentinel {
            name: ".pytest_cache",
            child: "CACHEDIR.TAG",
        }],
    },
    Rule {
        key: "ruff_cache",
        label: "Ruff cache",
        owner: "Ruff",
        why: "Ruff caches lint results here, tags the folder CACHEDIR.TAG, and writes it again on the next run.",
        source: "https://github.com/astral-sh/ruff/blob/main/crates/ruff/src/cache.rs",
        checked: CHECKED,
        caution: None,
        matchers: &[Matcher::ChildSentinel {
            name: ".ruff_cache",
            child: "CACHEDIR.TAG",
        }],
    },
    Rule {
        key: "nox_envs",
        label: "nox environments",
        owner: "nox",
        why: "nox keeps its session environments here, tags the folder CACHEDIR.TAG, and creates them again when a session runs.",
        source: "https://github.com/wntrblm/nox/blob/main/nox/virtualenv.py",
        checked: CHECKED,
        caution: None,
        matchers: &[Matcher::ChildSentinel {
            name: ".nox",
            child: "CACHEDIR.TAG",
        }],
    },
    // Caches and downloads at fixed paths under the home folder.
    Rule {
        key: "xcode_derived_data",
        label: "Xcode DerivedData",
        owner: "Xcode",
        why: "Xcode keeps precompiled headers, indexes and build products here and writes them again when it builds and indexes a project.",
        source: "https://developer.apple.com/library/archive/documentation/Xcode/Conceptual/RN-Xcode-Archive/Chapters/xc4_release_notes.html",
        checked: CHECKED,
        caution: None,
        matchers: &[Matcher::PathSuffix(&[
            "Library",
            "Developer",
            "Xcode",
            "DerivedData",
        ])],
    },
    Rule {
        key: "gradle_caches",
        label: "Gradle caches",
        owner: "Gradle",
        why: "Gradle creates, uses and cleans these caches itself and recreates them on first use.",
        source: "https://docs.gradle.org/current/userguide/directory_layout.html",
        checked: CHECKED,
        caution: None,
        matchers: &[Matcher::PathSuffix(&[".gradle", "caches"])],
    },
    Rule {
        key: "gradle_downloads",
        label: "Gradle wrapper and JDKs",
        owner: "Gradle",
        why: "Distributions downloaded by the Gradle Wrapper and JDKs downloaded by the toolchain support; Gradle downloads them again when a build needs them.",
        source: "https://docs.gradle.org/current/userguide/directory_layout.html",
        checked: CHECKED,
        caution: None,
        matchers: &[
            Matcher::PathSuffix(&[".gradle", "wrapper", "dists"]),
            Matcher::PathSuffix(&[".gradle", "jdks"]),
        ],
    },
    Rule {
        key: "maven_repository",
        label: "Maven local repository",
        owner: "Maven",
        why: "Maven caches remote downloads here and downloads them again when a build needs them.",
        source: "https://maven.apache.org/guides/introduction/introduction-to-repositories.html",
        checked: CHECKED,
        caution: Some(
            "It also holds what mvn install built from your own unreleased projects, which comes back only by building them again.",
        ),
        matchers: &[Matcher::PathSuffix(&[".m2", "repository"])],
    },
    Rule {
        key: "homebrew_cache",
        label: "Homebrew downloads",
        owner: "Homebrew",
        why: "Homebrew's download cache (brew --cache); brew downloads a bottle or source again when an install needs it.",
        source: "https://docs.brew.sh/FAQ",
        checked: CHECKED,
        caution: None,
        matchers: &[Matcher::PathSuffix(&["Library", "Caches", "Homebrew"])],
    },
    Rule {
        key: "pip_cache",
        label: "pip cache",
        owner: "pip",
        why: "pip's cache of downloads and built wheels (pip cache dir); pip downloads or builds a package again when it is not cached.",
        source: "https://pip.pypa.io/en/stable/topics/caching/",
        checked: CHECKED,
        caution: None,
        matchers: &[Matcher::PathSuffix(&["Library", "Caches", "pip"])],
    },
    Rule {
        key: "poetry_cache",
        label: "Poetry cache and environments",
        owner: "Poetry",
        why: "Poetry's cache and virtual environments; poetry install creates them again.",
        source: "https://python-poetry.org/docs/configuration/",
        checked: CHECKED,
        caution: None,
        matchers: &[Matcher::PathSuffix(&["Library", "Caches", "pypoetry"])],
    },
    Rule {
        key: "pipenv_virtualenvs",
        label: "pipenv environments",
        owner: "pipenv",
        why: "pipenv install builds the environment again from Pipfile and Pipfile.lock.",
        source: "https://pipenv.pypa.io/en/latest/virtualenv.html",
        checked: CHECKED,
        caution: None,
        matchers: &[Matcher::PathSuffix(&[".local", "share", "virtualenvs"])],
    },
    Rule {
        key: "uv_cache",
        label: "uv cache",
        owner: "uv",
        why: "uv's cache of downloaded and built packages; uv fills it again as it installs.",
        source: "https://docs.astral.sh/uv/concepts/cache/",
        checked: CHECKED,
        caution: None,
        matchers: &[Matcher::PathSuffix(&[".cache", "uv"])],
    },
    Rule {
        key: "npm_cache",
        label: "npm cache",
        owner: "npm",
        why: "npm's content-addressable cache of downloaded packages; npm fetches missing data again from the registry.",
        source: "https://docs.npmjs.com/cli/v10/commands/npm-cache",
        checked: CHECKED,
        caution: None,
        matchers: &[Matcher::PathSuffix(&[".npm", "_cacache"])],
    },
    Rule {
        key: "pnpm_store",
        label: "pnpm store",
        owner: "pnpm",
        why: "pnpm's package store; if a later install needs a package that is not in it, pnpm downloads it again.",
        source: "https://pnpm.io/cli/store",
        checked: CHECKED,
        caution: None,
        matchers: &[Matcher::PathSuffix(&["Library", "pnpm", "store"])],
    },
    Rule {
        key: "yarn_cache",
        label: "Yarn cache",
        owner: "Yarn",
        why: "Yarn's global cache; it is populated again the next time yarn or yarn install is run.",
        source: "https://classic.yarnpkg.com/lang/en/docs/cli/cache/",
        checked: CHECKED,
        caution: None,
        matchers: &[Matcher::PathSuffix(&["Library", "Caches", "Yarn"])],
    },
    Rule {
        key: "yarn_berry_cache",
        label: "Yarn global cache",
        owner: "Yarn",
        why: "Yarn's global package cache; packages are retrieved from the registry again on install.",
        source: "https://yarnpkg.com/configuration/yarnrc",
        checked: CHECKED,
        caution: None,
        matchers: &[Matcher::PathSuffix(&[".yarn", "berry", "cache"])],
    },
    Rule {
        key: "bun_cache",
        label: "Bun cache",
        owner: "Bun",
        why: "Bun downloads a package from the registry again when it is not in the cache.",
        source: "https://bun.sh/docs/install/cache",
        checked: CHECKED,
        caution: None,
        matchers: &[Matcher::PathSuffix(&[".bun", "install", "cache"])],
    },
    Rule {
        key: "deno_cache",
        label: "Deno cache",
        owner: "Deno",
        why: "Deno's cache of downloaded dependencies and compiled artefacts; Deno fills it again as it runs.",
        source: "https://docs.deno.com/runtime/getting_started/installation/",
        checked: CHECKED,
        caution: None,
        matchers: &[Matcher::PathSuffix(&["Library", "Caches", "deno"])],
    },
    Rule {
        key: "cargo_registry",
        label: "Cargo registry",
        owner: "Cargo",
        why: "Crates Cargo downloaded from crates.io and unpacked; cargo downloads them again when a build needs them.",
        source: "https://doc.rust-lang.org/cargo/guide/cargo-home.html",
        checked: CHECKED,
        caution: None,
        matchers: &[Matcher::PathSuffix(&[".cargo", "registry"])],
    },
    Rule {
        key: "go_modules",
        label: "Go module cache",
        owner: "Go",
        why: "The go command downloads modules into this cache again when a build needs them.",
        source: "https://go.dev/ref/mod",
        checked: CHECKED,
        caution: None,
        matchers: &[Matcher::PathSuffix(&["go", "pkg", "mod"])],
    },
    Rule {
        key: "go_build_cache",
        label: "Go build cache",
        owner: "Go",
        why: "The go command caches build outputs here and fills the cache again as it builds.",
        source: "https://pkg.go.dev/cmd/go#hdr-Build_and_test_caching",
        checked: CHECKED,
        caution: None,
        matchers: &[Matcher::PathSuffix(&["Library", "Caches", "go-build"])],
    },
    Rule {
        key: "cocoapods_cache",
        label: "CocoaPods cache",
        owner: "CocoaPods",
        why: "CocoaPods' download cache (pod cache); pod install fetches a pod again when it is not cached.",
        source: "https://guides.cocoapods.org/terminal/commands.html#pod_cache_clean",
        checked: CHECKED,
        caution: None,
        matchers: &[Matcher::PathSuffix(&["Library", "Caches", "CocoaPods"])],
    },
    Rule {
        key: "carthage",
        label: "Carthage cache and builds",
        owner: "Carthage",
        why: "carthage checkout repopulates the cache, and carthage build writes the frameworks again.",
        source: "https://github.com/Carthage/Carthage/blob/master/Documentation/Artifacts.md",
        checked: CHECKED,
        caution: None,
        matchers: &[
            Matcher::PathSuffix(&["Library", "Caches", "org.carthage.CarthageKit"]),
            Matcher::PathSuffix(&["Carthage", "Build"]),
        ],
    },
    Rule {
        key: "swiftpm_cache",
        label: "Swift Package Manager cache",
        owner: "Swift Package Manager",
        why: "SwiftPM's repository and manifest caches; it fetches them again when it resolves packages.",
        source: "https://github.com/swiftlang/swift-package-manager/blob/main/Sources/Basics/FileSystem/FileSystem%2BExtensions.swift",
        checked: CHECKED,
        caution: None,
        matchers: &[Matcher::PathSuffix(&[
            "Library",
            "Caches",
            "org.swift.swiftpm",
        ])],
    },
    Rule {
        key: "composer_cache",
        label: "Composer cache",
        owner: "Composer",
        why: "Composer's download cache; composer install downloads a package again when it is not cached.",
        source: "https://getcomposer.org/doc/03-cli.md",
        checked: CHECKED,
        caution: None,
        matchers: &[Matcher::PathSuffix(&[".composer", "cache"])],
    },
    Rule {
        key: "nuget",
        label: "NuGet packages and cache",
        owner: "NuGet",
        why: "NuGet downloads the packages again when you restore your projects.",
        source: "https://learn.microsoft.com/en-us/nuget/consume-packages/managing-the-global-packages-and-cache-folders",
        checked: CHECKED,
        caution: None,
        matchers: &[
            Matcher::PathSuffix(&[".nuget", "packages"]),
            Matcher::PathSuffix(&[".local", "share", "NuGet", "v3-cache"]),
        ],
    },
    Rule {
        key: "playwright_browsers",
        label: "Playwright browsers",
        owner: "Playwright",
        why: "npx playwright install downloads the browsers again.",
        source: "https://playwright.dev/docs/browsers",
        checked: CHECKED,
        caution: None,
        matchers: &[Matcher::PathSuffix(&["Library", "Caches", "ms-playwright"])],
    },
    Rule {
        key: "puppeteer_browsers",
        label: "Puppeteer browsers",
        owner: "Puppeteer",
        why: "npx puppeteer browsers install downloads the browsers again.",
        source: "https://pptr.dev/guides/configuration",
        checked: CHECKED,
        caution: None,
        matchers: &[Matcher::PathSuffix(&[".cache", "puppeteer"])],
    },
    Rule {
        key: "electron_cache",
        label: "Electron downloads",
        owner: "Electron",
        why: "Electron's download cache; @electron/get downloads an asset again when it is not cached.",
        source: "https://github.com/electron/get",
        checked: CHECKED,
        caution: None,
        matchers: &[Matcher::PathSuffix(&["Library", "Caches", "electron"])],
    },
    Rule {
        key: "sccache",
        label: "sccache",
        owner: "sccache",
        why: "sccache's compiler output cache, filled again by builds.",
        source: "https://github.com/mozilla/sccache/blob/main/docs/Local.md",
        checked: CHECKED,
        caution: None,
        matchers: &[Matcher::PathSuffix(&[
            "Library",
            "Caches",
            "Mozilla.sccache",
        ])],
    },
    Rule {
        key: "ccache",
        label: "ccache",
        owner: "ccache",
        why: "ccache's compiler output cache, filled again by builds.",
        source: "https://ccache.dev/manual/latest.html",
        checked: CHECKED,
        caution: None,
        matchers: &[Matcher::PathSuffix(&["Library", "Caches", "ccache"])],
    },
    Rule {
        key: "bazel_output",
        label: "Bazel output root",
        owner: "Bazel",
        why: "Bazel's output root (Bazel 9 and later); the next build writes it again.",
        source: "https://bazel.build/remote/output-directories",
        checked: CHECKED,
        caution: None,
        matchers: &[Matcher::PathSuffix(&["Library", "Caches", "bazel"])],
    },
    // Rebuildable, but holding something that does not come back on its own.
    Rule {
        key: "docker_disk",
        label: "Docker Desktop disk image",
        owner: "Docker",
        why: "Docker Desktop stores Linux containers and images in one large disk image here; images download again with a pull.",
        source: "https://docs.docker.com/desktop/troubleshoot-and-support/faqs/macfaqs/",
        checked: CHECKED,
        caution: Some(
            "It holds images, containers and volumes, and only the images download again.",
        ),
        matchers: &[Matcher::PathSuffix(&[
            "Library",
            "Containers",
            "com.docker.docker",
            "Data",
            "vms",
        ])],
    },
    Rule {
        key: "android_avd",
        label: "Android emulators",
        owner: "Android Studio",
        why: "Device Manager creates a virtual device again from a system image.",
        source: "https://developer.android.com/studio/run/managing-avds",
        checked: CHECKED,
        caution: Some(
            "Each virtual device keeps its installed apps, settings and emulated SD card, which do not come back.",
        ),
        matchers: &[Matcher::PathSuffix(&[".android", "avd"])],
    },
    Rule {
        key: "android_sdk",
        label: "Android SDK",
        owner: "Android Studio",
        why: "SDK Manager downloads SDK packages and system images again.",
        source: "https://developer.android.com/studio/intro/update",
        checked: CHECKED,
        caution: Some(
            "Your projects build against the exact platform versions installed here, which you would have to choose again.",
        ),
        matchers: &[Matcher::PathSuffix(&["Library", "Android", "sdk"])],
    },
    Rule {
        key: "huggingface_hub",
        label: "Hugging Face models and datasets",
        owner: "Hugging Face",
        why: "huggingface_hub's download cache; files are downloaded again from the Hub when a program asks for them.",
        source: "https://huggingface.co/docs/huggingface_hub/guides/manage-cache",
        checked: CHECKED,
        caution: Some(MODELS_CAUTION),
        matchers: &[Matcher::PathSuffix(&[".cache", "huggingface", "hub"])],
    },
    Rule {
        key: "ollama_models",
        label: "Ollama models",
        owner: "Ollama",
        why: "ollama pull downloads a model again.",
        source: "https://docs.ollama.com/faq",
        checked: CHECKED,
        caution: Some(MODELS_CAUTION),
        matchers: &[Matcher::PathSuffix(&[".ollama", "models"])],
    },
    Rule {
        key: "lmstudio_models",
        label: "LM Studio models",
        owner: "LM Studio",
        why: "LM Studio downloads a model again from its catalogue.",
        source: "https://lmstudio.ai/docs/app/advanced/import-model",
        checked: CHECKED,
        caution: Some(
            "Models imported from your own files share this folder and do not download again, and the downloaded ones are large and your choice.",
        ),
        matchers: &[Matcher::PathSuffix(&[".lmstudio", "models"])],
    },
    Rule {
        key: "torch_hub",
        label: "PyTorch hub models",
        owner: "PyTorch",
        why: "PyTorch hub stores downloaded models and weights here and downloads them again when they are missing.",
        source: "https://docs.pytorch.org/docs/main/hub.html",
        checked: CHECKED,
        caution: Some(MODELS_CAUTION),
        matchers: &[Matcher::PathSuffix(&[".cache", "torch", "hub"])],
    },
    Rule {
        key: "kagglehub",
        label: "Kaggle models and datasets",
        owner: "Kaggle",
        why: "kagglehub downloads files to this folder and downloads them again when a program asks for them.",
        source: "https://github.com/Kaggle/kagglehub",
        checked: CHECKED,
        caution: Some(MODELS_CAUTION),
        matchers: &[Matcher::PathSuffix(&[".cache", "kagglehub"])],
    },
    Rule {
        key: "conda_pkgs",
        label: "Conda package cache",
        owner: "Conda",
        why: "Conda's package cache; conda downloads a package again when an environment needs it.",
        source: "https://docs.conda.io/projects/conda/en/stable/commands/clean.html",
        checked: CHECKED,
        caution: Some(
            "Your environments link to files in this cache, so its size overlaps theirs and some environments rely on it.",
        ),
        matchers: &[Matcher::SiblingSentinel {
            name: "pkgs",
            sibling: "conda-meta",
        }],
    },
    Rule {
        key: "jetbrains_caches",
        label: "JetBrains caches",
        owner: "JetBrains",
        why: "JetBrains IDEs keep caches and indexes here and build them again.",
        source: "https://www.jetbrains.com/help/idea/directories-used-by-the-ide-to-store-settings-caches-plugins-and-logs.html",
        checked: CHECKED,
        caution: Some("Local History lives in the same folder and does not come back."),
        matchers: &[Matcher::PathSuffix(&["Library", "Caches", "JetBrains"])],
    },
    Rule {
        key: "vscode_extensions",
        label: "VS Code extensions",
        owner: "Visual Studio Code",
        why: "VS Code installs extensions again from the Marketplace.",
        source: "https://code.visualstudio.com/docs/configure/extensions/extension-marketplace",
        checked: CHECKED,
        caution: Some(
            "You chose this set, and extensions installed from a VSIX file or withdrawn from the Marketplace do not come back.",
        ),
        matchers: &[Matcher::PathSuffix(&[".vscode", "extensions"])],
    },
    Rule {
        key: "rustup_toolchains",
        label: "Rust toolchains",
        owner: "rustup",
        why: "rustup toolchain install downloads a toolchain again.",
        source: "https://rust-lang.github.io/rustup/environment-variables.html",
        checked: CHECKED,
        caution: Some(
            "You chose which toolchains are installed, and rustup does not reinstall them on its own.",
        ),
        matchers: &[Matcher::PathSuffix(&[".rustup", "toolchains"])],
    },
    Rule {
        key: "nvm_versions",
        label: "nvm Node.js versions",
        owner: "nvm",
        why: "nvm install downloads a Node.js version again.",
        source: "https://github.com/nvm-sh/nvm",
        checked: CHECKED,
        caution: Some(NODE_VERSIONS_CAUTION),
        matchers: &[Matcher::PathSuffix(&[".nvm", "versions"])],
    },
    Rule {
        key: "fnm_versions",
        label: "fnm Node.js versions",
        owner: "fnm",
        why: "fnm install downloads a Node.js version again.",
        source: "https://github.com/Schniz/fnm/blob/master/src/directories.rs",
        checked: CHECKED,
        caution: Some(NODE_VERSIONS_CAUTION),
        matchers: &[Matcher::PathSuffix(&["fnm", "node-versions"])],
    },
    Rule {
        key: "pub_cache",
        label: "Dart pub cache",
        owner: "Dart",
        why: "When pub gets a remote package, it downloads it into this cache again.",
        source: "https://dart.dev/tools/pub/glossary",
        checked: CHECKED,
        caution: Some(
            "It also holds the packages you activated globally, which pub does not bring back on its own.",
        ),
        matchers: &[Matcher::PathSuffix(&[".pub-cache"])],
    },
    Rule {
        key: "cocoapods_pods",
        label: "CocoaPods Pods",
        owner: "CocoaPods",
        why: "pod install recreates the same installation from Podfile.lock.",
        source: "https://guides.cocoapods.org/using/using-cocoapods.html",
        checked: CHECKED,
        caution: Some(
            "CocoaPods recommends keeping Pods in source control, so this folder may be part of your repository.",
        ),
        matchers: &[Matcher::SiblingSentinel {
            name: "Pods",
            sibling: "Podfile.lock",
        }],
    },
    Rule {
        key: "composer_vendor",
        label: "Composer vendor",
        owner: "Composer",
        why: "composer install downloads the dependencies' files into vendor again from composer.lock.",
        source: "https://getcomposer.org/doc/01-basic-usage.md",
        checked: CHECKED,
        caution: Some(COMMITTED_CAUTION),
        matchers: &[Matcher::SiblingSentinel {
            name: "vendor",
            sibling: "composer.json",
        }],
    },
    // Generic names, only beside the project file that explains them.
    Rule {
        key: "flutter_build",
        label: "Flutter build output",
        owner: "Flutter",
        why: "flutter build writes the app bundles and packages here again.",
        source: "https://docs.flutter.dev/deployment/android",
        checked: CHECKED,
        caution: Some(GENERIC_CAUTION),
        matchers: &[Matcher::SiblingSentinel {
            name: "build",
            sibling: "pubspec.yaml",
        }],
    },
    Rule {
        key: "next_export",
        label: "Next.js static exports",
        owner: "Next.js",
        why: "With a static export, next build creates the out folder again.",
        source: "https://nextjs.org/docs/app/guides/static-exports",
        checked: CHECKED,
        caution: Some(GENERIC_CAUTION),
        matchers: &[
            Matcher::SiblingSentinel {
                name: "out",
                sibling: "next.config.js",
            },
            Matcher::SiblingSentinel {
                name: "out",
                sibling: "next.config.mjs",
            },
            Matcher::SiblingSentinel {
                name: "out",
                sibling: "next.config.ts",
            },
        ],
    },
    Rule {
        key: "js_dist",
        label: "JavaScript dist folders",
        owner: "Vite and other bundlers",
        why: "Bundlers such as Vite write the production build to dist, and the build command writes it again.",
        source: "https://vite.dev/config/build-options",
        checked: CHECKED,
        caution: Some(COMMITTED_CAUTION),
        matchers: &[
            Matcher::SiblingSentinel {
                name: "dist",
                sibling: "vite.config.js",
            },
            Matcher::SiblingSentinel {
                name: "dist",
                sibling: "vite.config.ts",
            },
            Matcher::SiblingSentinel {
                name: "dist",
                sibling: "vite.config.mjs",
            },
            Matcher::SiblingSentinel {
                name: "dist",
                sibling: "package.json",
            },
        ],
    },
    Rule {
        key: "js_build",
        label: "JavaScript build folders",
        owner: "Create React App and others",
        why: "npm run build creates a build directory with a production build of the app.",
        source: "https://create-react-app.dev/docs/production-build/",
        checked: CHECKED,
        caution: Some(GENERIC_CAUTION),
        matchers: &[Matcher::SiblingSentinel {
            name: "build",
            sibling: "package.json",
        }],
    },
    Rule {
        key: "python_dist",
        label: "Python package builds",
        owner: "Python packaging",
        why: "python -m build writes the wheel and source archive into dist again.",
        source: "https://packaging.python.org/en/latest/tutorials/packaging-projects/",
        checked: CHECKED,
        caution: Some(
            "It holds the built packages you may be about to upload, and a repository can keep a hand-made dist beside its project file.",
        ),
        matchers: &[
            Matcher::SiblingSentinel {
                name: "dist",
                sibling: "pyproject.toml",
            },
            Matcher::SiblingSentinel {
                name: "dist",
                sibling: "setup.py",
            },
        ],
    },
    Rule {
        key: "cmake_build",
        label: "CMake build trees",
        owner: "CMake",
        why: "CMake writes the build system and build output into this build tree and creates it again when you configure the project.",
        source: "https://cmake.org/cmake/help/latest/manual/cmake.1.html",
        checked: CHECKED,
        caution: Some(
            "You chose this folder's name, and its CMakeCache.txt keeps the options you configured, which a fresh configure does not bring back.",
        ),
        matchers: &[Matcher::SiblingSentinel {
            name: "build",
            sibling: "CMakeLists.txt",
        }],
    },
    // Any other folder its tool tagged as a cache. Last, so a named rule wins.
    Rule {
        key: "cache_tag",
        label: "Marked as a cache",
        owner: "The tool that made it",
        why: "A program put a CACHEDIR.TAG file here, which marks the folder as cached data it can generate again.",
        source: "https://bford.info/cachedir/",
        checked: CHECKED,
        caution: Some(
            "The tag is only a hint: it can be committed to a repository, and a program can put it on a folder its user chose.",
        ),
        matchers: &[Matcher::AnyWithChild {
            child: "CACHEDIR.TAG",
        }],
    },
];

const MODELS_CAUTION: &str = "You chose these models and they are large to download again, and a model withdrawn upstream does not come back.";
const NODE_VERSIONS_CAUTION: &str = "You chose which versions are installed, and each one keeps its own global npm packages, which do not come back on their own.";
const COMMITTED_CAUTION: &str = "Some projects keep this folder in their repository, and hoardmap cannot tell a committed folder from one the tool wrote.";
const GENERIC_CAUTION: &str = "The name is common, so a folder you made yourself beside the same project file looks the same to hoardmap.";

/// One rule's matches below a root, largest first.
#[derive(Debug)]
pub struct Group {
    pub rule: &'static Rule,
    /// The total of every matched folder in the chosen mode.
    pub bytes: u64,
    /// Every matched folder, including those past [`MAX_FOLDERS`].
    pub count: usize,
    /// The largest [`MAX_FOLDERS`] folders, largest first.
    pub folders: Vec<Folder>,
    /// Matched folders left out of `folders`.
    pub more: usize,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Folder {
    pub id: NodeId,
    pub bytes: u64,
}

impl Matcher {
    /// Whether the directory `id`, whose path ends in `path`, matches.
    fn matches(&self, tree: &Tree, id: NodeId, path: &[&str]) -> bool {
        let node = &tree.nodes[id as usize];
        let has_child = |dir: NodeId, child: &str| {
            tree.nodes[dir as usize]
                .children
                .iter()
                .any(|&c| tree.nodes[c as usize].name == child)
        };
        match *self {
            Matcher::ChildSentinel { name, child } => node.name == name && has_child(id, child),
            Matcher::SiblingSentinel { name, sibling } => {
                node.name == name && node.parent.is_some_and(|p| has_child(p, sibling))
            }
            Matcher::PathSuffix(suffix) => path.ends_with(suffix),
            Matcher::AnyWithChild { child } => has_child(id, child),
        }
    }
}

/// The names from the file system root down to `id`: the scan root's full path split
/// into its components, then each name below it.
fn path_names(tree: &Tree, id: NodeId) -> Vec<&str> {
    let mut chain = tree.ancestors(id);
    chain.reverse();
    let mut names: Vec<&str> = Vec::new();
    for i in chain {
        let node = &tree.nodes[i as usize];
        if node.parent.is_none() {
            names.extend(node.name.split('/').filter(|n| !n.is_empty()));
        } else {
            names.push(&node.name);
        }
    }
    names
}

/// The rebuildable folders under `root` (`root` included), grouped by rule, groups
/// and their folders largest first in `mode`. One walk; a matched folder is not
/// descended into, so a nested match counts once, inside the outer one. Rules without
/// a match are left out.
pub fn groups(tree: &Tree, root: NodeId, mode: Mode) -> Vec<Group> {
    let mut found: Vec<Vec<Folder>> = RULES.iter().map(|_| Vec::new()).collect();
    // The names down to the node being looked at; depth-first order keeps its
    // ancestors in place, so a node at `depth` below `root` replaces everything after
    // its parent.
    let mut path = path_names(tree, root);
    let base = path.len();
    let mut stack = vec![(root, 0usize)];
    while let Some((id, depth)) = stack.pop() {
        let node = &tree.nodes[id as usize];
        if node.kind != Kind::Dir {
            continue;
        }
        if depth > 0 {
            path.truncate(base + depth - 1);
            path.push(&node.name);
        }
        let rule = RULES
            .iter()
            .position(|r| r.matchers.iter().any(|m| m.matches(tree, id, &path)));
        match rule {
            Some(r) => found[r].push(Folder {
                id,
                bytes: mode.bytes(node),
            }),
            None => stack.extend(node.children.iter().map(|&c| (c, depth + 1))),
        }
    }
    let mut groups: Vec<Group> = RULES
        .iter()
        .zip(found)
        .filter(|(_, folders)| !folders.is_empty())
        .map(|(rule, mut folders)| {
            folders.sort_by_key(|f| (Reverse(f.bytes), f.id));
            let count = folders.len();
            let bytes = folders.iter().map(|f| f.bytes).sum();
            folders.truncate(MAX_FOLDERS);
            Group {
                rule,
                bytes,
                count,
                more: count - folders.len(),
                folders,
            }
        })
        .collect();
    groups.sort_by_key(|g| (Reverse(g.bytes), g.rule.key));
    groups
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tree::Node;

    /// A tree rooted at `/scan/home` from relative paths: a path ending in `/` is a
    /// folder, any other a file of `len` bytes (logical; physical is twice that).
    /// The only place these tests build `Node` and `Tree` values.
    fn build(entries: &[(&str, u64)]) -> Tree {
        fn node(name: &str, parent: Option<NodeId>, kind: Kind, size: u64) -> Node {
            Node {
                name: name.into(),
                parent,
                kind,
                size,
                physical: size * 2,
                files: 0,
                dirs: 0,
                mtime: 0,
                children: Vec::new(),
                shared: crate::tree::Shared::None,
            }
        }
        let mut nodes = vec![node("/scan/home", None, Kind::Dir, 0)];
        for &(path, len) in entries {
            let mut parent: NodeId = 0;
            let is_dir = path.ends_with('/');
            let names: Vec<&str> = path.trim_end_matches('/').split('/').collect();
            for (i, name) in names.iter().enumerate() {
                let last = i + 1 == names.len();
                let existing = nodes[parent as usize]
                    .children
                    .iter()
                    .copied()
                    .find(|&c| nodes[c as usize].name == *name);
                let id = match existing {
                    Some(id) => id,
                    None => {
                        let id = nodes.len() as NodeId;
                        let file = last && !is_dir;
                        let kind = if file { Kind::File } else { Kind::Dir };
                        nodes.push(node(name, Some(parent), kind, if file { len } else { 0 }));
                        nodes[parent as usize].children.push(id);
                        id
                    }
                };
                parent = id;
            }
            if !is_dir {
                let mut up = nodes[parent as usize].parent;
                while let Some(p) = up {
                    nodes[p as usize].size += len;
                    nodes[p as usize].physical += len * 2;
                    up = nodes[p as usize].parent;
                }
            }
        }
        Tree {
            nodes,
            errors: Vec::new(),
            error_count: 0,
            clone_aware: false,
            clone_counted_once: 0,
        }
    }

    fn id(tree: &Tree, path: &str) -> NodeId {
        (0..tree.nodes.len() as NodeId)
            .find(|&i| tree.path(i) == std::path::Path::new("/scan/home").join(path))
            .unwrap_or_else(|| panic!("no node {path}"))
    }

    /// `(key, [folder paths])` for each group, in order.
    fn summary(tree: &Tree, root: NodeId, mode: Mode) -> Vec<(&'static str, u64, Vec<String>)> {
        groups(tree, root, mode)
            .iter()
            .map(|g| {
                let paths = g
                    .folders
                    .iter()
                    .map(|f| tree.path(f.id).to_string_lossy().into_owned())
                    .collect();
                (g.rule.key, g.bytes, paths)
            })
            .collect()
    }

    #[test]
    fn rule_text_is_sourced_and_never_advises() {
        let mut keys = std::collections::HashSet::new();
        for rule in RULES {
            assert!(keys.insert(rule.key), "duplicate key {}", rule.key);
            assert!(!rule.matchers.is_empty(), "{}: no matcher", rule.key);
            if let Some(caution) = rule.caution {
                assert!(
                    caution.ends_with('.') && caution.matches(". ").count() == 0,
                    "{}: caution is not one sentence: {caution}",
                    rule.key
                );
            }
            let caution = ("caution", rule.caution.unwrap_or("-"));
            for (field, text) in [
                ("key", rule.key),
                ("label", rule.label),
                ("owner", rule.owner),
                ("why", rule.why),
                caution,
            ] {
                assert!(!text.trim().is_empty(), "{}: empty {field}", rule.key);
                let lower = text.to_lowercase();
                for word in ["safe", "delete", "remove"] {
                    assert!(
                        !lower.contains(word),
                        "{}: {field} says {word:?}: {text}",
                        rule.key
                    );
                }
            }
            assert!(
                rule.source.starts_with("https://") && rule.source.len() > "https://".len(),
                "{}: source {:?} is not https",
                rule.key,
                rule.source
            );
            let date: Vec<&str> = rule.checked.split('-').collect();
            assert!(
                date.len() == 3
                    && [4, 2, 2].iter().zip(&date).all(|(n, part)| {
                        part.len() == *n && part.bytes().all(|b| b.is_ascii_digit())
                    }),
                "{}: checked {:?} is not YYYY-MM-DD",
                rule.key,
                rule.checked
            );
        }
        assert_eq!(CHECKED, "2026-10-30");
    }

    #[test]
    fn the_cache_tag_rule_is_last_and_the_only_any_name_rule() {
        let (last, named) = RULES.split_last().unwrap();
        assert_eq!(last.key, "cache_tag");
        assert!(last.caution.is_some());
        assert!(matches!(
            last.matchers,
            [Matcher::AnyWithChild {
                child: "CACHEDIR.TAG"
            }]
        ));
        for rule in named {
            assert!(
                !rule
                    .matchers
                    .iter()
                    .any(|m| matches!(m, Matcher::AnyWithChild { .. })),
                "{}: matches any folder name",
                rule.key
            );
        }
    }

    #[test]
    fn any_with_child_matches_a_tagged_folder_of_any_name_after_named_rules() {
        let tree = build(&[
            ("tool/data/CACHEDIR.TAG", 1),
            ("tool/data/blob", 60),
            ("rust/target/CACHEDIR.TAG", 1),
            ("rust/target/debug/app", 9),
            ("py/.mypy_cache/CACHEDIR.TAG", 1),
            ("py/.mypy_cache/3.12/x.json", 4),
            ("py/.ruff_cache/x", 2),
            (".cache/huggingface/hub/CACHEDIR.TAG", 1),
            (".cache/huggingface/hub/models--m/blob", 30),
            ("plain/CACHEDIR.TAG.bak", 5),
        ]);
        let found = summary(&tree, 0, Mode::Logical);
        assert_eq!(
            found,
            vec![
                ("cache_tag", 61, vec!["/scan/home/tool/data".to_string()]),
                (
                    "huggingface_hub",
                    31,
                    vec!["/scan/home/.cache/huggingface/hub".to_string()]
                ),
                (
                    "cargo_target",
                    10,
                    vec!["/scan/home/rust/target".to_string()]
                ),
                (
                    "mypy_cache",
                    5,
                    vec!["/scan/home/py/.mypy_cache".to_string()]
                ),
            ]
        );
        let groups = groups(&tree, 0, Mode::Logical);
        assert!(groups[0].rule.caution.is_some());
        assert!(groups[1].rule.caution.is_some());
        assert!(groups[2].rule.caution.is_none());
    }

    #[test]
    fn generic_names_need_a_project_file_and_specific_rules_win() {
        let tree = build(&[
            ("bare/dist/app.js", 3),
            ("bare/build/a.o", 5),
            ("bare/out/x", 7),
            ("web/package.json", 1),
            ("web/dist/app.js", 11),
            ("web/.next/server/page.js", 13),
            ("lib/pyproject.toml", 1),
            ("lib/dist/lib.whl", 17),
            ("android/build.gradle.kts", 1),
            ("android/package.json", 1),
            ("android/build/out.apk", 19),
            ("site/next.config.mjs", 1),
            ("site/out/index.html", 23),
            ("c/CMakeLists.txt", 1),
            ("c/build/CMakeCache.txt", 29),
            ("src/__pycache__/m.pyc", 2),
        ]);
        let keys: Vec<(&str, u64)> = summary(&tree, 0, Mode::Logical)
            .into_iter()
            .map(|(k, b, _)| (k, b))
            .collect();
        assert_eq!(
            keys,
            vec![
                ("cmake_build", 29),
                ("next_export", 23),
                ("gradle_build", 19),
                ("python_dist", 17),
                ("next_build", 13),
                ("js_dist", 11),
                ("python_bytecode", 2),
            ]
        );
    }

    #[test]
    fn cautioned_folders_at_fixed_paths() {
        let tree = build(&[
            (
                "Library/Containers/com.docker.docker/Data/vms/0/data/Docker.raw",
                80,
            ),
            (".android/avd/Pixel.avd/userdata.img", 40),
            ("miniforge3/conda-meta/history", 1),
            ("miniforge3/pkgs/numpy/x", 20),
            ("other/pkgs/y", 9),
        ]);
        let groups = groups(&tree, 0, Mode::Logical);
        let keys: Vec<&str> = groups.iter().map(|g| g.rule.key).collect();
        assert_eq!(keys, ["docker_disk", "android_avd", "conda_pkgs"]);
        assert!(groups.iter().all(|g| g.rule.caution.is_some()));
        assert_eq!(
            tree.path(groups[0].folders[0].id),
            std::path::Path::new("/scan/home/Library/Containers/com.docker.docker/Data/vms")
        );
    }

    #[test]
    fn child_sentinel_needs_the_child() {
        let tree = build(&[
            ("rust/target/CACHEDIR.TAG", 1),
            ("rust/target/debug/app", 100),
            ("work/target/real.dat", 50),
            ("py/.venv/pyvenv.cfg", 1),
            ("py/.venv/lib/x.py", 10),
            ("old/venv/pyvenv.cfg", 1),
            ("plain/venv/x", 3),
        ]);
        assert_eq!(
            summary(&tree, 0, Mode::Logical),
            vec![
                (
                    "cargo_target",
                    101,
                    vec!["/scan/home/rust/target".to_string()]
                ),
                (
                    "python_venv",
                    12,
                    vec![
                        "/scan/home/py/.venv".to_string(),
                        "/scan/home/old/venv".to_string()
                    ]
                ),
            ]
        );
    }

    #[test]
    fn sibling_sentinel_needs_the_sibling_and_counts_nested_once() {
        let tree = build(&[
            ("proj/package.json", 1),
            ("proj/node_modules/a/package.json", 1),
            ("proj/node_modules/a/node_modules/b/big", 40),
            ("bare/node_modules/c", 7),
            ("pkg/Package.swift", 1),
            ("pkg/.build/debug/bin", 20),
        ]);
        let groups = groups(&tree, 0, Mode::Logical);
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].rule.key, "node_modules");
        assert_eq!(groups[0].bytes, 41);
        assert_eq!(groups[0].count, 1);
        assert_eq!(
            groups[0].folders,
            vec![Folder {
                id: id(&tree, "proj/node_modules"),
                bytes: 41
            }]
        );
        assert_eq!(groups[1].rule.key, "swiftpm_build");
        assert_eq!(groups[1].bytes, 20);
    }

    #[test]
    fn path_suffix_matches_the_whole_tail_and_the_scan_root_path() {
        let tree = build(&[
            ("Library/Developer/Xcode/DerivedData/App-x/Build/obj", 30),
            ("Library/Caches/Homebrew/bottle.tar.gz", 9),
            ("Other/Xcode/DerivedData/y", 5),
            (".cache/huggingface/hub/models--m/blob", 70),
        ]);
        assert_eq!(
            summary(&tree, 0, Mode::Logical),
            vec![
                (
                    "huggingface_hub",
                    70,
                    vec!["/scan/home/.cache/huggingface/hub".to_string()]
                ),
                (
                    "xcode_derived_data",
                    30,
                    vec!["/scan/home/Library/Developer/Xcode/DerivedData".to_string()]
                ),
                (
                    "homebrew_cache",
                    9,
                    vec!["/scan/home/Library/Caches/Homebrew".to_string()]
                ),
            ]
        );
        // Below the scan root, the suffix still counts the names above the root.
        let xcode = id(&tree, "Library/Developer/Xcode");
        assert_eq!(
            summary(&tree, xcode, Mode::Logical)[0].0,
            "xcode_derived_data"
        );
        let derived = id(&tree, "Library/Developer/Xcode/DerivedData");
        assert_eq!(summary(&tree, derived, Mode::Logical)[0].1, 30);
        // A scan of `~/go/pkg`: the suffix starts in the scan root's full path.
        let mut go = build(&[("mod/cache/x", 4)]);
        go.nodes[0].name = "/Users/me/go/pkg".into();
        let found = groups(&go, 0, Mode::Logical);
        assert_eq!(found[0].rule.key, "go_modules");
        assert_eq!(found[0].folders[0].id, 1);
        // The scan root itself can match.
        go.nodes[0].name = "/Users/me/go/pkg/mod".into();
        let found = groups(&go, 0, Mode::Logical);
        assert_eq!(found[0].folders[0].id, 0);
    }

    #[test]
    fn mode_root_and_the_folder_cap() {
        let mut entries: Vec<(String, u64)> = (0..MAX_FOLDERS + 3)
            .flat_map(|i| {
                [
                    (format!("p{i}/package.json"), 1),
                    (format!("p{i}/node_modules/f"), i as u64 + 1),
                ]
            })
            .collect();
        entries.push(("rust/target/CACHEDIR.TAG".into(), 0));
        let refs: Vec<(&str, u64)> = entries.iter().map(|(p, n)| (p.as_str(), *n)).collect();
        let tree = build(&refs);
        let all = groups(&tree, 0, Mode::Logical);
        assert_eq!(all[0].count, MAX_FOLDERS + 3);
        assert_eq!(all[0].folders.len(), MAX_FOLDERS);
        assert_eq!(all[0].more, 3);
        assert_eq!(all[0].folders[0].bytes, (MAX_FOLDERS + 3) as u64);
        assert!(all[0].folders.windows(2).all(|w| w[0].bytes >= w[1].bytes));
        let n = (MAX_FOLDERS + 3) as u64;
        assert_eq!(all[0].bytes, n * (n + 1) / 2);
        assert_eq!(all[1].rule.key, "cargo_target");
        assert_eq!(all[1].bytes, 0);

        let physical = groups(&tree, 0, Mode::Physical);
        assert_eq!(physical[0].bytes, all[0].bytes * 2);

        let one = groups(&tree, id(&tree, "p7"), Mode::Logical);
        assert_eq!(one.len(), 1);
        assert_eq!(one[0].bytes, 8);
        assert!(groups(&tree, id(&tree, "p7/package.json"), Mode::Logical).is_empty());
    }
}
