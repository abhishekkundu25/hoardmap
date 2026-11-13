//! The disk a scanned folder is on: its size and free space and, on an APFS disk, how
//! the used space divides among the volumes sharing it. An APFS disk is one pool of
//! space; besides the volume the user sees, a startup disk carries macOS's own
//! volumes and the one that holds swap, and a folder scan reaches none of those.
//!
//! [`usage`] asks the system for figures it already keeps: one `statfs`, the mount
//! list, and one `getattrlist` per volume on the same disk. No folder is read.

use std::path::Path;

/// Where the startup disk's data volume is mounted.
#[cfg(any(target_os = "macos", test))]
const DATA: &str = "/System/Volumes/Data";

/// Where the volume that holds swap is mounted.
#[cfg(any(target_os = "macos", test))]
const VM: &str = "/System/Volumes/VM";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Usage {
    pub total: u64,
    /// What an ordinary user may still write.
    pub free: u64,
    /// The volume's name, such as "Macintosh HD".
    pub name: Option<String>,
    /// The folder is the volume's top folder, so a scan reads all of it.
    pub whole: bool,
    /// `None` where the disk is not APFS, or not a Mac's.
    pub shares: Option<Shares>,
}

/// Bytes in use on an APFS disk, by volume.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Shares {
    /// The volume the folder is on. For a folder on the startup disk's system volume,
    /// `/` included, this is the system and data volumes together, as a scan of `/`
    /// reads both.
    pub volume: u64,
    /// macOS's own volumes: the system volume where `volume` leaves it out, and the
    /// hidden ones such as Preboot and Recovery.
    pub system: u64,
    /// The volume that holds swap.
    pub swap: u64,
    /// The disk's other volumes that hold the user's files.
    pub volumes: u64,
}

/// One mounted volume of the disk.
#[cfg(any(target_os = "macos", test))]
#[derive(Clone, Debug)]
struct Mounted {
    on: String,
    /// Kept out of Finder: macOS's own.
    hidden: bool,
    used: u64,
}

/// `(total, free)` bytes of the volume holding `path`; free is what an ordinary user
/// may still write.
pub fn space(path: &Path) -> Option<(u64, u64)> {
    let cpath = std::ffi::CString::new(path.as_os_str().as_encoded_bytes()).ok()?;
    let mut stat: libc::statvfs = unsafe { std::mem::zeroed() };
    // SAFETY: a valid C string and a statvfs the call fills in.
    if unsafe { libc::statvfs(cpath.as_ptr(), &mut stat) } != 0 {
        return None;
    }
    let block = stat.f_frsize as u64;
    Some((stat.f_blocks as u64 * block, stat.f_bavail as u64 * block))
}

/// The disk holding `path`. Reading a network drive that stopped answering blocks, so
/// call it off any lock.
pub fn usage(path: &Path) -> Option<Usage> {
    let (total, free) = space(path)?;
    #[cfg(target_os = "macos")]
    let (name, whole, shares) = mac::describe(path);
    #[cfg(not(target_os = "macos"))]
    let (name, whole, shares) = (None, top_folder(path), None);
    Some(Usage {
        total,
        free,
        name,
        whole,
        shares,
    })
}

/// Whether `path` is where its file system is mounted: the folder above it is on
/// another one, or there is none.
#[cfg(not(target_os = "macos"))]
fn top_folder(path: &Path) -> bool {
    use std::os::unix::fs::MetadataExt;
    let dev = |p: &Path| std::fs::metadata(p).map(|m| m.dev()).ok();
    match path.parent() {
        Some(parent) => dev(path) != dev(parent),
        None => true,
    }
}

/// Divides the disk's volumes around the one mounted at `own`.
#[cfg(any(target_os = "macos", test))]
fn shares(own: &str, mounted: &[Mounted]) -> Shares {
    let mut shares = Shares::default();
    for volume in mounted {
        let on = volume.on.as_str();
        let part = if on == own || (own == "/" && on == DATA) {
            &mut shares.volume
        } else if on == VM {
            &mut shares.swap
        } else if on != DATA && (volume.hidden || on == "/") {
            &mut shares.system
        } else {
            &mut shares.volumes
        };
        *part += volume.used;
    }
    shares
}

/// The APFS disk a device belongs to: `disk3` for `/dev/disk3s5` and for the
/// snapshot `/dev/disk3s1s1`.
#[cfg(any(target_os = "macos", test))]
fn container(device: &str) -> Option<&str> {
    let name = device.strip_prefix("/dev/")?;
    let digits = name.strip_prefix("disk")?;
    let len = digits.bytes().take_while(u8::is_ascii_digit).count();
    (len > 0).then(|| &name[.."disk".len() + len])
}

