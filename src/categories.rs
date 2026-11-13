//! The scanned folder split into the kinds of thing a Mac keeps, such as applications,
//! photos and app data, for the disk bar.
//!
//! [`split`] is a pure function of the finished tree: no file system access. A
//! category is given by place: [`RULES`] maps paths from the file system root to
//! categories, and every rebuildable folder ([`crate::rebuildable`]) is its own
//! category wherever it lies. A file belongs to the nearest folder above it that has a
//! category, so each byte is counted once and the figures add up to the root's
//! `physical`.

use std::collections::HashMap;

use crate::rebuildable;
use crate::tree::{NodeId, ROOT, Tree};

/// In the order ties are listed.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Category {
    Applications,
    /// Folders a tool fills again on its own.
    Rebuildable,
    Photos,
    /// Music and Movies.
    Media,
    /// Mail and Messages.
    Mail,
    /// iCloud Drive and other cloud folders kept on the Mac.
    Cloud,
    /// What apps and tools keep for themselves: Library and the hidden folders of a
    /// home folder.
    AppData,
    /// macOS itself, where the scan reached it.
    System,
    Trash,
    /// Documents, Desktop and Downloads.
    Documents,
    /// Everything no rule names.
    Other,
}

const ALL: [Category; 11] = [
    Category::Applications,
    Category::Rebuildable,
    Category::Photos,
    Category::Media,
    Category::Mail,
    Category::Cloud,
    Category::AppData,
    Category::System,
    Category::Trash,
    Category::Documents,
    Category::Other,
];

impl Category {
    /// The name the API uses.
    pub fn key(self) -> &'static str {
        match self {
            Category::Applications => "applications",
            Category::Rebuildable => "rebuildable",
            Category::Photos => "photos",
            Category::Media => "media",
            Category::Mail => "mail",
            Category::Cloud => "cloud",
            Category::AppData => "app_data",
            Category::System => "system",
            Category::Trash => "bin",
            Category::Documents => "documents",
            Category::Other => "other",
        }
    }
}

