//! `hoardmap [PATH]`: opens the Hoardmap app on a folder. Debug builds also have the
//! test server the integration tests drive (`HOARDMAP_TEST_SERVER`); release builds
//! have no way to serve the page to a browser.

use std::path::PathBuf;
use std::process::{Command, ExitCode};

use hoardmap::cli::{self, USAGE};

/// The app's bundle identifier, which `open -b` finds wherever the app is installed.
const BUNDLE_ID: &str = "dev.hoardmap.app";

fn main() -> ExitCode {
    let path = match cli::parse(std::env::args().skip(1)) {
        Ok(path) => path,
        Err(cli::Error::Help) => {
            print!("{USAGE}");
            return ExitCode::SUCCESS;
        }
        Err(cli::Error::Usage(message)) => {
            eprint!("hoardmap: {message}\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    #[cfg(debug_assertions)]
    if std::env::var_os("HOARDMAP_TEST_SERVER").is_some() {
        test_server(path);
    }
    // `open` hands the app a file URL, so the folder must be absolute.
    let path = match path
        .map(|p| p.canonicalize().map_err(|e| (p, e)))
        .transpose()
    {
        Ok(path) => path,
        Err((p, error)) => {
            eprintln!("hoardmap: {}: {error}", p.display());
            return ExitCode::FAILURE;
        }
    };
    if let Some(p) = path.as_ref().filter(|p| !p.is_dir()) {
        eprintln!("hoardmap: {} is not a folder", p.display());
        return ExitCode::FAILURE;
    }
    open_app(path)
}

fn open_app(path: Option<PathBuf>) -> ExitCode {
    if !cfg!(target_os = "macos") {
        eprintln!("hoardmap: the Hoardmap app runs on macOS only");
        return ExitCode::FAILURE;
    }
    let mut open = Command::new("open");
    open.args(["-b", BUNDLE_ID]);
    open.args(path);
    match open.status() {
        Ok(status) if status.success() => ExitCode::SUCCESS,
        _ => {
            eprintln!(
                "hoardmap: could not open the Hoardmap app. Install it with\n  \
                 brew install --cask abhishekkundu25/tap/hoardmap"
            );
            ExitCode::FAILURE
        }
    }
}

/// Serves PATH (default: the home folder) the way the app's window sees it, prints
/// `Listening on <page address>` and runs until killed. For the integration tests.
#[cfg(debug_assertions)]
fn test_server(path: Option<PathBuf>) -> ! {
    use std::io::Write;

    use hoardmap::server::{self, Config};

    let path = path.unwrap_or_else(|| std::env::var_os("HOME").unwrap_or_default().into());
    let served = server::spawn(Config {
        path,
        port: 0,
        native: false,
        scan_at_start: true,
    })
    .expect("bind the test server");
    println!("Listening on {}", served.url);
    std::io::stdout().flush().ok();
    loop {
        std::thread::park();
    }
}
