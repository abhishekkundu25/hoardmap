//! Builds a [`crate::tree::Tree`] from the file system: a parallel walk that stays on
//! the root's device, counts hard links once, records allocated size and the latest
//! modification time per subtree, and reports progress while it runs.
//!
//! Symlinks are never followed: a symlink is a leaf of kind [`Kind::Symlink`] whose
//! `size` is its own lstat size (the target path's length), counted in `files` and in
//! `size` like any other non-directory. The root path itself is resolved (so `/tmp`
//! works); below it every entry is read with `lstat`.
//!
//! Workers build [`Local`] subtrees with rayon, one task per subdirectory. The flatten
//! is sequential and depth-first: siblings are visited largest first by their pre-dedup
//! size (ties by name), which fixes the arena order and thereby which occurrence of a
//! hard-linked file keeps its bytes. Directory totals are then recomputed from the
//! deduplicated children, and each `children` list is sorted by that final size.
//!
//! [`Progress::bytes`] counts every entry's logical size as it is seen, so a file with
//! several hard links is counted once per link there; the finished tree counts it once.

use std::cmp::Ordering;
use std::collections::HashSet;
use std::fs::{self, Metadata};
use std::io;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};

use rayon::prelude::*;

use crate::tree::{Kind, MAX_ERRORS, Node, NodeId, ScanError, Tree};

/// Counters a running scan increments and a reader samples.
#[derive(Debug, Default)]
pub struct Progress {
    /// Directories seen, the root excluded.
    pub dirs: AtomicU64,
    /// Non-directory entries seen.
    pub files: AtomicU64,
    /// Logical bytes of the non-directory entries seen.
    pub bytes: AtomicU64,
}

impl Progress {
    /// `(dirs, files, bytes)` so far.
    pub fn snapshot(&self) -> (u64, u64, u64) {
        (
            self.dirs.load(AtomicOrdering::Relaxed),
            self.files.load(AtomicOrdering::Relaxed),
            self.bytes.load(AtomicOrdering::Relaxed),
        )
    }
}

/// A node as a worker builds it, before hard-link accounting. `size` on a directory is
/// the plain sum of its children; the flatten recomputes every directory total.
struct Local {
    name: String,
    kind: Kind,
    size: u64,
    physical: u64,
    mtime: i64,
    ino: u64,
    nlink: u64,
    dev: u64,
    children: Vec<Local>,
}

impl Local {
    fn leaf(name: String, kind: Kind, meta: &Metadata) -> Local {
        Local {
            name,
            kind,
            size: meta.len(),
            physical: meta.blocks() * 512,
            mtime: meta.mtime(),
            ino: meta.ino(),
            nlink: meta.nlink(),
            dev: meta.dev(),
            children: Vec::new(),
        }
    }

    fn dir(name: String, meta: &Metadata, children: Vec<Local>) -> Local {
        Local {
            name,
            kind: Kind::Dir,
            size: children.iter().map(|c| c.size).sum(),
            physical: 0,
            mtime: meta.mtime(),
            ino: meta.ino(),
            nlink: meta.nlink(),
            dev: meta.dev(),
            children,
        }
    }
}

struct Ctx<'a> {
    root_dev: u64,
    progress: &'a Progress,
    errors: Mutex<Vec<ScanError>>,
    error_count: AtomicU64,
}

impl Ctx<'_> {
    fn record(&self, path: &Path, err: &io::Error) {
        // fetch_add hands out distinct ranks, so exactly the first MAX_ERRORS are kept.
        let rank = self.error_count.fetch_add(1, AtomicOrdering::Relaxed);
        if (rank as usize) < MAX_ERRORS {
            let error = ScanError {
                path: path.to_string_lossy().into_owned(),
                error: err.to_string(),
            };
            self.errors
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .push(error);
        }
    }
}

/// Scans `path` into a tree. `Err` only when the root cannot be read or is not a
/// directory; every other failure is recorded in the tree's `errors`.
pub fn scan(path: &Path, progress: &Progress) -> io::Result<Tree> {
    let meta = fs::metadata(path)?;
    if !meta.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::NotADirectory,
            format!("{} is not a directory", path.display()),
        ));
    }
    let rd = fs::read_dir(path)?;
    let ctx = Ctx {
        root_dev: meta.dev(),
        progress,
        errors: Mutex::new(Vec::new()),
        error_count: AtomicU64::new(0),
    };
    let children = read_entries(path, rd, &ctx);
    let root = Local::dir(path.to_string_lossy().into_owned(), &meta, children);

    let (dirs, files, _) = progress.snapshot();
    let mut nodes = Vec::with_capacity((dirs + files + 1) as usize);
    let mut seen = HashSet::new();
    flatten(root, None, &mut nodes, &mut seen);

    Ok(Tree {
        nodes,
        errors: ctx.errors.into_inner().unwrap_or_else(|e| e.into_inner()),
        error_count: ctx.error_count.load(AtomicOrdering::Relaxed),
    })
}

