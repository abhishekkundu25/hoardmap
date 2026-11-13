//! The scanned folder split into the kinds of thing a Mac keeps, such as applications,
//! photos and app data, for the disk bar.
//!
//! [`split`] is a pure function of the finished tree: no file system access. A
//! category is given by place: [`RULES`] maps paths from the file system root to
//! categories, and every rebuildable folder ([`crate::rebuildable`]) is its own
//! category wherever it lies. A file belongs to the nearest folder above it that has a
//! category, so each byte is counted once and the figures add up to the root's
//! `physical`.

use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

use crate::rebuildable;
use crate::tree::{Kind, Node, NodeId, ROOT, Tree};

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
    /// Its place in [`ALL`].
    fn index(self) -> usize {
        ALL.iter().position(|&c| c == self).unwrap()
    }

    /// The category the API calls `key`.
    pub fn from_key(key: &str) -> Option<Category> {
        ALL.into_iter().find(|c| c.key() == key)
    }

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

/// The entries that start a category. Everything below one belongs to it, down to the
/// next entry in here. The root is always one.
fn claims(tree: &Tree) -> HashMap<NodeId, Category> {
    let mut root = rebuildable::path_names(tree, ROOT);
    if root.starts_with(&DATA_VOLUME) {
        root.drain(..DATA_VOLUME.len());
    }
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
    claims
}

/// The bytes on disk of each category, largest first, empty ones left out.
fn tally(tree: &Tree, claims: &HashMap<NodeId, Category>) -> Vec<(Category, u64)> {
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
        totals[claims[&id].index()] += bytes;
    }
    let mut out: Vec<(usize, u64)> = totals.into_iter().enumerate().collect();
    out.retain(|&(_, bytes)| bytes > 0);
    out.sort_by_key(|&(at, bytes)| (std::cmp::Reverse(bytes), at));
    out.into_iter()
        .map(|(at, bytes)| (ALL[at], bytes))
        .collect()
}

/// The bytes on disk of each category in the tree, largest first, empty ones left
/// out. They add up to the root's `physical`.
pub fn split(tree: &Tree) -> Vec<(Category, u64)> {
    tally(tree, &claims(tree))
}

/// What one finished tree holds by category: the split for the disk bar, and a
/// [`Filter`] per category, made the first time it is asked for.
pub struct Kinds {
    claims: HashMap<NodeId, Category>,
    split: Vec<(Category, u64)>,
    filters: [OnceLock<Filter>; ALL.len()],
}

impl Kinds {
    pub fn new(tree: &Tree) -> Kinds {
        let claims = claims(tree);
        Kinds {
            split: tally(tree, &claims),
            claims,
            filters: Default::default(),
        }
    }

    /// As [`split`] gives it.
    pub fn split(&self) -> &[(Category, u64)] {
        &self.split
    }

    /// The filter that keeps `category` alone. `tree` is the tree this was made from.
    pub fn filter(&self, tree: &Tree, category: Category) -> &Filter {
        self.filters[category.index()].get_or_init(|| Filter::new(tree, &self.claims, category))
    }
}

impl std::fmt::Debug for Kinds {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Kinds").field("split", &self.split).finish()
    }
}

/// A subtree's figures, as [`Node`] keeps them.
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub struct Totals {
    pub size: u64,
    pub physical: u64,
    pub files: u32,
    pub dirs: u32,
}

impl Totals {
    fn of(node: &Node) -> Totals {
        Totals {
            size: node.size,
            physical: node.physical,
            files: node.files,
            dirs: node.dirs,
        }
    }
}

/// The tree with one category kept and everything else left out: which entries
/// belong to the category, and the figures of the folders that hold some of it
/// beside other things. Node ids stay the tree's own. It is one bit per entry, plus
/// the folders above a rule or a rebuildable folder.
pub struct Filter {
    /// One bit per node, set where the node belongs to the category.
    inside: Vec<u64>,
    /// The folders that have a category start below them and hold some of this
    /// category, with what they hold of it.
    mixed: HashMap<NodeId, Totals>,
}

