//! Builds a [`crate::tree::Tree`] from the file system: a parallel walk that stays on
//! the root's device, counts hard links once, records allocated size and the latest
//! modification time per subtree, and reports progress while it runs.
//!
//! Symlinks are never followed: a symlink is a leaf of kind [`Kind::Symlink`] whose
//! `size` is its own lstat size (the target path's length), counted in `files` and in
//! `size` like any other non-directory. The root path itself is resolved (so `/tmp`
//! works); below it entries are never followed.
//!
//! A folder is listed one of two ways, [`Method`]. On macOS, `getattrlistbulk` returns
//! names, types and sizes for a whole batch of entries in one call. Everywhere, the
//! portable way is `readdir` plus one `fstatat` per entry. Both give the same tree; set
//! `HOARDMAP_SCANNER=portable` to use the portable way on a Mac.
//!
//! Workers build [`Local`] subtrees with rayon, one task per subdirectory, in a pool of
//! [`default_threads`] threads: past the fast cores, more threads only queue for the
//! file system and make the scan slower. The flatten
//! is sequential and depth-first: siblings are visited largest first by their pre-dedup
//! size (ties by name), which fixes the arena order and thereby which occurrence of a
//! hard-linked file keeps its bytes. Directory totals are then recomputed from the
//! deduplicated children, and a `children` list is sorted again only when a hard link
//! changed a child's size.
//!
//! On a volume with clones the bulk listing also reads each file's clone id and
//! extended flags. The flatten counts a group of clones that share all their data
//! once, keyed by `(dev, clone id)` the way hard links are keyed by `(dev, inode)`, in
//! the same depth-first order. A clone that has partly changed since is counted in
//! full. The portable listing cannot see clones: its tree is not clone-aware.
//!
//! [`Progress::bytes`] and [`Progress::physical`] count every entry's logical and
//! allocated size as it is seen, so a file with several hard links is counted once per
//! link there; the finished tree counts it once.

use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::ffi::{CStr, CString, OsStr, OsString};
use std::fs::{File, Metadata};
use std::io;
use std::os::fd::{AsRawFd, FromRawFd};
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering as AtomicOrdering};
use std::{env, mem};

use rayon::prelude::*;

use crate::access;
use crate::tree::{Kind, MAX_ERRORS, Node, NodeId, ROOT, ScanError, Shared, Tree};

/// Counters a running scan increments and a reader samples.
#[derive(Debug, Default)]
pub struct Progress {
    /// Directories seen, the root excluded.
    pub dirs: AtomicU64,
    /// Non-directory entries seen.
    pub files: AtomicU64,
    /// Logical bytes of the non-directory entries seen.
    pub bytes: AtomicU64,
    /// Bytes allocated on disk for the non-directory entries seen.
    pub physical: AtomicU64,
    /// Set when the scan starts if it reads clone figures.
    pub clone_aware: AtomicBool,
    /// The entries down to [`LIVE_DEPTH`] that the scan has finished, for [`partial`].
    pub finished: Mutex<Vec<Finished>>,
    /// Set to end the scan early, checked between entry batches and folders.
    pub stop: AtomicBool,
}

/// How deep below the root a running scan reports finished entries.
pub const LIVE_DEPTH: usize = 3;

/// A file, or a folder with its subtree totals, that a running scan has finished.
#[derive(Clone, Debug)]
pub struct Finished {
    /// File names from the root down to this entry.
    pub path: Vec<OsString>,
    /// Only the empty-path root record carries a non-UTF-8 scan root.
    pub raw_root: Option<Box<OsStr>>,
    pub kind: Kind,
    pub size: u64,
    pub physical: u64,
    pub files: u32,
    pub dirs: u32,
    pub mtime: i64,
}

impl Progress {
    /// `(dirs, files, bytes, physical)` so far.
    pub fn snapshot(&self) -> (u64, u64, u64, u64) {
        (
            self.dirs.load(AtomicOrdering::Relaxed),
            self.files.load(AtomicOrdering::Relaxed),
            self.bytes.load(AtomicOrdering::Relaxed),
            self.physical.load(AtomicOrdering::Relaxed),
        )
    }
}

/// A node as a worker builds it, before hard-link accounting. `size` on a directory is
/// the plain sum of its children; the flatten recomputes every directory total.
struct Local {
    name: OsString,
    kind: Kind,
    size: u64,
    physical: u64,
    blocks: Blocks,
    mtime: i64,
    ino: u64,
    nlink: u64,
    dev: u64,
    /// Subtree counts, kept as the walk builds folders so a preview reads them at once.
    files: u32,
    dirs: u32,
    children: Vec<Local>,
}

impl Local {
    fn leaf(name: OsString, stat: &Stat) -> Local {
        Local {
            name,
            kind: stat.kind,
            size: stat.size,
            physical: stat.physical,
            blocks: stat.blocks,
            mtime: stat.mtime,
            ino: stat.ino,
            nlink: stat.nlink,
            dev: stat.dev,
            files: 0,
            dirs: 0,
            children: Vec::new(),
        }
    }

    /// A folder with the sums of its children: hard links counted each time, which the
    /// flatten corrects; good enough for a running scan's preview.
    fn dir(name: OsString, stat: &Stat, children: Vec<Local>) -> Local {
        let mut local = Local {
            name,
            kind: Kind::Dir,
            size: 0,
            physical: 0,
            blocks: Blocks::Own,
            mtime: stat.mtime,
            ino: stat.ino,
            nlink: stat.nlink,
            dev: stat.dev,
            files: 0,
            dirs: 0,
            children: Vec::new(),
        };
        for child in &children {
            local.size += child.size;
            local.physical += child.physical;
            local.mtime = local.mtime.max(child.mtime);
            if child.kind == Kind::Dir {
                local.files += child.files;
                local.dirs += 1 + child.dirs;
            } else {
                local.files += 1;
            }
        }
        local.children = children;
        local
    }
}

/// What the walk needs about one entry, from either listing method.
struct Stat {
    kind: Kind,
    size: u64,
    physical: u64,
    blocks: Blocks,
    mtime: i64,
    ino: u64,
    nlink: u64,
    dev: u64,
    /// Another volume is mounted here; the walk does not enter it.
    mount_point: bool,
    /// A cloud folder (iCloud Drive, Google Drive, ...) whose listing is not on this Mac.
    /// Reading it would download it, so the walk does not enter it.
    dataless: bool,
}

