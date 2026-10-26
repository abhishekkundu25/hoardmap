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
        // The Music, TV and Photos libraries, and their caches and system copies.
        "Music/Music",
        "Music/iTunes",
        "Movies/TV",
        "Pictures/Photos Library.photoslibrary",
        "Library/Photos",
        "Library/AppleMediaServices",
        "Library/Caches/com.apple.Music",
        "Library/Caches/com.apple.TV",
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

/// False when the app runs from somewhere Full Disk Access does not stick: the
/// installer disk, or the temporary copy macOS runs an app from when it was opened in
/// Downloads (App Translocation). True for the command.
pub fn installed() -> bool {
    bundle().is_none_or(|b| {
        !b.starts_with("/Volumes") && !b.to_string_lossy().contains("/AppTranslocation/")
    })
}

/// Where the app notes how far each launch got, for when one never shows a window.
pub fn launch_log() -> Option<PathBuf> {
    std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Library/Logs/Hoardmap/launch.log"))
}

/// Appends one line to the launch log; a log that cannot be written is skipped.
pub fn log_launch(line: &str) {
    use std::io::Write;
    let Some(path) = launch_log() else { return };
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).ok();
    }
    // Kept short: started over once it passes 64 KB.
    let append = std::fs::metadata(&path).is_ok_and(|m| m.len() < 64 * 1024);
    let file = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .append(append)
        .truncate(!append)
        .open(&path);
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    if let Ok(mut file) = file {
        writeln!(file, "{secs} pid {} {line}", std::process::id()).ok();
    }
}

/// Starts a new instance and ends this one shortly after, so the caller can still
/// answer its request. A new Full Disk Access grant applies to the new process.
pub fn relaunch() -> io::Result<()> {
    match bundle() {
        Some(bundle) => {
            // A helper outlives this process, waits for it to end so the new one gets
            // the same port (and with it the page's saved settings), then opens the
            // app. Its outcome goes to the launch log.
            log_launch("relaunch requested");
            let log = launch_log().unwrap_or_else(|| "/dev/null".into());
            Command::new("/bin/sh")
                .args([
                    "-c",
                    r#"while kill -0 "$1" 2>/dev/null; do sleep 0.1; done
                       if open "$2" >>"$3" 2>&1; then echo "$(date +%s) relaunch: opened" >>"$3"
                       else echo "$(date +%s) relaunch: open failed" >>"$3"; fi"#,
                    "relaunch",
                ])
                .arg(std::process::id().to_string())
                .arg(&bundle)
                .arg(log)
                .spawn()?;
        }
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