/// One name of a path.
enum Seg {
    Name(&'static str),
    /// Any one name, such as a user's.
    Any,
    /// A name starting with a dot.
    Hidden,
}

impl Seg {
    fn matches(&self, name: &str) -> bool {
        match *self {
            Seg::Name(n) => n == name,
            Seg::Any => true,
            Seg::Hidden => name.starts_with('.'),
        }
    }
}

use Category::*;
use Seg::{Any, Hidden, Name as N};

/// A path from the file system root, and the category of everything under it that no
/// longer rule names. Where two rules name the same entry, the earlier one wins.
const RULES: &[(&[Seg], Category)] = &[
    (&[N("Users"), Any, N(".Trash")], Trash),
    (&[N("Volumes"), Any, N(".Trashes")], Trash),
    (&[N("Applications")], Applications),
    (&[N("Users"), Any, N("Applications")], Applications),
    (&[N("Users"), Any, N("Pictures")], Photos),
    (&[N("Users"), Any, N("Music")], Media),
    (&[N("Users"), Any, N("Movies")], Media),
    (&[N("Users"), Any, N("Library"), N("Mail")], Mail),
    (&[N("Users"), Any, N("Library"), N("Messages")], Mail),
    (
        &[N("Users"), Any, N("Library"), N("Mobile Documents")],
        Cloud,
    ),
    (&[N("Users"), Any, N("Library"), N("CloudStorage")], Cloud),
    (&[N("Users"), Any, N("Library")], AppData),
    (&[N("Users"), Any, N("Documents")], Documents),
    (&[N("Users"), Any, N("Desktop")], Documents),
    (&[N("Users"), Any, N("Downloads")], Documents),
    (&[N("Users"), Any, Hidden], AppData),
    (&[N("Library")], AppData),
    (&[N("opt")], AppData),
    (&[N("usr"), N("local")], AppData),
    (&[N("private"), N("tmp")], AppData),
    (&[N("private"), N("var"), N("folders")], AppData),
    (&[N("System")], System),
    (&[N("usr")], System),
    (&[N("bin")], System),
    (&[N("sbin")], System),
    (&[N("private")], System),
];

/// Where the startup disk's files show up a second time; a path through it is the
/// same place as the path without it.
const DATA_VOLUME: [&str; 3] = ["System", "Volumes", "Data"];

fn covers(pattern: &[Seg], names: &[&str]) -> bool {
    pattern.len() == names.len()
        && pattern
            .iter()
            .zip(names)
            .all(|(seg, name)| seg.matches(name))
}

/// The entries `pattern` names, one name per level below the root.
fn below(tree: &Tree, pattern: &[Seg]) -> Vec<NodeId> {
    let mut at = vec![ROOT];
    for seg in pattern {
        at = at
            .iter()
            .flat_map(|&id| tree.nodes[id as usize].children.iter().copied())
            .filter(|&child| seg.matches(&tree.nodes[child as usize].name))
            .collect();
    }
    at
}

/// The bytes on disk of each category in the tree, largest first, empty ones left
/// out. They add up to the root's `physical`.
pub fn split(tree: &Tree) -> Vec<(Category, u64)> {
    let mut root = rebuildable::path_names(tree, ROOT);
    if root.starts_with(&DATA_VOLUME) {
        root.drain(..DATA_VOLUME.len());
    }
    // The entries that start a category. Everything below one belongs to it, down to
    // the next entry in here.
    let mut claims: HashMap<NodeId, Category> = HashMap::new();
    // The longest rule at or above the scanned folder says what the folder itself is.
    let mut base: Option<(usize, Category)> = None;
    for &(pattern, category) in RULES {
        if pattern.len() <= root.len() {
            let longer = base.is_none_or(|(len, _)| pattern.len() > len);
            if longer && covers(pattern, &root[..pattern.len()]) {
                base = Some((pattern.len(), category));
            }
        } else if covers(&pattern[..root.len()], &root) {
            for id in below(tree, &pattern[root.len()..]) {
                claims.entry(id).or_insert(category);
            }
        }
    }
    claims.insert(ROOT, base.map_or(Other, |(_, category)| category));
    for id in rebuildable::folders(tree, ROOT) {
        claims.insert(id, Rebuildable);
    }

    let physical = |id: NodeId| tree.nodes[id as usize].physical;
    let mut own: HashMap<NodeId, u64> = claims.keys().map(|&id| (id, physical(id))).collect();
    for &id in claims.keys() {
        let mut up = tree.nodes[id as usize].parent;
        while let Some(above) = up {
            if let Some(bytes) = own.get_mut(&above) {
                *bytes = bytes.saturating_sub(physical(id));
                break;
            }
            up = tree.nodes[above as usize].parent;
        }
    }
    let mut totals = [0u64; ALL.len()];
    for (id, bytes) in own {
        let at = ALL.iter().position(|&c| c == claims[&id]).unwrap();
        totals[at] += bytes;
    }
    let mut out: Vec<(usize, u64)> = totals.into_iter().enumerate().collect();
    out.retain(|&(_, bytes)| bytes > 0);
    out.sort_by_key(|&(at, bytes)| (std::cmp::Reverse(bytes), at));
    out.into_iter()
        .map(|(at, bytes)| (ALL[at], bytes))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tree::{Kind, Node, Shared};

    /// A tree rooted at `root` from relative paths: a path ending in `/` is a folder,
    /// any other a file of `len` bytes on disk.
    fn build(root: &str, entries: &[(&str, u64)]) -> Tree {
        fn node(name: &str, parent: Option<NodeId>, kind: Kind, len: u64) -> Node {
            Node {
                name: name.into(),
                parent,
                kind,
                size: len,
                physical: len,
                files: 0,
                dirs: 0,
                mtime: 0,
                children: Vec::new(),
                shared: Shared::None,
            }
        }
        let mut nodes = vec![node(root, None, Kind::Dir, 0)];
        for &(path, len) in entries {
            let file = !path.ends_with('/');
            let names: Vec<&str> = path.trim_end_matches('/').split('/').collect();
            let mut at: NodeId = ROOT;
            for (i, name) in names.iter().enumerate() {
                let leaf = file && i + 1 == names.len();
                let found = nodes[at as usize]
                    .children
                    .iter()
                    .copied()
                    .find(|&c| nodes[c as usize].name == *name);
                at = found.unwrap_or_else(|| {
                    let id = nodes.len() as NodeId;
                    let kind = if leaf { Kind::File } else { Kind::Dir };
                    nodes.push(node(name, Some(at), kind, 0));
                    nodes[at as usize].children.push(id);
                    id
                });
            }
            let mut up = file.then_some(at);
            while let Some(id) = up {
                nodes[id as usize].size += len;
                nodes[id as usize].physical += len;
                up = nodes[id as usize].parent;
            }
        }
        Tree {
            nodes,
            raw_names: HashMap::new(),
            errors: Vec::new(),
            error_count: 0,
            clone_aware: false,
            clone_counted_once: 0,
        }
    }

    fn keys(tree: &Tree) -> Vec<(&'static str, u64)> {
        let split = split(tree);
        let total: u64 = split.iter().map(|&(_, bytes)| bytes).sum();
        assert_eq!(total, tree.root().physical, "the split adds up to the scan");
        split
            .into_iter()
            .map(|(c, bytes)| (c.key(), bytes))
            .collect()
    }

    #[test]
    fn a_home_folder_splits_by_place_largest_first() {
        let tree = build(
            "/Users/me",
            &[
                ("Library/Containers/app/data.bin", 500),
                ("Library/Mail/V10/inbox.mbox", 40),
                ("Library/Messages/chat.db", 10),
                ("Library/Mobile Documents/notes.txt", 30),
                (
                    "Pictures/Photos Library.photoslibrary/originals/a.heic",
                    200,
                ),
                ("Music/song.m4a", 60),
                ("Movies/film.mov", 65),
                ("Applications/Tool.app/Contents/MacOS/tool", 70),
                ("Documents/report.pdf", 20),
                ("Desktop/shot.png", 5),
                ("Downloads/disk.dmg", 80),
                (".Trash/old.zip", 7),
                (".config/tool/state.db", 300),
                (".zshrc", 1),
                ("repos/site/index.html", 9),
            ],
        );
        assert_eq!(
            keys(&tree),
            [
                ("app_data", 801),
                ("photos", 200),
                ("media", 125),
                ("documents", 105),
                ("applications", 70),
                ("mail", 50),
                ("cloud", 30),
                ("other", 9),
                ("bin", 7),
            ]
        );
    }

    #[test]
    fn rebuildable_folders_are_their_own_category_wherever_they_lie() {
        let tree = build(
            "/Users/me",
            &[
                ("repos/site/package.json", 1),
                ("repos/site/node_modules/.package-lock.json", 1),
                ("repos/site/node_modules/left-pad/index.js", 400),
                ("repos/site/src/main.js", 20),
                (
                    "Library/Developer/Xcode/DerivedData/App-abc/Build/app.o",
                    250,
                ),
                ("Library/Preferences/app.plist", 3),
            ],
        );
        assert_eq!(
            keys(&tree),
            [("rebuildable", 651), ("other", 21), ("app_data", 3)]
        );
    }

    #[test]
    fn the_startup_disk_splits_into_system_apps_and_app_data() {
        let entries = [
            ("System/Library/kernel", 900),
            ("usr/bin/ls", 10),
            ("usr/local/bin/tool", 15),
            ("private/var/db/store", 50),
            ("private/var/folders/zz/cache.bin", 70),
            ("Applications/Safari.app/Contents/MacOS/Safari", 300),
            ("Library/Application Support/thing", 25),
            ("opt/homebrew/bin/brew", 35),
            ("Users/me/Pictures/a.jpg", 5),
            ("Users/Shared/notes.txt", 2),
        ];
        let expected = [
            ("system", 960),
            ("applications", 300),
            ("app_data", 145),
            ("photos", 5),
            ("other", 2),
        ];
        assert_eq!(keys(&build("/", &entries)), expected);
        // The Data volume's own mount point is the same place.
        assert_eq!(keys(&build("/System/Volumes/Data", &entries)), expected);
    }

    #[test]
    fn a_folder_inside_a_category_keeps_it() {
        let inside = build("/Users/me/Library/Containers", &[("app/data.bin", 12)]);
        assert_eq!(keys(&inside), [("app_data", 12)]);
        let hidden = build("/Users/me/.cache/pip", &[("wheels/a.whl", 8)]);
        assert_eq!(keys(&hidden), [("app_data", 8)]);
        let emptied = build("/Users/me/.Trash", &[("old.zip", 4)]);
        assert_eq!(keys(&emptied), [("bin", 4)]);
        let drive = build(
            "/Volumes/Backup",
            &[("films/a.mov", 6), (".Trashes/501/b", 2)],
        );
        assert_eq!(keys(&drive), [("other", 6), ("bin", 2)]);
    }

    #[test]
    fn an_empty_folder_has_no_categories() {
        assert_eq!(keys(&build("/Users/me/empty", &[])), []);
    }

    #[test]
    fn keys_are_distinct() {
        for (i, a) in ALL.iter().enumerate() {
            assert!(ALL[i + 1..].iter().all(|b| a.key() != b.key()));
        }
    }
}