#[cfg(target_os = "macos")]
mod mac {
    use std::ffi::{CStr, CString, c_char, c_void};
    use std::path::Path;

    use super::{DATA, Mounted, Shares, container, shares};

    fn text(field: &[c_char]) -> String {
        // SAFETY: the kernel NUL-terminates these fields.
        unsafe { CStr::from_ptr(field.as_ptr()) }
            .to_string_lossy()
            .into_owned()
    }

    fn statfs(path: &Path) -> Option<libc::statfs> {
        let cpath = CString::new(path.as_os_str().as_encoded_bytes()).ok()?;
        let mut stat: libc::statfs = unsafe { std::mem::zeroed() };
        // SAFETY: a valid C string and a statfs the call fills in.
        (unsafe { libc::statfs(cpath.as_ptr(), &mut stat) } == 0).then_some(stat)
    }

    /// Every mount, from the kernel's own list: nothing is asked of the volumes.
    fn mounts() -> Vec<libc::statfs> {
        // SAFETY: a null buffer asks only for the count.
        let count = unsafe { libc::getfsstat(std::ptr::null_mut(), 0, libc::MNT_NOWAIT) };
        // Room for volumes mounted between the two calls.
        let room = usize::try_from(count).unwrap_or(0) + 8;
        let mut list: Vec<libc::statfs> = Vec::with_capacity(room);
        let bytes = room * std::mem::size_of::<libc::statfs>();
        // SAFETY: the buffer holds `room` entries; the call says how many it wrote.
        let filled = unsafe { libc::getfsstat(list.as_mut_ptr(), bytes as i32, libc::MNT_NOWAIT) };
        // SAFETY: the kernel wrote `filled` entries, at most `room`.
        unsafe { list.set_len(usize::try_from(filled).unwrap_or(0).min(room)) };
        list
    }

    /// Bytes in use on the volume mounted at `on`, which shares its free space with
    /// the rest of the disk, so `statfs` cannot tell.
    fn used(on: &str) -> Option<u64> {
        #[repr(C, packed)]
        struct Answer {
            length: u32,
            used: i64,
        }
        let cpath = CString::new(on).ok()?;
        let mut list: libc::attrlist = unsafe { std::mem::zeroed() };
        list.bitmapcount = libc::ATTR_BIT_MAP_COUNT;
        list.volattr = libc::ATTR_VOL_INFO | libc::ATTR_VOL_SPACEUSED;
        let mut answer = Answer { length: 0, used: 0 };
        // SAFETY: a valid C string, an attribute list, and a buffer of the size given.
        let status = unsafe {
            libc::getattrlist(
                cpath.as_ptr(),
                (&raw mut list).cast::<c_void>(),
                (&raw mut answer).cast::<c_void>(),
                std::mem::size_of::<Answer>(),
                0,
            )
        };
        let Answer { length, used } = answer;
        (status == 0 && length as usize == std::mem::size_of::<Answer>())
            .then(|| u64::try_from(used).ok())
            .flatten()
    }

    /// The name of the volume mounted at `on`, such as "Macintosh HD".
    fn volume_name(on: &str) -> Option<String> {
        /// The answer's length, then where the name is and how long, then the name.
        #[repr(C)]
        struct Answer {
            length: u32,
            name: libc::attrreference_t,
            text: [u8; 1024],
        }
        let cpath = CString::new(on).ok()?;
        let mut list: libc::attrlist = unsafe { std::mem::zeroed() };
        list.bitmapcount = libc::ATTR_BIT_MAP_COUNT;
        list.volattr = libc::ATTR_VOL_INFO | libc::ATTR_VOL_NAME;
        let mut answer: Answer = unsafe { std::mem::zeroed() };
        // SAFETY: a valid C string, an attribute list, and a buffer of the size given.
        let status = unsafe {
            libc::getattrlist(
                cpath.as_ptr(),
                (&raw mut list).cast::<c_void>(),
                (&raw mut answer).cast::<c_void>(),
                std::mem::size_of::<Answer>(),
                0,
            )
        };
        if status != 0 {
            return None;
        }
        // The name starts `attr_dataoffset` bytes after its reference and ends in a NUL.
        let start = usize::try_from(answer.name.attr_dataoffset)
            .ok()?
            .checked_sub(std::mem::size_of::<libc::attrreference_t>())?;
        let len = (answer.name.attr_length as usize).checked_sub(1)?;
        let bytes = answer.text.get(start..start.checked_add(len)?)?;
        let name = String::from_utf8_lossy(bytes).into_owned();
        (!name.is_empty()).then_some(name)
    }

