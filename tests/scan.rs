//! Scanner acceptance tests on a fixture built under the system temp folder.

use std::fs::{self, File};
use std::io::Write;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant, SystemTime};

use hoardmap::scan::{
    LIVE_DEPTH, Method, NOT_SCANNED, Progress, partial, scan, scan_skipping, scan_with_threads,
};
use hoardmap::tree::{Kind, NodeId, ROOT, Shared, Tree};

/// The fixture folder; removed on drop, with `locked/` made writable again first.
struct Fixture {
    root: PathBuf,
    /// The mtime set on `a/small.txt`, one day ahead of the clock.
    future: SystemTime,
    /// False when the process can read a mode-000 directory (running as root).
    locked_unreadable: bool,
}

impl Fixture {
    fn build() -> Fixture {
        // The clock ticks in microseconds on macOS, so parallel tests need the counter.
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let root = std::env::temp_dir().join(format!(
            "hoardmap-scan-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(root.join("a")).unwrap();
        fs::create_dir_all(root.join("b/c")).unwrap();
        fs::create_dir_all(root.join("locked")).unwrap();
        write(&root.join("a/big.bin"), 300_000);
        write(&root.join("a/small.txt"), 10);
        write(&root.join("b/x.jpg"), 2000);
        write(&root.join(".hidden"), 5);
        write(&root.join("locked/secret.bin"), 100);
        fs::hard_link(root.join("a/small.txt"), root.join("link.txt")).unwrap();
        symlink("a/big.bin", root.join("sym")).unwrap();

        let future = SystemTime::now() + Duration::from_secs(86_400);
        File::options()
            .write(true)
            .open(root.join("a/small.txt"))
            .unwrap()
            .set_modified(future)
            .unwrap();

        fs::set_permissions(root.join("locked"), fs::Permissions::from_mode(0o000)).unwrap();
        let locked_unreadable = fs::read_dir(root.join("locked")).is_err();
        if !locked_unreadable {
            println!("locked/ is readable (running as root): permission checks skipped");
        }
        Fixture {
            root,
            future,
            locked_unreadable,
        }
    }

    fn future_secs(&self) -> i64 {
        self.future
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::set_permissions(self.root.join("locked"), fs::Permissions::from_mode(0o755));
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn write(path: &Path, len: usize) {
    File::create(path)
        .unwrap()
        .write_all(&vec![b'x'; len])
        .unwrap();
}

fn child(tree: &Tree, parent: NodeId, name: &str) -> NodeId {
    *tree.nodes[parent as usize]
        .children
        .iter()
        .find(|&&c| tree.nodes[c as usize].name == name)
        .unwrap_or_else(|| panic!("no child {name} under node {parent}"))
}

fn names(tree: &Tree, parent: NodeId) -> Vec<&str> {
    tree.nodes[parent as usize]
        .children
        .iter()
        .map(|&c| tree.nodes[c as usize].name.as_str())
        .collect()
}

fn scan_fixture(fx: &Fixture) -> (Tree, Progress) {
    let progress = Progress::default();
    let tree = scan(&fx.root, &progress).expect("scan the fixture");
    (tree, progress)
}

#[test]
fn root_totals_and_order() {
    let fx = Fixture::build();
    let (tree, _) = scan_fixture(&fx);
    let root = tree.root();
    assert_eq!(root.name, fx.root.to_string_lossy());
    assert_eq!(root.parent, None);
    assert_eq!(root.kind, Kind::Dir);

    let sym_len = fs::symlink_metadata(fx.root.join("sym")).unwrap().len();
    assert_eq!(sym_len, "a/big.bin".len() as u64);
    let (locked_size, locked_files) = if fx.locked_unreadable {
        (0, 0)
    } else {
        (100, 1)
    };
    assert_eq!(root.size, 302_015 + sym_len + locked_size);
    assert_eq!(root.files, 6 + locked_files);
    assert_eq!(root.dirs, 4);

    let order = names(&tree, ROOT);
    assert_eq!(order[0], "a");
    assert_eq!(order[1], "b");
    assert!(order.contains(&"link.txt") && order.contains(&"locked"));
    for ids in tree.nodes.iter().map(|n| &n.children) {
        for pair in ids.windows(2) {
            let (x, y) = (&tree.nodes[pair[0] as usize], &tree.nodes[pair[1] as usize]);
            assert!(
                x.size > y.size || (x.size == y.size && x.name < y.name),
                "{} ({}) must come before {} ({})",
                x.name,
                x.size,
                y.name,
                y.size
            );
        }
    }

    let a = child(&tree, ROOT, "a");
    assert_eq!(names(&tree, a), vec!["big.bin", "small.txt"]);
    assert_eq!(tree.nodes[a as usize].size, 300_010);
    assert_eq!(tree.nodes[a as usize].files, 2);
    assert_eq!(tree.nodes[a as usize].dirs, 0);
    assert_eq!(
        tree.path(child(&tree, a, "big.bin")),
        fx.root.join("a/big.bin")
    );

    let sym = &tree.nodes[child(&tree, ROOT, "sym") as usize];
    assert_eq!(sym.kind, Kind::Symlink);
    assert_eq!(sym.size, sym_len);
    assert_eq!(tree.nodes[child(&tree, ROOT, ".hidden") as usize].size, 5);
    assert_eq!(tree.nodes.len(), 1 + 4 + 6 + locked_files as usize);
}

#[test]
fn hard_link_counted_once() {
    let fx = Fixture::build();
    let (tree, _) = scan_fixture(&fx);
    let a = child(&tree, ROOT, "a");
    let first = &tree.nodes[child(&tree, a, "small.txt") as usize];
    let second = &tree.nodes[child(&tree, ROOT, "link.txt") as usize];
    assert_eq!(first.shared, Shared::None);
    assert_eq!(first.size, 10);
    assert!(first.physical >= 10);
    assert_eq!(second.shared, Shared::HardLink);
    assert_eq!(second.size, 0);
    assert_eq!(second.physical, 0);
    assert_eq!(second.kind, Kind::File);
}

#[test]
fn physical_is_whole_blocks() {
    let fx = Fixture::build();
    let (tree, _) = scan_fixture(&fx);
    let a = child(&tree, ROOT, "a");
    let big = &tree.nodes[child(&tree, a, "big.bin") as usize];
    assert_eq!(big.physical % 512, 0);
    assert!(
        big.physical >= 300_000,
        "physical {} < logical",
        big.physical
    );
    let root = tree.root();
    let sum: u64 = root
        .children
        .iter()
        .map(|&c| tree.nodes[c as usize].physical)
        .sum();
    assert_eq!(root.physical, sum);
}

#[test]
fn mtime_rolls_up_to_the_latest() {
    let fx = Fixture::build();
    let (tree, _) = scan_fixture(&fx);
    let latest = tree.nodes.iter().map(|n| n.mtime).max().unwrap();
    assert_eq!(tree.root().mtime, fx.future_secs());
    assert_eq!(tree.root().mtime, latest);
    let a = child(&tree, ROOT, "a");
    assert_eq!(tree.nodes[a as usize].mtime, fx.future_secs());
    let b = child(&tree, ROOT, "b");
    assert!(tree.nodes[b as usize].mtime < fx.future_secs());
}

#[test]
fn unreadable_dir_is_counted_and_listed() {
    let fx = Fixture::build();
    let (tree, _) = scan_fixture(&fx);
    let locked_id = child(&tree, ROOT, "locked");
    let locked = &tree.nodes[locked_id as usize];
    assert_eq!(locked.kind, Kind::Dir);
    if !fx.locked_unreadable {
        println!("skipped: unreadable_dir_is_counted_and_listed (running as root)");
        return;
    }
    assert_eq!(tree.error_count, 1);
    assert_eq!(tree.errors.len(), 1);
    assert_eq!(tree.errors[0].path, tree.path(locked_id).to_string_lossy());
    assert!(!tree.errors[0].error.is_empty());
    assert_eq!(locked.size, 0);
    assert_eq!(locked.files, 0);
    assert!(locked.children.is_empty());
}

#[test]
fn skipped_dir_is_counted_empty_and_listed() {
    let fx = Fixture::build();
    let skipped = fx.root.join("b");
    let progress = Progress::default();
    let tree = scan_skipping(
        &fx.root,
        &progress,
        Method::default(),
        vec![skipped.clone()],
    )
    .unwrap();
    let b = &tree.nodes[child(&tree, ROOT, "b") as usize];
    assert_eq!(b.kind, Kind::Dir);
    assert!(b.children.is_empty());
    assert_eq!(b.size, 0);
    let listed: Vec<_> = tree
        .errors
        .iter()
        .filter(|e| e.error == NOT_SCANNED)
        .collect();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].path, skipped.to_string_lossy());
}

#[test]
fn the_preview_of_a_finished_scan_matches_its_folders() {
    let fx = Fixture::build();
    let (tree, progress) = scan_fixture(&fx);
    let finished = progress.finished.lock().unwrap().clone();
    assert!(finished.iter().all(|f| f.path.len() <= LIVE_DEPTH));
    let preview = partial("root", &finished);
    assert_eq!(preview.nodes[ROOT as usize].name, "root");
    for name in ["a", "b"] {
        let full = &tree.nodes[child(&tree, ROOT, name) as usize];
        let seen = &preview.nodes[child(&preview, ROOT, name) as usize];
        assert_eq!(
            (seen.size, seen.files, seen.dirs),
            (full.size, full.files, full.dirs),
            "{name}"
        );
    }
    let c = child(&preview, child(&preview, ROOT, "b"), "c");
    assert_eq!(preview.nodes[c as usize].kind, Kind::Dir);
    // Largest first, like the scanned tree.
    let sizes: Vec<u64> = preview.nodes[ROOT as usize]
        .children
        .iter()
        .map(|&id| preview.nodes[id as usize].size)
        .collect();
    assert!(sizes.windows(2).all(|w| w[0] >= w[1]), "{sizes:?}");
}

#[test]
fn a_preview_keeps_its_ids_as_the_scan_goes_on() {
    let fx = Fixture::build();
    let (_, progress) = scan_fixture(&fx);
    let finished = progress.finished.lock().unwrap().clone();
    let early = partial("root", &finished[..finished.len() / 2]);
    let later = partial("root", &finished);
    for (id, node) in early.nodes.iter().enumerate() {
        assert_eq!(later.nodes[id].name, node.name, "id {id}");
        assert_eq!(later.nodes[id].parent, node.parent, "id {id}");
    }
}

#[test]
fn a_preview_adds_up_folders_still_being_read() {
    let file = |path: &[&str], size| hoardmap::scan::Finished {
        path: path.iter().map(|s| s.to_string()).collect(),
        kind: Kind::File,
        size,
        physical: size,
        files: 0,
        dirs: 0,
        mtime: 7,
    };
    let preview = partial(
        "/r",
        &[
            file(&["x", "one"], 10),
            file(&["x", "y", "two"], 5),
            file(&["top"], 1),
        ],
    );
    let x = &preview.nodes[child(&preview, ROOT, "x") as usize];
    assert_eq!((x.size, x.files, x.dirs, x.mtime), (15, 2, 1, 7));
    let root = &preview.nodes[ROOT as usize];
    assert_eq!((root.size, root.files, root.dirs), (16, 3, 2));
}

#[test]
fn empty_dir_is_a_dir_node() {
    let fx = Fixture::build();
    let (tree, _) = scan_fixture(&fx);
    let b = child(&tree, ROOT, "b");
    let c = &tree.nodes[child(&tree, b, "c") as usize];
    assert_eq!(c.kind, Kind::Dir);
    assert_eq!(c.size, 0);
    assert_eq!(c.physical, 0);
    assert!(c.children.is_empty());
    assert_eq!(c.parent, Some(b));
    let b = &tree.nodes[b as usize];
    assert_eq!(b.dirs, 1);
    assert_eq!(b.files, 1);
    assert_eq!(b.size, 2000);
    assert_eq!(names(&tree, child(&tree, ROOT, "b")), vec!["x.jpg", "c"]);
}

#[test]
fn progress_ends_at_the_tree_totals() {
    let fx = Fixture::build();
    let (tree, progress) = scan_fixture(&fx);
    let (dirs, files, bytes, physical) = progress.snapshot();
    assert_eq!(dirs, u64::from(tree.root().dirs));
    assert_eq!(files, u64::from(tree.root().files));
    assert!(bytes >= tree.root().size);
    assert!(physical >= tree.root().physical);
}

#[test]
fn root_must_be_a_readable_directory() {
    let fx = Fixture::build();
    let progress = Progress::default();
    assert!(scan(&fx.root.join("a/big.bin"), &progress).is_err());
    assert!(scan(&fx.root.join("missing"), &progress).is_err());
    if fx.locked_unreadable {
        assert!(scan(&fx.root.join("locked"), &progress).is_err());
    }
}

#[test]
fn scans_the_target_folder() {
    let target = Path::new(env!("CARGO_MANIFEST_DIR")).join("target");
    if !target.is_dir() {
        println!("skipped: no target folder");
        return;
    }
    let progress = Progress::default();
    let started = Instant::now();
    let tree = scan(&target, &progress).expect("scan target");
    let elapsed = started.elapsed();
    println!(
        "target scan: {} nodes, {} dirs, {} files, {} bytes, {} errors, {:.3?}",
        tree.nodes.len(),
        tree.root().dirs,
        tree.root().files,
        tree.root().size,
        tree.error_count,
        elapsed
    );
    assert!(tree.nodes.len() > 1);
    assert_eq!(
        tree.nodes.len(),
        1 + tree.root().dirs as usize + tree.root().files as usize
    );
}

/// Every field of every node, for comparing two scans.
fn fingerprint(tree: &Tree) -> Vec<String> {
    tree.nodes
        .iter()
        .map(|n| {
            format!(
                "{} {:?} {:?} {} {} {} {} {} {:?} {:?}",
                n.name,
                n.parent,
                n.kind,
                n.size,
                n.physical,
                n.files,
                n.dirs,
                n.mtime,
                n.children,
                n.shared
            )
        })
        .collect()
}

/// The fields both listing methods know on any folder, clones or not: no allocation
/// figures, and hard links but not clones.
#[cfg(target_os = "macos")]
fn logical_fingerprint(tree: &Tree) -> Vec<String> {
    tree.nodes
        .iter()
        .map(|n| {
            format!(
                "{} {:?} {:?} {} {} {} {} {:?} {}",
                n.name,
                n.parent,
                n.kind,
                n.size,
                n.files,
                n.dirs,
                n.mtime,
                n.children,
                n.shared == Shared::HardLink
            )
        })
        .collect()
}

#[cfg(target_os = "macos")]
fn assert_same(a: Vec<String>, b: Vec<String>) {
    assert_eq!(a.len(), b.len(), "node counts differ");
    for (x, y) in a.iter().zip(&b) {
        assert_eq!(x, y);
    }
}

#[cfg(target_os = "macos")]
fn both_ways(root: &Path) -> (Tree, Tree) {
    use hoardmap::scan::scan_with;
    let bulk = scan_with(root, &Progress::default(), Method::Bulk).expect("bulk scan");
    let portable = scan_with(root, &Progress::default(), Method::Portable).expect("portable scan");
    assert_eq!(bulk.error_count, portable.error_count);
    assert!(!portable.clone_aware);
    (bulk, portable)
}

#[cfg(target_os = "macos")]
#[test]
fn bulk_and_portable_listing_agree_on_the_fixture() {
    let fx = Fixture::build();
    let (bulk, portable) = both_ways(&fx.root);
    assert_same(fingerprint(&bulk), fingerprint(&portable));
}

/// The build folder may hold clones (a copy-on-write copy of a build cache), so the
/// methods need only agree on what both know, and clones only lower On disk.
#[cfg(target_os = "macos")]
#[test]
fn bulk_and_portable_listing_agree_on_the_target_folder() {
    let target = Path::new(env!("CARGO_MANIFEST_DIR")).join("target/debug/build");
    if target.is_dir() {
        let (bulk, portable) = both_ways(&target);
        assert_same(logical_fingerprint(&bulk), logical_fingerprint(&portable));
        assert!(bulk.root().physical <= portable.root().physical);
    }
}

/// `orig` is 4 MB of random bytes; `full` and `part` are clones of it, `part` with its
/// second megabyte rewritten and so counted in full; `copy` is a plain copy; `small`
/// and `link` are one file.
/// Clones made with `/bin/cp -c`.
#[cfg(target_os = "macos")]
#[test]
fn clones_are_counted_once() {
    use std::process::Command;
    const MB: u64 = 1 << 20;
    let root =
        Scratch(std::env::temp_dir().join(format!("hoardmap-clones-{}", std::process::id())));
    fs::create_dir_all(&root.0).unwrap();
    let mut random = vec![0u8; 4 * MB as usize];
    let mut state = 0x2545_f491_4f6c_dd1du64;
    for byte in &mut random {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        *byte = state as u8;
    }
    fs::write(root.0.join("orig"), &random).unwrap();
    fs::write(root.0.join("copy"), &random).unwrap();
    for name in ["full", "part"] {
        let cp = Command::new("/bin/cp")
            .arg("-c")
            .args([root.0.join("orig"), root.0.join(name)])
            .status()
            .unwrap();
        assert!(cp.success());
    }
    let mut part = File::options()
        .write(true)
        .open(root.0.join("part"))
        .unwrap();
    std::io::Seek::seek(&mut part, std::io::SeekFrom::Start(MB)).unwrap();
    part.write_all(&vec![7u8; MB as usize]).unwrap();
    part.sync_all().unwrap();
    drop(part);
    write(&root.0.join("small"), 10);
    fs::hard_link(root.0.join("small"), root.0.join("link")).unwrap();

    let (bulk, portable) = both_ways(&root.0);
    assert_same(logical_fingerprint(&bulk), logical_fingerprint(&portable));
    if !bulk.clone_aware {
        println!("skipped: the temp folder's volume has no clones");
        return;
    }
    fn node<'a>(tree: &'a Tree, name: &str) -> &'a hoardmap::tree::Node {
        &tree.nodes[child(tree, ROOT, name) as usize]
    }
    for name in ["orig", "full", "part", "copy"] {
        assert_eq!(node(&bulk, name).size, 4 * MB, "{name}");
        assert_eq!(node(&portable, name).physical, 4 * MB, "{name}");
    }
    // Both are clones; the pair sorts by name, so `full` keeps the shared data.
    let (first, clone) = (node(&bulk, "full"), node(&bulk, "orig"));
    assert_eq!((first.shared, first.physical), (Shared::Clone, 4 * MB));
    assert_eq!((clone.shared, clone.physical), (Shared::Clone, 0));
    // A clone with part of its data rewritten no longer shares all of it.
    let part = node(&bulk, "part");
    assert_eq!((part.shared, part.physical), (Shared::None, 4 * MB));
    let copy = node(&bulk, "copy");
    assert_eq!((copy.shared, copy.physical), (Shared::None, 4 * MB));

