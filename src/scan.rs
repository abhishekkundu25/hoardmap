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
//! portable way is `read_dir` plus one `lstat` per entry. Both give the same tree; set
//! `HOARDMAP_SCANNER=portable` to use the portable way on a Mac.
//!
//! Workers build [`Local`] subtrees with rayon, one task per subdirectory. The flatten
//! is sequential and depth-first: siblings are visited largest first by their pre-dedup
//! size (ties by name), which fixes the arena order and thereby which occurrence of a
//! hard-linked file keeps its bytes. Directory totals are then recomputed from the
//! deduplicated children, and each `children` list is sorted by that final size.
//!
//! [`Progress::bytes`] and [`Progress::physical`] count every entry's logical and
//! allocated size as it is seen, so a file with several hard links is counted once per
//! link there; the finished tree counts it once.

use std::cmp::Ordering;
use std::collections::HashSet;
use std::fs::{self, Metadata};
use std::io;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};
#[cfg(target_os = "macos")]
use std::{env, mem};

use rayon::prelude::*;

use crate::access;
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
    /// Bytes allocated on disk for the non-directory entries seen.
    pub physical: AtomicU64,
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
    fn leaf(name: String, stat: &Stat) -> Local {
        Local {
            name,
            kind: stat.kind,
            size: stat.size,
            physical: stat.physical,
            mtime: stat.mtime,
            ino: stat.ino,
            nlink: stat.nlink,
            dev: stat.dev,
            children: Vec::new(),
        }
    }

    fn dir(name: String, stat: &Stat, children: Vec<Local>) -> Local {
        Local {
            name,
            kind: Kind::Dir,
            size: children.iter().map(|c| c.size).sum(),
            physical: 0,
            mtime: stat.mtime,
            ino: stat.ino,
            nlink: stat.nlink,
            dev: stat.dev,
            children,
        }
    }
}

/// What the walk needs about one entry, from either listing method.
struct Stat {
    kind: Kind,
    size: u64,
    physical: u64,
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

/// `SF_DATALESS` from <sys/stat.h>: the item's contents are in the cloud, not on disk.
#[cfg(target_os = "macos")]
const SF_DATALESS: u32 = 0x4000_0000;

impl Stat {
    fn of(meta: &Metadata) -> Stat {
        let ft = meta.file_type();
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
            physical: meta.blocks() * 512,
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
    /// `read_dir` and one `lstat` per entry; every Unix.
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
    root_dev: u64,
    progress: &'a Progress,
    errors: Mutex<Vec<ScanError>>,
    error_count: AtomicU64,
    /// Folders below the root that are counted but not read.
    skip: Vec<PathBuf>,
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
    let meta = fs::metadata(path)?;
    if !meta.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::NotADirectory,
            format!("{} is not a directory", path.display()),
        ));
    }
    #[cfg(target_os = "macos")]
    never_download();
    let ctx = Ctx {
        method,
        root_dev: meta.dev(),
        progress,
        errors: Mutex::new(Vec::new()),
        error_count: AtomicU64::new(0),
        skip,
    };
    let entries = list(path, &ctx)?;
    let children = build(path, entries, &ctx);
    let root = Local::dir(
        path.to_string_lossy().into_owned(),
        &Stat::of(&meta),
        children,
    );

    let (dirs, files, ..) = progress.snapshot();
    let mut nodes = Vec::with_capacity((dirs + files + 1) as usize);
    let mut seen = HashSet::new();
    flatten(root, None, &mut nodes, &mut seen);

    Ok(Tree {
        nodes,
        errors: ctx.errors.into_inner().unwrap(),
        error_count: ctx.error_count.load(AtomicOrdering::Relaxed),
    })
}

/// The entries of `dir`. `Err` when the folder cannot be opened; a single entry that
/// cannot be read is recorded and left out.
fn list(dir: &Path, ctx: &Ctx) -> io::Result<Vec<(String, Stat)>> {
    match ctx.method {
        Method::Portable => list_portable(dir, ctx),
        #[cfg(target_os = "macos")]
        Method::Bulk => bulk::list(dir, ctx),
    }
}

fn list_portable(dir: &Path, ctx: &Ctx) -> io::Result<Vec<(String, Stat)>> {
    let mut out = Vec::new();
    for entry in fs::read_dir(dir)? {
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
        out.push((name, Stat::of(&meta)));
    }
    Ok(out)
}