/// What a file shares with other files, as APFS reports it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Blocks {
    /// Shares nothing, only part of its data, or the scan cannot tell.
    Own,
    /// Shares all its data with the other files of this clone id.
    All(u64),
}

/// `SF_DATALESS` from <sys/stat.h>: the item's contents are in the cloud, not on disk.
#[cfg(target_os = "macos")]
const SF_DATALESS: u32 = 0x4000_0000;

impl Stat {
    // libc's integer widths differ across Unix targets.
    #[allow(clippy::unnecessary_cast)]
    fn of_raw(meta: &libc::stat) -> Stat {
        let kind = match meta.st_mode & libc::S_IFMT {
            libc::S_IFDIR => Kind::Dir,
            libc::S_IFREG => Kind::File,
            libc::S_IFLNK => Kind::Symlink,
            _ => Kind::Other,
        };
        Stat {
            kind,
            size: meta.st_size as u64,
            physical: meta.st_blocks as u64 * 512,
            blocks: Blocks::Own,
            mtime: meta.st_mtime as i64,
            ino: meta.st_ino as u64,
            nlink: meta.st_nlink as u64,
            #[cfg(target_os = "macos")]
            dev: meta.st_dev as u32 as u64,
            #[cfg(not(target_os = "macos"))]
            dev: meta.st_dev as u64,
            mount_point: false,
            #[cfg(target_os = "macos")]
            dataless: meta.st_flags & SF_DATALESS != 0,
            #[cfg(not(target_os = "macos"))]
            dataless: false,
        }
    }

    fn of(meta: &Metadata) -> Stat {
        let ft = meta.file_type();
        let physical = meta.blocks() * 512;
        let kind = if ft.is_dir() {
            Kind::Dir
        } else if ft.is_file() {
            Kind::File
        } else if ft.is_symlink() {
            Kind::Symlink
        } else {
            Kind::Other
        };
        Stat {
            kind,
            size: meta.len(),
            physical,
            blocks: Blocks::Own,
            mtime: meta.mtime(),
            ino: meta.ino(),
            nlink: meta.nlink(),
            dev: meta.dev(),
            mount_point: false,
            #[cfg(target_os = "macos")]
            dataless: std::os::macos::fs::MetadataExt::st_flags(meta) & SF_DATALESS != 0,
            #[cfg(not(target_os = "macos"))]
            dataless: false,
        }
    }
}

/// How a folder is listed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Method {
    /// POSIX directory iteration and one `fstatat` per entry; every Unix.
    Portable,
    /// `getattrlistbulk`: a batch of entries with their attributes per call.
    #[cfg(target_os = "macos")]
    Bulk,
}

impl Default for Method {
    /// The bulk listing on macOS unless `HOARDMAP_SCANNER=portable`; portable elsewhere.
    fn default() -> Method {
        #[cfg(target_os = "macos")]
        if env::var_os("HOARDMAP_SCANNER").is_none_or(|v| v != "portable") {
            return Method::Bulk;
        }
        Method::Portable
    }
}

struct Ctx<'a> {
    method: Method,
    /// The bulk listing reads clone figures.
    clones: bool,
    root_dev: u64,
    progress: &'a Progress,
    errors: Mutex<Vec<ScanError>>,
    error_count: AtomicU64,
    /// Folders below the root that are counted but not read.
    skip: Vec<PathBuf>,
    root: &'a Path,
    listing_slots: [Mutex<()>; MAX_LISTINGS],
}

impl Ctx<'_> {
    fn observe(&self, stat: &Stat) {
        if stat.kind == Kind::Dir {
            self.progress.dirs.fetch_add(1, AtomicOrdering::Relaxed);
        } else {
            self.progress.files.fetch_add(1, AtomicOrdering::Relaxed);
            self.progress
                .bytes
                .fetch_add(stat.size, AtomicOrdering::Relaxed);
            self.progress
                .physical
                .fetch_add(stat.physical, AtomicOrdering::Relaxed);
        }
    }

    /// Hands entries of a folder near the root to [`Progress::finished`].
    fn finish(&self, dir: &Path, done: &[&Local]) {
        let Ok(rel) = dir.strip_prefix(self.root) else {
            return;
        };
        let parent: Vec<OsString> = rel.components().map(|c| c.as_os_str().to_owned()).collect();
        let entries = done.iter().map(|local| {
            let mut path = parent.clone();
            path.push(local.name.clone());
            Finished {
                path,
                raw_root: None,
                kind: local.kind,
                size: local.size,
                physical: local.physical,
                files: local.files,
                dirs: local.dirs,
                mtime: local.mtime,
            }
        });
        let mut finished = self.progress.finished.lock().unwrap();
        if finished.is_empty() && self.root.to_str().is_none() {
            finished.push(Finished {
                path: Vec::new(),
                raw_root: Some(self.root.as_os_str().into()),
                kind: Kind::Dir,
                size: 0,
                physical: 0,
                files: 0,
                dirs: 0,
                mtime: 0,
            });
        }
        finished.extend(entries);
    }
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
            self.errors.lock().unwrap().push(error);
        }
    }
}

/// Scans `path` into a tree with the platform's default [`Method`]. `Err` only when
/// the root cannot be read or is not a directory; every other failure is recorded in
/// the tree's `errors`.
pub fn scan(path: &Path, progress: &Progress) -> io::Result<Tree> {
    scan_with(path, progress, Method::default())
}

