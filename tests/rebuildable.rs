//! `/api/rebuildable` end to end: the binary scans a fixture of project folders and
//! decoys, and the endpoint groups exactly the folders with a sentinel or a fixed path.

mod common;

use std::fs::{self, File};
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, SystemTime};

use common::{OpenStub, Server, header};
use serde_json::Value;

static NEXT: AtomicUsize = AtomicUsize::new(0);

/// Logical bytes under each matched folder.
const NODE_MODULES: u64 = 200_001;
const CARGO_TARGET: u64 = 300_003 + 177;
const VENV: u64 = 50_005 + 41;
const DERIVED_DATA: u64 = 100_007;

/// Project folders in the system temp folder, removed on drop:
/// - `proj/package.json` and `proj/node_modules/a/node_modules/b/big` (nested, once);
/// - `rust/target/CACHEDIR.TAG` and `rust/target/debug/app`;
/// - `py/.venv/pyvenv.cfg` and `py/.venv/lib/site.py`;
/// - `Library/Developer/Xcode/DerivedData/App-x/Build/obj`;
/// - decoys: `work/target/real.dat` (no sentinel) and `bare/node_modules/c` (no
///   `package.json` beside it).
struct Projects {
    root: PathBuf,
}

impl Projects {
    fn new() -> Projects {
        let nanos = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "hoardmap-rebuildable-{}-{}-{nanos}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let write = |rel: &str, len: u64| {
            let path = root.join(rel);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            File::create(path)
                .unwrap()
                .write_all(&vec![b'x'; len as usize])
                .unwrap();
        };
        write("proj/package.json", 30);
        write("proj/node_modules/a/node_modules/b/big", NODE_MODULES);
        write("rust/target/CACHEDIR.TAG", 177);
        write("rust/target/debug/app", 300_003);
        write("py/.venv/pyvenv.cfg", 41);
        write("py/.venv/lib/site.py", 50_005);
        write(
            "Library/Developer/Xcode/DerivedData/App-x/Build/obj",
            DERIVED_DATA,
        );
        write("work/target/real.dat", 400_009);
        write("bare/node_modules/c", 5_003);
        Projects { root }
    }

    fn path(&self, rel: &str) -> String {
        self.root.join(rel).to_string_lossy().into_owned()
    }
}