fn read_children(dir: &Path, ctx: &Ctx) -> Vec<Local> {
    match fs::read_dir(dir) {
        Ok(rd) => read_entries(dir, rd, ctx),
        Err(e) => {
            ctx.record(dir, &e);
            Vec::new()
        }
    }
}

fn read_entries(dir: &Path, rd: fs::ReadDir, ctx: &Ctx) -> Vec<Local> {
    let mut children = Vec::new();
    let mut subdirs: Vec<(String, Metadata, PathBuf)> = Vec::new();
    for entry in rd {
        let entry = match entry {
            Ok(e) => e,
            Err(e) => {
                ctx.record(dir, &e);
                continue;
            }
        };
        // On Unix DirEntry::metadata is an lstat relative to the open directory (fstatat
        // with AT_SYMLINK_NOFOLLOW), so it never follows a symlink and builds no path.
        let meta = match entry.metadata() {
            Ok(m) => m,
            Err(e) => {
                ctx.record(&entry.path(), &e);
                continue;
            }
        };
        let name = match entry.file_name().into_string() {
            Ok(s) => s,
            Err(os) => os.to_string_lossy().into_owned(),
        };
        let ft = meta.file_type();
        if ft.is_dir() {
            ctx.progress.dirs.fetch_add(1, AtomicOrdering::Relaxed);
            if meta.dev() == ctx.root_dev {
                subdirs.push((name, meta, entry.path()));
            } else {
                children.push(Local::dir(name, &meta, Vec::new()));
            }
        } else {
            let kind = if ft.is_file() {
                Kind::File
            } else if ft.is_symlink() {
                Kind::Symlink
            } else {
                Kind::Other
            };
            ctx.progress.files.fetch_add(1, AtomicOrdering::Relaxed);
            ctx.progress
                .bytes
                .fetch_add(meta.len(), AtomicOrdering::Relaxed);
            children.push(Local::leaf(name, kind, &meta));
        }
    }
    match subdirs.len() {
        0 => {}
        1 => {
            let (name, meta, path) = subdirs.pop().unwrap();
            children.push(Local::dir(name, &meta, read_children(&path, ctx)));
        }
        _ => children.par_extend(
            subdirs
                .into_par_iter()
                .map(|(name, meta, path)| Local::dir(name, &meta, read_children(&path, ctx))),
        ),
    }
    children
}

/// Largest first, ties by name.
fn by_size_then_name(a: (u64, &str), b: (u64, &str)) -> Ordering {
    b.0.cmp(&a.0).then_with(|| a.1.cmp(b.1))
}

fn flatten(
    local: Local,
    parent: Option<NodeId>,
    nodes: &mut Vec<Node>,
    seen: &mut HashSet<(u64, u64)>,
) -> NodeId {
    let Local {
        name,
        kind,
        mut size,
        mut physical,
        mut mtime,
        ino,
        nlink,
        dev,
        mut children,
    } = local;
    let id = nodes.len() as NodeId;
    let mut hard_link = false;
    if kind != Kind::Dir && nlink > 1 && !seen.insert((dev, ino)) {
        size = 0;
        physical = 0;
        hard_link = true;
    }
    nodes.push(Node {
        name,
        parent,
        kind,
        size,
        physical,
        files: 0,
        dirs: 0,
        mtime,
        children: Vec::new(),
        hard_link,
    });
    if kind != Kind::Dir {
        return id;
    }

    children.sort_by(|a, b| by_size_then_name((a.size, &a.name), (b.size, &b.name)));
    let mut ids = Vec::with_capacity(children.len());
    let (mut size, mut physical, mut files, mut dirs) = (0u64, 0u64, 0u32, 0u32);
    for child in children {
        let cid = flatten(child, Some(id), nodes, seen);
        let c = &nodes[cid as usize];
        size += c.size;
        physical += c.physical;
        mtime = mtime.max(c.mtime);
        if c.kind == Kind::Dir {
            dirs += 1 + c.dirs;
            files += c.files;
        } else {
            files += 1;
        }
        ids.push(cid);
    }
    ids.sort_by(|&a, &b| {
        let (a, b) = (&nodes[a as usize], &nodes[b as usize]);
        by_size_then_name((a.size, &a.name), (b.size, &b.name))
    });
    let node = &mut nodes[id as usize];
    node.size = size;
    node.physical = physical;
    node.files = files;
    node.dirs = dirs;
    node.mtime = mtime;
    node.children = ids;
    id
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn order_is_size_descending_then_name() {
        let mut v = vec![(5, "b"), (10, "z"), (5, "a"), (0, "x")];
        v.sort_by(|a, b| by_size_then_name(*a, *b));
        assert_eq!(v, vec![(10, "z"), (5, "a"), (5, "b"), (0, "x")]);
    }

    #[test]
    fn progress_snapshot_reads_all_three() {
        let p = Progress::default();
        p.dirs.fetch_add(1, AtomicOrdering::Relaxed);
        p.files.fetch_add(2, AtomicOrdering::Relaxed);
        p.bytes.fetch_add(3, AtomicOrdering::Relaxed);
        assert_eq!(p.snapshot(), (1, 2, 3));
    }
}