    assert_eq!(bulk.clone_counted_once, 4 * MB);
    // `link` sorts first, so it keeps the bytes.
    let (kept, link) = (node(&bulk, "link"), node(&bulk, "small"));
    assert_eq!(kept.shared, Shared::None);
    assert_eq!((link.shared, link.physical), (Shared::HardLink, 0));
    assert_eq!(bulk.root().physical, 12 * MB + kept.physical);
    assert_eq!(portable.root().physical, 16 * MB + kept.physical);
    assert_eq!(portable.clone_counted_once, 0);
}

fn same_tree_on_one_thread_and_eight(root: &Path) {
    let scan = |threads| {
        scan_with_threads(root, &Progress::default(), Method::default(), threads).expect("scan")
    };
    let (one, eight) = (scan(1), scan(8));
    assert_eq!(one.error_count, eight.error_count);
    let figures = |t: &Tree| (t.clone_aware, t.clone_counted_once);
    assert_eq!(figures(&one), figures(&eight));
    let (a, b) = (fingerprint(&one), fingerprint(&eight));
    assert_eq!(a.len(), b.len(), "node counts differ");
    for (x, y) in a.iter().zip(&b) {
        assert_eq!(x, y);
    }
}

#[test]
fn one_thread_and_many_build_the_same_tree() {
    let fx = Fixture::build();
    same_tree_on_one_thread_and_eight(&fx.root);
    let target = Path::new(env!("CARGO_MANIFEST_DIR")).join("target/debug/build");
    if target.is_dir() {
        same_tree_on_one_thread_and_eight(&target);
    }
}

/// A folder removed on drop.
struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn a_hard_link_can_reorder_folders() {
    // x/ and z/ hold the same 100-byte file; y/ holds 60 bytes. Before the hard link is
    // counted once, x and z tie at 100 above y; after, z keeps nothing.
    let root = Scratch(std::env::temp_dir().join(format!("hoardmap-links-{}", std::process::id())));
    for dir in ["x", "y", "z"] {
        fs::create_dir_all(root.0.join(dir)).unwrap();
    }
    write(&root.0.join("x/f.bin"), 100);
    write(&root.0.join("y/g.bin"), 60);
    fs::hard_link(root.0.join("x/f.bin"), root.0.join("z/f.bin")).unwrap();

    let tree = scan(&root.0, &Progress::default()).expect("scan");
    assert_eq!(names(&tree, ROOT), ["x", "y", "z"]);
    let size = |name| tree.nodes[child(&tree, ROOT, name) as usize].size;
    assert_eq!((size("x"), size("y"), size("z")), (100, 60, 0));
}
