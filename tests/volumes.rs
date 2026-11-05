//! The Time Machine check behind the Disks menu: a drive is hidden only when it has
//! Time Machine's own markers.

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use hoardmap::volumes::is_backup;

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