impl Drop for Projects {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn start_done(projects: &Projects) -> Server {
    let server = Server::start_in(&projects.root, &[], OpenStub::new());
    server.wait_for("done", Duration::from_secs(10));
    server
}

fn get_json(server: &Server, path: &str) -> (u16, Value) {
    let (status, headers, body) = server.get(path);
    assert_eq!(
        header(&headers, "Content-Type"),
        Some("application/json"),
        "{path}"
    );
    let json = serde_json::from_str(&body).unwrap_or_else(|e| panic!("{path}: {e}: {body}"));
    (status, json)
}

fn rebuildable(server: &Server, query: &str) -> Value {
    let (status, json) = get_json(server, &format!("/api/rebuildable{query}"));
    assert_eq!(status, 200, "{json}");
    json
}

fn child_id(server: &Server, parent: u64, name: &str) -> u64 {
    let (_, json) = get_json(server, &format!("/api/node/{parent}"));
    json["children"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["name"] == name)
        .unwrap_or_else(|| panic!("no child {name} under node {parent}"))["id"]
        .as_u64()
        .unwrap()
}

/// `(key, bytes, count, [paths])` per group, in the order served.
fn summary(json: &Value) -> Vec<(String, u64, u64, Vec<String>)> {
    json["groups"]
        .as_array()
        .unwrap()
        .iter()
        .map(|g| {
            let paths = g["folders"]
                .as_array()
                .unwrap()
                .iter()
                .map(|f| f["path"].as_str().unwrap().to_string())
                .collect();
            (
                g["key"].as_str().unwrap().to_string(),
                g["bytes"].as_u64().unwrap(),
                g["count"].as_u64().unwrap(),
                paths,
            )
        })
        .collect()
}

#[test]
fn groups_the_four_kinds_and_skips_the_decoys() {
    let projects = Projects::new();
    let server = start_done(&projects);
    let json = rebuildable(&server, "");
    assert_eq!(
        summary(&json),
        vec![
            (
                "cargo_target".to_string(),
                CARGO_TARGET,
                1,
                vec![projects.path("rust/target")]
            ),
            (
                "node_modules".to_string(),
                NODE_MODULES,
                1,
                vec![projects.path("proj/node_modules")]
            ),
            (
                "xcode_derived_data".to_string(),
                DERIVED_DATA,
                1,
                vec![projects.path("Library/Developer/Xcode/DerivedData")]
            ),
            (
                "python_venv".to_string(),
                VENV,
                1,
                vec![projects.path("py/.venv")]
            ),
        ]
    );
    assert_eq!(
        json["total"],
        NODE_MODULES + CARGO_TARGET + VENV + DERIVED_DATA
    );
    let text = json.to_string();
    assert!(!text.contains("work/target"), "{text}");
    assert!(!text.contains("bare/node_modules"), "{text}");
    for group in json["groups"].as_array().unwrap() {
        for key in ["label", "owner", "why"] {
            assert!(!group[key].as_str().unwrap().is_empty(), "{group}");
        }
        assert!(group["source"].as_str().unwrap().starts_with("https://"));
        assert_eq!(group["more"], 0);
        let folder = &group["folders"][0];
        assert_eq!(folder["bytes"], group["bytes"]);
        let (_, node) = get_json(&server, &format!("/api/node/{}", folder["id"]));
        assert_eq!(node["path"], folder["path"]);
    }
}

#[test]
fn root_narrows_and_mode_changes_the_bytes() {
    let projects = Projects::new();
    let server = start_done(&projects);
    let proj = child_id(&server, 0, "proj");
    let json = rebuildable(&server, &format!("?root={proj}"));
    assert_eq!(
        summary(&json),
        vec![(
            "node_modules".to_string(),
            NODE_MODULES,
            1,
            vec![projects.path("proj/node_modules")]
        )]
    );
    assert_eq!(json["total"], NODE_MODULES);
    let work = child_id(&server, 0, "work");
    let json = rebuildable(&server, &format!("?root={work}"));
    assert_eq!(json["groups"].as_array().unwrap().len(), 0);
    assert_eq!(json["total"], 0);

    let logical = rebuildable(&server, "?mode=logical");
    let physical = rebuildable(&server, "?mode=physical");
    assert_ne!(physical["total"], logical["total"]);
    for group in physical["groups"].as_array().unwrap() {
        let key = &group["key"];
        let same = logical["groups"]
            .as_array()
            .unwrap()
            .iter()
            .find(|g| &g["key"] == key)
            .unwrap_or_else(|| panic!("{key} only in physical"));
        assert!(
            group["bytes"].as_u64() >= same["bytes"].as_u64(),
            "{group} vs {same}"
        );
        let (_, node) = get_json(&server, &format!("/api/node/{}", group["folders"][0]["id"]));
        assert_eq!(group["bytes"], node["physical"]);
    }
}

#[test]
fn bad_queries_and_methods() {
    let projects = Projects::new();
    let server = start_done(&projects);
    assert_eq!(get_json(&server, "/api/rebuildable?mode=bogus").0, 400);
    assert_eq!(get_json(&server, "/api/rebuildable?root=x").0, 400);
    assert_eq!(get_json(&server, "/api/rebuildable?root=999999").0, 404);
    let (status, headers, _) = server.post("/api/rebuildable", Some("application/json"), "{}");
    assert_eq!(status, 405);
    assert_eq!(header(&headers, "Allow"), Some("GET"));
}

#[test]
fn conflict_without_a_tree() {
    let missing = std::env::temp_dir().join(format!(
        "hoardmap-rebuildable-missing-{}",
        std::process::id()
    ));
    let server = Server::start_in(&missing, &[], OpenStub::new());
    assert_eq!(server.status()["state"], "failed");
    let (status, json) = get_json(&server, "/api/rebuildable");
    assert_eq!(status, 409, "{json}");
    assert_eq!(json["error"], "no tree loaded");
}
