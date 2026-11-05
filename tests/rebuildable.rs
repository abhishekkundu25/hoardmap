//! `/api/rebuildable` end to end: the binary scans a fixture of project folders and
//! decoys, and the endpoint groups exactly the folders with a sentinel, a fixed path or
//! a cache tag, with a caution where the rule has one.

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
const DOCKER: u64 = 120_011;
const TAGGED: u64 = 20_013 + 43;
const NEXT_BUILD: u64 = 10_009;

/// Project folders in the system temp folder, removed on drop:
/// - `proj/package.json` and `proj/node_modules/a/node_modules/b/big` (nested, once);
/// - `rust/target/CACHEDIR.TAG` and `rust/target/debug/app`;
/// - `py/.venv/pyvenv.cfg` and `py/.venv/lib/site.py`;
/// - `Library/Developer/Xcode/DerivedData/App-x/Build/obj`;
/// - `Library/Containers/com.docker.docker/Data/vms/0/data/Docker.raw` (cautioned);
/// - `tool/data/CACHEDIR.TAG` and `tool/data/blob` (a tagged folder of any name);
/// - `web/package.json` and `web/.next/cache/x`;
/// - decoys: `work/target/real.dat` (no sentinel), `bare/node_modules/c` (no
///   `package.json` beside it) and `loose/dist/app.js` (a bare `dist`).
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
        let projects = Projects { root };
        projects.write("proj/package.json", 30);
        projects.write("proj/node_modules/a/node_modules/b/big", NODE_MODULES);
        projects.write("rust/target/CACHEDIR.TAG", 177);
        projects.write("rust/target/debug/app", 300_003);
        projects.write("py/.venv/pyvenv.cfg", 41);
        projects.write("py/.venv/lib/site.py", 50_005);
        projects.write(
            "Library/Developer/Xcode/DerivedData/App-x/Build/obj",
            DERIVED_DATA,
        );
        projects.write(
            "Library/Containers/com.docker.docker/Data/vms/0/data/Docker.raw",
            DOCKER,
        );
        projects.write("tool/data/CACHEDIR.TAG", 43);
        projects.write("tool/data/blob", 20_013);
        projects.write("web/package.json", 30);
        projects.write("web/.next/cache/x", NEXT_BUILD);
        projects.write("loose/dist/app.js", 6_007);
        projects.write("work/target/real.dat", 400_009);
        projects.write("bare/node_modules/c", 5_003);
        projects
    }

    fn write(&self, rel: &str, len: u64) {
        let path = self.root.join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        File::create(path)
            .unwrap()
            .write_all(&vec![b'x'; len as usize])
            .unwrap();
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
fn groups_each_kind_and_skips_the_decoys() {
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
                "docker_disk".to_string(),
                DOCKER,
                1,
                vec![projects.path("Library/Containers/com.docker.docker/Data/vms")]
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
            (
                "cache_tag".to_string(),
                TAGGED,
                1,
                vec![projects.path("tool/data")]
            ),
            (
                "next_build".to_string(),
                NEXT_BUILD,
                1,
                vec![projects.path("web/.next")]
            ),
        ]
    );
    assert_eq!(
        json["total"],
        NODE_MODULES + CARGO_TARGET + VENV + DERIVED_DATA + DOCKER + TAGGED + NEXT_BUILD
    );
    let text = json.to_string();
    assert!(!text.contains("work/target"), "{text}");
    assert!(!text.contains("bare/node_modules"), "{text}");
    assert!(!text.contains("loose/dist"), "{text}");
    for group in json["groups"].as_array().unwrap() {
        for key in ["label", "owner", "why"] {
            assert!(!group[key].as_str().unwrap().is_empty(), "{group}");
        }
        assert!(group["source"].as_str().unwrap().starts_with("https://"));
        let key = group["key"].as_str().unwrap();
        if ["docker_disk", "cache_tag"].contains(&key) {
            assert!(
                group["caution"].as_str().is_some_and(|c| !c.is_empty()),
                "{group}"
            );
        } else {
            assert!(group["caution"].is_null(), "{group}");
        }
        assert_eq!(group["more"], 0);
        let folder = &group["folders"][0];
        assert_eq!(folder["bytes"], group["bytes"]);
        let (_, node) = get_json(&server, &format!("/api/node/{}", folder["id"]));
        assert_eq!(node["path"], folder["path"]);
    }
}