    /// The name of the volume holding `path`, whether `path` is its top folder, and
    /// how its disk divides when it is a local APFS one.
    pub fn describe(path: &Path) -> (Option<String>, bool, Option<Shares>) {
        let Some(stat) = statfs(path) else {
            return (None, false, None);
        };
        let own = text(&stat.f_mntonname);
        let whole = path == Path::new(&own);
        // The data volume is named "Data"; the startup disk's name is the system
        // volume's. Any other volume is mounted under its name.
        let name = if own == "/" || own == DATA {
            volume_name("/")
        } else {
            Path::new(&own)
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
        };
        let local = stat.f_flags & libc::MNT_LOCAL as u32 != 0;
        if !local || text(&stat.f_fstypename) != "apfs" {
            return (name, whole, None);
        }
        let device = text(&stat.f_mntfromname);
        let Some(disk) = container(&device) else {
            return (name, whole, None);
        };
        let mounted: Vec<Mounted> = mounts()
            .iter()
            .filter(|m| text(&m.f_fstypename) == "apfs")
            .filter(|m| container(&text(&m.f_mntfromname)) == Some(disk))
            .filter_map(|m| {
                let on = text(&m.f_mntonname);
                Some(Mounted {
                    used: used(&on)?,
                    hidden: m.f_flags & libc::MNT_DONTBROWSE as u32 != 0,
                    on,
                })
            })
            .collect();
        // Without the folder's own volume there is nothing to measure the scan against.
        if !mounted.iter().any(|m| m.on == own) {
            return (name, whole, None);
        }
        (name, whole, Some(shares(&own, &mounted)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn startup() -> Vec<Mounted> {
        [
            ("/", true, 14),
            (VM, true, 9),
            ("/System/Volumes/Preboot", true, 12),
            ("/System/Volumes/Update", true, 0),
            (DATA, true, 574),
            ("/Volumes/Recovery", true, 4),
            ("/Volumes/Work", false, 30),
        ]
        .into_iter()
        .map(|(on, hidden, used)| Mounted {
            on: on.into(),
            hidden,
            used,
        })
        .collect()
    }

    #[test]
    fn a_folder_on_the_data_volume_leaves_the_system_volume_to_macos() {
        let shares = shares(DATA, &startup());
        assert_eq!(
            shares,
            Shares {
                volume: 574,
                system: 14 + 12 + 4,
                swap: 9,
                volumes: 30,
            }
        );
    }

    #[test]
    fn the_startup_disk_itself_is_the_system_and_data_volumes() {
        let shares = shares("/", &startup());
        assert_eq!(
            shares,
            Shares {
                volume: 14 + 574,
                system: 12 + 4,
                swap: 9,
                volumes: 30,
            }
        );
    }

    #[test]
    fn another_volume_of_the_disk_counts_the_data_volume_as_another_volume() {
        let shares = shares("/Volumes/Work", &startup());
        assert_eq!(
            shares,
            Shares {
                volume: 30,
                system: 14 + 12 + 4,
                swap: 9,
                volumes: 574,
            }
        );
    }

    #[test]
    fn a_device_names_its_disk() {
        assert_eq!(container("/dev/disk3s5"), Some("disk3"));
        assert_eq!(container("/dev/disk3s1s1"), Some("disk3"));
        assert_eq!(container("/dev/disk12"), Some("disk12"));
        assert_eq!(container("/dev/disks1"), None);
        assert_eq!(container("//server/share"), None);
        assert_eq!(container("map auto_home"), None);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn the_startup_disk_adds_up_to_its_used_space() {
        let usage = usage(Path::new("/")).expect("the startup disk");
        assert!(
            usage.name.as_deref().is_some_and(|n| n != "Data"),
            "{usage:?}"
        );
        assert_eq!(super::usage(Path::new("/Users")).unwrap().name, usage.name);
        assert!(usage.whole);
        assert!(!super::usage(Path::new("/usr")).unwrap().whole);
        let shares = usage.shares.expect("an APFS startup disk");
        assert!(shares.volume > 0 && shares.swap <= usage.total);
        // Each figure is read a moment apart, and a volume that is not mounted is in
        // none of them, so the parts come to a little under the used space.
        let parts = shares.volume + shares.system + shares.swap + shares.volumes;
        let used = usage.total - usage.free;
        assert!(parts <= used + used / 20, "{parts} of {used}");
        assert!(parts >= used / 2, "{parts} of {used}");
    }
}