impl Filter {
    fn new(tree: &Tree, claims: &HashMap<NodeId, Category>, category: Category) -> Filter {
        let count = tree.nodes.len();
        // Where a category starts, as a table: one lookup per entry in the walk below.
        const NONE: u8 = u8::MAX;
        let mut starts = vec![NONE; count];
        // Every folder above a start: only these can hold more than one category.
        let mut above: HashSet<NodeId> = HashSet::new();
        for (&id, &claimed) in claims {
            starts[id as usize] = claimed.index() as u8;
            let mut up = tree.nodes[id as usize].parent;
            while let Some(parent) = up {
                if !above.insert(parent) {
                    break;
                }
                up = tree.nodes[parent as usize].parent;
            }
        }
        let wanted = category.index() as u8;
        let mut inside = vec![0u64; count.div_ceil(64)];
        let mut stack = vec![(ROOT, NONE)];
        while let Some((id, inherited)) = stack.pop() {
            let own = match starts[id as usize] {
                NONE => inherited,
                start => start,
            };
            if own == wanted {
                inside[id as usize / 64] |= 1 << (id % 64);
            }
            let children = &tree.nodes[id as usize].children;
            stack.extend(children.iter().map(|&child| (child, own)));
        }
        let mut filter = Filter {
            inside,
            mixed: HashMap::new(),
        };
        let mut seen = HashMap::new();
        filter.gather(tree, ROOT, &above, &mut seen);
        filter.mixed = seen
            .into_iter()
            .filter_map(|(id, (totals, any))| any.then_some((id, totals)))
            .collect();
        filter
    }

    /// What `id`, a folder in `above`, holds of the category, and whether it holds
    /// any entry of it at all. Folders in `above` below it are gathered first.
    fn gather(
        &self,
        tree: &Tree,
        id: NodeId,
        above: &HashSet<NodeId>,
        seen: &mut HashMap<NodeId, (Totals, bool)>,
    ) -> (Totals, bool) {
        let node = &tree.nodes[id as usize];
        let mut totals = Totals::default();
        let mut any = self.inside(id);
        // What the folder's children take of its own figures; the rest is the folder
        // entry itself.
        let (mut size, mut physical) = (0u64, 0u64);
        for &child in &node.children {
            let entry = &tree.nodes[child as usize];
            size += entry.size;
            physical += entry.physical;
            let within = self.inside(child);
            let (held, some) = if above.contains(&child) {
                self.gather(tree, child, above, seen)
            } else if within {
                (Totals::of(entry), true)
            } else {
                (Totals::default(), false)
            };
            any |= some;
            totals.size += held.size;
            totals.physical += held.physical;
            let folder = entry.kind == Kind::Dir;
            totals.files += held.files + u32::from(within && !folder);
            totals.dirs += held.dirs + u32::from(within && folder);
        }
        if self.inside(id) {
            totals.size += node.size.saturating_sub(size);
            totals.physical += node.physical.saturating_sub(physical);
        }
        seen.insert(id, (totals, any));
        (totals, any)
    }

    /// Whether the entry belongs to the category.
    pub fn inside(&self, id: NodeId) -> bool {
        self.inside
            .get(id as usize / 64)
            .is_some_and(|word| word & (1 << (id % 64)) != 0)
    }

    /// Whether the entry is of the category or holds some of it: what a filtered view
    /// shows.
    pub fn shows(&self, id: NodeId) -> bool {
        self.inside(id) || self.mixed.contains_key(&id)
    }

