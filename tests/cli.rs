//! The `hoardmap` command: it hands a folder to the app with `open -b` and serves
//! nothing. A stub `open` on PATH records the call.
#![cfg(target_os = "macos")]

mod common;

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::{Command, Output};

use common::{OpenStub, fixture};

fn run(stub_path: &str, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_hoardmap"))
        .args(args)
        .env("PATH", stub_path)
        .env_remove("HOARDMAP_TEST_SERVER")
        .output()
        .expect("run hoardmap")
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

#[test]
fn a_folder_is_opened_in_the_app() {
    let fx = fixture();
    let stub = OpenStub::new();
    let output = run(&stub.path_env(), &[fx.root.to_str().unwrap()]);
    assert!(output.status.success(), "{}", stderr(&output));
    assert!(
        output.stdout.is_empty(),
        "it printed something: no server, no address"
    );
    let folder = fx.root.canonicalize().unwrap();
    assert_eq!(
        stub.open_args().as_deref(),
        Some(format!("-b\ndev.hoardmap.app\n{}\n", folder.display()).as_str())
    );
}

#[test]
fn a_relative_folder_is_made_absolute() {
    let fx = fixture();
    let stub = OpenStub::new();
    let output = Command::new(env!("CARGO_BIN_EXE_hoardmap"))
        .arg("a")
        .current_dir(&fx.root)
        .env("PATH", stub.path_env())
        .env_remove("HOARDMAP_TEST_SERVER")
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", stderr(&output));
    let folder = fx.root.join("a").canonicalize().unwrap();
    assert!(
        stub.open_args()
            .unwrap()
            .ends_with(&format!("\n{}\n", folder.display())),
        "{:?}",
        stub.open_args()
    );
}

#[test]
fn no_folder_just_opens_the_app() {
    let stub = OpenStub::new();
    let output = run(&stub.path_env(), &[]);
    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(stub.open_args().as_deref(), Some("-b\ndev.hoardmap.app\n"));
}

#[test]
fn a_missing_folder_or_a_file_opens_nothing() {
    let fx = fixture();
    let stub = OpenStub::new();
    for path in [fx.root.join("nope"), fx.root.join("a/big.bin")] {
        let output = run(&stub.path_env(), &[path.to_str().unwrap()]);
        assert_eq!(output.status.code(), Some(1), "{}", path.display());
        assert!(stderr(&output).contains(path.file_name().unwrap().to_str().unwrap()));
    }
    assert_eq!(stub.open_args(), None);
}

#[test]
fn the_browser_options_are_refused() {
    let stub = OpenStub::new();
    for option in ["--port", "--no-open", "--linger"] {
        let output = run(&stub.path_env(), &[option]);
        assert_eq!(output.status.code(), Some(2), "{option}");
    }
    assert_eq!(stub.open_args(), None);
}

#[test]
fn a_missing_app_says_how_to_install_it() {
    let stub = OpenStub::new();
    let failing = stub.dir.join("open");
    fs::write(&failing, "#!/bin/sh\nexit 1\n").unwrap();
    fs::set_permissions(&failing, fs::Permissions::from_mode(0o755)).unwrap();
    let output = run(&stub.path_env(), &[Path::new("/").to_str().unwrap()]);
    assert_eq!(output.status.code(), Some(1));
    assert!(
        stderr(&output).contains("brew install --cask"),
        "{}",
        stderr(&output)
    );
}