/// A tree of what a running scan has finished so far, named `root`. Folders still
/// being read show the sum of their finished entries; finished folders deeper than
/// [`LIVE_DEPTH`] show their totals without children.
pub fn partial(root: &str, finished: &[Finished]) -> Tree {
    let mut nodes = vec![Node {
        name: root.to_string(),
        parent: None,
        kind: Kind::Dir,
        size: 0,
        physical: 0,
        files: 0,
        dirs: 0,
        mtime: 0,
        children: Vec::new(),
        shared: Shared::None,
    }];
    let mut raw_names = HashMap::new();
    if let Some(raw) = finished
        .first()
        .and_then(|entry| entry.raw_root.as_ref())
        .filter(|raw| raw.to_string_lossy() == root)
    {
        raw_names.insert(ROOT, raw.clone());
    }
    // Finished nodes keep their reported totals; the others add up their children.
    let mut fixed = vec![false];
    let mut index: HashMap<(NodeId, &OsStr), NodeId> = HashMap::new();
    for entry in finished {
        let mut at = ROOT;
        for (depth, name) in entry.path.iter().enumerate() {
            let last = depth + 1 == entry.path.len();
            at = match index.get(&(at, name.as_os_str())) {
                Some(&id) => id,
                None => {
                    let id = nodes.len() as NodeId;
                    let display = match name.clone().into_string() {
                        Ok(name) => name,
                        Err(raw) => {
                            let display = raw.to_string_lossy().into_owned();
                            raw_names.insert(id, raw.into_boxed_os_str());
                            display
                        }
                    };
                    nodes.push(Node {
                        name: display,
                        parent: Some(at),
                        kind: Kind::Dir,
                        size: 0,
                        physical: 0,
                        files: 0,
                        dirs: 0,
                        mtime: 0,
                        children: Vec::new(),
                        shared: Shared::None,
                    });
                    fixed.push(false);
                    nodes[at as usize].children.push(id);
                    index.insert((at, name.as_os_str()), id);
                    id
                }
            };
            if last {
                let node = &mut nodes[at as usize];
                node.kind = entry.kind;
                node.size = entry.size;
                node.physical = entry.physical;
                node.files = entry.files;
                node.dirs = entry.dirs;
                node.mtime = entry.mtime;
                fixed[at as usize] = true;
            }
        }
    }
    // A parent is always created before its children, so a reverse pass sums upward.
    for id in (1..nodes.len()).rev() {
        let child = &nodes[id];
        let (size, physical, mtime) = (child.size, child.physical, child.mtime);
        let (files, dirs) = if child.kind == Kind::Dir {
            (child.files, 1 + child.dirs)
        } else {
            (1, 0)
        };
        let parent = child.parent.unwrap() as usize;
        if !fixed[parent] {
            let p = &mut nodes[parent];
            p.size += size;
            p.physical += physical;
            p.files += files;
            p.dirs += dirs;
            p.mtime = p.mtime.max(mtime);
        }
    }
    for id in 0..nodes.len() {
        let mut children = mem::take(&mut nodes[id].children);
        children.sort_by(|&a, &b| {
            let (a, b) = (&nodes[a as usize], &nodes[b as usize]);
            by_size_then_name((a.size, &a.name), (b.size, &b.name))
        });
        nodes[id].children = children;
    }
    Tree {
        nodes,
        errors: Vec::new(),
        error_count: 0,
        clone_aware: false,
        clone_counted_once: 0,
        raw_names,
    }
}

/// Makes reading an item that lives only in the cloud fail at once instead of
/// downloading it, for the whole process: a scan reads sizes and must never fetch files.
#[cfg(target_os = "macos")]
fn never_download() {
    // From <sys/resource.h>; not in the libc crate.
    const IOPOL_TYPE_VFS_MATERIALIZE_DATALESS_FILES: libc::c_int = 3;
    const IOPOL_SCOPE_PROCESS: libc::c_int = 0;
    const IOPOL_MATERIALIZE_DATALESS_FILES_OFF: libc::c_int = 1;
    unsafe extern "C" {
        fn setiopolicy_np(
            iotype: libc::c_int,
            scope: libc::c_int,
            policy: libc::c_int,
        ) -> libc::c_int;
    }
    // SAFETY: plain integers; the call only sets a policy on this process.
    unsafe {
        setiopolicy_np(
            IOPOL_TYPE_VFS_MATERIALIZE_DATALESS_FILES,
            IOPOL_SCOPE_PROCESS,
            IOPOL_MATERIALIZE_DATALESS_FILES_OFF,
        );
    }
}

/// The error a folder left out by [`skipping`] is listed with.
pub const NOT_SCANNED: &str = "Not scanned: needs Full Disk Access";

/// [`scan`] with a chosen listing method.
pub fn scan_with(path: &Path, progress: &Progress, method: Method) -> io::Result<Tree> {
    scan_skipping(path, progress, method, skipping())
}

/// [`scan_with`] on `threads` walker threads instead of [`default_threads`].
pub fn scan_with_threads(
    path: &Path,
    progress: &Progress,
    method: Method,
    threads: usize,
) -> io::Result<Tree> {
    walk(path, progress, method, skipping(), threads)
}

/// How many threads walk the file system: `HOARDMAP_THREADS` when set, else
/// [`pick_threads`] for this machine.
pub fn default_threads() -> usize {
    if let Some(n) = env::var("HOARDMAP_THREADS")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .filter(|&n| n > 0)
    {
        return n;
    }
    let logical = std::thread::available_parallelism().map_or(1, |n| n.get());
    #[cfg(target_os = "macos")]
    {
        let levels: Vec<usize> = (0..cores::sysctl("hw.nperflevels").unwrap_or(0))
            .map_while(|i| cores::sysctl(&format!("hw.perflevel{i}.physicalcpu")))
            .collect();
        pick_threads(&levels, cores::sysctl("hw.physicalcpu"), logical)
    }
    #[cfg(not(target_os = "macos"))]
    pick_threads(&[], None, logical)
}

/// The walker thread count from the CPU layout: the cores of every performance level
/// but the last (the efficiency cores) when there are several, else the physical
/// cores, else the logical CPUs; never more than the logical CPUs or fewer than 1.
/// Measured on an M6 (2 Super, 4 Performance, 6 Efficiency): 6 threads scan a home
/// folder in 1.1 s, 12 in 1.3 s.
fn pick_threads(levels: &[usize], physical: Option<usize>, logical: usize) -> usize {
    let n = match levels {
        [_, .., _] => levels[..levels.len() - 1].iter().sum(),
        _ => physical.unwrap_or(logical),
    };
    n.clamp(1, logical.max(1))
}

#[cfg(target_os = "macos")]
mod cores {
    use std::ffi::CString;

    /// An integer `sysctl` value, `None` when the name is unknown.
    pub(super) fn sysctl(name: &str) -> Option<usize> {
        let name = CString::new(name).ok()?;
        let mut value: libc::c_int = 0;
        let mut len = size_of::<libc::c_int>();
        // SAFETY: `value` and `len` describe a buffer of `len` bytes; no new value is set.
        let rc = unsafe {
            libc::sysctlbyname(
                name.as_ptr(),
                (&raw mut value).cast(),
                &mut len,
                std::ptr::null_mut(),
                0,
            )
        };
        (rc == 0 && value > 0).then_some(value as usize)
    }
}

/// Without Full Disk Access, the folders that would each stop the scan with a macOS
/// prompt; they are listed as [`NOT_SCANNED`] instead.
fn skipping() -> Vec<PathBuf> {
    match env::var_os("HOME") {
        Some(home) if !access::granted() => access::prompting(Path::new(&home)),
        _ => Vec::new(),
    }
}