    /// The figures of `node`, the entry `id`, counting the category alone.
    pub fn totals(&self, id: NodeId, node: &Node) -> Totals {
        match self.mixed.get(&id) {
            Some(&totals) => totals,
            None if self.inside(id) => Totals::of(node),
            None => Totals::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tree::Shared;

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
                    let mut up = Some(at);
                    while let Some(above) = up {
                        let counted = &mut nodes[above as usize];
                        *(if leaf {
                            &mut counted.files
                        } else {
                            &mut counted.dirs
                        }) += 1;
                        up = counted.parent;
                    }
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

    fn id(tree: &Tree, path: &str) -> NodeId {
        let full = std::path::Path::new(&tree.root().name).join(path);
        (0..tree.nodes.len() as NodeId)
            .find(|&i| tree.path(i) == full)
            .unwrap_or_else(|| panic!("no node {path}"))
    }

    fn mixed_home() -> Tree {
        build(
            "/Users/me",
            &[
                ("Library/Containers/app/data.bin", 500),
                ("Library/Mail/V10/inbox.mbox", 40),
                ("Library/Messages/chat.db", 10),
                ("Library/Caches/pip/http/a", 25),
                ("Pictures/a.heic", 200),
                ("Applications/Tool.app/Contents/MacOS/tool", 70),
                ("Documents/report.pdf", 20),
                (".Trash/old.zip", 7),
                (".zshrc", 1),
                ("repos/site/package.json", 1),
                ("repos/site/node_modules/.package-lock.json", 1),
                ("repos/site/node_modules/left-pad/index.js", 400),
                ("repos/site/src/main.js", 9),
                ("repos/empty/", 0),
            ],
        )
    }

    #[test]
    fn the_filters_share_every_entry_out_exactly_once() {
        let tree = mixed_home();
        let kinds = Kinds::new(&tree);
        for (id, node) in tree.nodes.iter().enumerate() {
            let id = id as NodeId;
            let mut sum = Totals::default();
            let mut inside = 0;
            for category in ALL {
                let filter = kinds.filter(&tree, category);
                let held = filter.totals(id, node);
                sum.size += held.size;
                sum.physical += held.physical;
                sum.files += held.files;
                sum.dirs += held.dirs;
                inside += u32::from(filter.inside(id));
                assert!(filter.shows(id) || held == Totals::default());
            }
            assert_eq!(sum, Totals::of(node), "{}", tree.path(id).display());
            assert_eq!(inside, 1, "{}", tree.path(id).display());
        }
    }

    #[test]
    fn a_filter_at_the_root_holds_what_the_split_says() {
        let tree = mixed_home();
        let kinds = Kinds::new(&tree);
        for category in ALL {
            let held = kinds.filter(&tree, category).totals(ROOT, tree.root());
            let split = kinds.split().iter().find(|&&(c, _)| c == category);
            assert_eq!(
                held.physical,
                split.map_or(0, |&(_, bytes)| bytes),
                "{category:?}"
            );
        }
    }

    #[test]
    fn a_filter_keeps_one_kind_and_the_folders_that_lead_to_it() {
        let tree = mixed_home();
        let kinds = Kinds::new(&tree);
        let mail = kinds.filter(&tree, Mail);
        // The folder above holds mail beside other things: it shows, with mail's
        // figures only.
        let library = id(&tree, "Library");
        assert!(mail.shows(library) && !mail.inside(library));
        assert_eq!(
            mail.totals(library, &tree.nodes[library as usize]),
            Totals {
                size: 50,
                physical: 50,
                files: 2,
                dirs: 3,
            }
        );
        assert!(mail.inside(id(&tree, "Library/Mail/V10/inbox.mbox")));
        for hidden in ["Library/Containers", "Pictures", "repos", ".zshrc"] {
            let hidden = id(&tree, hidden);
            assert!(!mail.shows(hidden));
            assert_eq!(
                mail.totals(hidden, &tree.nodes[hidden as usize]),
                Totals::default()
            );
        }

        // A rebuildable folder inside an "other" folder: each filter keeps its side.
        let site = id(&tree, "repos/site");
        let rebuildable = kinds.filter(&tree, Rebuildable);
        assert_eq!(
            rebuildable.totals(site, &tree.nodes[site as usize]).size,
            401
        );
        assert!(!rebuildable.shows(id(&tree, "repos/site/src")));
        let other = kinds.filter(&tree, Other);
        assert_eq!(
            other.totals(site, &tree.nodes[site as usize]),
            Totals {
                size: 10,
                physical: 10,
                files: 2,
                dirs: 1,
            }
        );
        assert!(!other.shows(id(&tree, "repos/site/node_modules")));
        // An empty folder of the kind still shows.
        assert!(other.shows(id(&tree, "repos/empty")));
        // A hidden file in the home folder is app data; the home folder is not.
        let app_data = kinds.filter(&tree, AppData);
        assert!(app_data.inside(id(&tree, ".zshrc")) && !app_data.inside(ROOT));
        assert!(app_data.shows(ROOT));
    }

    #[test]
    fn a_kind_the_scan_does_not_have_filters_to_nothing() {
        let tree = build("/Users/me/repos", &[("site/index.html", 9)]);
        let kinds = Kinds::new(&tree);
        let photos = kinds.filter(&tree, Photos);
        assert!(!photos.shows(ROOT));
        assert_eq!(photos.totals(ROOT, tree.root()), Totals::default());
        let other = kinds.filter(&tree, Other);
        assert_eq!(other.totals(ROOT, tree.root()).size, 9);
    }

    #[test]
    fn keys_name_their_categories() {
        for category in ALL {
            assert_eq!(Category::from_key(category.key()), Some(category));
        }
        assert_eq!(Category::from_key("everything"), None);
    }

    #[test]
    fn keys_are_distinct() {
        for (i, a) in ALL.iter().enumerate() {
            assert!(ALL[i + 1..].iter().all(|b| a.key() != b.key()));
        }
    }
}
