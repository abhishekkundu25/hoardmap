//! The disks a scan can start from: the startup disk and the drives mounted under
//! `/Volumes`, with their size and free space, and the macOS folder picker.

use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct Volume {
    pub name: String,
    pub path: String,
    pub total: u64,
    pub free: u64,
    /// `startup`, `drive` (local, mounted under /Volumes) or `network`.
    pub kind: &'static str,
}

/// The startup disk first, then the other drives by name. System volumes, hidden
/// mounts and Time Machine backup disks are left out.
#[cfg(target_os = "macos")]
pub fn list() -> Vec<Volume> {
    use std::ffi::CStr;

    let mut mounts: *mut libc::statfs = std::ptr::null_mut();
    // SAFETY: getmntinfo points `mounts` at a buffer it owns and returns its length.
    let count = unsafe { libc::getmntinfo(&mut mounts, libc::MNT_NOWAIT) };
    if count <= 0 {
        return Vec::new();
    }
    // SAFETY: `count` entries, valid until the next getmntinfo call on this thread.
    let mounts = unsafe { std::slice::from_raw_parts(mounts, count as usize) };
    let mut out = Vec::new();
    for mount in mounts {
        // SAFETY: the kernel NUL-terminates the mount path.
        let on = unsafe { CStr::from_ptr(mount.f_mntonname.as_ptr()) };
        let path = PathBuf::from(on.to_string_lossy().into_owned());
        let flags = mount.f_flags as i32;
        let local = flags & libc::MNT_LOCAL != 0;
        let kind = if path == Path::new("/") {
            "startup"
        } else if flags & libc::MNT_DONTBROWSE != 0 || !path.starts_with("/Volumes") {
            continue;
        } else if !local {
            "network"
        } else {
            "drive"
        };
        let name = match path.file_name() {
            Some(name) => name.to_string_lossy().into_owned(),
            None => startup_name(),
        };
        // Reading a network drive's root could hang, so only local ones are checked.
        if name.starts_with('.') || (local && kind == "drive" && is_backup(&path)) {
            continue;
        }
        let block = u64::from(mount.f_bsize);
        out.push(Volume {
            name,
            path: path.to_string_lossy().into_owned(),
            total: mount.f_blocks * block,
            free: mount.f_bavail * block,
            kind,
        });
    }
    out.sort_by(|a, b| (a.kind != "startup", &a.name).cmp(&(b.kind != "startup", &b.name)));
    out
}

/// Off macOS: the root file system alone.
#[cfg(not(target_os = "macos"))]
pub fn list() -> Vec<Volume> {
    let Some((total, free)) = crate::server::disk(Path::new("/")) else {
        return Vec::new();
    };
    vec![Volume {
        name: "/".into(),
        path: "/".into(),
        total,
        free,
        kind: "startup",
    }]
}

/// The startup disk's name, such as "Macintosh HD": the /Volumes entry linking to /.
#[cfg(target_os = "macos")]
fn startup_name() -> String {
    fs::read_dir("/Volumes")
        .into_iter()
        .flatten()
        .flatten()
        .find(|e| fs::read_link(e.path()).is_ok_and(|t| t == Path::new("/")))
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .unwrap_or_else(|| "Startup disk".into())
}

/// A Time Machine backup disk: millions of hard links whose sizes mean little.
#[cfg(target_os = "macos")]
fn is_backup(root: &Path) -> bool {
    let Ok(entries) = fs::read_dir(root) else {
        return false;
    };
    entries.flatten().any(|e| {
        let name = e.file_name();
        let name = name.to_string_lossy();
        name == "Backups.backupdb" || name.ends_with(".previous") || name.ends_with(".inprogress")
    })
}

/// Shows the macOS folder picker and returns the chosen folder; `None` when it was
/// cancelled or cannot be shown.
pub fn choose_folder() -> Option<PathBuf> {
    let output = std::process::Command::new("osascript")
        .args([
            "-e",
            "POSIX path of (choose folder with prompt \"Choose a folder to scan\")",
        ])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
    (!path.is_empty()).then(|| PathBuf::from(path))
}
