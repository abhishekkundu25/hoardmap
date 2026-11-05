//! Rebuildable folders: the folders a tool fills again on its own, such as
//! `node_modules`, Cargo's `target` and Python's `.venv`, gathered from the finished
//! tree into one group per rule.
//!
//! [`RULES`] is the table; each rule names its owner, says how the folder comes back
//! in the owner's words, and links the owner's documentation. [`groups`] is a pure
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
    /// A folder matches the rule when any of these match.
    pub matchers: &'static [Matcher],
}

const CHECKED: &str = "2026-10-30";

pub const RULES: &[Rule] = &[
    Rule {
        key: "node_modules",
        label: "node_modules",
        owner: "npm",
        why: "npm install installs the packages again from package.json and its lock file.",
        source: "https://docs.npmjs.com/cli/v10/commands/npm-install",
        checked: CHECKED,
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
        matchers: &[Matcher::SiblingSentinel {
            name: ".build",
            sibling: "Package.swift",
        }],
    },
    Rule {
        key: "xcode_derived_data",
        label: "Xcode DerivedData",
        owner: "Xcode",
        why: "Xcode keeps precompiled headers, indexes and build products here and writes them again when it builds and indexes a project.",
        source: "https://developer.apple.com/library/archive/documentation/Xcode/Conceptual/RN-Xcode-Archive/Chapters/xc4_release_notes.html",
        checked: CHECKED,
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
        matchers: &[Matcher::PathSuffix(&[".gradle", "caches"])],
    },
    Rule {
        key: "homebrew_cache",
        label: "Homebrew downloads",
        owner: "Homebrew",
        why: "Homebrew's download cache (brew --cache); brew downloads a bottle or source again when an install needs it.",
        source: "https://docs.brew.sh/FAQ",
        checked: CHECKED,
        matchers: &[Matcher::PathSuffix(&["Library", "Caches", "Homebrew"])],
    },
    Rule {
        key: "pip_cache",
        label: "pip cache",
        owner: "pip",
        why: "pip's cache of downloads and built wheels (pip cache dir); pip downloads or builds a package again when it is not cached.",
        source: "https://pip.pypa.io/en/stable/topics/caching/",
        checked: CHECKED,
        matchers: &[Matcher::PathSuffix(&["Library", "Caches", "pip"])],
    },
    Rule {
        key: "npm_cache",
        label: "npm cache",
        owner: "npm",
        why: "npm's content-addressable cache of downloaded packages; npm fetches missing data again from the registry.",
        source: "https://docs.npmjs.com/cli/v10/commands/npm-cache",
        checked: CHECKED,
        matchers: &[Matcher::PathSuffix(&[".npm", "_cacache"])],
    },
    Rule {
        key: "pnpm_store",
        label: "pnpm store",
        owner: "pnpm",
        why: "pnpm's package store; if a later install needs a package that is not in it, pnpm downloads it again.",
        source: "https://pnpm.io/cli/store",
        checked: CHECKED,
        matchers: &[Matcher::PathSuffix(&["Library", "pnpm", "store"])],
    },
    Rule {
        key: "cargo_registry",
        label: "Cargo registry",
        owner: "Cargo",
        why: "Crates Cargo downloaded from crates.io and unpacked; cargo downloads them again when a build needs them.",
        source: "https://doc.rust-lang.org/cargo/guide/cargo-home.html",
        checked: CHECKED,
        matchers: &[Matcher::PathSuffix(&[".cargo", "registry"])],
    },
    Rule {
        key: "go_modules",
        label: "Go module cache",
        owner: "Go",
        why: "The go command downloads modules into this cache again when a build needs them.",
        source: "https://go.dev/ref/mod",
        checked: CHECKED,
        matchers: &[Matcher::PathSuffix(&["go", "pkg", "mod"])],
    },
    Rule {
        key: "uv_cache",
        label: "uv cache",
        owner: "uv",
        why: "uv's cache of downloaded and built packages; uv fills it again as it installs.",
        source: "https://docs.astral.sh/uv/concepts/cache/",
        checked: CHECKED,
        matchers: &[Matcher::PathSuffix(&[".cache", "uv"])],
    },
    Rule {
        key: "huggingface_hub",
        label: "Hugging Face models and datasets",
        owner: "Hugging Face",
        why: "huggingface_hub's download cache; files are downloaded again from the Hub when a program asks for them.",
        source: "https://huggingface.co/docs/huggingface_hub/guides/manage-cache",
        checked: CHECKED,
        matchers: &[Matcher::PathSuffix(&[".cache", "huggingface", "hub"])],
    },
];

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
            for (field, text) in [
                ("key", rule.key),
                ("label", rule.label),
                ("owner", rule.owner),
                ("why", rule.why),
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