fn read_children(dir: &Path, ctx: &Ctx) -> Vec<Local> {
    match list(dir, ctx) {
        Ok(entries) => build(dir, entries, ctx),
        Err(e) => {
            ctx.record(dir, &e);
            Vec::new()
        }
    }
}

/// Counts `entries`, then reads the subfolders on the same volume in parallel.
fn build(dir: &Path, entries: Vec<(String, Stat)>, ctx: &Ctx) -> Vec<Local> {
    let mut children = Vec::with_capacity(entries.len());
    let mut subdirs: Vec<(String, Stat)> = Vec::new();
    for (name, stat) in entries {
        if stat.kind == Kind::Dir {
            ctx.progress.dirs.fetch_add(1, AtomicOrdering::Relaxed);
            if stat.dev == ctx.root_dev && !stat.mount_point && !stat.dataless {
                subdirs.push((name, stat));
            } else {
                children.push(Local::dir(name, &stat, Vec::new()));
            }
        } else {
            ctx.progress.files.fetch_add(1, AtomicOrdering::Relaxed);
            ctx.progress
                .bytes
                .fetch_add(stat.size, AtomicOrdering::Relaxed);
            ctx.progress
                .physical
                .fetch_add(stat.physical, AtomicOrdering::Relaxed);
            children.push(Local::leaf(name, &stat));
        }
    }
    let descend = |(name, stat): (String, Stat)| {
        let path = dir.join(&name);
        if ctx.skip.contains(&path) {
            ctx.record(&path, &io::Error::other(NOT_SCANNED));
            return Local::dir(name, &stat, Vec::new());
        }
        let grandchildren = read_children(&path, ctx);
        Local::dir(name, &stat, grandchildren)
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
/// order: common ones by bit, then directory ones, then file ones.
#[cfg(target_os = "macos")]
mod bulk {
    use std::cell::RefCell;
    use std::ffi::CString;
    use std::io;
    use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
    use std::os::unix::ffi::OsStrExt;
    use std::path::Path;
    use std::ptr;

    use super::{Ctx, Kind, SF_DATALESS, Stat, mem};

    /// Not in the libc crate; from <sys/attr.h>.
    const ATTR_CMN_ERROR: u32 = 0x2000_0000;
    /// `enum vtype` from <sys/vnode.h>.
    const VREG: u32 = 1;
    const VDIR: u32 = 2;
    const VLNK: u32 = 5;
    const BUFFER: usize = 256 * 1024;

    thread_local! {
        static BUF: RefCell<Vec<u8>> = RefCell::new(vec![0; BUFFER]);
    }

    pub(super) fn list(dir: &Path, ctx: &Ctx) -> io::Result<Vec<(String, Stat)>> {
        let cpath = CString::new(dir.as_os_str().as_bytes())?;
        // SAFETY: a valid C string; the descriptor is owned and closed on drop.
        let fd = unsafe {
            libc::open(
                cpath.as_ptr(),
                libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC,
            )
        };
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: open returned a fresh descriptor that nothing else owns.
        let fd = unsafe { OwnedFd::from_raw_fd(fd) };
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
            forkattr: 0,
        };
        let mut out = Vec::new();
        BUF.with_borrow_mut(|buf| {
            loop {
                // SAFETY: the request and buffer outlive the call; the kernel writes at
                // most buf.len() bytes.
                let count = unsafe {
                    libc::getattrlistbulk(
                        fd.as_raw_fd(),
                        (&raw mut request).cast(),
                        buf.as_mut_ptr().cast(),
                        buf.len(),
                        0,
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
    fn parse(entry: &[u8], dir: &Path, ctx: &Ctx) -> Option<(String, Stat)> {
        let returned: libc::attribute_set_t = read(entry, 4);
        let mut at = 4 + mem::size_of::<libc::attribute_set_t>();
        let common = returned.commonattr;
        let mut name = None;
        let mut stat = Stat {
            kind: Kind::Other,
            size: 0,
            physical: 0,
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
            name = Some(String::from_utf8_lossy(bytes).into_owned());
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
        }
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
    fn progress_snapshot_reads_all_four() {
        let p = Progress::default();
        p.dirs.fetch_add(1, AtomicOrdering::Relaxed);
        p.files.fetch_add(2, AtomicOrdering::Relaxed);
        p.bytes.fetch_add(3, AtomicOrdering::Relaxed);
        p.physical.fetch_add(4, AtomicOrdering::Relaxed);
        assert_eq!(p.snapshot(), (1, 2, 3, 4));
    }
}