#[test]
fn output_rules_keep_the_specific_owner_and_cautions_in_json() {
    let projects = Projects::new();
    let cases: &[(&str, &[&str], &str, &str)] = &[
        (
            "groovy",
            &["build.gradle", "package.json"],
            "build",
            "gradle_build",
        ),
        (
            "kotlin",
            &["build.gradle.kts", "package.json"],
            "build",
            "gradle_build",
        ),
        (
            "cpp",
            &["CMakeLists.txt", "package.json"],
            "build",
            "cmake_build",
        ),
        (
            "flutter",
            &["pubspec.yaml", "package.json"],
            "build",
            "flutter_build",
        ),
        (
            "python",
            &["pyproject.toml", "package.json"],
            "dist",
            "python_dist",
        ),
        (
            "legacy",
            &["setup.py", "package.json"],
            "dist",
            "python_dist",
        ),
        (
            "vite",
            &["vite.config.ts", "pyproject.toml", "package.json"],
            "dist",
            "js_dist",
        ),
        (
            "next",
            &["next.config.mjs", "package.json"],
            "out",
            "next_export",
        ),
        ("models", &[], ".ollama/models", "ollama_models"),
    ];
    for &(project, markers, output, _) in cases {
        for marker in markers {
            projects.write(&format!("{project}/{marker}"), 1);
        }
        projects.write(&format!("{project}/{output}/artifact"), 100);
    }
    let server = start_done(&projects);
    let json = rebuildable(&server, "");
    for &(project, _, output, expected) in cases {
        let path = projects.path(&format!("{project}/{output}"));
        let found: Vec<_> = json["groups"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|group| {
                group["folders"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|f| f["path"] == path)
            })
            .collect();
        assert_eq!(found.len(), 1, "{path}: {json}");
        let group = found[0];
        assert_eq!(group["key"], expected, "{path}");
        let caution = group["caution"].as_str().expect("output caution");
        match expected {
            "gradle_build" => assert_eq!(
                caution,
                "The name is common, so a folder you made yourself beside the same project file looks the same to hoardmap."
            ),
            "cmake_build" => assert!(caution.contains("CMakeCache.txt"), "{caution}"),
            "ollama_models" => assert_eq!(
                caution,
                "Models you made with ollama create do not download again, and models you downloaded are large to fetch again and may be withdrawn upstream."
            ),
            _ => assert!(!caution.is_empty(), "{group}"),
        }
    }
}

#[test]
fn sentinel_directories_and_symlinks_do_not_identify_rebuildable_folders() {
    let projects = Projects::new();
    for path in [
        "tag-decoy/data/CACHEDIR.TAG/entry",
        "cargo-decoy/target/CACHEDIR.TAG/entry",
        "venv-decoy/.venv/pyvenv.cfg/entry",
        "sibling-decoy/package.json/entry",
        "sibling-decoy/build/artifact",
        "gradle-decoy/build.gradle/entry",
        "gradle-decoy/build/artifact",
        "cmake-decoy/CMakeLists.txt/entry",
        "cmake-decoy/build/artifact",
        "conda-decoy/conda-meta",
        "conda-decoy/pkgs/artifact",
    ] {
        projects.write(path, 1);
    }
    projects.write("marker", 1);
    fs::create_dir_all(projects.root.join("link-decoy/data")).unwrap();
    std::os::unix::fs::symlink(
        projects.root.join("marker"),
        projects.root.join("link-decoy/data/CACHEDIR.TAG"),
    )
    .unwrap();
    projects.write("conda/conda-meta/history", 1);
    projects.write("conda/pkgs/artifact", 1);
    let server = start_done(&projects);
    let json = rebuildable(&server, "");
    assert!(!json.to_string().contains("-decoy"), "{json}");
    let conda = json["groups"]
        .as_array()
        .unwrap()
        .iter()
        .find(|g| g["key"] == "conda_pkgs")
        .expect("a real conda-meta directory still matches");
    assert_eq!(conda["count"], 1);
    assert_eq!(conda["folders"][0]["path"], projects.path("conda/pkgs"));
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
    assert!(json["groups"][0]["caution"].is_null(), "{json}");
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

/// Times `groups` on a real home folder, or a fixture at HOARDMAP_TIMING_ROOT.
/// Run by hand, in release:
/// `cargo test --release --test rebuildable -- --ignored --nocapture`. The budget is
/// 50 ms for about a million entries.
#[test]
#[ignore = "scans the home folder; run by hand"]
fn groups_on_the_home_folder() {
    use hoardmap::scan::{Progress, scan};
    use hoardmap::treemap::Mode;
    use std::time::Instant;
    let root = PathBuf::from(
        std::env::var_os("HOARDMAP_TIMING_ROOT")
            .or_else(|| std::env::var_os("HOME"))
            .expect("HOARDMAP_TIMING_ROOT or HOME"),
    );
    let tree = scan(&root, &Progress::default()).expect("scan");
    let start = Instant::now();
    let groups = hoardmap::rebuildable::groups(&tree, 0, Mode::Physical);
    let took = start.elapsed();
    println!(
        "groups: {} entries, {} groups, {:.1} ms",
        tree.nodes.len(),
        groups.len(),
        took.as_secs_f64() * 1000.0
    );
    for g in &groups {
        println!(
            "  {:<20} {:>14} bytes {:>6} folders",
            g.rule.key, g.bytes, g.count
        );
    }
}
