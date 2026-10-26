//! Scanner acceptance tests on a fixture built under the system temp folder.

use std::fs::{self, File};
use std::io::Write;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant, SystemTime};

use hoardmap::scan::{Progress, scan};
use hoardmap::tree::{Kind, NodeId, ROOT, Tree};

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
    assert!(!first.hard_link);
    assert_eq!(first.size, 10);
    assert!(first.physical >= 10);
    assert!(second.hard_link);
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
    let (dirs, files, bytes) = progress.snapshot();
    assert_eq!(dirs, u64::from(tree.root().dirs));
    assert_eq!(files, u64::from(tree.root().files));
    assert!(bytes >= tree.root().size);
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
