//! Resource hygiene: the server costs nothing while a tab sits idle, and it exits once
//! the last tab has been gone for the linger period.

mod common;

use std::thread;
use std::time::Duration;

use common::{Server, assert_idle, header};

const IDLE: &[&str] = &["--no-open", "--linger", "1"];

#[test]
fn first_stdout_line_is_the_listening_url() {
    let server = Server::start(IDLE);
    let digits = server
        .first_line
        .strip_prefix("Listening on http://127.0.0.1:")
        .and_then(|r| r.strip_suffix('/'))
        .expect("listening line");
    assert!(
        !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()),
        "{}",
        server.first_line
    );
    assert_eq!(server.url(), format!("http://127.0.0.1:{}/", server.port));
}

#[test]
fn static_files_have_their_types_and_unknown_paths_are_404() {
    let server = Server::start(IDLE);
    let (status, headers, body) = server.get("/");
    assert_eq!(status, 200);
    assert!(
        header(&headers, "Content-Type")
            .unwrap()
            .starts_with("text/html")
    );
    assert_eq!(header(&headers, "Cache-Control"), Some("no-store"));
    assert!(body.contains("macdirtree"), "{body}");

    let (status, headers, _) = server.get("/app.js");
    assert_eq!(status, 200);
    assert!(
        header(&headers, "Content-Type")
            .unwrap()
            .starts_with("application/javascript")
    );

    let (status, headers, _) = server.get("/style.css");
    assert_eq!(status, 200);
    assert!(
        header(&headers, "Content-Type")
            .unwrap()
            .starts_with("text/css")
    );

    let (status, _, _) = server.get("/nope");
    assert_eq!(status, 404);
}

#[test]
fn status_is_idle_and_other_api_paths_are_404_json() {
    let server = Server::start(IDLE);
    let (status, headers, body) = server.get("/api/status");
    assert_eq!(status, 200);
    assert_eq!(header(&headers, "Content-Type"), Some("application/json"));
    let json: serde_json::Value = serde_json::from_str(&body).expect("JSON body");
    assert_eq!(json["state"], "idle");

    let (status, headers, body) = server.get("/api/nope");
    assert_eq!(status, 404);
    assert_eq!(header(&headers, "Content-Type"), Some("application/json"));
    let json: serde_json::Value = serde_json::from_str(&body).expect("JSON body");
    assert!(json["error"].is_string());
}

#[test]
fn an_idle_server_with_a_tab_connected_uses_no_cpu() {
    let server = Server::start(IDLE);
    let _events = server.open_events();
    assert_idle(&server, "tab connected");
    assert_idle(&server, "tab connected, second sample");
}

#[test]
fn exits_0_after_the_last_tab_disconnects() {
    let mut server = Server::start(IDLE);
    let events = server.open_events();
    drop(events);
    let status = server
        .wait_exit(Duration::from_secs(4))
        .expect("the server did not exit within linger + 3 s");
    assert_eq!(status.code(), Some(0));
}

#[test]
fn a_reload_within_linger_keeps_it_alive() {
    let mut server = Server::start(IDLE);
    let events = server.open_events();
    drop(events);
    thread::sleep(Duration::from_millis(300));
    let events = server.open_events();
    thread::sleep(Duration::from_secs(3));
    assert!(
        server.is_running(),
        "the server exited despite the reconnect"
    );
    drop(events);
    let status = server
        .wait_exit(Duration::from_secs(4))
        .expect("the server did not exit after the reconnected tab left");
    assert_eq!(status.code(), Some(0));
}

#[test]
fn waits_indefinitely_for_the_first_tab() {
    let mut server = Server::start(IDLE);
    thread::sleep(Duration::from_secs(3));
    assert!(
        server.is_running(),
        "the server exited before any tab connected"
    );
}

#[test]
fn no_open_runs_nothing() {
    let server = Server::start(IDLE);
    thread::sleep(Duration::from_millis(500));
    assert_eq!(server.stub.open_args(), None);
    let (status, _, _) = server.get("/api/status");
    assert_eq!(status, 200);
}

#[test]
fn without_no_open_the_url_is_opened() {
    let server = Server::start(&["--linger", "1"]);
    let mut args = None;
    for _ in 0..40 {
        args = server.stub.open_args();
        if args.is_some() {
            break;
        }
        thread::sleep(Duration::from_millis(50));
    }
    assert_eq!(args.as_deref().map(str::trim), Some(server.url().as_str()));
}
