//! The disks a scan can start from: the startup disk and the drives mounted under
//! `/Volumes`, with their size and free space, and the macOS folder picker.

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
    use std::sync::Mutex;

    // getmntinfo fills one buffer shared by the whole process; requests come in on
    // several threads, so one at a time reads it.
    static GETMNTINFO: Mutex<()> = Mutex::new(());
    let _guard = GETMNTINFO.lock().unwrap_or_else(|e| e.into_inner());
    let mut mounts: *mut libc::statfs = std::ptr::null_mut();
    // SAFETY: getmntinfo points `mounts` at its buffer and returns its length.
    let count = unsafe { libc::getmntinfo(&mut mounts, libc::MNT_NOWAIT) };
    if count <= 0 {
        return Vec::new();
    }
    // SAFETY: `count` entries, valid until the next getmntinfo call, which the lock
    // held until the end of this function keeps out.
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
    std::fs::read_dir("/Volumes")
        .into_iter()
        .flatten()
        .flatten()
        .find(|e| std::fs::read_link(e.path()).is_ok_and(|t| t == Path::new("/")))
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .unwrap_or_else(|| "Startup disk".into())
}

/// A Time Machine backup disk, whose hard links or snapshots skew scan totals.
#[cfg(any(target_os = "macos", test))]
fn is_backup(root: &Path) -> bool {
    // tmutil(8), BACKUP STRUCTURE: HFS+ has Backups.backupdb at the volume root.
    // APFS has a backup_manifest.plist file at its root, as observed in:
    // https://eclecticlight.co/2021/03/22/time-machine-to-apfs-backup-structure-and-access/
    std::fs::symlink_metadata(root.join("Backups.backupdb")).is_ok_and(|m| m.is_dir())
        || std::fs::symlink_metadata(root.join("backup_manifest.plist")).is_ok_and(|m| m.is_file())
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
    // The picker ends folder paths with a slash; the scan names its root without one.
    let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let path = match path.trim_end_matches('/') {
        "" if path.is_empty() => return None,
        "" => "/".to_string(),
        trimmed => trimmed.to_string(),
    };
    Some(PathBuf::from(path))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static NEXT: AtomicUsize = AtomicUsize::new(0);

    struct Disk(PathBuf);

    impl Disk {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "hoardmap-volume-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&root).unwrap();
            Self(root)
        }
    }

    impl Drop for Disk {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }

    #[test]
    fn suffixes_do_not_make_an_ordinary_drive_a_backup_disk() {
        let disk = Disk::new();
        assert!(!is_backup(&disk.0));
        for name in [
            "project.previous",
            "project.inprogress",
            "2026-10-30-123456.previous",
            "2026-10-30-123456.inprogress",
        ] {
            fs::create_dir(disk.0.join(name)).unwrap();
            assert!(!is_backup(&disk.0), "{name}");
        }
    }

    #[test]
    fn hfs_backup_store_is_recognised() {
        let disk = Disk::new();
        fs::create_dir(disk.0.join("Backups.backupdb")).unwrap();
        assert!(is_backup(&disk.0));
    }

    #[test]
    fn apfs_backup_manifest_is_recognised_without_a_mounted_snapshot() {
        let disk = Disk::new();
        fs::write(disk.0.join("backup_manifest.plist"), b"").unwrap();
        assert!(is_backup(&disk.0));
    }

    #[test]
    fn backup_markers_require_their_own_kind_and_not_a_symlink() {
        let disk = Disk::new();
        let wrong = disk.0.join("wrong");
        fs::create_dir(&wrong).unwrap();
        fs::write(wrong.join("Backups.backupdb"), b"").unwrap();
        fs::create_dir(wrong.join("backup_manifest.plist")).unwrap();
        assert!(!is_backup(&wrong));

        let links = disk.0.join("links");
        fs::create_dir(&links).unwrap();
        std::os::unix::fs::symlink(&wrong, links.join("Backups.backupdb")).unwrap();
        std::os::unix::fs::symlink(
            wrong.join("Backups.backupdb"),
            links.join("backup_manifest.plist"),
        )
        .unwrap();
        assert!(!is_backup(&links));
        assert!(!is_backup(&disk.0.join("missing")));
    }
}
