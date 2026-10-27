//! Resource hygiene: the server costs nothing while its page sits idle, and it neither
//! exits nor opens anything on its own.

mod common;

use std::thread;
use std::time::Duration;

use common::{Server, assert_idle, header};

const IDLE: &[&str] = &[];

#[test]
fn first_stdout_line_is_the_listening_url() {
    let server = Server::start(IDLE);
    let (digits, token) = server
        .first_line
        .strip_prefix("Listening on http://127.0.0.1:")
        .and_then(|r| r.split_once("/?token="))
        .expect("listening line");
    assert!(
        !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()),
        "{}",
        server.first_line
    );
    assert_eq!(token.len(), 64, "{}", server.first_line);
    assert_eq!(server.first_line, format!("Listening on {}", server.url()));
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
    assert!(body.contains("Hoardmap"), "{body}");

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

    let (status, headers, body) = server.get("/logo.svg");
    assert_eq!(status, 200);
    assert_eq!(header(&headers, "Content-Type"), Some("image/svg+xml"));
    assert!(body.starts_with("<svg"), "{body}");

    let (status, _, _) = server.get("/nope");
    assert_eq!(status, 404);
}

#[test]
fn status_is_a_scan_state_and_other_api_paths_are_404_json() {
    let server = Server::start(IDLE);
    let (status, headers, body) = server.get("/api/status");
    assert_eq!(status, 200);
    assert_eq!(header(&headers, "Content-Type"), Some("application/json"));
    let json: serde_json::Value = serde_json::from_str(&body).expect("JSON body");
    let state = json["state"].as_str().expect("state");
    assert!(["scanning", "done"].contains(&state), "{state}");

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
fn keeps_running_with_no_page_connected() {
    let mut server = Server::start(IDLE);
    drop(server.open_events());
    thread::sleep(Duration::from_secs(2));
    assert!(server.is_running(), "the server exited when its page left");
    let (status, _, _) = server.get("/api/status");
    assert_eq!(status, 200);
    assert_eq!(server.stub.open_args(), None, "it opened something");
}
