//! The JSON API end to end: the binary scans the fixture at start and every endpoint
//! answers with the spec's shapes, codes and figures.

mod common;

use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::Path;
use std::thread;
use std::time::{Duration, Instant};

use common::{Fixture, OpenStub, Server, assert_idle, fixture, header};
use serde_json::Value;

const ARGS: &[&str] = &["--no-open", "--linger", "1"];
const JSON: Option<&str> = Some("application/json");

fn lossy(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

fn start(fx: &Fixture) -> Server {
    Server::start_in(&fx.root, ARGS, OpenStub::new())
}

/// Starts on the fixture and waits for the scan to finish.
fn start_done(fx: &Fixture) -> Server {
    let server = start(fx);
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

fn post_json(server: &Server, path: &str, body: &str) -> (u16, Value) {
    let (status, _, text) = server.post(path, JSON, body);
    let json = if text.is_empty() {
        Value::Null
    } else {
        serde_json::from_str(&text).unwrap_or_else(|e| panic!("{path}: {e}: {text}"))
    };
    (status, json)
}

fn node(server: &Server, id: u64) -> Value {
    let (status, json) = get_json(server, &format!("/api/node/{id}"));
    assert_eq!(status, 200, "{json}");
    json
}

fn child_id(server: &Server, parent: u64, name: &str) -> u64 {
    node(server, parent)["children"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["name"] == name)
        .unwrap_or_else(|| panic!("no child {name} under node {parent}"))["id"]
        .as_u64()
        .unwrap()
}

/// Opens `/api/events` without consuming anything, so the first status block is seen.
fn open_events_raw(server: &Server) -> TcpStream {
    let mut stream = server.connect();
    write!(
        stream,
        "GET /api/events HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nAccept: text/event-stream\r\n\r\n",
        server.port
    )
    .unwrap();
    stream
}

/// Reads from the stream into `text` until `needle` appears at or after `from`.
fn read_until(stream: &mut TcpStream, text: &mut String, from: usize, needle: &str) {
    let mut buf = [0u8; 4096];
    loop {
        if text[from..].contains(needle) {
            return;
        }
        let n = stream.read(&mut buf).expect("read the event stream");
        assert!(n > 0, "the event stream closed before {needle:?}:\n{text}");
        text.push_str(&String::from_utf8_lossy(&buf[..n]));
    }
}

fn wait_open_args(stub: &OpenStub, expected: &str) {
    let end = Instant::now() + Duration::from_secs(5);
    loop {
        let args = stub.open_args();
        if args.as_deref() == Some(expected) {
            return;
        }
        assert!(
            Instant::now() < end,
            "the stub open received {args:?}, expected {expected:?}"
        );
        thread::sleep(Duration::from_millis(50));
    }
}

#[test]
fn status_reaches_done_with_the_fixture_totals() {
    let fx = fixture();
    let server = start(&fx);
    let started = Instant::now();
    let status = server.wait_for("done", Duration::from_secs(10));
    println!("# fixture scan done after {:.3?}", started.elapsed());
    assert_eq!(status["path"], lossy(&fx.root));
    assert_eq!(status["bytes"], fx.total_bytes());
    assert_eq!(status["dirs"], 4);
    assert_eq!(status["files"], if fx.locked_unreadable { 6 } else { 7 });
    assert_eq!(
        status["error_count"],
        if fx.locked_unreadable { 1 } else { 0 }
    );
    assert!(status["elapsed_ms"].is_u64());
    assert!(status["error"].is_null());
}

#[test]
fn events_carry_scanning_then_done() {
    let fx = fixture();
    let server = start(&fx);
    let mut stream = open_events_raw(&server);
    let mut text = String::new();
    read_until(&mut stream, &mut text, 0, "\"state\":\"done\"");
    assert!(text.contains("event: status\ndata: {"), "{text}");

    // The fixture is too small to catch a tick, so real folders are rescanned with the
    // socket open: `start_scan` sends one `scanning` status at once, the ticker adds one
    // every 250 ms, and a bigger folder is tried when the first scan beat the ticker.
    let mut ticks = 0;
    for folder in ["/usr/share", "/System/Library"] {
        if !Path::new(folder).is_dir() {
            continue;
        }
        let from = text.len();
        let (status, _) = post_json(&server, "/api/scan", &format!(r#"{{"path": {folder:?}}}"#));
        assert_eq!(status, 202);
        let started = Instant::now();
        read_until(&mut stream, &mut text, from, "\"state\":\"done\"");
        let after = &text[from..];
        ticks = after.matches("\"state\":\"scanning\"").count();
        println!(
            "# {folder} scan: {ticks} scanning statuses then done after {:.3?}",
            started.elapsed()
        );
        assert!(ticks >= 1, "no scanning status on the stream:\n{after}");
        let done_at = after.find("\"state\":\"done\"").unwrap();
        let scanning_at = after.find("\"state\":\"scanning\"").unwrap();
        assert!(scanning_at < done_at, "{after}");
        if ticks >= 2 {
            break;
        }
    }
    if ticks < 2 {
        println!("# note: every scan ended before the first 250 ms tick; the ticker was not seen");
    }
}

#[test]
fn node_0_lists_children_largest_first_and_unknown_ids_are_404() {
    let fx = fixture();
    let server = start_done(&fx);
    let root = node(&server, 0);
    assert_eq!(root["id"], 0);
    assert_eq!(root["path"], lossy(&fx.root));
    assert_eq!(root["name"], lossy(&fx.root));
    assert_eq!(root["kind"], "dir");
    assert_eq!(root["size"], fx.total_bytes());
    assert_eq!(root["has_children"], true);
    assert!(root["parent"].is_null());
    let children = root["children"].as_array().unwrap();
    assert_eq!(children[0]["name"], "a");
    assert_eq!(children[0]["size"], 300_010);
    assert_eq!(children[0]["has_children"], true);
    assert_eq!(children[1]["name"], "b");
    for pair in children.windows(2) {
        assert!(
            pair[0]["size"].as_u64() >= pair[1]["size"].as_u64(),
            "{} before {}",
            pair[0]["name"],
            pair[1]["name"]
        );
    }
    for key in [
        "id",
        "name",
        "kind",
        "size",
        "physical",
        "files",
        "dirs",
        "mtime",
        "hard_link",
        "has_children",
    ] {
        assert!(!children[0][key].is_null(), "child lacks {key}");
    }
    let link = children.iter().find(|c| c["name"] == "link.txt").unwrap();
    assert_eq!(link["hard_link"], true);
    assert_eq!(link["has_children"], false);

    let a = child_id(&server, 0, "a");
    let big = child_id(&server, a, "big.bin");
    let big = node(&server, big);
    assert_eq!(big["path"], lossy(&fx.root.join("a/big.bin")));
    assert_eq!(big["parent"], a);
    assert_eq!(big["children"].as_array().unwrap().len(), 0);

    let (status, json) = get_json(&server, "/api/node/999999");
    assert_eq!(status, 404);
    assert!(json["error"].is_string());
    assert_eq!(get_json(&server, "/api/node/x").0, 404);
    assert_eq!(get_json(&server, "/api/node/").0, 404);
}

#[test]
fn icicle_bars_stack_one_row_per_depth_under_their_parents() {
    let fx = fixture();
    let server = start_done(&fx);
    let (status, layout) = get_json(&server, "/api/icicle?root=0&w=400&h=300");
    assert_eq!(status, 200, "{layout}");
    let bars = layout["rects"].as_array().unwrap();
    let rows = layout["rows"].as_u64().unwrap();
    assert!(rows >= 2, "{layout}");
    let row = 300.0 / rows as f64;
    assert_eq!(bars[0]["id"], 0);
    assert_eq!(bars[0]["w"], 400.0);
    let by_id = |id: &Value| bars.iter().find(|b| b["id"] == *id);
    for b in bars {
        let f = |k: &str| b[k].as_f64().unwrap();
        assert!(
            (f("y") - b["depth"].as_f64().unwrap() * row).abs() < 0.01,
            "{b}"
        );
        assert!(f("x") >= -0.01 && f("x") + f("w") <= 400.01, "{b}");
        assert!(b["name"].is_string(), "{b}");
        if b["depth"] == 0 {
            continue;
        }
        let (_, node) = get_json(&server, &format!("/api/node/{}", b["id"]));
        let parent = by_id(&node["parent"]).unwrap_or_else(|| panic!("no parent bar for {b}"));
        let p = |k: &str| parent[k].as_f64().unwrap();
        assert!(
            f("x") >= p("x") - 0.01 && f("x") + f("w") <= p("x") + p("w") + 0.01,
            "{b}"
        );
    }
    let names: Vec<&str> = bars.iter().map(|b| b["name"].as_str().unwrap()).collect();
    assert!(names.contains(&"big.bin"), "{names:?}");
    let (status, _) = get_json(&server, "/api/icicle?w=0&h=300");
    assert_eq!(status, 400);
    let (status, _) = get_json(&server, "/api/icicle?root=999999&w=400&h=300");
    assert_eq!(status, 404);
}

#[test]
fn treemap_rects_fit_the_viewport_with_extension_colours() {
    let fx = fixture();
    let server = start_done(&fx);
    let (status, layout) = get_json(&server, "/api/treemap?root=0&w=400&h=300");
    assert_eq!(status, 200, "{layout}");
    assert_eq!(layout["root"], 0);
    assert_eq!(layout["w"], 400.0);
    assert_eq!(layout["h"], 300.0);
    let rects = layout["rects"].as_array().unwrap();
    assert!(!rects.is_empty());
    let (_, exts) = get_json(&server, "/api/extensions");
    let colour_of = |ext: &str| {
        exts["items"]
            .as_array()
            .unwrap()
            .iter()
            .find(|i| i["ext"] == ext)
            .unwrap_or_else(|| panic!("no extension {ext:?}"))["color"]
            .clone()
    };
    let mut area = 0.0;
    for r in rects {
        let f = |k: &str| r[k].as_f64().unwrap();
        assert!(f("x") >= -0.01 && f("y") >= -0.01, "{r}");
        assert!(
            f("x") + f("w") <= 400.01 && f("y") + f("h") <= 300.01,
            "{r}"
        );
        assert!(f("w") > 0.0 && f("h") > 0.0, "{r}");
        area += f("w") * f("h");
        if r["kind"] != "dir" {
            assert_eq!(r["color"], colour_of(r["ext"].as_str().unwrap()), "{r}");
        }
    }
    assert!(
        area <= 400.0 * 300.0 * 1.001 && area > 400.0 * 300.0 * 0.99,
        "{area}"
    );
    let dirs = layout["dirs"].as_array().unwrap();
    assert_eq!(dirs[0]["id"], 0);
    let ids: Vec<u64> = rects.iter().map(|r| r["id"].as_u64().unwrap()).collect();
    let a = child_id(&server, 0, "a");
    assert!(ids.contains(&child_id(&server, a, "big.bin")));

    let (status, physical) = get_json(&server, "/api/treemap?root=0&w=400&h=300&mode=physical");
    assert_eq!(status, 200, "{physical}");
    let a_layout = get_json(&server, &format!("/api/treemap?root={a}&w=100&h=100&min=2"));
    assert_eq!(a_layout.0, 200);
    assert_eq!(a_layout.1["root"], a);

    for bad in [
        "/api/treemap",
        "/api/treemap?w=400",
        "/api/treemap?w=0&h=300",
        "/api/treemap?w=400&h=nan",
        "/api/treemap?w=inf&h=300",
        "/api/treemap?w=400&h=300&min=x",
        "/api/treemap?w=400&h=300&mode=bytes",
        "/api/treemap?w=400&h=300&root=-1",
    ] {
        let (status, json) = get_json(&server, bad);
        assert_eq!(status, 400, "{bad}: {json}");
        assert!(json["error"].is_string());
    }
    assert_eq!(
        get_json(&server, "/api/treemap?root=999999&w=400&h=300").0,
        404
    );
    assert_eq!(get_json(&server, "/api/treemap?w=400&h=300&min=0").0, 200);
    assert_eq!(
        get_json(&server, "/api/treemap?w=400&h=300&min=1000").0,
        200
    );
}

#[test]
fn extensions_list_the_fixture_types() {
    let fx = fixture();
    let server = start_done(&fx);
    let (status, json) = get_json(&server, "/api/extensions");
    assert_eq!(status, 200);
    assert_eq!(json["total"], fx.total_bytes());
    let items = json["items"].as_array().unwrap();
    let find = |ext: &str| {
        items
            .iter()
            .find(|i| i["ext"] == ext)
            .unwrap_or_else(|| panic!("no extension {ext:?} in {json}"))
    };
    assert_eq!(find("bin")["bytes"], 300_000);
    assert_eq!(find("bin")["files"], 1);
    assert_eq!(find("jpg")["bytes"], 2000);
    assert_eq!(find("jpg")["files"], 1);
    assert_eq!(find("txt")["bytes"], 10);
    assert_eq!(find("txt")["files"], 2);
    assert_eq!(find("")["bytes"], 5 + fx.sym_len());
    assert_eq!(find("")["files"], 2);
    assert_eq!(items.len(), 4);
    for pair in items.windows(2) {
        assert!(pair[0]["bytes"].as_u64() >= pair[1]["bytes"].as_u64());
    }
    assert_eq!(items[0]["ext"], "bin");
    assert_eq!(items[0]["color"], "#d9534f");
    for item in items {
        assert!(item["color"].as_str().unwrap().starts_with('#'));
        assert!(item["physical"].as_u64().unwrap() >= item["bytes"].as_u64().unwrap());
    }

    let b = child_id(&server, 0, "b");
    let (_, under_b) = get_json(&server, &format!("/api/extensions?root={b}&mode=physical"));
    assert_eq!(under_b["items"].as_array().unwrap().len(), 1);
    assert_eq!(under_b["items"][0]["ext"], "jpg");
    assert_eq!(under_b["total"], under_b["items"][0]["physical"]);
    assert_eq!(get_json(&server, "/api/extensions?root=999999").0, 404);
    assert_eq!(get_json(&server, "/api/extensions?mode=x").0, 400);
}

#[test]
fn largest_two_are_big_bin_then_x_jpg() {
    let fx = fixture();
    let server = start_done(&fx);
    let (status, json) = get_json(&server, "/api/largest?n=2");
    assert_eq!(status, 200);
    let items = json["items"].as_array().unwrap();
    assert_eq!(items.len(), 2);
    assert_eq!(items[0]["path"], lossy(&fx.root.join("a/big.bin")));
    assert_eq!(items[0]["size"], 300_000);
    assert_eq!(items[1]["path"], lossy(&fx.root.join("b/x.jpg")));
    assert_eq!(items[1]["size"], 2000);
    for item in items {
        for key in ["id", "path", "size", "physical", "mtime"] {
            assert!(!item[key].is_null(), "item lacks {key}");
        }
    }
    let a = child_id(&server, 0, "a");
    assert_eq!(items[0]["id"], child_id(&server, a, "big.bin"));

    let (_, all) = get_json(&server, "/api/largest");
    assert_eq!(all["items"].as_array().unwrap().len(), 6);
    let (_, under_a) = get_json(
        &server,
        &format!("/api/largest?root={a}&n=5000&mode=physical"),
    );
    assert_eq!(under_a["items"].as_array().unwrap().len(), 2);
    assert_eq!(get_json(&server, "/api/largest?n=x").0, 400);
    assert_eq!(get_json(&server, "/api/largest?root=999999").0, 404);
}

#[test]
fn errors_report_the_locked_dir() {
    let fx = fixture();
    let server = start_done(&fx);
    let (status, json) = get_json(&server, "/api/errors");
    assert_eq!(status, 200);
    if !fx.locked_unreadable {
        println!("skipped: errors_report_the_locked_dir (running as root)");
        assert_eq!(json["count"], 0);
        return;
    }
    assert_eq!(json["count"], 1);
    let items = json["items"].as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["path"], lossy(&fx.root.join("locked")));
    assert!(items[0]["error"].as_str().is_some_and(|e| !e.is_empty()));
}

#[test]
fn scan_refuses_files_and_a_second_scan_then_rescans() {
    let fx = fixture();
    let server = start_done(&fx);
    let file = fx.root.join("a/big.bin");
    let (status, json) = post_json(&server, "/api/scan", &format!(r#"{{"path": {file:?}}}"#));
    assert_eq!(status, 400, "{json}");
    assert!(json["error"].is_string());
    let (status, _) = post_json(&server, "/api/scan", r#"{"path": "/nonexistent/folder"}"#);
    assert_eq!(status, 400);
    let (status, _) = post_json(&server, "/api/scan", r#"{"path": ""}"#);
    assert_eq!(status, 400);
    let (status, _) = post_json(&server, "/api/scan", r#"{"path": 3}"#);
    assert_eq!(status, 400);
    assert_eq!(server.post("/api/scan", None, r#"{"path": "/usr"}"#).0, 415);
    assert_eq!(server.status()["state"], "done");

    let (status, first) = post_json(&server, "/api/scan", r#"{"path": "/usr"}"#);
    assert_eq!(status, 202, "{first}");
    assert_eq!(first["state"], "scanning");
    assert_eq!(first["path"], "/usr");
    let started = Instant::now();
    let (status, second) = post_json(&server, "/api/scan", r#"{"path": "/usr"}"#);
    match status {
        409 => {
            assert_eq!(second["error"], "a scan is running");
            println!("# second scan request during the /usr scan: 409");
        }
        202 => println!("# note: the /usr scan ended before the second request; 409 not observed"),
        other => panic!("second scan request: {other} {second}"),
    }
    let done = server.wait_for("done", Duration::from_secs(90));
    println!(
        "# /usr scan: {} dirs, {} files, {} bytes, {} errors, {:.3?} ({} ms by the server)",
        done["dirs"],
        done["files"],
        done["bytes"],
        done["error_count"],
        started.elapsed(),
        done["elapsed_ms"]
    );
    assert_eq!(done["path"], "/usr");
    assert!(done["files"].as_u64().unwrap() > 100);

    let (status, _) = post_json(
        &server,
        "/api/scan",
        &format!(r#"{{"path": {:?}}}"#, fx.root),
    );
    assert_eq!(status, 202);
    let done = server.wait_for("done", Duration::from_secs(10));
    assert_eq!(done["path"], lossy(&fx.root));
    assert_eq!(done["bytes"], fx.total_bytes());
    assert_eq!(node(&server, 0)["path"], lossy(&fx.root));
}

#[test]
fn open_runs_the_stub_with_r_for_files_and_bare_for_dirs() {
    let fx = fixture();
    let before = fx.listing();
    let server = start_done(&fx);
    let a = child_id(&server, 0, "a");
    let big = child_id(&server, a, "big.bin");

    let (status, _, body) = server.post("/api/open", None, &format!(r#"{{"id": {big}}}"#));
    assert_eq!(status, 415, "{body}");
    let (status, _, _) = server.post(
        "/api/open",
        Some("text/plain"),
        &format!(r#"{{"id": {big}}}"#),
    );
    assert_eq!(status, 415);
    assert_eq!(server.stub.open_args(), None);

    let (status, headers, body) = server.post(
        "/api/open",
        Some("application/json; charset=utf-8"),
        &format!(r#"{{"id": {big}}}"#),
    );
    assert_eq!(status, 204, "{body}");
    assert_eq!(header(&headers, "Content-Length"), Some("0"));
    assert!(body.is_empty());
    wait_open_args(
        &server.stub,
        &format!("-R\n{}\n", fx.root.join("a/big.bin").display()),
    );

    let (status, _) = post_json(&server, "/api/open", &format!(r#"{{"id": {a}}}"#));
    assert_eq!(status, 204);
    wait_open_args(&server.stub, &format!("{}\n", fx.root.join("a").display()));

    let (status, _) = post_json(&server, "/api/open", r#"{"id": 0}"#);
    assert_eq!(status, 204);
    wait_open_args(&server.stub, &format!("{}\n", fx.root.display()));

    let (status, json) = post_json(&server, "/api/open", r#"{"id": 999999}"#);
    assert_eq!(status, 404, "{json}");
    let (status, _) = post_json(&server, "/api/open", r#"{"id": "0"}"#);
    assert_eq!(status, 400);
    let (status, _) = post_json(&server, "/api/open", r#"{"path": "/"}"#);
    assert_eq!(status, 400);
    let (status, _) = post_json(&server, "/api/open", "{");
    assert_eq!(status, 400);
    let (status, _, _) = server.post("/api/open", JSON, "");
    assert_eq!(status, 400);

    drop(server);
    assert_eq!(fx.listing(), before, "the fixture changed under the server");
    assert_eq!(before.len(), 10, "{before:?}");
}

#[test]
fn an_oversized_post_body_is_413() {
    let fx = fixture();
    let server = start_done(&fx);
    let body = format!(r#"{{"path": "{}"}}"#, "x".repeat(70 * 1024));
    let (status, _, _) = server.post("/api/scan", JSON, &body);
    assert_eq!(status, 413);
}

#[test]
fn idle_after_done_with_a_tab_connected_uses_no_cpu() {
    let fx = fixture();
    let server = start_done(&fx);
    let _events = server.open_events();
    assert_idle(&server, "after the scan, tab connected");
}

#[test]
fn host_header_must_name_loopback_on_the_bound_port() {
    let fx = fixture();
    let server = start_done(&fx);
    let port = server.port;
    let request = |host: &str| {
        let host = if host.is_empty() {
            String::new()
        } else {
            format!("Host: {host}\r\n")
        };
        format!("GET /api/status HTTP/1.1\r\n{host}Connection: close\r\n\r\n")
    };
    for host in [
        &format!("127.0.0.1:{port}"),
        &format!("localhost:{port}"),
        "127.0.0.1",
        "localhost",
    ] {
        let (status, _, body) = server.raw(&request(host));
        assert_eq!(status, 200, "Host: {host}: {body}");
    }
    for host in [
        &format!("evil.example:{port}"),
        &format!("127.0.0.1.evil.example:{port}"),
        &format!("127.0.0.1:{}", port.wrapping_add(1)),
        "evil.example",
        &format!("[::1]:{port}"),
        "",
    ] {
        let (status, _, body) = server.raw(&request(host));
        assert_eq!(status, 403, "Host: {host:?}: {body}");
        assert_eq!(body, "forbidden\n");
    }
    let (status, _, _) = server.raw(&format!(
        "GET / HTTP/1.1\r\nHost: evil.example:{port}\r\nConnection: close\r\n\r\n"
    ));
    assert_eq!(status, 403);
    let (status, _, _) = server.raw(&format!(
        "POST /api/scan HTTP/1.1\r\nHost: evil.example:{port}\r\nContent-Type: application/json\r\nContent-Length: 13\r\nConnection: close\r\n\r\n{{\"path\": \"/\"}}"
    ));
    assert_eq!(status, 403);
    assert_eq!(server.status()["state"], "done");
}

#[test]
fn source_never_deletes_moves_or_renames() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut checked = 0;
    for entry in std::fs::read_dir(&src).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|e| e == "rs") {
            let text = std::fs::read_to_string(&path).unwrap();
            for needle in [
                "remove_",
                "rename(",
                "remove_dir",
                "remove_file",
                "fs::rename",
                "trash",
            ] {
                assert!(
                    !text.contains(needle),
                    "{} contains {needle:?}",
                    path.display()
                );
            }
            checked += 1;
        }
    }
    assert!(checked >= 7, "only {checked} source files checked");
}

#[test]
fn access_reports_the_grant_and_opens_the_settings_pane() {
    let fx = fixture();
    let mut server = start_done(&fx);
    let (status, access) = get_json(&server, "/api/access");
    assert_eq!(status, 200, "{access}");
    assert!(access["granted"].is_boolean(), "{access}");
    assert_eq!(access["native"], false);

    let (status, _, _) = server.post("/api/access/settings", None, "{}");
    assert_eq!(status, 415);
    let (status, _) = post_json(&server, "/api/access/settings", "{}");
    assert_eq!(status, 204);
    wait_open_args(
        &server.stub,
        &format!("{}\n", macdirtree::access::SETTINGS_URL),
    );

    // Relaunching and revealing the bundle belong to the native app only.
    let (status, _) = post_json(&server, "/api/relaunch", "{}");
    assert_eq!(status, 404);
    let (status, _) = post_json(&server, "/api/access/reveal", "{}");
    assert_eq!(status, 404);
    assert!(server.is_running());
}
