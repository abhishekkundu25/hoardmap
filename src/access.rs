//! Full Disk Access: whether this process has it, the Settings pane that grants it,
//! and relaunching the app so a new grant takes effect. macOS offers no prompt for
//! this permission; the user turns it on in System Settings.

use std::ffi::OsStr;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::Duration;

/// The Privacy & Security > Full Disk Access pane.
pub const SETTINGS_URL: &str =
    "x-apple.systempreferences:com.apple.preference.security?Privacy_AllFiles";

/// Readable only with Full Disk Access, and present on every Mac.
const PROBE: &str = "/Library/Application Support/com.apple.TCC/TCC.db";

/// Whether this process may read folders macOS keeps private. Always true off macOS.
pub fn granted() -> bool {
    !cfg!(target_os = "macos") || std::fs::File::open(PROBE).is_ok()
}

/// Folders in `home` that make macOS stop and ask the user, one prompt each, when an
/// app without Full Disk Access reads them. A scan without access leaves them out
/// rather than set off a run of prompts. Empty off macOS.
pub fn prompting(home: &Path) -> Vec<PathBuf> {
    if !cfg!(target_os = "macos") {
        return Vec::new();
    }
    [
        "Desktop",
        "Documents",
        "Downloads",
        // iCloud Drive, and the folders of Google Drive, OneDrive, Dropbox and the like.
        "Library/Mobile Documents",
        "Library/CloudStorage",
        // Other apps' data.
        "Library/Containers",
        "Library/Group Containers",
        // The Music and Photos libraries.
        "Music/Music",
        "Music/iTunes",
        "Pictures/Photos Library.photoslibrary",
    ]
    .iter()
    .map(|name| home.join(name))
    .collect()
}

/// The `.app` bundle this executable runs from, if any.
pub fn bundle() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    exe.ancestors()
        .find(|p| p.extension() == Some(OsStr::new("app")))
        .map(PathBuf::from)
}

pub fn open_settings() -> io::Result<()> {
    open([OsStr::new(SETTINGS_URL)])
}

/// Shows the app bundle selected in Finder, so it can be dragged into the Settings list.
pub fn reveal_app() -> io::Result<()> {
    let bundle = bundle().ok_or_else(|| io::Error::other("not running from an app bundle"))?;
    open([OsStr::new("-R"), bundle.as_os_str()])
}

/// Starts a new instance and ends this one shortly after, so the caller can still
/// answer its request. A new Full Disk Access grant applies to the new process.
pub fn relaunch() -> io::Result<()> {
    match bundle() {
        Some(bundle) => open([OsStr::new("-n"), bundle.as_os_str()])?,
        None => {
            Command::new(std::env::current_exe()?)
                .args(std::env::args_os().skip(1))
                .spawn()?;
        }
    }
    thread::spawn(|| {
        thread::sleep(Duration::from_millis(300));
        std::process::exit(0);
    });
    Ok(())
}

fn open<'a>(args: impl IntoIterator<Item = &'a OsStr>) -> io::Result<()> {
    let mut child = Command::new("open").args(args).spawn()?;
    thread::spawn(move || child.wait().ok());
    Ok(())
}