/// [`scan_with`], counting the folders in `skip` without reading them.
pub fn scan_skipping(
    path: &Path,
    progress: &Progress,
    method: Method,
    skip: Vec<PathBuf>,
) -> io::Result<Tree> {
    walk(path, progress, method, skip, default_threads())
}

fn walk(
    path: &Path,
    progress: &Progress,
    method: Method,
    skip: Vec<PathBuf>,
    threads: usize,
) -> io::Result<Tree> {
    #[cfg(target_os = "macos")]
    never_download();
    let directory = open_directory(path, None)?;
    let meta = directory.metadata()?;
    let clones = match method {
        Method::Portable => false,
        #[cfg(target_os = "macos")]
        Method::Bulk => bulk::has_clones(path),
    };
    progress.clone_aware.store(clones, AtomicOrdering::Relaxed);
    let ctx = Ctx {
        method,
        clones,
        root_dev: meta.dev(),
        progress,
        errors: Mutex::new(Vec::new()),
        error_count: AtomicU64::new(0),
        skip,
        root: path,
        listing_slots: std::array::from_fn(|_| Mutex::new(())),
    };
    ctx.finish(path, &[]);
    let entries = list(path, directory, &ctx)?;
    let children = match rayon::ThreadPoolBuilder::new().num_threads(threads).build() {
        Ok(pool) => pool.install(|| build(path, entries, &ctx, 0)),
        Err(_) => build(path, entries, &ctx, 0),
    };
    let root = Local::dir(path.as_os_str().to_owned(), &Stat::of(&meta), children);

    let (dirs, files, ..) = progress.snapshot();
    let mut nodes = Vec::with_capacity((dirs + files + 1) as usize);
    let mut seen = Seen::default();
    flatten(root, None, &mut nodes, &mut seen);

    Ok(Tree {
        nodes,
        errors: ctx.errors.into_inner().unwrap(),
        error_count: ctx.error_count.load(AtomicOrdering::Relaxed),
        clone_aware: clones,
        clone_counted_once: seen.clone_counted_once,
        raw_names: seen.raw_names,
    })
}

const ENTRY_BATCH: usize = 256;
const DIRECTORY_CHANGED: &str = "Folder changed while scanning; skipped";

// Each portable listing holds a directory and its iterator's duplicate descriptor.
const MAX_LISTINGS: usize = 32;

/// Open children without following a symlink, and verify the listed identity.
/// The root may itself be a symlink, as documented by [`scan`].
fn open_directory(path: &Path, expected: Option<&Stat>) -> io::Result<File> {
    let name = CString::new(path.as_os_str().as_bytes())?;
    let flags = libc::O_RDONLY
        | libc::O_DIRECTORY
        | libc::O_CLOEXEC
        | if expected.is_some() {
            libc::O_NOFOLLOW
        } else {
            0
        };
    // SAFETY: name is NUL terminated; the fresh descriptor is owned below.
    let fd = unsafe { libc::open(name.as_ptr(), flags) };
    if fd < 0 {
        let error = io::Error::last_os_error();
        return Err(match error.raw_os_error() {
            Some(libc::ELOOP | libc::ENOTDIR | libc::ENOENT) if expected.is_some() => {
                io::Error::other(DIRECTORY_CHANGED)
            }
            Some(libc::ENOTDIR) => io::Error::new(
                io::ErrorKind::NotADirectory,
                format!("{} is not a directory", path.display()),
            ),
            _ => error,
        });
    }
    // SAFETY: open/openat returned a fresh descriptor that nothing else owns.
    let directory = unsafe { File::from_raw_fd(fd) };
    if let Some(expected) = expected {
        let actual = directory.metadata()?;
        if (actual.dev(), actual.ino()) != (expected.dev, expected.ino) {
            return Err(io::Error::other(DIRECTORY_CHANGED));
        }
    }
    Ok(directory)
}

/// The entries of `dir`. Close its descriptor before descending.
fn list(dir: &Path, directory: File, ctx: &Ctx) -> io::Result<Vec<(OsString, Stat)>> {
    match ctx.method {
        Method::Portable => list_portable(dir, &directory, ctx),
        #[cfg(target_os = "macos")]
        Method::Bulk => bulk::list(dir, &directory, ctx),
    }
}

/// `read_dir` has no descriptor interface. Use its POSIX equivalents to enumerate
/// the opened and identity-checked directory, including if an ancestor is renamed.
fn list_portable(dir: &Path, directory: &File, ctx: &Ctx) -> io::Result<Vec<(OsString, Stat)>> {
    struct Entries(*mut libc::DIR);
    impl Drop for Entries {
        fn drop(&mut self) {
            // SAFETY: fdopendir transferred ownership of this stream to Entries.
            unsafe { libc::closedir(self.0) };
        }
    }

    let fd = directory.try_clone()?;
    // SAFETY: fd is an open directory descriptor; on success fdopendir owns it.
    let stream = unsafe { libc::fdopendir(fd.as_raw_fd()) };
    if stream.is_null() {
        return Err(io::Error::last_os_error());
    }
    mem::forget(fd);
    let entries = Entries(stream);
    let mut out = Vec::new();
    let mut read = 0usize;
    loop {
        if read % ENTRY_BATCH == 0 && ctx.progress.stop.load(AtomicOrdering::Relaxed) {
            break;
        }
        // SAFETY: errno is thread-local. Clearing it distinguishes EOF from an error.
        unsafe {
            #[cfg(any(
                target_os = "macos",
                target_os = "ios",
                target_os = "freebsd",
                target_os = "dragonfly"
            ))]
            {
                *libc::__error() = 0;
            }
            #[cfg(any(target_os = "linux", target_os = "hurd"))]
            {
                *libc::__errno_location() = 0;
            }
            #[cfg(any(target_os = "android", target_os = "netbsd", target_os = "openbsd"))]
            {
                *libc::__errno() = 0;
            }
            #[cfg(any(target_os = "solaris", target_os = "illumos"))]
            {
                *libc::___errno() = 0;
            }
        }
        // SAFETY: entries owns a live stream, used only on this thread. The returned
        // entry remains valid until the next readdir call.
        let entry = unsafe { libc::readdir(entries.0) };
        if entry.is_null() {
            let error = io::Error::last_os_error();
            if error.kind() == io::ErrorKind::Interrupted {
                continue;
            }
            if error.raw_os_error() != Some(0) {
                ctx.record(dir, &error);
            }
            break;
        }
        read += 1;
        // SAFETY: readdir returns a dirent with a NUL-terminated name.
        let name = unsafe { CStr::from_ptr((*entry).d_name.as_ptr()) };
        if matches!(name.to_bytes(), b"." | b"..") {
            continue;
        }
        let mut meta = mem::MaybeUninit::<libc::stat>::uninit();
        // SAFETY: directory and name are valid; fstatat initializes meta on success.
        let rc = unsafe {
            libc::fstatat(
                directory.as_raw_fd(),
                name.as_ptr(),
                meta.as_mut_ptr(),
                libc::AT_SYMLINK_NOFOLLOW,
            )
        };
        if rc != 0 {
            ctx.record(
                &dir.join(OsStr::from_bytes(name.to_bytes())),
                &io::Error::last_os_error(),
            );
            continue;
        }
        // SAFETY: fstatat succeeded and initialized meta.
        let stat = Stat::of_raw(unsafe { meta.assume_init_ref() });
        ctx.observe(&stat);
        out.push((OsString::from_vec(name.to_bytes().to_vec()), stat));
    }
    Ok(out)
}

/// `depth` is how far `dir` is below the root, which is 0.
fn read_children(dir: &Path, stat: &Stat, ctx: &Ctx, depth: usize) -> Vec<Local> {
    if ctx.progress.stop.load(AtomicOrdering::Relaxed) {
        return Vec::new();
    }
    let entries = {
        // shortcut: saturated listings wait for slot 0; use a semaphore if contention matters.
        let _slot = ctx
            .listing_slots
            .iter()
            .find_map(|slot| slot.try_lock().ok())
            .unwrap_or_else(|| ctx.listing_slots[0].lock().unwrap());
        if ctx.progress.stop.load(AtomicOrdering::Relaxed) {
            return Vec::new();
        }
        // shortcut: renamed or overlong ancestors may be skipped; retain budgeted fds if needed.
        open_directory(dir, Some(stat)).and_then(|directory| list(dir, directory, ctx))
    };
    match entries {
        Ok(entries) => build(dir, entries, ctx, depth),
        Err(e) => {
            ctx.record(dir, &e);
            Vec::new()
        }
    }
}

/// Builds `entries`, then reads the subfolders on the same volume in parallel. `depth`
/// is how far `dir` is below the root, which is 0.
fn build(dir: &Path, entries: Vec<(OsString, Stat)>, ctx: &Ctx, depth: usize) -> Vec<Local> {
    // Only entries this shallow go into the running scan's preview.
    let live = depth < LIVE_DEPTH;
    let mut children = Vec::with_capacity(entries.len());
    let mut subdirs = Vec::new();
    for (name, stat) in entries {
        if stat.kind == Kind::Dir {
            if stat.dev == ctx.root_dev && !stat.mount_point && !stat.dataless {
                subdirs.push((name, stat));
            } else {
                children.push(Local::dir(name, &stat, Vec::new()));
            }
        } else {
            children.push(Local::leaf(name, &stat));
        }
    }
    if live {
        ctx.finish(dir, &children.iter().collect::<Vec<_>>());
    }
    let descend = |(name, stat): (OsString, Stat)| {
        let path = dir.join(&name);
        let grandchildren = if ctx.skip.contains(&path) {
            ctx.record(&path, &io::Error::other(NOT_SCANNED));
            Vec::new()
        } else {
            read_children(&path, &stat, ctx, depth + 1)
        };
        let local = Local::dir(name, &stat, grandchildren);
        if live {
            ctx.finish(dir, &[&local]);
        }
        local
    };
    match subdirs.len() {
        0 => {}
        1 => children.push(descend(subdirs.pop().unwrap())),
        _ => children.par_extend(subdirs.into_par_iter().map(descend)),
    }
    children
}

/// The `getattrlistbulk` listing. One call fills a buffer with as many entries as fit,
/// each a length, the set of attributes returned, then those attributes in a fixed
/// order: common ones by bit, then directory ones, then file ones, then the extended
/// common ones (asked for in the `forkattr` word) by bit.
#[cfg(target_os = "macos")]
mod bulk {
    use std::cell::RefCell;
    use std::ffi::{CString, OsString};
    use std::fs::File;
    use std::io;
    use std::os::fd::AsRawFd;
    use std::os::unix::ffi::{OsStrExt, OsStringExt};
    use std::path::Path;
    use std::ptr;

    use super::{Blocks, Ctx, Kind, SF_DATALESS, Stat, mem};

    /// Not in the libc crate; from <sys/attr.h>.
    const ATTR_CMN_ERROR: u32 = 0x2000_0000;
    /// Not in the libc crate; from <sys/stat.h>.
    const EF_SHARES_ALL_BLOCKS: u64 = 0x40;
    /// `enum vtype` from <sys/vnode.h>.
    const VREG: u32 = 1;
    const VDIR: u32 = 2;
    const VLNK: u32 = 5;
    const BUFFER: usize = 256 * 1024;

    thread_local! {
        static BUF: RefCell<Vec<u8>> = RefCell::new(vec![0; BUFFER]);
    }

    /// Whether the volume holding `path` supports clones (`VOL_CAP_INT_CLONE`).
    pub(super) fn has_clones(path: &Path) -> bool {
        let Ok(cpath) = CString::new(path.as_os_str().as_bytes()) else {
            return false;
        };
        let mut request = libc::attrlist {
            bitmapcount: libc::ATTR_BIT_MAP_COUNT,
            reserved: 0,
            commonattr: 0,
            volattr: libc::ATTR_VOL_INFO | libc::ATTR_VOL_CAPABILITIES,
            dirattr: 0,
            fileattr: 0,
            forkattr: 0,
        };
        let mut buf = [0u8; 4 + mem::size_of::<libc::vol_capabilities_attr_t>()];
        // SAFETY: the request and buffer outlive the call; the kernel writes at most
        // buf.len() bytes.
        let rc = unsafe {
            libc::getattrlist(
                cpath.as_ptr(),
                (&raw mut request).cast(),
                buf.as_mut_ptr().cast(),
                buf.len(),
                0,
            )
        };
        if rc != 0 {
            return false;
        }
        let caps: libc::vol_capabilities_attr_t = read(&buf, 4);
        let i = libc::VOL_CAPABILITIES_INTERFACES;
        caps.valid[i] & caps.capabilities[i] & libc::VOL_CAP_INT_CLONE != 0
    }

    pub(super) fn list(dir: &Path, fd: &File, ctx: &Ctx) -> io::Result<Vec<(OsString, Stat)>> {
        let mut request = libc::attrlist {
            bitmapcount: libc::ATTR_BIT_MAP_COUNT,
            reserved: 0,
            commonattr: libc::ATTR_CMN_RETURNED_ATTRS
                | libc::ATTR_CMN_NAME
                | libc::ATTR_CMN_DEVID
                | libc::ATTR_CMN_OBJTYPE
                | libc::ATTR_CMN_MODTIME
                | libc::ATTR_CMN_FLAGS
                | libc::ATTR_CMN_FILEID
                | ATTR_CMN_ERROR,
            volattr: 0,
            dirattr: libc::ATTR_DIR_MOUNTSTATUS,
            fileattr: libc::ATTR_FILE_LINKCOUNT
                | libc::ATTR_FILE_ALLOCSIZE
                | libc::ATTR_FILE_DATALENGTH,
            forkattr: if ctx.clones {
                libc::ATTR_CMNEXT_CLONEID | libc::ATTR_CMNEXT_EXT_FLAGS
            } else {
                0
            },
        };
        let options = if ctx.clones {
            libc::FSOPT_ATTR_CMN_EXTENDED as u64
        } else {
            0
        };
        let mut out = Vec::new();
        BUF.with_borrow_mut(|buf| {
            loop {
                if ctx.progress.stop.load(super::AtomicOrdering::Relaxed) {
                    return Ok(());
                }
                // SAFETY: the request and buffer outlive the call; the kernel writes at
                // most buf.len() bytes.
                let count = unsafe {
                    libc::getattrlistbulk(
                        fd.as_raw_fd(),
                        (&raw mut request).cast(),
                        buf.as_mut_ptr().cast(),
                        buf.len(),
                        options,
                    )
                };
                if count < 0 {
                    let error = io::Error::last_os_error();
                    if error.kind() == io::ErrorKind::Interrupted {
                        continue;
                    }
                    if out.is_empty() {
                        return Err(error);
                    }
                    ctx.record(dir, &error);
                    return Ok(());
                }
                if count == 0 {
                    return Ok(());
                }
                let mut at = 0;
                for _ in 0..count {
                    let length = read::<u32>(buf, at) as usize;
                    if let Some(entry) = parse(&buf[at..at + length], dir, ctx) {
                        ctx.observe(&entry.1);
                        out.push(entry);
                    }
                    at += length;
                }
            }
        })?;
        Ok(out)
    }

    /// One entry; `None` when the kernel reports an error for it (recorded) or it has no
    /// name.
    fn parse(entry: &[u8], dir: &Path, ctx: &Ctx) -> Option<(OsString, Stat)> {
        let returned: libc::attribute_set_t = read(entry, 4);
        let mut at = 4 + mem::size_of::<libc::attribute_set_t>();
        let common = returned.commonattr;
        let mut name = None;
        let mut stat = Stat {
            kind: Kind::Other,
            size: 0,
            physical: 0,
            blocks: Blocks::Own,
            mtime: 0,
            ino: 0,
            nlink: 1,
            dev: 0,
            mount_point: false,
            dataless: false,
        };
        if common & libc::ATTR_CMN_NAME != 0 {
            let reference: libc::attrreference_t = read(entry, at);
            let start = (at as isize + reference.attr_dataoffset as isize) as usize;
            // The length counts the terminating NUL.
            let bytes = &entry[start..start + reference.attr_length as usize - 1];
            name = Some(OsString::from_vec(bytes.to_vec()));
            at += mem::size_of::<libc::attrreference_t>();
        }
        if common & libc::ATTR_CMN_DEVID != 0 {
            stat.dev = read::<libc::dev_t>(entry, at) as u32 as u64;
            at += mem::size_of::<libc::dev_t>();
        }
        if common & libc::ATTR_CMN_OBJTYPE != 0 {
            stat.kind = match read::<u32>(entry, at) {
                VREG => Kind::File,
                VDIR => Kind::Dir,
                VLNK => Kind::Symlink,
                _ => Kind::Other,
            };
            at += mem::size_of::<u32>();
        }
        if common & libc::ATTR_CMN_MODTIME != 0 {
            stat.mtime = read::<libc::timespec>(entry, at).tv_sec;
            at += mem::size_of::<libc::timespec>();
        }
        if common & libc::ATTR_CMN_FLAGS != 0 {
            stat.dataless = read::<u32>(entry, at) & SF_DATALESS != 0;
            at += mem::size_of::<u32>();
        }
        if common & libc::ATTR_CMN_FILEID != 0 {
            stat.ino = read::<u64>(entry, at);
            at += mem::size_of::<u64>();
        }
        let name = name?;
        if common & ATTR_CMN_ERROR != 0 {
            let errno = read::<u32>(entry, at);
            at += mem::size_of::<u32>();
            if errno != 0 {
                ctx.record(
                    &dir.join(&name),
                    &io::Error::from_raw_os_error(errno as i32),
                );
                return None;
            }
        }
        if returned.dirattr & libc::ATTR_DIR_MOUNTSTATUS != 0 {
            stat.mount_point = read::<u32>(entry, at) & libc::DIR_MNTSTATUS_MNTPOINT != 0;
            at += mem::size_of::<u32>();
        }
        let file = returned.fileattr;
        if file & libc::ATTR_FILE_LINKCOUNT != 0 {
            stat.nlink = u64::from(read::<u32>(entry, at));
            at += mem::size_of::<u32>();
        }
        if file & libc::ATTR_FILE_ALLOCSIZE != 0 {
            stat.physical = read::<libc::off_t>(entry, at) as u64;
            at += mem::size_of::<libc::off_t>();
        }
        if file & libc::ATTR_FILE_DATALENGTH != 0 {
            stat.size = read::<libc::off_t>(entry, at) as u64;
            at += mem::size_of::<libc::off_t>();
        }
        let ext = returned.forkattr;
        let mut clone = 0;
        let mut flags = 0;
        if ext & libc::ATTR_CMNEXT_CLONEID != 0 {
            clone = read::<u64>(entry, at);
            at += mem::size_of::<u64>();
        }
        if ext & libc::ATTR_CMNEXT_EXT_FLAGS != 0 {
            flags = read::<u64>(entry, at);
        }
        stat.blocks = if flags & EF_SHARES_ALL_BLOCKS != 0 && clone != 0 {
            Blocks::All(clone)
        } else {
            Blocks::Own
        };
        Some((name, stat))
    }

    /// A value at byte `at`; attributes are only four-byte aligned.
    fn read<T: Copy>(buf: &[u8], at: usize) -> T {
        assert!(
            at + mem::size_of::<T>() <= buf.len(),
            "attribute past the entry"
        );
        // SAFETY: bounds checked above; read_unaligned allows any alignment, and every
        // T used here is plain old data.
        unsafe { ptr::read_unaligned(buf.as_ptr().add(at).cast()) }
    }
}

/// Largest first, ties by name.
fn by_size_then_name<T: Ord + ?Sized>(a: (u64, &T), b: (u64, &T)) -> Ordering {
    b.0.cmp(&a.0).then_with(|| a.1.cmp(b.1))
}

/// The files the flatten has counted, and what it left out of `physical`.
#[derive(Default)]
struct Seen {
    /// `(dev, inode)` of hard-linked files.
    links: HashSet<(u64, u64)>,
    /// `(dev, clone id)` of files that share all their data.
    clones: HashSet<(u64, u64)>,
    clone_counted_once: u64,
    raw_names: HashMap<NodeId, Box<OsStr>>,
}

fn flatten(local: Local, parent: Option<NodeId>, nodes: &mut Vec<Node>, seen: &mut Seen) -> NodeId {
    let Local {
        name,
        kind,
        mut size,
        mut physical,
        blocks,
        mut mtime,
        ino,
        nlink,
        dev,
        mut children,
        ..
    } = local;
    let id = nodes.len() as NodeId;
    let mut shared = Shared::None;
    if kind != Kind::Dir {
        if nlink > 1 && !seen.links.insert((dev, ino)) {
            (size, physical) = (0, 0);
            shared = Shared::HardLink;
        } else {
            if let Blocks::All(clone) = blocks {
                if !seen.clones.insert((dev, clone)) {
                    seen.clone_counted_once += physical;
                    physical = 0;
                }
                shared = Shared::Clone;
            }
        }
    }
    let name = match name.into_string() {
        Ok(name) => name,
        Err(raw) => {
            let name = raw.to_string_lossy().into_owned();
            seen.raw_names.insert(id, raw.into_boxed_os_str());
            name
        }
    };
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
        shared,
    });
    if kind != Kind::Dir {
        return id;
    }

    children.sort_by(|a, b| by_size_then_name((a.size, &a.name), (b.size, &b.name)));
    let mut ids = Vec::with_capacity(children.len());
    let (mut size, mut physical, mut files, mut dirs) = (0u64, 0u64, 0u32, 0u32);
    let mut resized = false;
    for child in children {
        let walked = child.size;
        let cid = flatten(child, Some(id), nodes, seen);
        let c = &nodes[cid as usize];
        resized |= c.size != walked;
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
    // Already in order unless a hard link took bytes from a child.
    if resized {
        ids.sort_by(|&a, &b| {
            let name = |id: NodeId| {
                seen.raw_names
                    .get(&id)
                    .map_or_else(|| OsStr::new(&nodes[id as usize].name), |raw| raw.as_ref())
            };
            by_size_then_name(
                (nodes[a as usize].size, name(a)),
                (nodes[b as usize].size, name(b)),
            )
        });
    }
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
    use std::fs;
    use std::os::unix::fs::symlink;

    struct Scratch(PathBuf);

    impl Scratch {
        fn new() -> Self {
            static NEXT: AtomicU64 = AtomicU64::new(0);
            let path = env::temp_dir().join(format!(
                "hoardmap-scan-unit-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, AtomicOrdering::Relaxed)
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn context<'a>(root: &'a Path, progress: &'a Progress, method: Method) -> Ctx<'a> {
        Ctx {
            method,
            clones: false,
            root_dev: fs::metadata(root).unwrap().dev(),
            progress,
            errors: Mutex::new(Vec::new()),
            error_count: AtomicU64::new(0),
            skip: Vec::new(),
            root,
            listing_slots: std::array::from_fn(|_| Mutex::new(())),
        }
    }

    #[test]
    fn changed_child_is_skipped_and_recorded() {
        for method in [
            Method::Portable,
            #[cfg(target_os = "macos")]
            Method::Bulk,
        ] {
            for replacement in [
                "symlink",
                "same-inode symlink",
                "directory",
                "file",
                "missing",
            ] {
                let scratch = Scratch::new();
                let root = scratch.0.join("scan");
                let child = root.join("child");
                let original = scratch.0.join("original");
                let outside = scratch.0.join("outside");
                fs::create_dir_all(&child).unwrap();
                fs::create_dir(&outside).unwrap();
                fs::write(child.join("original-file"), b"original").unwrap();
                fs::write(outside.join("outside-file"), b"outside").unwrap();
                let progress = Progress::default();
                let ctx = context(&root, &progress, method);
                let listing = list(&root, File::open(&root).unwrap(), &ctx).unwrap();

                // The race occurs precisely between the parent's listing and recursion.
                fs::rename(&child, &original).unwrap();
                match replacement {
                    "symlink" => symlink(&outside, &child).unwrap(),
                    "same-inode symlink" => symlink(&original, &child).unwrap(),
                    "file" => fs::write(&child, b"replacement").unwrap(),
                    "missing" => (),
                    _ => {
                        fs::create_dir(&child).unwrap();
                        fs::write(child.join("replacement-file"), b"replacement").unwrap();
                    }
                }
                let children = build(&root, listing, &ctx, 0);
                assert_eq!(children.len(), 1, "{method:?}, {replacement}");
                assert!(children[0].children.is_empty(), "{method:?}, {replacement}");
                assert_eq!(progress.files.load(AtomicOrdering::Relaxed), 0);
                assert_eq!(ctx.error_count.load(AtomicOrdering::Relaxed), 1);
                let errors = ctx.errors.lock().unwrap();
                assert_eq!(errors[0].path, child.to_string_lossy());
                assert_eq!(errors[0].error, DIRECTORY_CHANGED);
            }
        }
    }

    #[test]
    fn a_replaced_ancestor_is_skipped_when_children_are_reopened() {
        for method in [
            Method::Portable,
            #[cfg(target_os = "macos")]
            Method::Bulk,
        ] {
            let scratch = Scratch::new();
            let root = scratch.0.join("scan");
            let outside = scratch.0.join("outside");
            fs::create_dir_all(root.join("child")).unwrap();
            fs::create_dir_all(outside.join("child")).unwrap();
            fs::write(root.join("child/original"), b"original").unwrap();
            fs::write(outside.join("child/replacement"), b"replacement").unwrap();
            let progress = Progress::default();
            let ctx = context(&root, &progress, method);
            let listing = list(&root, File::open(&root).unwrap(), &ctx).unwrap();
            fs::rename(&root, scratch.0.join("moved")).unwrap();
            symlink(&outside, &root).unwrap();

            let children = build(&root, listing, &ctx, 0);
            assert_eq!(children.len(), 1, "{method:?}");
            assert!(children[0].children.is_empty(), "{method:?}");
            assert_eq!(ctx.error_count.load(AtomicOrdering::Relaxed), 1);
            let errors = ctx.errors.lock().unwrap();
            assert_eq!(errors[0].path, root.join("child").to_string_lossy());
            assert_eq!(errors[0].error, DIRECTORY_CHANGED);
        }
    }

    #[test]
    fn flatten_preserves_raw_names_and_orders_display_name_collisions() {
        let scratch = Scratch::new();
        let dir_stat = Stat::of(&fs::metadata(&scratch.0).unwrap());
        let file = scratch.0.join("file");
        fs::write(&file, b"data").unwrap();
        let file_stat = Stat::of(&fs::metadata(file).unwrap());
        let root_name = OsString::from_vec(b"/scan/\xff".to_vec());
        let first = OsString::from_vec(b"dir\xfe".to_vec());
        let second = OsString::from_vec(b"dir\xff".to_vec());
        let local = Local::dir(
            root_name.clone(),
            &dir_stat,
            [second.clone(), first.clone()]
                .into_iter()
                .map(|name| {
                    Local::dir(
                        name,
                        &dir_stat,
                        vec![Local::leaf("file".into(), &file_stat)],
                    )
                })
                .collect(),
        );
        let mut nodes = Vec::new();
        let mut seen = Seen::default();
        flatten(local, None, &mut nodes, &mut seen);
        let tree = Tree {
            nodes,
            raw_names: seen.raw_names,
            errors: Vec::new(),
            error_count: 0,
            clone_aware: false,
            clone_counted_once: 0,
        };
        assert_eq!(tree.raw_names.len(), 3);
        assert_eq!(tree.nodes[1].name, tree.nodes[3].name);
        assert_eq!(tree.root().files, 2);
        assert_eq!(tree.path(ROOT), PathBuf::from(&root_name));
        assert_eq!(
            tree.path(2),
            PathBuf::from(&root_name).join(first).join("file")
        );
        assert_eq!(
            tree.path(4),
            PathBuf::from(root_name).join(second).join("file")
        );
    }

    #[test]
    fn previews_keep_raw_paths_and_colliding_names_after_stop() {
        let scratch = Scratch::new();
        let file = scratch.0.join("file");
        fs::write(&file, b"data").unwrap();
        let dir_stat = Stat::of(&fs::metadata(&scratch.0).unwrap());
        let file_stat = Stat::of(&fs::metadata(file).unwrap());
        let root = scratch.0.join(OsString::from_vec(b"root\xff".to_vec()));
        let locals: Vec<_> = (*b"\xfe\xff")
            .into_iter()
            .map(|byte| {
                Local::dir(
                    OsString::from_vec(vec![b'd', byte]),
                    &dir_stat,
                    vec![Local::leaf(
                        OsString::from_vec(vec![b'f', byte]),
                        &file_stat,
                    )],
                )
            })
            .collect();
        let progress = Progress::default();
        let mut ctx = context(&scratch.0, &progress, Method::Portable);
        ctx.root = &root;
        ctx.finish(&root, &[]);
        let empty = partial(&root.to_string_lossy(), &progress.finished.lock().unwrap());
        assert_eq!(empty.path(ROOT), root);
        assert_eq!(empty.nodes.len(), 1);
        ctx.finish(&root.join(&locals[0].name), &[&locals[0].children[0]]);
        let early = partial(&root.to_string_lossy(), &progress.finished.lock().unwrap());
        ctx.finish(&root.join(&locals[1].name), &[&locals[1].children[0]]);
        ctx.finish(&root, &locals.iter().collect::<Vec<_>>());
        progress.stop.store(true, AtomicOrdering::Relaxed);
        let stopped = partial(&root.to_string_lossy(), &progress.finished.lock().unwrap());
        assert_eq!(
            (
                stopped.root().dirs,
                stopped.root().files,
                stopped.root().size
            ),
            (2, 2, 8)
        );
        assert_eq!(stopped.path(ROOT), root);
        assert_eq!(stopped.raw_names.len(), 5);
        for (index, local) in locals.iter().enumerate() {
            let id = 2 + 2 * index as NodeId;
            assert_eq!(
                stopped.path(id),
                root.join(&local.name).join(&local.children[0].name)
            );
        }
        assert_eq!(early.path(2), stopped.path(2));
        assert!(partial("/ordinary", &[]).raw_names.is_empty());
    }

    #[test]
    fn order_is_size_descending_then_name() {
        let mut v = vec![(5, "b"), (10, "z"), (5, "a"), (0, "x")];
        v.sort_by(|a, b| by_size_then_name(*a, *b));
        assert_eq!(v, vec![(10, "z"), (5, "a"), (5, "b"), (0, "x")]);
    }

    #[test]
    fn threads_are_the_cores_above_the_efficiency_level() {
        // M6: 2 Super, 4 Performance, 6 Efficiency.
        assert_eq!(pick_threads(&[2, 4, 6], Some(12), 12), 6);
        // M1: 4 Performance, 4 Efficiency.
        assert_eq!(pick_threads(&[4, 4], Some(8), 8), 4);
    }

    #[test]
    fn threads_fall_back_to_physical_then_logical_cores() {
        // Intel: one level, hyperthreaded.
        assert_eq!(pick_threads(&[8], Some(8), 16), 8);
        assert_eq!(pick_threads(&[], Some(8), 16), 8);
        assert_eq!(pick_threads(&[], None, 4), 4);
    }

    #[test]
    fn threads_stay_between_one_and_the_logical_count() {
        assert_eq!(pick_threads(&[16, 4], None, 8), 8);
        assert_eq!(pick_threads(&[0, 4], None, 8), 1);
        assert_eq!(pick_threads(&[], None, 0), 1);
    }

    #[test]
    fn progress_snapshot_reads_all_four() {
        let p = Progress::default();
        p.dirs.fetch_add(1, AtomicOrdering::Relaxed);
        p.files.fetch_add(2, AtomicOrdering::Relaxed);
        p.bytes.fetch_add(3, AtomicOrdering::Relaxed);
        p.physical.fetch_add(4, AtomicOrdering::Relaxed);
        assert_eq!(p.snapshot(), (1, 2, 3, 4));
    }
}
